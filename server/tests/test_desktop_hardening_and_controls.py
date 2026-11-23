from __future__ import annotations

import json
import shutil
import sqlite3
import subprocess
from pathlib import Path
from types import SimpleNamespace

import httpx
import pytest
from sqlalchemy import inspect, select

from app.crypto import SigningService
from app.db import build_async_engine, connect_with_backoff, ensure_schema_upgrades
from app.models import AuditLog, Device, Group, Job
from tests.conftest import enroll_test_device

REPO_ROOT = Path(__file__).resolve().parents[2]
PACKAGING_DIR = REPO_ROOT / "agent" / "packaging"


@pytest.mark.asyncio
async def test_live_schema_migration_preserves_legacy_sqlite_data_and_pg_sql(
    tmp_path: Path,
) -> None:
    legacy_db = tmp_path / "legacy_ns8_swm.sqlite3"
    conn = sqlite3.connect(legacy_db)
    try:
        conn.executescript(
            """
            CREATE TABLE groups (
                id VARCHAR(36) NOT NULL PRIMARY KEY,
                name VARCHAR(128) NOT NULL UNIQUE,
                description VARCHAR(512) NOT NULL,
                reboot_policy VARCHAR(32) NOT NULL,
                grace_period_minutes INTEGER NOT NULL,
                created_at DATETIME NOT NULL
            );
            INSERT INTO groups (id, name, description, reboot_policy, grace_period_minutes, created_at)
            VALUES ('grp-legacy-1', 'LegacyWorkstations', 'Pre-upgrade group', 'grace_period', 120, '2026-11-01T10:00:00+00:00');

            CREATE TABLE devices (
                id VARCHAR(36) NOT NULL PRIMARY KEY,
                hostname VARCHAR(255) NOT NULL,
                domain VARCHAR(255) NOT NULL,
                os_family VARCHAR(32) NOT NULL,
                os_name VARCHAR(128) NOT NULL,
                os_version VARCHAR(128) NOT NULL,
                kernel_or_build VARCHAR(128) NOT NULL,
                arch VARCHAR(32) NOT NULL,
                agent_version VARCHAR(64) NOT NULL,
                machine_uid VARCHAR(255) NOT NULL,
                token_hash VARCHAR(64) NOT NULL UNIQUE,
                status VARCHAR(32) NOT NULL,
                is_online BOOLEAN NOT NULL,
                reboot_required BOOLEAN NOT NULL,
                hardware JSON NOT NULL,
                ip_addresses JSON NOT NULL,
                logged_in_users JSON NOT NULL,
                tags JSON NOT NULL,
                software_hash VARCHAR(128),
                last_seen_at DATETIME,
                created_at DATETIME NOT NULL
            );
            INSERT INTO devices (
                id, hostname, domain, os_family, os_name, os_version, kernel_or_build,
                arch, agent_version, machine_uid, token_hash, status, is_online, reboot_required,
                hardware, ip_addresses, logged_in_users, tags, software_hash, last_seen_at, created_at
            ) VALUES (
                'dev-legacy-1', 'legacy-pc-01', 'corp.local', 'windows', 'Windows 11 Pro', '23H2', '22631',
                'amd64', '1.0.0', 'uid-legacy-pc-01', '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
                'approved', 1, 0, '{}', '["10.0.0.25"]', '["CORP\\\\jdoe"]', '["finance"]', NULL, NULL, '2026-11-01T10:05:00+00:00'
            );
            """
        )
        conn.commit()
    finally:
        conn.close()

    engine = build_async_engine(f"sqlite+aiosqlite:///{legacy_db.as_posix()}")
    try:
        # Run twice to prove both lossless live migration and idempotency
        await connect_with_backoff(engine, max_retries=2, base_delay=0.01)
        await connect_with_backoff(engine, max_retries=2, base_delay=0.01)

        async with engine.connect() as aconn:
            group_cols = await aconn.run_sync(
                lambda sc: {c["name"] for c in inspect(sc).get_columns("groups")}
            )
            device_cols = await aconn.run_sync(
                lambda sc: {c["name"] for c in inspect(sc).get_columns("devices")}
            )
            assert {
                "enforce_tray",
                "allow_self_service",
                "allow_user_os_updates",
                "silent_notifications",
            }.issubset(group_cols)
            assert "center_running" in device_cols

        from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

        session_factory = async_sessionmaker(bind=engine, expire_on_commit=False, class_=AsyncSession)
        async with session_factory() as session:
            grp = await session.get(Group, "grp-legacy-1")
            assert grp is not None
            assert grp.name == "LegacyWorkstations"
            assert grp.grace_period_minutes == 120
            assert grp.enforce_tray is True
            assert grp.allow_self_service is True
            assert grp.allow_user_os_updates is True
            assert grp.silent_notifications is False

            dev = await session.get(Device, "dev-legacy-1")
            assert dev is not None
            assert dev.hostname == "legacy-pc-01"
            assert dev.status == "approved"
            assert dev.center_running is False
    finally:
        await engine.dispose()

    # Verify PostgreSQL dialect emits BOOLEAN NOT NULL DEFAULT TRUE/FALSE
    executed_pg_sql: list[str] = []

    class FakePgInspector:
        def get_table_names(self) -> list[str]:
            return ["groups", "devices"]

        def get_columns(self, table_name: str) -> list[dict[str, str]]:
            return [{"name": "id"}]

    class FakePgConn:
        dialect = SimpleNamespace(name="postgresql")

        def execute(self, clause) -> None:
            executed_pg_sql.append(str(clause))

    import app.db as db_mod

    orig_inspect = db_mod.inspect
    db_mod.inspect = lambda _conn: FakePgInspector()  # type: ignore[assignment]
    try:
        ensure_schema_upgrades(FakePgConn())  # type: ignore[arg-type]
    finally:
        db_mod.inspect = orig_inspect  # type: ignore[assignment]

    assert any(
        "ALTER TABLE groups ADD COLUMN enforce_tray BOOLEAN NOT NULL DEFAULT TRUE" in s
        for s in executed_pg_sql
    )
    assert any(
        "ALTER TABLE groups ADD COLUMN silent_notifications BOOLEAN NOT NULL DEFAULT FALSE" in s
        for s in executed_pg_sql
    )
    assert any(
        "ALTER TABLE devices ADD COLUMN center_running BOOLEAN NOT NULL DEFAULT FALSE" in s
        for s in executed_pg_sql
    )


