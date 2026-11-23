from __future__ import annotations

from pathlib import Path
from unittest.mock import patch

import httpx
import pytest
from sqlalchemy.exc import OperationalError

from app.config import Settings
from app.db import build_async_engine, connect_with_backoff


@pytest.mark.asyncio
async def test_health_endpoint(client: httpx.AsyncClient) -> None:
    resp = await client.get("/api/v1/health")
    assert resp.status_code == 200
    data = resp.json()
    assert data == {
        "status": "ok",
        "version": "1.2.0",
        "database": "connected",
    }


@pytest.mark.asyncio
async def test_database_url_postgres_normalization() -> None:
    s1 = Settings(DATABASE_URL="postgresql://swm:secret@127.0.0.1:5432/swm")
    assert s1.DATABASE_URL == "postgresql+asyncpg://swm:secret@127.0.0.1:5432/swm"
    s2 = Settings(DATABASE_URL="postgres://swm:secret@127.0.0.1:5432/swm")
    assert s2.DATABASE_URL == "postgresql+asyncpg://swm:secret@127.0.0.1:5432/swm"


@pytest.mark.asyncio
async def test_exponential_db_connect_backoff_recovers(tmp_path: Path) -> None:
    db_file = tmp_path / "backoff.sqlite3"
    real_engine = build_async_engine(f"sqlite+aiosqlite:///{db_file.as_posix()}")
    calls = {"count": 0}

    class FlakyEngineProxy:
        def begin(self):
            calls["count"] += 1
            if calls["count"] < 3:
                raise OperationalError("SELECT 1", {}, Exception("DB starting up"))
            return real_engine.begin()

    await connect_with_backoff(FlakyEngineProxy(), max_retries=4, base_delay=0.01)  # type: ignore[arg-type]
    assert calls["count"] == 3
    await real_engine.dispose()


@pytest.mark.asyncio
async def test_static_agent_download_and_spa_fallback(
    client: httpx.AsyncClient, test_settings: Settings
) -> None:
    # Missing agent binary -> 404
    miss = await client.get("/dl/agents/windows/swm-agent.exe")
    assert miss.status_code == 404

    # Present agent binary -> 200
    win_dir = test_settings.STATIC_AGENTS_DIR / "windows"
    win_dir.mkdir(parents=True, exist_ok=True)
    (win_dir / "swm-agent.exe").write_bytes(b"MZ-test-agent-binary")

    hit = await client.get("/dl/agents/windows/swm-agent.exe")
    assert hit.status_code == 200
    assert hit.content == b"MZ-test-agent-binary"

    # SPA fallback when WEB_DIST_DIR has index.html
    test_settings.WEB_DIST_DIR.mkdir(parents=True, exist_ok=True)
    (test_settings.WEB_DIST_DIR / "index.html").write_text(
        "<!doctype html><title>SWM Admin</title>", encoding="utf-8"
    )
    assets_dir = test_settings.WEB_DIST_DIR / "assets"
    assets_dir.mkdir(parents=True, exist_ok=True)
    (assets_dir / "app.js").write_text("console.log('swm');", encoding="utf-8")

    root_resp = await client.get("/")
    assert root_resp.status_code == 200
    assert "SWM Admin" in root_resp.text

    route_resp = await client.get("/devices/1234")
    assert route_resp.status_code == 200
    assert "SWM Admin" in route_resp.text

    asset_resp = await client.get("/assets/app.js")
    assert asset_resp.status_code == 200
    assert "console.log('swm')" in asset_resp.text

    # API and /dl 404s must NOT return index.html
    api_404 = await client.get("/api/v1/does-not-exist")
    assert api_404.status_code == 404
    dl_404 = await client.get("/dl/does-not-exist")
    assert dl_404.status_code == 404


@pytest.mark.asyncio
async def test_agent_artifacts_metadata_and_idempotent_gpo_bootstrap(
    client: httpx.AsyncClient,
    admin_headers: dict[str, str],
    test_settings: Settings,
) -> None:
    import hashlib

    win_dir = test_settings.STATIC_AGENTS_DIR / "windows"
    lin_dir = test_settings.STATIC_AGENTS_DIR / "linux-amd64"
    win_dir.mkdir(parents=True, exist_ok=True)
    lin_dir.mkdir(parents=True, exist_ok=True)
    win_bytes = b"MZ-win-agent-payload"
    lin_bytes = b"\x7fELF-linux-agent-payload"
    (win_dir / "swm-agent.exe").write_bytes(win_bytes)
    (lin_dir / "swm-agent").write_bytes(lin_bytes)

    meta_resp = await client.get("/api/v1/admin/agent-artifacts", headers=admin_headers)
    assert meta_resp.status_code == 200
    meta = meta_resp.json()
    assert "server_url" in meta
    artifacts = {f"{a['platform']}/{a['filename']}": a for a in meta["artifacts"]}
    assert artifacts["windows/swm-agent.exe"]["available"] is True
    assert artifacts["windows/swm-agent.exe"]["sha256"] == hashlib.sha256(win_bytes).hexdigest()
    assert artifacts["linux-amd64/swm-agent"]["available"] is True
    assert artifacts["linux-amd64/swm-agent"]["sha256"] == hashlib.sha256(lin_bytes).hexdigest()
    assert artifacts["windows/swm-center.exe"]["available"] is False

    # Verify idempotent GPO/Ansible skip_if_enrolled=1 bootstrap scripts
    ps1_gpo = await client.get("/dl/install.ps1?token=tok-gpo-1&headless=0&skip_if_enrolled=1")
    assert ps1_gpo.status_code == 200
    assert "$SkipIfEnrolled = 1" in ps1_gpo.text
    assert "NS8SoftwareManagerAgent" in ps1_gpo.text
    assert "NS8 Software Center.lnk" in ps1_gpo.text

    sh_ans = await client.get("/dl/install.sh?token=tok-ans-1&headless=0&skip_if_enrolled=1")
    assert sh_ans.status_code == 200
    assert "SKIP_IF_ENROLLED=1" in sh_ans.text
    assert "/usr/share/applications/ns8-swm-center.desktop" in sh_ans.text
