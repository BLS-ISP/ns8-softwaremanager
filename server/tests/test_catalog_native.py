from __future__ import annotations

import httpx
import pytest


@pytest.mark.asyncio
async def test_native_catalog_crud_and_filters(
    client: httpx.AsyncClient, admin_headers: dict[str, str]
) -> None:
    entries = [
        {
            "name": "Mozilla Firefox",
            "version": "132.0",
            "vendor": "Mozilla",
            "description": "Enterprise Web Browser",
            "category": "Browsers",
            "os_family": "windows",
            "source_type": "winget",
            "package_ref": "Mozilla.Firefox",
            "blocked_processes": ["firefox.exe"],
            "allow_uninstall": True,
        },
        {
            "name": "GIMP",
            "version": "2.10.38",
            "vendor": "GIMP Team",
            "description": "GNU Image Manipulation Program",
            "category": "Graphics",
            "os_family": "linux",
            "source_type": "flatpak",
            "package_ref": "org.gimp.GIMP",
            "blocked_processes": ["gimp-2.10"],
            "allow_uninstall": False,
        },
        {
            "name": "curl",
            "version": "8.5.0",
            "vendor": "haxx",
            "category": "CLI",
            "os_family": "linux",
            "source_type": "apt",
            "package_ref": "curl",
        },
    ]

    created_ids = []
    for e in entries:
        r = await client.post("/api/v1/admin/catalog", headers=admin_headers, json=e)
        assert r.status_code == 200, r.text
        created_ids.append(r.json()["id"])

    win_list = (
        await client.get(
            "/api/v1/admin/catalog?os_family=windows", headers=admin_headers
        )
    ).json()
    assert len(win_list) == 1
    assert win_list[0]["blocked_processes"] == ["firefox.exe"]

    flatpak_list = (
        await client.get(
            "/api/v1/admin/catalog?source_type=flatpak", headers=admin_headers
        )
    ).json()
    assert len(flatpak_list) == 1
    assert flatpak_list[0]["allow_uninstall"] is False

    # Patch Firefox version
    upd = await client.patch(
        f"/api/v1/admin/catalog/{created_ids[0]}",
        headers=admin_headers,
        json={"version": "132.0.1"},
    )
    assert upd.status_code == 200
    assert upd.json()["version"] == "132.0.1"

    # Delete curl
    rem = await client.delete(
        f"/api/v1/admin/catalog/{created_ids[2]}", headers=admin_headers
    )
    assert rem.status_code == 200
    remaining = (
        await client.get("/api/v1/admin/catalog", headers=admin_headers)
    ).json()
    assert len(remaining) == 2