@pytest.mark.asyncio
async def test_desktop_policy_resolution_across_zero_one_and_multiple_groups(
    client: httpx.AsyncClient,
    admin_headers: dict[str, str],
    test_app,
) -> None:
    enr = await enroll_test_device(client, admin_headers, hostname="ws-policy-01")
    dev_id = enr["device_id"]
    agent_headers = {"Authorization": f"Bearer {enr['device_token']}"}

    # 0 groups -> defaults
    st0 = await client.get("/api/v1/agent/status", headers=agent_headers)
    assert st0.status_code == 200
    assert st0.json()["desktop_policy"] == {
        "enforce_tray": True,
        "allow_self_service": True,
        "allow_user_os_updates": True,
        "silent_notifications": False,
    }

    # 1 group with relaxed tray & custom flags
    g1_resp = await client.post(
        "/api/v1/admin/groups",
        headers=admin_headers,
        json={
            "name": "Developers",
            "description": "Dev workstations",
            "reboot_policy": "grace_period",
            "grace_period_minutes": 180,
            "enforce_tray": False,
            "allow_self_service": True,
            "allow_user_os_updates": True,
            "silent_notifications": False,
        },
    )
    assert g1_resp.status_code == 200
    g1 = g1_resp.json()
    assert g1["enforce_tray"] is False
    assert g1["allow_self_service"] is True

    assign1 = await client.patch(
        f"/api/v1/admin/devices/{dev_id}",
        headers=admin_headers,
        json={"group_ids": [g1["id"]]},
    )
    assert assign1.status_code == 200

    st1 = await client.get("/api/v1/agent/status", headers=agent_headers)
    assert st1.status_code == 200
    assert st1.json()["desktop_policy"] == {
        "enforce_tray": False,
        "allow_self_service": True,
        "allow_user_os_updates": True,
        "silent_notifications": False,
    }

    # Live update on group pushes catalog_updated to connected WS clients & updates status immediately
    pushed: list[dict] = []

    class RecWs:
        async def send_json(self, msg: dict) -> None:
            pushed.append(msg)

    rec_ws = RecWs()
    test_app.state.ws_hub.register(dev_id, rec_ws)  # type: ignore[arg-type]
    try:
        upd_g1 = await client.patch(
            f"/api/v1/admin/groups/{g1['id']}",
            headers=admin_headers,
            json={"allow_user_os_updates": False},
        )
        assert upd_g1.status_code == 200
        assert upd_g1.json()["allow_user_os_updates"] is False
        assert any(m.get("type") == "catalog_updated" for m in pushed)
    finally:
        test_app.state.ws_hub.unregister(dev_id, rec_ws)  # type: ignore[arg-type]

    groups_list = (await client.get("/api/v1/admin/groups", headers=admin_headers)).json()
    g1_listed = next(g for g in groups_list if g["id"] == g1["id"])
    assert g1_listed["enforce_tray"] is False
    assert g1_listed["allow_user_os_updates"] is False

    # 2nd group: restrictive combination across multiple groups
    g2 = (
        await client.post(
            "/api/v1/admin/groups",
            headers=admin_headers,
            json={
                "name": "KioskTerminals",
                "enforce_tray": True,
                "allow_self_service": False,
                "allow_user_os_updates": True,
                "silent_notifications": True,
            },
        )
    ).json()

    await client.patch(
        f"/api/v1/admin/devices/{dev_id}",
        headers=admin_headers,
        json={"group_ids": [g1["id"], g2["id"]]},
    )

    st_multi = await client.get("/api/v1/agent/status", headers=agent_headers)
    assert st_multi.status_code == 200
    assert st_multi.json()["desktop_policy"] == {
        "enforce_tray": True,  # any(False, True) -> True
        "allow_self_service": False,  # all(True, False) -> False
        "allow_user_os_updates": False,  # all(False, True) -> False
        "silent_notifications": True,  # any(False, True) -> True
    }


