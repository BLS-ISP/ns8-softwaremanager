from __future__ import annotations

import shutil
import subprocess
from datetime import datetime, timedelta, timezone
from pathlib import Path

import httpx
import pytest
from fastapi.testclient import TestClient

from app.config import Settings
from app.crypto import SigningService
from app.main import create_app
from app.models import Deployment, Device
from app.services import run_scheduler_tick
from tests.conftest import enroll_test_device

REPO_ROOT = Path(__file__).resolve().parents[2]


@pytest.mark.asyncio
async def test_run_scheduler_tick_sweeps_stale_online_devices(
    client: httpx.AsyncClient,
    admin_headers: dict[str, str],
    test_app,
    test_settings: Settings,
) -> None:
    now = datetime(2026, 11, 21, 12, 0, 0, tzinfo=timezone.utc)
    stale_dev = await enroll_test_device(client, admin_headers, hostname="stale-pc")
    fresh_dev = await enroll_test_device(client, admin_headers, hostname="fresh-pc")
    ws_dev = await enroll_test_device(client, admin_headers, hostname="ws-pc")

    ws_hub = test_app.state.ws_hub
    signing = test_app.state.signing

    class DummySocket:
        pass

    dummy_ws = DummySocket()
    ws_hub.register(ws_dev["device_id"], dummy_ws)  # type: ignore[arg-type]

    try:
        async with test_app.state.db.session_factory() as session:
            d_stale = await session.get(Device, stale_dev["device_id"])
            d_fresh = await session.get(Device, fresh_dev["device_id"])
            d_ws = await session.get(Device, ws_dev["device_id"])
            assert d_stale is not None and d_fresh is not None and d_ws is not None

            d_stale.is_online = True
            d_stale.last_seen_at = now - timedelta(
                seconds=test_settings.ONLINE_STALE_SECONDS + 45
            )

            d_fresh.is_online = True
            d_fresh.last_seen_at = now - timedelta(seconds=30)

            d_ws.is_online = True
            d_ws.last_seen_at = now - timedelta(
                seconds=test_settings.ONLINE_STALE_SECONDS + 300
            )
            await session.commit()

        async with test_app.state.db.session_factory() as session:
            tick_stats = await run_scheduler_tick(
                session, test_settings, signing, ws_hub, now=now
            )
            assert tick_stats["swept_offline"] == 1

        async with test_app.state.db.session_factory() as session:
            d_stale_after = await session.get(Device, stale_dev["device_id"])
            d_fresh_after = await session.get(Device, fresh_dev["device_id"])
            d_ws_after = await session.get(Device, ws_dev["device_id"])
            assert d_stale_after is not None and d_stale_after.is_online is False
            assert d_fresh_after is not None and d_fresh_after.is_online is True
            assert d_ws_after is not None and d_ws_after.is_online is True
    finally:
        ws_hub.unregister(ws_dev["device_id"], dummy_ws)  # type: ignore[arg-type]


