from __future__ import annotations

import httpx
import pytest

from tests.conftest import enroll_test_device


@pytest.mark.asyncio
async def test_linux_apt_dnf_security_patching_and_compliance_dashboard(
    client: httpx.AsyncClient, admin_headers: dict[str, str]
) -> None:
    deb = await enroll_test_device(
        client, admin_headers, hostname="deb-web-01", os_family="linux"
    )
    rocky = await enroll_test_device(
        client, admin_headers, hostname="rocky-db-01", os_family="linux"
    )
    win = await enroll_test_device(
        client, admin_headers, hostname="win-clean-01", os_family="windows"
    )

    deb_headers = {"Authorization": f"Bearer {deb['device_token']}"}
    rocky_headers = {"Authorization": f"Bearer {rocky['device_token']}"}

    await client.post(
        "/api/v1/agent/inventory",
        headers=deb_headers,
        json={
            "os_name": "Ubuntu 24.04 LTS",
            "pending_os_updates": [
                {
                    "update_id": "openssl-3.0.13-1ubuntu3.4",
                    "title": "OpenSSL Security Update (USN-6999-1)",
                    "severity": "critical",
                    "category": "security",
                    "size_bytes": 2_100_000,
                },
                {
                    "update_id": "htop-3.3.0-4",
                    "title": "htop minor bugfix",
                    "severity": "low",
                    "category": "bugfix",
                    "size_bytes": 160_000,
                },
            ],
        },
    )
    await client.post(
        "/api/v1/agent/inventory",
        headers=rocky_headers,
        json={
            "os_name": "Rocky Linux 9.4",
            "pending_os_updates": [
                {
                    "update_id": "kernel-5.14.0-427.40.1.el9_4",
                    "title": "Important: kernel security update",
                    "severity": "important",
                    "category": "security",
                    "size_bytes": 68_000_000,
                }
            ],
        },
    )

    # Check cross-platform compliance dashboard before patching:
    # 3 approved devices (deb, rocky have critical/important security updates; win is clean -> 1/3 = 33.3%)
    dash1 = (
        await client.get("/api/v1/admin/dashboard/summary", headers=admin_headers)
    ).json()
    assert dash1["total_devices"] == 3
    assert dash1["os_breakdown"] == {"windows": 1, "linux": 2}
    assert dash1["pending_critical_patches"] == 2
    assert dash1["patch_compliance_percent"] == 33.3

    # Approve security-only updates on Debian/Rocky
    appr = await client.post(
        "/api/v1/admin/patches/approve",
        headers=admin_headers,
        json={
            "update_ids": [
                "openssl-3.0.13-1ubuntu3.4",
                "htop-3.3.0-4",
                "kernel-5.14.0-427.40.1.el9_4",
            ],
            "security_only": True,
            "respect_maintenance_window": False,
        },
    )
    assert appr.status_code == 200
    assert appr.json()["jobs_created"] == 2

    deb_jobs = (
        await client.get("/api/v1/agent/jobs/pending", headers=deb_headers)
    ).json()["jobs"]
    assert len(deb_jobs) == 1
    # Only the security update is included when security_only=True!
    assert deb_jobs[0]["update_ids"] == ["openssl-3.0.13-1ubuntu3.4"]
    assert deb_jobs[0]["security_only"] is True
    assert deb_jobs[0]["command"] == "security"

    rocky_jobs = (
        await client.get("/api/v1/agent/jobs/pending", headers=rocky_headers)
    ).json()["jobs"]
    assert len(rocky_jobs) == 1

    # Complete both patch jobs
    await client.post(
        f"/api/v1/agent/jobs/{deb_jobs[0]['job_id']}/result",
        headers=deb_headers,
        json={"status": "succeeded", "exit_code": 0, "reboot_required": False},
    )
    await client.post(
        f"/api/v1/agent/jobs/{rocky_jobs[0]['job_id']}/result",
        headers=rocky_headers,
        json={"status": "succeeded", "exit_code": 0, "reboot_required": True},
    )

    # Now all 3 devices have 0 pending critical/security updates -> 100.0% compliance!
    dash2 = (
        await client.get("/api/v1/admin/dashboard/summary", headers=admin_headers)
    ).json()
    assert dash2["pending_critical_patches"] == 0
    assert dash2["patch_compliance_percent"] == 100.0
    assert dash2["reboot_required_devices"] == 1
    assert win["device_id"] != ""
