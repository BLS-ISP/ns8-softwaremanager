from __future__ import annotations

import sys
from collections.abc import AsyncGenerator
from pathlib import Path

import httpx
import pytest
import pytest_asyncio

SERVER_ROOT = Path(__file__).resolve().parents[1]
if str(SERVER_ROOT) not in sys.path:
    sys.path.insert(0, str(SERVER_ROOT))

from app.auth import ensure_bootstrap_admin  # noqa: E402
from app.config import Settings  # noqa: E402
from app.main import create_app  # noqa: E402


@pytest.fixture
def test_settings(tmp_path: Path) -> Settings:
    data_dir = tmp_path / "data"
    agents_dir = tmp_path / "static_agents"
    web_dist_dir = tmp_path / "web_dist"
    db_path = tmp_path / "test_swm.sqlite3"
    return Settings(
        DATABASE_URL=f"sqlite+aiosqlite:///{db_path.as_posix()}",
        DATA_DIR=data_dir,
        STATIC_AGENTS_DIR=agents_dir,
        WEB_DIST_DIR=web_dist_dir,
        PUBLIC_BASE_URL="https://swm.ns8.example.com",
        JWT_SECRET="unit-test-jwt-secret-key-32bytes-minimum!!",
        ADMIN_USERNAME="admin",
        ADMIN_PASSWORD="TestAdminPassword123!",
        DB_CONNECT_MAX_RETRIES=3,
        DB_CONNECT_BASE_DELAY=0.05,
    )


@pytest_asyncio.fixture
async def test_app(test_settings: Settings):
    app = create_app(test_settings)
    test_settings.DATA_DIR.mkdir(parents=True, exist_ok=True)
    test_settings.STATIC_AGENTS_DIR.mkdir(parents=True, exist_ok=True)
    app.state.storage.ensure_dirs()
    app.state.signing.ensure_keys()
    await app.state.db.initialize()
    async with app.state.db.session_factory() as session:
        await ensure_bootstrap_admin(session, test_settings)
    yield app
    await app.state.db.dispose()


@pytest_asyncio.fixture
async def client(test_app) -> AsyncGenerator[httpx.AsyncClient, None]:
    transport = httpx.ASGITransport(app=test_app)
    async with httpx.AsyncClient(
        transport=transport, base_url="https://swm.ns8.example.com"
    ) as ac:
        yield ac


@pytest_asyncio.fixture
async def admin_headers(
    client: httpx.AsyncClient, test_settings: Settings
) -> dict[str, str]:
    resp = await client.post(
        "/api/v1/admin/auth/login",
        json={
            "username": test_settings.ADMIN_USERNAME,
            "password": test_settings.ADMIN_PASSWORD,
            "auth_source": "local",
        },
    )
    assert resp.status_code == 200, resp.text
    token = resp.json()["access_token"]
    return {"Authorization": f"Bearer {token}"}


async def enroll_test_device(
    client: httpx.AsyncClient,
    admin_headers: dict[str, str],
    *,
    hostname: str = "win-ws-01",
    os_family: str = "windows",
    auto_approve: bool = True,
    group_id: str | None = None,
    machine_uid: str | None = None,
) -> dict[str, str]:
    tok_resp = await client.post(
        "/api/v1/admin/enrollment-tokens",
        headers=admin_headers,
        json={
            "name": f"tok-{hostname}",
            "expires_in_hours": 24,
            "auto_approve": auto_approve,
            "group_id": group_id,
        },
    )
    assert tok_resp.status_code == 200, tok_resp.text
    enroll_tok = tok_resp.json()["token"]

    enr_resp = await client.post(
        "/api/v1/agent/enroll",
        json={
            "enrollment_token": enroll_tok,
            "hostname": hostname,
            "os_family": os_family,
            "os_name": "Windows 11 Pro" if os_family == "windows" else "Ubuntu 24.04",
            "os_version": "23H2" if os_family == "windows" else "24.04",
            "arch": "amd64",
            "agent_version": "1.0.0",
            "machine_uid": machine_uid or f"uid-{hostname}",
        },
    )
    assert enr_resp.status_code == 200, enr_resp.text
    return enr_resp.json()