@pytest.mark.asyncio
async def test_inventory_center_running_reported_and_serialized(
    client: httpx.AsyncClient,
    admin_headers: dict[str, str],
) -> None:
    enr = await enroll_test_device(client, admin_headers, hostname="tray-pc-01")
    dev_id = enr["device_id"]
    agent_headers = {"Authorization": f"Bearer {enr['device_token']}"}

    init_detail = (
        await client.get(f"/api/v1/admin/devices/{dev_id}", headers=admin_headers)
    ).json()
    assert init_detail["center_running"] is False

    inv_on = await client.post(
        "/api/v1/agent/inventory",
        headers=agent_headers,
        json={
            "hostname": "tray-pc-01",
            "logged_in_users": ["CORP\\alice"],
            "center_running": True,
            "reboot_required": False,
        },
    )
    assert inv_on.status_code == 200

    devs = (await client.get("/api/v1/admin/devices", headers=admin_headers)).json()
    dev_row = next(d for d in devs if d["id"] == dev_id)
    assert dev_row["center_running"] is True

    detail_on = (
        await client.get(f"/api/v1/admin/devices/{dev_id}", headers=admin_headers)
    ).json()
    assert detail_on["center_running"] is True

    inv_off = await client.post(
        "/api/v1/agent/inventory",
        headers=agent_headers,
        json={
            "hostname": "tray-pc-01",
            "logged_in_users": [],
            "center_running": False,
        },
    )
    assert inv_off.status_code == 200
    detail_off = (
        await client.get(f"/api/v1/admin/devices/{dev_id}", headers=admin_headers)
    ).json()
    assert detail_off["center_running"] is False


