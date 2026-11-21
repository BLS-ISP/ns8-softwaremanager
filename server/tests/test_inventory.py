from __future__ import annotations

import httpx
import pytest

from tests.conftest import enroll_test_device


@pytest.mark.asyncio
async def test_hardware_os_and_reboot_inventory_end_to_end(
    client: httpx.AsyncClient, admin_headers: dict[str, str]
) -> None:
    # Pending device must be rejected (403) when submitting inventory
    pending = await enroll_test_device(
        client, admin_headers, hostname="pending-box", auto_approve=False
    )
    rej = await client.post(
        "/api/v1/agent/inventory",
        headers={"Authorization": f"Bearer {pending['device_token']}"},
        json={"reboot_required": False},
    )
    assert rej.status_code == 403

    # Approve device and submit rich Windows hardware/OS/reboot inventory
    await client.post(
        f"/api/v1/admin/devices/{pending['device_id']}/approve",
        headers=admin_headers,
    )
    inv_resp = await client.post(
        "/api/v1/agent/inventory",
        headers={"Authorization": f"Bearer {pending['device_token']}"},
        json={
            "hostname": "win-srv-2022",
            "domain": "CORP.LOCAL",
            "os_family": "windows",
            "os_name": "Windows Server 2022 Standard",
            "os_version": "21H2",
            "kernel_or_build": "20348.2762",
            "arch": "amd64",
            "agent_version": "1.0.2",
            "hardware": {
                "cpu_model": "Intel Xeon E-2388G",
                "cpu_cores": 16,
                "ram_total_mb": 65536,
                "ram_free_mb": 42000,
                "disks": [
                    {"mount": "C:\\", "total_gb": 500.0, "free_gb": 312.4},
                    {"mount": "D:\\", "total_gb": 2000.0, "free_gb": 1540.0},
                ],
                "serial_number": "SRV-SN-9901",
            },
            "ip_addresses": ["10.10.1.25", "10.10.2.25"],
            "logged_in_users": ["CORP\\Administrator"],
            "reboot_required": True,
        },
    )
    assert inv_resp.status_code == 200
    assert inv_resp.json()["status"] == "ok"

    detail = await client.get(
        f"/api/v1/admin/devices/{pending['device_id']}", headers=admin_headers
    )
    assert detail.status_code == 200
    body = detail.json()
    assert body["hostname"] == "win-srv-2022"
    assert body["domain"] == "CORP.LOCAL"
    assert body["kernel_or_build"] == "20348.2762"
    assert body["reboot_required"] is True
    assert body["hardware"]["cpu_cores"] == 16
    assert len(body["hardware"]["disks"]) == 2
    assert "10.10.1.25" in body["ip_addresses"]

    # Dashboard reflects reboot_required_devices == 1
    dash = await client.get("/api/v1/admin/dashboard/summary", headers=admin_headers)
    assert dash.status_code == 200
    assert dash.json()["reboot_required_devices"] == 1
