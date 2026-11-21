from __future__ import annotations

import httpx
import pytest

from app.crypto import SigningService
from tests.conftest import enroll_test_device


@pytest.mark.asyncio
async def test_deployments_required_3rd_party_upgrade_uninstall_and_self_service(
    client: httpx.AsyncClient, admin_headers: dict[str, str]
) -> None:
    grp = (
        await client.post(
            "/api/v1/admin/groups",
            headers=admin_headers,
            json={"name": "Workstations", "reboot_policy": "grace_period"},
        )
    ).json()

    dev = await enroll_test_device(
        client,
        admin_headers,
        hostname="ws-office-10",
        os_family="windows",
        group_id=grp["id"],
    )
    dev_headers = {"Authorization": f"Bearer {dev['device_token']}"}
    pub_b64 = dev["server_public_key_b64"]

    # 1) Create catalog item 7-Zip 24.08 (Required) and Notepad++ (Available, allow_uninstall=False)
    pkg_7z = (
        await client.post(
            "/api/v1/admin/catalog",
            headers=admin_headers,
            json={
                "name": "7-Zip",
                "version": "24.08",
                "vendor": "Igor Pavlov",
                "os_family": "windows",
                "source_type": "winget",
                "package_ref": "7zip.7zip",
                "install_args": "--silent",
            },
        )
    ).json()

    pkg_npp = (
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
                "allow_uninstall": False,
            },
        )
    ).json()

    pkg_unassigned = (
        await client.post(
            "/api/v1/admin/catalog",
            headers=admin_headers,
            json={
                "name": "Restricted Admin Tool",
                "version": "1.0.0",
                "os_family": "windows",
                "source_type": "winget",
                "package_ref": "Corp.AdminTool",
            },
        )
    ).json()

    # Device initially reports outdated 7-Zip 24.07 installed (3rd-party update scenario!)
    await client.post(
        "/api/v1/agent/inventory",
        headers=dev_headers,
        json={
            "installed_software": [
                {
                    "name": "7-Zip 24.07 (x64)",
                    "version": "24.07",
                    "publisher": "Igor Pavlov",
                    "source": "winget",
                    "package_ref": "7zip.7zip",
                }
            ]
        },
    )

    # Deploy 7-Zip 24.08 as REQUIRED -> automatically enqueues an "update" job!
    dep_req = await client.post(
        "/api/v1/admin/deployments",
        headers=admin_headers,
        json={
            "package_id": pkg_7z["id"],
            "target_group_ids": [grp["id"]],
            "mode": "required",
            "respect_maintenance_window": False,
        },
    )
    assert dep_req.status_code == 200
    assert dep_req.json()["stats"]["pending"] == 1

    # Deploy Notepad++ as AVAILABLE -> appears in SignedCatalogItem, 0 auto-jobs!
    dep_avail = await client.post(
        "/api/v1/admin/deployments",
        headers=admin_headers,
        json={
            "package_id": pkg_npp["id"],
            "target_group_ids": [grp["id"]],
            "mode": "available",
            "respect_maintenance_window": False,
        },
    )
    assert dep_avail.status_code == 200

    pending = (
        await client.get("/api/v1/agent/jobs/pending", headers=dev_headers)
    ).json()
    assert len(pending["jobs"]) == 1
    up_job = pending["jobs"][0]
    assert up_job["target_id"] == pkg_7z["id"]
    assert up_job["action"] == "update"
    assert up_job["version"] == "24.08"
    assert up_job["package_meta"]["filename"] == ""  # Never None for Rust serde String!

    # Available catalog has Notepad++ with valid Ed25519 signature
    assert len(pending["available_catalog"]) == 1
    cat_item = pending["available_catalog"][0]
    assert cat_item["package_id"] == pkg_npp["id"]
    assert cat_item["allow_uninstall"] is False
    cat_canonical = SigningService.canonical_catalog_string(
        package_id=cat_item["package_id"],
        device_id=cat_item["device_id"],
        source_type=cat_item["source_type"],
        package_ref=cat_item["package_ref"],
        version=cat_item["version"],
        sha256=cat_item["sha256"],
        install_cmd=cat_item["install_cmd"],
        uninstall_cmd=cat_item["uninstall_cmd"],
        allow_uninstall=cat_item["allow_uninstall"],
    )
    assert SigningService.verify_canonical_with_b64_pubkey(
        pub_b64, cat_canonical, cat_item["signature_b64"]
    )

    # Complete the 7-Zip update job -> deployment stats switch to installed == 1
    await client.post(
        f"/api/v1/agent/jobs/{up_job['job_id']}/result",
        headers=dev_headers,
        json={"status": "succeeded", "exit_code": 0},
    )
    deps_after = (
        await client.get("/api/v1/admin/deployments", headers=admin_headers)
    ).json()
    req_stats = next(d for d in deps_after if d["id"] == dep_req.json()["id"])["stats"]
    assert req_stats["installed"] == 1
    assert req_stats["pending"] == 0

    # Self-service trigger: unassigned package MUST be rejected with 403
    unauth_trig = await client.post(
        "/api/v1/agent/self-service/trigger",
        headers=dev_headers,
        json={"package_id": pkg_unassigned["id"], "action": "install"},
    )
    assert unauth_trig.status_code == 403

    # Self-service trigger: uninstall when allow_uninstall=False MUST be rejected with 403
    disallowed_uninst = await client.post(
        "/api/v1/agent/self-service/trigger",
        headers=dev_headers,
        json={"package_id": pkg_npp["id"], "action": "uninstall"},
    )
    assert disallowed_uninst.status_code == 403

    # Self-service trigger: install assigned available package -> returns valid SignedJob immediately!
    ok_trig = await client.post(
        "/api/v1/agent/self-service/trigger",
        headers=dev_headers,
        json={"package_id": pkg_npp["id"], "action": "install"},
    )
    assert ok_trig.status_code == 200
    ss_job = ok_trig.json()
    assert ss_job["target_id"] == pkg_npp["id"]
    assert ss_job["action"] == "install"
    job_canonical = SigningService.canonical_job_string(
        job_id=ss_job["job_id"],
        device_id=ss_job["device_id"],
        job_type=ss_job["job_type"],
        action=ss_job["action"],
        target_id=ss_job["target_id"],
        version=ss_job["version"],
        sha256=ss_job["sha256"],
        command=ss_job["command"],
        expires_at_unix=ss_job["expires_at_unix"],
    )
    assert SigningService.verify_canonical_with_b64_pubkey(
        pub_b64, job_canonical, ss_job["signature_b64"]
    )

    # Test UNINSTALL deployment mode: remove required deployment and deploy uninstall for 7-Zip
    await client.delete(
        f"/api/v1/admin/deployments/{dep_req.json()['id']}", headers=admin_headers
    )
    dep_uninst = await client.post(
        "/api/v1/admin/deployments",
        headers=admin_headers,
        json={
            "package_id": pkg_7z["id"],
            "target_device_ids": [dev["device_id"]],
            "mode": "uninstall",
            "respect_maintenance_window": False,
        },
    )
    assert dep_uninst.status_code == 200

    pending_uninst = (
        await client.get("/api/v1/agent/jobs/pending", headers=dev_headers)
    ).json()["jobs"]
    uninst_jobs = [
        j
        for j in pending_uninst
        if j["target_id"] == pkg_7z["id"] and j["action"] == "uninstall"
    ]
    assert len(uninst_jobs) == 1