@pytest.mark.asyncio
async def test_signed_remote_commands_sync_notify_reboot_and_ed25519_verification(
    client: httpx.AsyncClient,
    admin_headers: dict[str, str],
    test_app,
) -> None:
    enr = await enroll_test_device(client, admin_headers, hostname="remote-ctrl-pc")
    dev_id = enr["device_id"]
    pub_key_b64 = enr["server_public_key_b64"]
    agent_headers = {"Authorization": f"Bearer {enr['device_token']}"}

    pending_enr = await enroll_test_device(
        client, admin_headers, hostname="unapproved-pc", auto_approve=False
    )

    # Unauthenticated calls must return 401
    for path, body in (
        (f"/api/v1/admin/devices/{dev_id}/sync", None),
        (f"/api/v1/admin/devices/{dev_id}/notify", {"title": "T", "message": "M"}),
        (f"/api/v1/admin/devices/{dev_id}/reboot", {"mode": "cancel"}),
        ("/api/v1/admin/devices/bulk-action", {"device_ids": [dev_id], "action": "sync"}),
    ):
        unauth = await client.post(path, json=body)
        assert unauth.status_code == 401

    # Unapproved device must be rejected with 400
    bad_sync = await client.post(
        f"/api/v1/admin/devices/{pending_enr['device_id']}/sync",
        headers=admin_headers,
    )
    assert bad_sync.status_code == 400

    pushed_ws: list[dict] = []

    class RecWs:
        async def send_json(self, msg: dict) -> None:
            pushed_ws.append(msg)

    rec_ws = RecWs()
    test_app.state.ws_hub.register(dev_id, rec_ws)  # type: ignore[arg-type]
    try:
        # 1. /sync
        sync_resp = await client.post(
            f"/api/v1/admin/devices/{dev_id}/sync",
            headers=admin_headers,
        )
        assert sync_resp.status_code == 200
        sync_job = sync_resp.json()
        assert sync_job["job_type"] == "remote_control"
        assert sync_job["action"] == "force_sync"
        assert sync_job["target_id"] == "agent"
        assert sync_job["command"] == ""
        assert sync_job["respect_maintenance_window"] is False

        # 2. /notify (with non-ASCII German characters)
        notify_resp = await client.post(
            f"/api/v1/admin/devices/{dev_id}/notify",
            headers=admin_headers,
            json={
                "title": "Wartungshinweis ÄÖÜ",
                "message": "Bitte schließen Sie offene Dokumente vor 18:00 Uhr.",
                "urgent": True,
            },
        )
        assert notify_resp.status_code == 200
        notify_job = notify_resp.json()
        assert notify_job["job_type"] == "remote_control"
        assert notify_job["action"] == "admin_notify"
        assert notify_job["target_id"] == "desktop"
        assert "ÄÖÜ" in notify_job["command"]
        assert json.loads(notify_job["command"]) == {
            "title": "Wartungshinweis ÄÖÜ",
            "message": "Bitte schließen Sie offene Dokumente vor 18:00 Uhr.",
            "urgent": True,
        }

        # 3. /reboot (grace_period, immediate, cancel)
        for mode, grace_in, expected_grace in (
            ("grace_period", 15, 15),
            ("immediate", 60, 60),
            ("cancel", None, 60),
        ):
            payload = {"mode": mode} if grace_in is None else {"mode": mode, "grace_minutes": grace_in}
            rb_resp = await client.post(
                f"/api/v1/admin/devices/{dev_id}/reboot",
                headers=admin_headers,
                json=payload,
            )
            assert rb_resp.status_code == 200
            rb_job = rb_resp.json()
            assert rb_job["job_type"] == "remote_control"
            assert rb_job["action"] == "remote_reboot"
            assert rb_job["target_id"] == mode
            assert json.loads(rb_job["command"]) == {
                "mode": mode,
                "grace_minutes": expected_grace,
            }

        assert len(pushed_ws) == 5
        assert all(m["type"] == "job_available" for m in pushed_ws)
    finally:
        test_app.state.ws_hub.unregister(dev_id, rec_ws)  # type: ignore[arg-type]

    # Fetch via /api/v1/agent/jobs/pending and cryptographically verify all 5 Ed25519 signatures
    pending_jobs_resp = await client.get(
        "/api/v1/agent/jobs/pending", headers=agent_headers
    )
    assert pending_jobs_resp.status_code == 200
    sjobs = pending_jobs_resp.json()["jobs"]
    assert len(sjobs) == 5
    for sj in sjobs:
        canonical = SigningService.canonical_job_string(
            job_id=sj["job_id"],
            device_id=sj["device_id"],
            job_type=sj["job_type"],
            action=sj["action"],
            target_id=sj["target_id"],
            version=sj["version"],
            sha256=sj["sha256"],
            command=sj["command"],
            expires_at_unix=sj["expires_at_unix"],
        )
        assert SigningService.verify_canonical_with_b64_pubkey(
            pub_key_b64, canonical, sj["signature_b64"]
        )

    # Verify AuditLog entries recorded
    async with test_app.state.db.session_factory() as session:
        logs = list(
            (
                await session.execute(
                    select(AuditLog).where(AuditLog.target_id == dev_id)
                )
            )
            .scalars()
            .all()
        )
        actions = {log.action for log in logs}
        assert {"device.sync", "device.notify", "device.reboot"}.issubset(actions)


