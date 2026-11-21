from __future__ import annotations

import httpx
import pytest

from app.crypto import SigningService
from tests.conftest import enroll_test_device


@pytest.mark.asyncio
async def test_reboot_policies_exit_3010_and_signed_self_update(
    client: httpx.AsyncClient, admin_headers: dict[str, str]
) -> None:
    g_never = (
        await client.post(
            "/api/v1/admin/groups",
            headers=admin_headers,
            json={"name": "Critical-Servers", "reboot_policy": "never"},
        )
    ).json()
    g_grace = (
        await client.post(
            "/api/v1/admin/groups",
            headers=admin_headers,
            json={
                "name": "Desktops-Grace",
                "reboot_policy": "grace_period",
                "grace_period_minutes": 180,
            },
        )
    ).json()

    desk = await enroll_test_device(
        client,
        admin_headers,
        hostname="desk-01",
        os_family="windows",
        group_id=g_grace["id"],
    )
    desk_headers = {"Authorization": f"Bearer {desk['device_token']}"}

    st1 = (await client.get("/api/v1/agent/status", headers=desk_headers)).json()
    assert st1["reboot_policy"] == "grace_period"
    assert st1["grace_period_minutes"] == 180

    # Adding Critical-Servers (never) overrides grace_period conservatively
    await client.patch(
        f"/api/v1/admin/devices/{desk['device_id']}",
        headers=admin_headers,
        json={"group_ids": [g_grace["id"], g_never["id"]]},
    )
    st2 = (await client.get("/api/v1/agent/status", headers=desk_headers)).json()
    assert st2["reboot_policy"] == "never"

    # Trigger signed agent self-update
    fake_sha = "b" * 64
    su = await client.post(
        "/api/v1/admin/self-update",
        headers=admin_headers,
        json={
            "version": "1.2.0",
            "os_family": "windows",
            "sha256": fake_sha,
            "download_url": "/dl/agents/windows/swm-agent.exe",
            "respect_maintenance_window": False,
        },
    )
    assert su.status_code == 200
    assert su.json()["jobs_created"] == 1

    pending = (
        await client.get("/api/v1/agent/jobs/pending", headers=desk_headers)
    ).json()
    assert len(pending["jobs"]) == 1
    su_job = pending["jobs"][0]
    assert su_job["job_type"] == "self_update"
    assert su_job["version"] == "1.2.0"
    assert su_job["sha256"] == fake_sha

    canonical = SigningService.canonical_job_string(
        job_id=su_job["job_id"],
        device_id=su_job["device_id"],
        job_type=su_job["job_type"],
        action=su_job["action"],
        target_id=su_job["target_id"],
        version=su_job["version"],
        sha256=su_job["sha256"],
        command=su_job["command"],
        expires_at_unix=su_job["expires_at_unix"],
    )
    assert SigningService.verify_canonical_with_b64_pubkey(
        desk["server_public_key_b64"], canonical, su_job["signature_b64"]
    )

    # Exit code 3010 marks job and device reboot_required = True
    await client.post(
        f"/api/v1/agent/jobs/{su_job['job_id']}/result",
        headers=desk_headers,
        json={"status": "succeeded", "exit_code": 3010, "reboot_required": False},
    )
    dev_detail = (
        await client.get(
            f"/api/v1/admin/devices/{desk['device_id']}", headers=admin_headers
        )
    ).json()
    assert dev_detail["reboot_required"] is True
