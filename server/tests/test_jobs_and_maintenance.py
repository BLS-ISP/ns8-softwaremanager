from __future__ import annotations

from datetime import datetime, timezone

import httpx
import pytest

from app.crypto import SigningService
from tests.conftest import enroll_test_device


@pytest.mark.asyncio
async def test_maintenance_windows_catch_up_and_signed_job_lifecycle(
    client: httpx.AsyncClient,
    admin_headers: dict[str, str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # Monday 2026-11-16 10:00 UTC (weekday() == 0)
    t_monday_morning = datetime(2026, 11, 16, 10, 0, tzinfo=timezone.utc)
    # Wednesday 2026-11-18 02:30 UTC (weekday() == 2, inside 02:00-04:00 UTC window)
    t_wed_inside = datetime(2026, 11, 18, 2, 30, tzinfo=timezone.utc)
    # Thursday 2026-11-19 09:00 UTC (after Wednesday window passed)
    t_thu_after = datetime(2026, 11, 19, 9, 0, tzinfo=timezone.utc)

    current = {"now": t_monday_morning}
    monkeypatch.setattr("app.services.scheduler.current_utc_time", lambda: current["now"])
    monkeypatch.setattr("app.api.agent_api.current_utc_time", lambda: current["now"])
    monkeypatch.setattr(
        "app.api.admin_catalog_deployments.current_utc_time", lambda: current["now"]
    )

    grp_catchup = (
        await client.post(
            "/api/v1/admin/groups",
            headers=admin_headers,
            json={"name": "Laptops-CatchUp", "reboot_policy": "grace_period"},
        )
    ).json()
    grp_strict = (
        await client.post(
            "/api/v1/admin/groups",
            headers=admin_headers,
            json={"name": "Servers-Strict", "reboot_policy": "maintenance_window"},
        )
    ).json()

    # Window on Wednesday (day 2) 02:00-04:00 UTC
    mw1 = await client.post(
        "/api/v1/admin/maintenance-windows",
        headers=admin_headers,
        json={
            "group_id": grp_catchup["id"],
            "name": "Wed Night CatchUp",
            "days_of_week": [2],
            "start_time": "02:00",
            "end_time": "04:00",
            "timezone": "UTC",
            "catch_up": True,
        },
    )
    assert mw1.status_code == 200
    mw2 = await client.post(
        "/api/v1/admin/maintenance-windows",
        headers=admin_headers,
        json={
            "group_id": grp_strict["id"],
            "name": "Wed Night Strict",
            "days_of_week": [2],
            "start_time": "02:00",
            "end_time": "04:00",
            "timezone": "UTC",
            "catch_up": False,
        },
    )
    assert mw2.status_code == 200

    dev_catchup = await enroll_test_device(
        client, admin_headers, hostname="lap-01", group_id=grp_catchup["id"]
    )
    dev_strict = await enroll_test_device(
        client, admin_headers, hostname="srv-01", group_id=grp_strict["id"]
    )

    pkg = (
        await client.post(
            "/api/v1/admin/catalog",
            headers=admin_headers,
            json={
                "name": "Git",
                "version": "2.47.0",
                "os_family": "windows",
                "source_type": "winget",
                "package_ref": "Git.Git",
            },
        )
    ).json()

    # Create required deployment on Monday morning respecting maintenance window
    dep = await client.post(
        "/api/v1/admin/deployments",
        headers=admin_headers,
        json={
            "package_id": pkg["id"],
            "target_group_ids": [grp_catchup["id"], grp_strict["id"]],
            "mode": "required",
            "respect_maintenance_window": True,
        },
    )
    assert dep.status_code == 200

    # 1) On Monday morning (before Wed window): BOTH devices get 0 jobs!
    p_lap_mon = (
        await client.get(
            "/api/v1/agent/jobs/pending",
            headers={"Authorization": f"Bearer {dev_catchup['device_token']}"},
        )
    ).json()
    p_srv_mon = (
        await client.get(
            "/api/v1/agent/jobs/pending",
            headers={"Authorization": f"Bearer {dev_strict['device_token']}"},
        )
    ).json()
    assert p_lap_mon["jobs"] == []
    assert p_srv_mon["jobs"] == []

    # 2) Suppose Wednesday window passes while BOTH devices were offline -> now Thursday 09:00 UTC!
    current["now"] = t_thu_after

    # Strict server (catch_up=False) STILL holds the job back outside window!
    p_srv_thu = (
        await client.get(
            "/api/v1/agent/jobs/pending",
            headers={"Authorization": f"Bearer {dev_strict['device_token']}"},
        )
    ).json()
    assert p_srv_thu["jobs"] == []

    # Catch-up laptop (catch_up=True) dispatches the missed job immediately!
    p_lap_thu = (
        await client.get(
            "/api/v1/agent/jobs/pending",
            headers={"Authorization": f"Bearer {dev_catchup['device_token']}"},
        )
    ).json()
    assert len(p_lap_thu["jobs"]) == 1
    sjob = p_lap_thu["jobs"][0]

    # Verify Ed25519 signature of SignedJob using public_key_b64 from enroll
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
        dev_catchup["server_public_key_b64"], canonical, sjob["signature_b64"]
    )

    # 3) Inside Wednesday window -> strict server ALSO gets its job!
    current["now"] = t_wed_inside
    p_srv_wed = (
        await client.get(
            "/api/v1/agent/jobs/pending",
            headers={"Authorization": f"Bearer {dev_strict['device_token']}"},
        )
    ).json()
    assert len(p_srv_wed["jobs"]) == 1

    # Report job result with stdout/stderr and exit_code
    res_post = await client.post(
        f"/api/v1/agent/jobs/{sjob['job_id']}/result",
        headers={"Authorization": f"Bearer {dev_catchup['device_token']}"},
        json={
            "status": "succeeded",
            "exit_code": 0,
            "reboot_required": False,
            "stdout": "Successfully installed Git 2.47.0\n",
            "stderr": "",
        },
    )
    assert res_post.status_code == 200

    job_admin = (
        await client.get(f"/api/v1/admin/jobs/{sjob['job_id']}", headers=admin_headers)
    ).json()
    assert job_admin["status"] == "succeeded"
    assert job_admin["exit_code"] == 0
    assert "Git 2.47.0" in job_admin["stdout"]