@pytest.mark.asyncio
async def test_bulk_device_actions_approve_assign_group_sync_scan_and_notify(
    client: httpx.AsyncClient,
    admin_headers: dict[str, str],
    test_app,
) -> None:
    d1 = await enroll_test_device(
        client, admin_headers, hostname="bulk-pc-01", auto_approve=False
    )
    d2 = await enroll_test_device(
        client, admin_headers, hostname="bulk-pc-02", auto_approve=False
    )
    d_rev = await enroll_test_device(
        client, admin_headers, hostname="bulk-pc-rev", auto_approve=True
    )
    await client.post(
        f"/api/v1/admin/devices/{d_rev['device_id']}/revoke",
        headers=admin_headers,
    )

    grp = (
        await client.post(
            "/api/v1/admin/groups",
            headers=admin_headers,
            json={"name": "BulkRolloutRing"},
        )
    ).json()
    pkg = (
        await client.post(
            "/api/v1/admin/catalog",
            headers=admin_headers,
            json={
                "name": "Mozilla Firefox ESR",
                "version": "128.4.0",
                "vendor": "Mozilla",
                "os_family": "windows",
                "source_type": "winget",
                "package_ref": "Mozilla.Firefox.ESR",
            },
        )
    ).json()
    await client.post(
        "/api/v1/admin/deployments",
        headers=admin_headers,
        json={
            "package_id": pkg["id"],
            "target_group_ids": [grp["id"]],
            "mode": "required",
            "respect_maintenance_window": False,
        },
    )

    # Validation errors on missing group_id / title+message
    assert (
        await client.post(
            "/api/v1/admin/devices/bulk-action",
            headers=admin_headers,
            json={"device_ids": [d1["device_id"]], "action": "assign_group"},
        )
    ).status_code == 400
    assert (
        await client.post(
            "/api/v1/admin/devices/bulk-action",
            headers=admin_headers,
            json={"device_ids": [d1["device_id"]], "action": "notify", "title": ""},
        )
    ).status_code == 400

    # 1. Bulk approve
    res_approve = await client.post(
        "/api/v1/admin/devices/bulk-action",
        headers=admin_headers,
        json={
            "device_ids": [d1["device_id"], d2["device_id"]],
            "action": "approve",
        },
    )
    assert res_approve.status_code == 200
    assert res_approve.json() == {"affected": 2, "job_ids": []}

    # 2. Bulk assign_group -> assigns group AND evaluates required deployment for both approved PCs
    res_group = await client.post(
        "/api/v1/admin/devices/bulk-action",
        headers=admin_headers,
        json={
            "device_ids": [d1["device_id"], d2["device_id"]],
            "action": "assign_group",
            "group_id": grp["id"],
        },
    )
    assert res_group.status_code == 200
    grp_body = res_group.json()
    assert grp_body["affected"] == 2
    assert len(grp_body["job_ids"]) == 2

    # 3. Bulk sync (revoked device is skipped cleanly)
    res_sync = await client.post(
        "/api/v1/admin/devices/bulk-action",
        headers=admin_headers,
        json={
            "device_ids": [d1["device_id"], d2["device_id"], d_rev["device_id"]],
            "action": "sync",
        },
    )
    assert res_sync.status_code == 200
    assert res_sync.json()["affected"] == 2
    assert len(res_sync.json()["job_ids"]) == 2

    # 4. Bulk scan_updates
    res_scan = await client.post(
        "/api/v1/admin/devices/bulk-action",
        headers=admin_headers,
        json={
            "device_ids": [d1["device_id"], d2["device_id"]],
            "action": "scan_updates",
        },
    )
    assert res_scan.status_code == 200
    assert res_scan.json()["affected"] == 2
    assert len(res_scan.json()["job_ids"]) == 2

    # 5. Bulk notify + Ed25519 signature verification
    res_notify = await client.post(
        "/api/v1/admin/devices/bulk-action",
        headers=admin_headers,
        json={
            "device_ids": [d1["device_id"], d2["device_id"]],
            "action": "notify",
            "title": "IT-Rundruf",
            "message": "VPN-Wartung heute um 20:00 Uhr.",
            "urgent": True,
        },
    )
    assert res_notify.status_code == 200
    notify_body = res_notify.json()
    assert notify_body["affected"] == 2
    assert len(notify_body["job_ids"]) == 2

    async with test_app.state.db.session_factory() as session:
        for jid in notify_body["job_ids"]:
            job = await session.get(Job, jid)
            assert job is not None
            assert job.job_type == "remote_control"
            assert job.action == "admin_notify"
            canonical = SigningService.canonical_job_string(
                job_id=job.id,
                device_id=job.device_id,
                job_type=job.job_type,
                action=job.action,
                target_id=job.target_id,
                version=job.version,
                sha256=job.sha256,
                command=job.command,
                expires_at_unix=job.expires_at_unix,
            )
            assert SigningService.verify_canonical_with_b64_pubkey(
                d1["server_public_key_b64"], canonical, job.signature_b64
            )


