from __future__ import annotations

import secrets
from pathlib import Path
from pydantic import field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        extra="ignore",
    )

    APP_NAME: str = "NS8-SoftwareManager"
    APP_VERSION: str = "1.1.0"

    DATABASE_URL: str = "sqlite+aiosqlite:///./data/swm.db"
    DATA_DIR: Path = Path("./data")
    STATIC_AGENTS_DIR: Path = Path("./static/agents")
    WEB_DIST_DIR: Path = Path("../web/dist")
    PUBLIC_BASE_URL: str = ""

    JWT_SECRET: str = ""
    JWT_ALGORITHM: str = "HS256"
    JWT_EXPIRE_MINUTES: int = 480

    ADMIN_USERNAME: str = "admin"
    ADMIN_PASSWORD: str = "admin"

    LDAP_HOST: str = ""
    LDAP_PORT: int = 389
    LDAP_USE_SSL: bool = False
    LDAP_BASE_DN: str = ""
    LDAP_BIND_DN: str = ""
    LDAP_BIND_PASSWORD: str = ""
    LDAP_USER_FILTER: str = "(|(uid={username})(sAMAccountName={username})(cn={username}))"

    DB_CONNECT_MAX_RETRIES: int = 8
    DB_CONNECT_BASE_DELAY: float = 0.25
    JOB_SIGNATURE_TTL_SECONDS: int = 86400

    ENABLE_BACKGROUND_SCHEDULER: bool = True
    SCHEDULER_INTERVAL_SECONDS: float = 30.0
    ONLINE_STALE_SECONDS: int = 180

    @field_validator("DATABASE_URL", mode="before")
    @classmethod
    def normalize_database_url(cls, v: str) -> str:
        if isinstance(v, str):
            if v.startswith("postgres://"):
                return "postgresql+asyncpg://" + v[len("postgres://") :]
            if v.startswith("postgresql://"):
                return "postgresql+asyncpg://" + v[len("postgresql://") :]
        return v

    @field_validator("LDAP_PORT", mode="before")
    @classmethod
    def normalize_ldap_port(cls, v: object) -> object:
        if v is None or (isinstance(v, str) and not v.strip()):
            return 389
        return v

    @field_validator("LDAP_USE_SSL", mode="before")
    @classmethod
    def normalize_ldap_ssl(cls, v: object) -> object:
        if v is None or (isinstance(v, str) and not v.strip()):
            return False
        return v

    def effective_jwt_secret(self) -> str:
        if self.JWT_SECRET:
            return self.JWT_SECRET
        keys_dir = self.DATA_DIR / "keys"
        keys_dir.mkdir(parents=True, exist_ok=True)
        secret_file = keys_dir / "jwt.secret"
        if secret_file.exists():
            return secret_file.read_text(encoding="utf-8").strip()
        generated = secrets.token_urlsafe(48)
        secret_file.write_text(generated, encoding="utf-8")
        return generated


def get_settings() -> Settings:
    return Settings()
