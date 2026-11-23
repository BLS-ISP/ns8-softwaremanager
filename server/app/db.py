from __future__ import annotations

import asyncio
import logging
from collections.abc import AsyncGenerator
from pathlib import Path
from typing import Optional

from sqlalchemy import Connection, event, inspect, text
from sqlalchemy.ext.asyncio import (
    AsyncEngine,
    AsyncSession,
    async_sessionmaker,
    create_async_engine,
)
from sqlalchemy.orm import DeclarativeBase

from app.config import Settings

logger = logging.getLogger(__name__)


class Base(DeclarativeBase):
    pass


def _ensure_sqlite_parent_dir(database_url: str) -> None:
    prefix = "sqlite+aiosqlite:///"
    if database_url.startswith(prefix) and ":memory:" not in database_url:
        raw_path = database_url[len(prefix) :].split("?")[0]
        if raw_path:
            Path(raw_path).parent.mkdir(parents=True, exist_ok=True)


def build_async_engine(database_url: str) -> AsyncEngine:
    if database_url.startswith("sqlite"):
        engine = create_async_engine(
            database_url,
            connect_args={"check_same_thread": False},
            future=True,
        )

        @event.listens_for(engine.sync_engine, "do_connect")
        def _prepare_sqlite_dir(_dialect, _conn_rec, _cargs, _cparams) -> None:
            _ensure_sqlite_parent_dir(database_url)

        @event.listens_for(engine.sync_engine, "connect")
        def _set_sqlite_pragma(dbapi_connection, _connection_record) -> None:
            cursor = dbapi_connection.cursor()
            cursor.execute("PRAGMA foreign_keys=ON")
            cursor.close()

        return engine

    return create_async_engine(
        database_url,
        pool_pre_ping=True,
        future=True,
    )


def ensure_schema_upgrades(sync_conn: Connection) -> None:
    """Idempotently add newly introduced columns on existing SQLite / PostgreSQL databases."""
    inspector = inspect(sync_conn)
    existing_tables = set(inspector.get_table_names())
    is_sqlite = sync_conn.dialect.name == "sqlite"

    table_upgrades: tuple[tuple[str, tuple[tuple[str, bool], ...]], ...] = (
        (
            "groups",
            (
                ("enforce_tray", True),
                ("allow_self_service", True),
                ("allow_user_os_updates", True),
                ("silent_notifications", False),
            ),
        ),
        (
            "devices",
            (
                ("center_running", False),
            ),
        ),
    )

    for table_name, columns in table_upgrades:
        if table_name not in existing_tables:
            continue
        present_cols = {col["name"] for col in inspector.get_columns(table_name)}
        for col_name, default_bool in columns:
            if col_name in present_cols:
                continue
            default_sql = (
                ("1" if default_bool else "0")
                if is_sqlite
                else ("TRUE" if default_bool else "FALSE")
            )
            sync_conn.execute(
                text(
                    f"ALTER TABLE {table_name} "
                    f"ADD COLUMN {col_name} BOOLEAN NOT NULL DEFAULT {default_sql}"
                )
            )
            logger.info("Applied live schema upgrade: %s.%s", table_name, col_name)


async def connect_with_backoff(
    engine: AsyncEngine,
    max_retries: int = 8,
    base_delay: float = 0.25,
) -> None:
    """Verify database connectivity on startup with exponential retry backoff."""
    last_error: Optional[Exception] = None
    attempts = max(1, max_retries)
    for attempt in range(attempts):
        try:
            async with engine.begin() as conn:
                await conn.execute(text("SELECT 1"))
                await conn.run_sync(Base.metadata.create_all)
                await conn.run_sync(ensure_schema_upgrades)
            if attempt > 0:
                logger.info("Database connected after %d retry attempts.", attempt)
            return
        except Exception as exc:
            last_error = exc
            if attempt == attempts - 1:
                logger.error(
                    "Database connection failed after %d attempts: %s",
                    attempts,
                    exc,
                )
                raise
            delay = min(base_delay * (2**attempt), 15.0)
            logger.warning(
                "Database not ready (attempt %d/%d): %s. Retrying in %.2fs...",
                attempt + 1,
                attempts,
                exc,
                delay,
            )
            await asyncio.sleep(delay)

    if last_error:
        raise last_error


class DatabaseManager:
    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self.engine: AsyncEngine = build_async_engine(settings.DATABASE_URL)
        self.session_factory: async_sessionmaker[AsyncSession] = async_sessionmaker(
            bind=self.engine,
            expire_on_commit=False,
            class_=AsyncSession,
        )

    async def initialize(self) -> None:
        _ensure_sqlite_parent_dir(self.settings.DATABASE_URL)
        await connect_with_backoff(
            self.engine,
            max_retries=self.settings.DB_CONNECT_MAX_RETRIES,
            base_delay=self.settings.DB_CONNECT_BASE_DELAY,
        )

    async def dispose(self) -> None:
        await self.engine.dispose()

    async def ping(self) -> bool:
        try:
            async with self.engine.connect() as conn:
                await conn.execute(text("SELECT 1"))
            return True
        except Exception:
            return False


async def get_db_session(
    session_factory: async_sessionmaker[AsyncSession],
) -> AsyncGenerator[AsyncSession, None]:
    async with session_factory() as session:
        yield session