@pytest.mark.asyncio
async def test_hardened_bootstrap_installers_and_packaging_files(
    client: httpx.AsyncClient,
) -> None:
    token = "enroll-tok-hardened-2026"

    # 1. Windows PowerShell bootstrap (headless=0 & headless=1)
    ps1_ws = (await client.get(f"/dl/install.ps1?token={token}&headless=0")).text
    ps1_hl = (await client.get(f"/dl/install.ps1?token={token}&headless=1")).text

    for script_txt in (ps1_ws, ps1_hl):
        assert (
            "& sc.exe failure $ServiceName reset= 86400 actions= restart/5000/restart/10000/restart/30000 | Out-Null"
            in script_txt
        )
        assert "& sc.exe failureflag $ServiceName 1 | Out-Null" in script_txt

    assert "NS8SoftwareCenterGuardian" in ps1_ws
    assert 'New-ScheduledTaskPrincipal -GroupId "S-1-5-32-545" -RunLevel Limited' in ps1_ws
    assert "-AtLogOn" in ps1_ws
    assert "-MultipleInstances IgnoreNew" in ps1_ws
    assert 'Register-ScheduledTask -TaskName "NS8SoftwareCenterGuardian"' in ps1_ws
    assert 'Start-ScheduledTask -TaskName "NS8SoftwareCenterGuardian" -ErrorAction SilentlyContinue' in ps1_ws

    assert "Set-ItemProperty" not in ps1_hl
    assert (
        'Unregister-ScheduledTask -TaskName "NS8SoftwareCenterGuardian" -Confirm:$false -ErrorAction SilentlyContinue'
        in ps1_hl
    )

    static_ps1 = (PACKAGING_DIR / "install.ps1").read_text(encoding="utf-8")
    pwsh_bin = shutil.which("powershell") or shutil.which("pwsh")
    if pwsh_bin:
        for label, body in (
            ("dl_headless_0", ps1_ws),
            ("dl_headless_1", ps1_hl),
            ("packaging_install_ps1", static_ps1),
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
                input=body,
                text=True,
                capture_output=True,
                check=False,
            )
            assert proc.returncode == 0, f"PowerShell AST error in {label}: {proc.stderr}"

    # 2. Linux Bash bootstrap (headless=0 & headless=1) + agent/packaging/*
    sh_ws = (await client.get(f"/dl/install.sh?token={token}&headless=0")).text
    sh_hl = (await client.get(f"/dl/install.sh?token={token}&headless=1")).text

    assert "\r" not in sh_ws
    assert "/etc/xdg/autostart/ns8-swm-center.desktop" in sh_ws
    assert "/etc/systemd/user/ns8-swm-center.service" in sh_ws
    assert "Restart=always" in sh_ws
    assert "RestartSec=5" in sh_ws
    assert "ExecStart=/usr/local/bin/swm-center gui --tray-only" in sh_ws
    assert (
        "ln -sf /etc/systemd/user/ns8-swm-center.service /etc/systemd/user/default.target.wants/ns8-swm-center.service"
        in sh_ws
    )

    assert "\r" not in sh_hl
    assert "Exec=/usr/local/bin/swm-center gui --tray-only" not in sh_hl
    assert "ExecStart=/usr/local/bin/swm-center gui --tray-only" not in sh_hl
    assert "rm -f /etc/xdg/autostart/ns8-swm-center.desktop" in sh_hl
    assert "/etc/systemd/user/default.target.wants/ns8-swm-center.service" in sh_hl
    assert "/etc/systemd/user/ns8-swm-center.service" in sh_hl

    # Verify agent/packaging/* files exist, configure user service + symlink, and have pure LF (\n)
    nfpm_txt = (PACKAGING_DIR / "nfpm.yaml").read_text(encoding="utf-8")
    user_unit_txt = (PACKAGING_DIR / "ns8-swm-center.service").read_text(encoding="utf-8")
    pkg_sh_txt = (PACKAGING_DIR / "install.sh").read_text(encoding="utf-8")

    assert "/etc/systemd/user/ns8-swm-center.service" in nfpm_txt
    assert "/etc/systemd/user/default.target.wants/ns8-swm-center.service" in nfpm_txt
    assert 'type: "symlink"' in nfpm_txt

    assert "ExecStart=/usr/local/bin/swm-center gui --tray-only" in user_unit_txt
    assert "Restart=always" in user_unit_txt
    assert "RestartSec=5" in user_unit_txt
    assert "/etc/systemd/user/ns8-swm-center.service" in pkg_sh_txt

    for pkg_file in PACKAGING_DIR.iterdir():
        if pkg_file.is_file() and pkg_file.suffix.lower() != ".ps1":
            raw = pkg_file.read_bytes()
            assert b"\r" not in raw, f"CR/CRLF found in {pkg_file}"