def test_scheduler_tick_and_lifespan_proactive_ws_dispatch_on_window_open(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # Monday 2026-11-16 10:00 UTC (outside Wednesday 02:00-04:00 UTC window)
    t_monday_closed = datetime(2026, 11, 16, 10, 0, 0, tzinfo=timezone.utc)
    # Wednesday 2026-11-18 02:15 UTC (inside Wednesday 02:00-04:00 UTC window)
    t_wed_open = datetime(2026, 11, 18, 2, 15, 0, tzinfo=timezone.utc)

    current = {"now": t_monday_closed}
    monkeypatch.setattr("app.services.scheduler.current_utc_time", lambda: current["now"])
    monkeypatch.setattr("app.api.agent_api.current_utc_time", lambda: current["now"])
    monkeypatch.setattr(
        "app.api.admin_catalog_deployments.current_utc_time", lambda: current["now"]
    )

    settings = Settings(
        DATABASE_URL=f"sqlite+aiosqlite:///{(tmp_path / 'sched.sqlite3').as_posix()}",
        DATA_DIR=tmp_path / "data",
        STATIC_AGENTS_DIR=tmp_path / "agents",
        WEB_DIST_DIR=tmp_path / "web_dist",
        PUBLIC_BASE_URL="https://swm.ns8.example.com",
        JWT_SECRET="scheduler-test-jwt-secret-key-32bytes!!",
        ADMIN_USERNAME="admin",
        ADMIN_PASSWORD="TestAdminPassword123!",
        ENABLE_BACKGROUND_SCHEDULER=True,
        SCHEDULER_INTERVAL_SECONDS=0.05,
        ONLINE_STALE_SECONDS=180,
    )
    app = create_app(settings)

    with TestClient(app) as tc:
        assert app.state.scheduler_task is not None
        assert not app.state.scheduler_task.done()

        login = tc.post(
            "/api/v1/admin/auth/login",
            json={
                "username": settings.ADMIN_USERNAME,
                "password": settings.ADMIN_PASSWORD,
                "auth_source": "local",
            },
        )
        assert login.status_code == 200
        headers = {"Authorization": f"Bearer {login.json()['access_token']}"}

        grp = tc.post(
            "/api/v1/admin/groups",
            headers=headers,
            json={"name": "NightPatchServers", "reboot_policy": "maintenance_window"},
        ).json()

        mw = tc.post(
            "/api/v1/admin/maintenance-windows",
            headers=headers,
            json={
                "group_id": grp["id"],
                "name": "Wed 02-04 UTC",
                "days_of_week": [2],
                "start_time": "02:00",
                "end_time": "04:00",
                "timezone": "UTC",
                "catch_up": False,
            },
        )
        assert mw.status_code == 200

        tok = tc.post(
            "/api/v1/admin/enrollment-tokens",
            headers=headers,
            json={"name": "srv-tok", "auto_approve": True, "group_id": grp["id"]},
        ).json()["token"]

        enr = tc.post(
            "/api/v1/agent/enroll",
            json={
                "enrollment_token": tok,
                "hostname": "srv-mw-01",
                "os_family": "windows",
                "os_name": "Windows Server 2022",
                "os_version": "21H2",
                "arch": "amd64",
                "agent_version": "1.0.0",
                "machine_uid": "uid-srv-mw-01",
            },
        ).json()
        dev_id = enr["device_id"]
        dev_tok = enr["device_token"]
        pub_key_b64 = enr["server_public_key_b64"]

        pkg = tc.post(
            "/api/v1/admin/catalog",
            headers=headers,
            json={
                "name": "7-Zip",
                "version": "24.08",
                "vendor": "Igor Pavlov",
                "os_family": "windows",
                "source_type": "winget",
                "package_ref": "7zip.7zip",
            },
        ).json()

        with tc.websocket_connect(f"/api/v1/agent/ws?token={dev_tok}") as ws:
            # Create required deployment while maintenance window is CLOSED
            dep = tc.post(
                "/api/v1/admin/deployments",
                headers=headers,
                json={
                    "package_id": pkg["id"],
                    "target_group_ids": [grp["id"]],
                    "mode": "required",
                    "respect_maintenance_window": True,
                },
            )
            assert dep.status_code == 200

            # Verify job is still queued and NO premature WS push occurred while closed
            jobs_closed = tc.get(
                f"/api/v1/admin/jobs?device_id={dev_id}", headers=headers
            ).json()
            assert len(jobs_closed) == 1
            queued_job_id = jobs_closed[0]["id"]
            assert jobs_closed[0]["status"] == "queued"

            ws.send_json({"type": "heartbeat"})
            ack = ws.receive_json()
            assert ack["type"] == "heartbeat_ack"

            # Transition clock into the OPEN maintenance window -> background scheduler loop
            # proactively unlocks the queued job, marks it dispatched, and pushes job_available!
            current["now"] = t_wed_open
            pushed = ws.receive_json()
            assert pushed["type"] == "job_available"
            assert pushed["job_id"] == queued_job_id
            assert isinstance(pushed.get("job"), dict)
            sjob = pushed["job"]
            assert sjob["job_id"] == queued_job_id
            assert sjob["device_id"] == dev_id
            assert sjob["target_id"] == pkg["id"]

            canonical = SigningService.canonical_job_string(
                job_id=sjob["job_id"],
                device_id=sjob["device_id"],
                job_type=sjob["job_type"],
                action=sjob["action"],
                target_id=sjob["target_id"],
                version=sjob["version"],
                sha256=sjob["sha256"],
                command=sjob["command"],
                expires_at_unix=sjob["expires_at_unix"],
            )
            assert SigningService.verify_canonical_with_b64_pubkey(
                pub_key_b64, canonical, sjob["signature_b64"]
            )

            jobs_open = tc.get(
                f"/api/v1/admin/jobs?device_id={dev_id}", headers=headers
            ).json()
            assert jobs_open[0]["status"] == "dispatched"
            assert jobs_open[0]["dispatched_at"] is not None

            # Subsequent scheduler ticks do NOT re-push already-dispatched jobs
            ws.send_json({"type": "heartbeat"})
            ack2 = ws.receive_json()
            assert ack2["type"] == "heartbeat_ack"

    # Lifespan shutdown cleanly cancelled and cleared the background task
    assert app.state.scheduler_task is None


@pytest.mark.asyncio
async def test_bootstrap_scripts_desktop_autostart_and_no_stray_root_data_dir(
    client: httpx.AsyncClient,
) -> None:
    token = "enroll-tok-autostart-2026"

    # 1. Windows PowerShell: headless=0 vs headless=1
    ps1_workstation = await client.get(f"/dl/install.ps1?token={token}&headless=0")
    assert ps1_workstation.status_code == 200
    ps1_ws_text = ps1_workstation.text
    assert "$Headless = 0" in ps1_ws_text
    assert 'Test-Path "$InstallDir\\swm-center.exe"' in ps1_ws_text
    expected_run_cmd = (
        'Set-ItemProperty -Path "HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" '
        '-Name "NS8SoftwareCenter" '
        '-Value \'"C:\\Program Files\\NS8-SoftwareManager\\swm-center.exe" gui --tray-only\''
    )
    assert expected_run_cmd in ps1_ws_text

    ps1_headless = await client.get(f"/dl/install.ps1?token={token}&headless=1")
    assert ps1_headless.status_code == 200
    ps1_hl_text = ps1_headless.text
    assert "$Headless = 1" in ps1_hl_text
    assert "Set-ItemProperty" not in ps1_hl_text
    assert (
        'Remove-ItemProperty -Path "HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" '
        '-Name "NS8SoftwareCenter"'
    ) in ps1_hl_text

    pwsh_bin = shutil.which("powershell") or shutil.which("pwsh")
    if pwsh_bin:
        for script_label, script_body in (
            ("headless=0", ps1_ws_text),
            ("headless=1", ps1_hl_text),
        ):
            proc = subprocess.run(
                [
                    pwsh_bin,
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    (
                        "$src = [Console]::In.ReadToEnd(); "
                        "$tokens = $null; $errors = $null; "
                        "[void][System.Management.Automation.Language.Parser]::ParseInput($src, [ref]$tokens, [ref]$errors); "
                        "if ($errors.Count -gt 0) { $errors | Out-String | Write-Error; exit 1 }"
                    ),
                ],
                input=script_body,
                text=True,
                capture_output=True,
                check=False,
            )
            assert proc.returncode == 0, f"PowerShell syntax error ({script_label}): {proc.stderr}"

    # 2. Linux Bash: headless=0 vs headless=1
    sh_workstation = await client.get(f"/dl/install.sh?token={token}&headless=0")
    assert sh_workstation.status_code == 200
    sh_ws_text = sh_workstation.text
    assert "\r\n" not in sh_ws_text
    assert "HEADLESS=0" in sh_ws_text
    assert "mkdir -p /etc/xdg/autostart" in sh_ws_text
    assert "/etc/xdg/autostart/ns8-swm-center.desktop" in sh_ws_text
    assert "[Desktop Entry]" in sh_ws_text
    assert "Type=Application" in sh_ws_text
    assert "Name=NS8 Software Center" in sh_ws_text
    assert "Exec=/usr/local/bin/swm-center gui --tray-only" in sh_ws_text
    assert "Terminal=false" in sh_ws_text
    assert "X-GNOME-Autostart-enabled=true" in sh_ws_text

    sh_headless = await client.get(f"/dl/install.sh?token={token}&headless=1")
    assert sh_headless.status_code == 200
    sh_hl_text = sh_headless.text
    assert "\r\n" not in sh_hl_text
    assert "HEADLESS=1" in sh_hl_text
    assert "Exec=/usr/local/bin/swm-center gui --tray-only" not in sh_hl_text
    assert "rm -f /etc/xdg/autostart/ns8-swm-center.desktop" in sh_hl_text

    # 3. Ensure no test or module import created a stray repo-root data/ directory
    assert not (REPO_ROOT / "data").exists()


@pytest.mark.asyncio
async def test_run_scheduler_tick_evaluates_missing_deployment_jobs_for_connected_device(
    client: httpx.AsyncClient,
    admin_headers: dict[str, str],
    test_app,
    test_settings: Settings,
) -> None:
    dev = await enroll_test_device(client, admin_headers, hostname="online-ws-eval")
    pkg = (
        await client.post(
            "/api/v1/admin/catalog",
            headers=admin_headers,
            json={
                "name": "Notepad++",
                "version": "8.7.1",
                "vendor": "Don Ho",
                "os_family": "windows",
                "source_type": "winget",
                "package_ref": "Notepad++.Notepad++",
            },
        )
    ).json()

    ws_hub = test_app.state.ws_hub
    signing = test_app.state.signing
    pushed_messages: list[dict] = []

    class RecordingSocket:
        async def send_json(self, message: dict) -> None:
            pushed_messages.append(message)

    rec_ws = RecordingSocket()
    ws_hub.register(dev["device_id"], rec_ws)  # type: ignore[arg-type]

    try:
        # Insert a required deployment directly so no job exists yet in DB
        async with test_app.state.db.session_factory() as session:
            dep = Deployment(
                package_id=pkg["id"],
                target_group_ids=[],
                target_device_ids=[dev["device_id"]],
                mode="required",
                respect_maintenance_window=False,
            )
            session.add(dep)
            await session.commit()

        async with test_app.state.db.session_factory() as session:
            stats = await run_scheduler_tick(session, test_settings, signing, ws_hub)
            assert stats["enqueued_jobs"] == 1
            assert stats["dispatched_jobs"] == 1

        assert len(pushed_messages) == 1
        assert pushed_messages[0]["type"] == "job_available"
        assert pushed_messages[0]["job"]["target_id"] == pkg["id"]
        assert pushed_messages[0]["job"]["action"] == "install"
    finally:
        ws_hub.unregister(dev["device_id"], rec_ws)  # type: ignore[arg-type]

