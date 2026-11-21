from __future__ import annotations

import asyncio
import logging
from contextlib import asynccontextmanager, suppress
from pathlib import Path
from typing import Optional

from fastapi import FastAPI, HTTPException, Request, status
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse, JSONResponse

from sqlalchemy import update

import app.models  # noqa: F401 - ensure all ORM tables are registered
from app.models import Device
from app.api import (
    admin_auth,
    admin_catalog_deployments,
    admin_devices_groups,
    admin_patches,
    agent_api,
    enrollment,
    health_and_dl,
)
from app.auth import ensure_bootstrap_admin
from app.config import Settings, get_settings
from app.crypto import SigningService
from app.db import DatabaseManager
from app.services import PackageStorageService, WebSocketHub, run_scheduler_tick

logger = logging.getLogger(__name__)


def create_app(settings: Optional[Settings] = None) -> FastAPI:
    cfg = settings or get_settings()
    db = DatabaseManager(cfg)
    signing = SigningService(cfg.DATA_DIR)
    storage = PackageStorageService(cfg.DATA_DIR)
    ws_hub = WebSocketHub()

    @asynccontextmanager
    async def lifespan(application: FastAPI):
        cfg.DATA_DIR.mkdir(parents=True, exist_ok=True)
        cfg.STATIC_AGENTS_DIR.mkdir(parents=True, exist_ok=True)
        storage.ensure_dirs()
        signing.ensure_keys()
        await db.initialize()
        async with db.session_factory() as session:
            await ensure_bootstrap_admin(session, cfg)
            await session.execute(update(Device).values(is_online=False))
            await session.commit()

        scheduler_task: Optional[asyncio.Task[None]] = None
        if cfg.ENABLE_BACKGROUND_SCHEDULER:

            async def _scheduler_loop() -> None:
                interval = max(0.02, float(cfg.SCHEDULER_INTERVAL_SECONDS))
                while True:
                    await asyncio.sleep(interval)
                    try:
                        async with db.session_factory() as tick_session:
                            await run_scheduler_tick(
                                tick_session, cfg, signing, ws_hub
                            )
                    except asyncio.CancelledError:
                        raise
                    except Exception:
                        logger.exception("Background scheduler tick error")

            scheduler_task = asyncio.create_task(
                _scheduler_loop(), name="swm-background-scheduler"
            )
            application.state.scheduler_task = scheduler_task

        try:
            yield
        finally:
            if scheduler_task is not None:
                scheduler_task.cancel()
                with suppress(asyncio.CancelledError):
                    await scheduler_task
                application.state.scheduler_task = None
            await db.dispose()

    application = FastAPI(
        title=cfg.APP_NAME,
        version=cfg.APP_VERSION,
        lifespan=lifespan,
    )
    application.state.settings = cfg
    application.state.db = db
    application.state.signing = signing
    application.state.storage = storage
    application.state.ws_hub = ws_hub
    application.state.scheduler_task = None

    application.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
        expose_headers=["Accept-Ranges", "Content-Range", "X-Package-SHA256"],
    )

    application.include_router(health_and_dl.router)
    application.include_router(admin_auth.router)
    application.include_router(enrollment.router)
    application.include_router(admin_devices_groups.router)
    application.include_router(admin_patches.router)
    application.include_router(admin_catalog_deployments.router)
    application.include_router(agent_api.router)

    @application.get("/{full_path:path}", include_in_schema=False)
    async def spa_or_static_fallback(full_path: str, request: Request):
        if (
            full_path in ("api", "dl")
            or full_path.startswith("api/")
            or full_path.startswith("dl/")
        ):
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail="Endpoint not found",
            )

        current_cfg: Settings = request.app.state.settings
        dist_dir: Path = current_cfg.WEB_DIST_DIR.resolve()
        index_html = dist_dir / "index.html"

        if index_html.is_file():
            if full_path:
                candidate = (dist_dir / full_path).resolve()
                if str(candidate).startswith(str(dist_dir)) and candidate.is_file():
                    return FileResponse(candidate)
            return FileResponse(index_html)

        if full_path == "":
            return JSONResponse(
                {
                    "service": current_cfg.APP_NAME,
                    "version": current_cfg.APP_VERSION,
                    "status": "ok",
                }
            )
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Not found",
        )

    return application


app = create_app()
