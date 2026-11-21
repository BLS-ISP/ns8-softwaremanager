from __future__ import annotations

import httpx
import pytest

from tests.conftest import enroll_test_device


@pytest.mark.asyncio
async def test_groups_tags_and_software_delta_inventory(
    client: httpx.AsyncClient, admin_headers: dict[str, str]
) -> None:
    # Create two groups
    g1 = (
        await client.post(
            "/api/v1/admin/groups",
            headers=admin_headers,
            json={
                "name": "Pilot-Ring",
                "description": "Early adopters",
                "reboot_policy": "grace_period",
                "grace_period_minutes": 120,
            },
        )
    ).json()
    g2 = (
        await client.post(
            "/api/v1/admin/groups",
            headers=admin_headers,
            json={"name": "Buchhaltung", "reboot_policy": "never"},
        )
    ).json()

    dev = await enroll_test_device(client, admin_headers, hostname="acct-pc-01")
    dev_id = dev["device_id"]
    dev_headers = {"Authorization": f"Bearer {dev['device_token']}"}

    # Assign group and tags via PATCH /api/v1/admin/devices/{id}
    patched = await client.patch(
        f"/api/v1/admin/devices/{dev_id}",
        headers=admin_headers,
        json={"group_ids": [g1["id"], g2["id"]], "tags": ["finance", "laptop"]},
    )
    assert patched.status_code == 200
    assert set(patched.json()["group_ids"]) == {g1["id"], g2["id"]}
    assert patched.json()["tags"] == ["finance", "laptop"]

    # Submit initial software inventory with explicit software_hash
    sw_list = [
        {
            "name": "Mozilla Firefox",
            "version": "131.0.3",
            "publisher": "Mozilla",
            "source": "winget",
            "scope": "machine",
            "package_ref": "Mozilla.Firefox",
        },
        {
            "name": "7-Zip 24.08 (x64)",
            "version": "24.08",
            "publisher": "Igor Pavlov",
            "source": "registry",
            "scope": "machine",
            "package_ref": "7zip.7zip",
        },
    ]
    r1 = await client.post(
        "/api/v1/agent/inventory",
        headers=dev_headers,
        json={"software_hash": "hash-v1", "installed_software": sw_list},
    )
    assert r1.status_code == 200
    assert r1.json()["software_updated"] is True

    # Re-sending identical software_hash does not rewrite rows
    r2 = await client.post(
        "/api/v1/agent/inventory",
        headers=dev_headers,
        json={"software_hash": "hash-v1", "installed_software": sw_list},
    )
    assert r2.status_code == 200
    assert r2.json()["software_updated"] is False

    # Omitting installed_software (None) on heartbeat-inventory preserves software inventory!
    r3 = await client.post(
        "/api/v1/agent/inventory",
        headers=dev_headers,
        json={"reboot_required": False, "installed_software": None},
    )
    assert r3.status_code == 200
    assert r3.json()["software_updated"] is False

    detail = (
        await client.get(f"/api/v1/admin/devices/{dev_id}", headers=admin_headers)
    ).json()
    assert detail["installed_software_count"] == 2

    # Filter devices by group_id, search (tag / software name), and software query
    by_grp = (
        await client.get(
            f"/api/v1/admin/devices?group_id={g1['id']}", headers=admin_headers
        )
    ).json()
    assert len(by_grp) == 1 and by_grp[0]["id"] == dev_id

    by_search = (
        await client.get(
            "/api/v1/admin/devices?search=131.0.3", headers=admin_headers
        )
    ).json()
    assert len(by_search) == 1 and by_search[0]["id"] == dev_id

    by_sw = (
        await client.get(
            "/api/v1/admin/devices?software=7zip.7zip", headers=admin_headers
        )
    ).json()
    assert len(by_sw) == 1

    # Update & delete group
    upd_grp = await client.patch(
        f"/api/v1/admin/groups/{g2['id']}",
        headers=admin_headers,
        json={"description": "Updated accounting"},
    )
    assert upd_grp.status_code == 200
    assert upd_grp.json()["device_count"] == 1

    del_grp = await client.delete(
        f"/api/v1/admin/groups/{g2['id']}", headers=admin_headers
    )
    assert del_grp.status_code == 200
