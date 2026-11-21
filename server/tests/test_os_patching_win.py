from __future__ import annotations

import httpx
import pytest

from tests.conftest import enroll_test_device


@pytest.mark.asyncio
async def test_windows_wua_patching_kb_exclusion_and_approval(
    client: httpx.AsyncClient, admin_headers: dict[str, str]
) -> None:
    dev = await enroll_test_device(
        client, admin_headers, hostname="win11-ws-05", os_family="windows"
    )
    dev_headers = {"Authorization": f"Bearer {dev['device_token']}"}

    # Agent reports pending Windows Updates
    inv = await client.post(
        "/api/v1/agent/inventory",
        headers=dev_headers,
        json={
            "pending_os_updates": [
                {
                    "update_id": "KB5044285",
                    "title": "2024-10 Cumulative Update for Windows 11 23H2",
                    "severity": "critical",
                    "category": "security",
                    "size_bytes": 540_000_000,
                    "reboot_behavior": "requires_reboot",
                },
                {
                    "update_id": "KB5034441",
                    "title": "Windows Recovery Environment Update",
                    "severity": "important",
                    "category": "security",
                    "size_bytes": 25_000_000,
                    "reboot_behavior": "may_require_reboot",
                },
            ]
        },
    )
    assert inv.status_code == 200

    patches = (await client.get("/api/v1/admin/patches", headers=admin_headers)).json()
    assert len(patches) == 2

    # Admin excludes broken KB5034441
    excl = await client.post(
        "/api/v1/admin/patches/exclude",
        headers=admin_headers,
        json={"update_ids": ["KB5034441"], "excluded": True},
    )
    assert excl.status_code == 200

    # Agent status endpoint exposes excluded_updates
    ag_status = (await client.get("/api/v1/agent/status", headers=dev_headers)).json()
    assert "KB5034441" in ag_status["excluded_updates"]

    # Admin approves KB5044285 immediately (respect_maintenance_window=False)
    appr = await client.post(
        "/api/v1/admin/patches/approve",
        headers=admin_headers,
        json={
            "update_ids": ["KB5044285"],
            "group_ids": [],
            "respect_maintenance_window": False,
        },
    )
    assert appr.status_code == 200
    assert appr.json()["jobs_created"] == 1

    pending = (await client.get("/api/v1/agent/jobs/pending", headers=dev_headers)).json()
    assert len(pending["jobs"]) == 1
    pjob = pending["jobs"][0]
    assert pjob["job_type"] == "os_patch"
    assert pjob["update_ids"] == ["KB5044285"]
    assert "KB5034441" not in pjob["update_ids"]