@pytest.mark.asyncio
async def test_custom_required_package_not_duplicated_on_real_os_inventory_resync(
    client: httpx.AsyncClient, admin_headers: dict[str, str]
) -> None:
    dev = await enroll_test_device(
        client, admin_headers, hostname="win-custom-01", os_family="windows"
    )
    dev_headers = {"Authorization": f"Bearer {dev['device_token']}"}

    # Device reports initial real OS Registry software
    await client.post(
        "/api/v1/agent/inventory",
        headers=dev_headers,
        json={
            "software_hash": "os-hash-1",
            "installed_software": [
                {
                    "name": "Microsoft Edge",
                    "version": "130.0",
                    "publisher": "Microsoft",
                    "source": "registry",
                    "scope": "machine",
                }
            ],
        },
    )

    # Create a custom .ps1 script package and deploy as required
    custom_pkg = (
        await client.post(
            "/api/v1/admin/catalog",
            headers=admin_headers,
            json={
                "name": "Corp Root CA Installer",
                "version": "1.0.0",
                "vendor": "Corp IT",
                "os_family": "windows",
                "source_type": "custom",
                "package_ref": "install-ca.ps1",
                "install_args": "-ExecutionPolicy Bypass",
            },
        )
    ).json()
    await client.post(
        f"/api/v1/admin/catalog/{custom_pkg['id']}/upload",
        headers=admin_headers,
        files={"file": ("install-ca.ps1", b"Write-Output 'CA installed'", "text/plain")},
    )
    await client.post(
        "/api/v1/admin/deployments",
        headers=admin_headers,
        json={
            "package_id": custom_pkg["id"],
            "target_device_ids": [dev["device_id"]],
            "mode": "required",
            "respect_maintenance_window": False,
        },
    )

    jobs1 = (await client.get("/api/v1/agent/jobs/pending", headers=dev_headers)).json()[
        "jobs"
    ]
    assert len(jobs1) == 1

    # Agent completes custom script installation
    await client.post(
        f"/api/v1/agent/jobs/{jobs1[0]['job_id']}/result",
        headers=dev_headers,
        json={"status": "succeeded", "exit_code": 0, "stdout": "CA installed"},
    )

    # Subsequent OS inventory syncs (both same hash and new OS software hash without the custom script in HKLM)
    # MUST NOT wipe out custom software status or re-enqueue duplicate install jobs!
    inv_same = (
        await client.post(
            "/api/v1/agent/inventory",
            headers=dev_headers,
            json={
                "software_hash": "os-hash-1",
                "installed_software": [
                    {
                        "name": "Microsoft Edge",
                        "version": "130.0",
                        "publisher": "Microsoft",
                        "source": "registry",
                        "scope": "machine",
                    }
                ],
            },
        )
    ).json()
    assert inv_same["software_updated"] is False
    assert inv_same["enqueued_jobs"] == 0

    inv_changed = (
        await client.post(
            "/api/v1/agent/inventory",
            headers=dev_headers,
            json={
                "software_hash": "os-hash-2",
                "installed_software": [
                    {
                        "name": "Microsoft Edge",
                        "version": "131.0",
                        "publisher": "Microsoft",
                        "source": "registry",
                        "scope": "machine",
                    }
                ],
            },
        )
    ).json()
    assert inv_changed["software_updated"] is True
    assert inv_changed["enqueued_jobs"] == 0
    assert (
        await client.get("/api/v1/agent/jobs/pending", headers=dev_headers)
    ).json()["jobs"] == []

    # Bumping custom catalog item version to 2.0.0 MUST enqueue a single update job!
    await client.patch(
        f"/api/v1/admin/catalog/{custom_pkg['id']}",
        headers=admin_headers,
        json={"version": "2.0.0"},
    )
    jobs_v2 = (
        await client.get("/api/v1/agent/jobs/pending", headers=dev_headers)
    ).json()["jobs"]
    assert len(jobs_v2) == 1
    assert jobs_v2[0]["action"] == "update"
    assert jobs_v2[0]["version"] == "2.0.0"
