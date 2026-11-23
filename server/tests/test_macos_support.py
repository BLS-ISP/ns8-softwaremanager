from __future__ import annotations

import hashlib
import plistlib
import re
import shutil
import subprocess
from pathlib import Path

import httpx
import pytest

from app.config import Settings
from app.crypto import SigningService
from tests.conftest import enroll_test_device

REPO_ROOT = Path(__file__).resolve().parents[2]
MACOS_PACKAGING_DIR = REPO_ROOT / "agent" / "packaging" / "macos"


@pytest.mark.asyncio
async def test_macos_enrollment_normalization_inventory_and_dashboard_breakdown(
    client: httpx.AsyncClient,
    admin_headers: dict[str, str],
) -> None:
    tok_resp = await client.post(
        "/api/v1/admin/enrollment-tokens",
        headers=admin_headers,
        json={"name": "mac-mdm-token", "expires_in_hours": 24, "auto_approve": True},
    )
    assert tok_resp.status_code == 200
    token = tok_resp.json()["token"]

    # Verify all macOS alias strings normalize to "macos"
    for idx, raw_os in enumerate(
        ("darwin", "Darwin", "mac", "macos", "osx", "Mac OS X")
    ):
        enr = await client.post(
            "/api/v1/agent/enroll",
            json={
                "enrollment_token": token,
                "hostname": f"macbook-pro-{idx:02d}",
                "os_family": raw_os,
                "os_name": "macOS Sequoia",
                "os_version": "15.1",
                "arch": "arm64" if idx % 2 == 0 else "amd64",
                "agent_version": "1.2.0",
                "machine_uid": f"IOPlatformUUID-0000-000{idx}",
            },
        )
        assert enr.status_code == 200, f"Failed for {raw_os}: {enr.text}"
        dev_id = enr.json()["device_id"]
        detail = (
            await client.get(f"/api/v1/admin/devices/{dev_id}", headers=admin_headers)
        ).json()
        assert detail["os_family"] == "macos"

    # Unsupported OS family rejected with 422
    bad_os = await client.post(
        "/api/v1/agent/enroll",
        json={
            "enrollment_token": token,
            "hostname": "bsd-box",
            "os_family": "freebsd",
            "machine_uid": "uid-freebsd-1",
        },
    )
    assert bad_os.status_code == 422

    # Enroll 1 Windows and 1 Linux endpoint to verify 3-way OS breakdown
    await enroll_test_device(
        client, admin_headers, hostname="win-ws-mac-test", os_family="windows"
    )
    await enroll_test_device(
        client, admin_headers, hostname="lin-srv-mac-test", os_family="linux"
    )

    # Submit rich macOS inventory (with Darwin os_family normalization + brew/app_bundle software)
    mac_enr = await enroll_test_device(
        client, admin_headers, hostname="mbp-m3-max", os_family="darwin"
    )
    mac_headers = {"Authorization": f"Bearer {mac_enr['device_token']}"}
    inv_resp = await client.post(
        "/api/v1/agent/inventory",
        headers=mac_headers,
        json={
            "hostname": "mbp-m3-max",
            "domain": "corp.bls-isp.de",
            "os_family": "Darwin",
            "os_name": "macOS",
            "os_version": "15.1.1",
            "kernel_or_build": "24B91",
            "arch": "arm64",
            "agent_version": "1.2.0",
            "center_running": True,
            "logged_in_users": ["m.mustermann"],
            "hardware": {
                "cpu_model": "Apple M3 Max",
                "cpu_cores": 14,
                "ram_total_mb": 36864,
            },
            "installed_software": [
                {
                    "name": "Firefox",
                    "version": "132.0.2",
                    "publisher": "org.mozilla.firefox",
                    "source": "app_bundle",
                    "scope": "machine",
                    "package_ref": "org.mozilla.firefox",
                },
                {
                    "name": "jq",
                    "version": "1.7.1",
                    "publisher": "Homebrew",
                    "source": "brew",
                    "scope": "machine",
                    "package_ref": "jq",
                },
            ],
        },
    )
    assert inv_resp.status_code == 200
    assert inv_resp.json()["software_updated"] is True

    mac_detail = (
        await client.get(
            f"/api/v1/admin/devices/{mac_enr['device_id']}", headers=admin_headers
        )
    ).json()
    assert mac_detail["os_family"] == "macos"
    assert mac_detail["kernel_or_build"] == "24B91"
    assert mac_detail["center_running"] is True
    assert mac_detail["installed_software_count"] == 2

    mac_list = (
        await client.get(
            "/api/v1/admin/devices?os_family=macos", headers=admin_headers
        )
    ).json()
    assert len(mac_list) == 7  # 6 alias enrollments + mbp-m3-max

    dash = (
        await client.get("/api/v1/admin/dashboard/summary", headers=admin_headers)
    ).json()
    assert dash["total_devices"] == 9
    assert dash["os_breakdown"] == {"windows": 1, "linux": 1, "macos": 7}


@pytest.mark.asyncio
async def test_macos_catalog_brew_cask_pkg_dmg_zip_and_detection_rules(
    client: httpx.AsyncClient,
    admin_headers: dict[str, str],
) -> None:
    mac = await enroll_test_device(
        client, admin_headers, hostname="imac-design-01", os_family="macos"
    )
    mac_headers = {"Authorization": f"Bearer {mac['device_token']}"}
    pub_key_b64 = mac["server_public_key_b64"]

    # 1. Homebrew Formula (`brew`)
    brew_formula = await client.post(
        "/api/v1/admin/catalog",
        headers=admin_headers,
        json={
            "name": "ripgrep",
            "version": "14.1.1",
            "vendor": "BurntSushi",
            "category": "Developer Tools",
            "os_family": "macos",
            "source_type": "brew",
            "package_ref": "ripgrep",
            "detection_rule": {"type": "brew", "package_ref": "ripgrep", "min_version": "14.1.1"},
        },
    )
    assert brew_formula.status_code == 200, brew_formula.text

    # 2. Homebrew Cask (`brew_cask`) with app_bundle detection rule
    brew_cask = await client.post(
        "/api/v1/admin/catalog",
        headers=admin_headers,
        json={
            "name": "Visual Studio Code",
            "version": "1.95.3",
            "vendor": "Microsoft",
            "category": "IDEs",
            "os_family": "macos",
            "source_type": "brew_cask",
            "package_ref": "visual-studio-code",
            "blocked_processes": ["Electron"],
            "detection_rule": {
                "type": "app_bundle",
                "path": "/Applications/Visual Studio Code.app",
                "min_version": "1.95.3",
            },
        },
    )
    assert brew_cask.status_code == 200, brew_cask.text
    cask_id = brew_cask.json()["id"]

    # 3. Custom macOS installers: .pkg (pkgutil rule), .dmg (app_bundle rule), .app.zip, .sh
    custom_specs = [
        (
            "Zoom Workplace IT",
            "ZoomInstallerIT.pkg",
            b"xar!-mock-macos-pkg-payload",
            {"type": "pkgutil", "package_id": "us.zoom.pkg.videomeeting", "min_version": "6.2.6"},
        ),
        (
            "Mozilla Firefox macOS",
            "Firefox-132.0.dmg",
            b"koly-mock-macos-dmg-payload",
            {"type": "app_bundle", "path": "/Applications/Firefox.app", "min_version": "132.0"},
        ),
        (
            "Internal Mac Helper App",
            "CorpHelper.app.zip",
            b"PK\x03\x04-mock-macos-app-zip-payload",
            {"type": "app_bundle", "path": "/Applications/CorpHelper.app", "min_version": "2.1.0"},
        ),
        (
            "macOS FileVault Escrow Audit",
            "escrow-check.sh",
            b"#!/bin/bash\nfdesetup status\n",
            {"type": "file", "path": "/Library/Application Support/Corp/escrow.ok"},
        ),
    ]

    uploaded_pkg_ids: list[str] = []
    for name, fname, raw_bytes, det_rule in custom_specs:
        c_res = await client.post(
            "/api/v1/admin/catalog",
            headers=admin_headers,
            json={
                "name": name,
                "version": "1.0.0",
                "vendor": "BLS-ISP",
                "os_family": "macos",
                "source_type": "custom",
                "detection_rule": det_rule,
            },
        )
        assert c_res.status_code == 200, c_res.text
        pid = c_res.json()["id"]
        uploaded_pkg_ids.append(pid)

        up_res = await client.post(
            f"/api/v1/admin/catalog/{pid}/upload",
            headers=admin_headers,
            files={"file": (fname, raw_bytes, "application/octet-stream")},
        )
        assert up_res.status_code == 200, up_res.text
        up_body = up_res.json()
        assert up_body["filename"] == fname
        assert up_body["sha256"] == hashlib.sha256(raw_bytes).hexdigest()

        # Verify HTTP 206 Range download for macOS agent
        dl_range = await client.get(
            f"/api/v1/agent/packages/{pid}/download",
            headers={**mac_headers, "Range": "bytes=0-3"},
        )
        assert dl_range.status_code == 206
        assert dl_range.content == raw_bytes[:4]

    # Filter catalog by os_family=macos & source_type=brew_cask
    mac_cat = (
        await client.get("/api/v1/admin/catalog?os_family=macos", headers=admin_headers)
    ).json()
    assert len(mac_cat) == 6
    cask_cat = (
        await client.get(
            "/api/v1/admin/catalog?os_family=macos&source_type=brew_cask",
            headers=admin_headers,
        )
    ).json()
    assert len(cask_cat) == 1
    assert cask_cat[0]["package_ref"] == "visual-studio-code"

    # Update catalog item source_type/version via PATCH
    patched_cask = await client.patch(
        f"/api/v1/admin/catalog/{cask_id}",
        headers=admin_headers,
        json={"version": "1.95.4", "os_family": "macos", "source_type": "brew_cask"},
    )
    assert patched_cask.status_code == 200
    assert patched_cask.json()["version"] == "1.95.4"

    # Deploy .pkg as required and brew_cask as available to the Mac
    dep_pkg = await client.post(
        "/api/v1/admin/deployments",
        headers=admin_headers,
        json={
            "package_id": uploaded_pkg_ids[0],
            "target_device_ids": [mac["device_id"]],
            "mode": "required",
            "respect_maintenance_window": False,
        },
    )
    assert dep_pkg.status_code == 200

    dep_cask = await client.post(
        "/api/v1/admin/deployments",
        headers=admin_headers,
        json={
            "package_id": cask_id,
            "target_device_ids": [mac["device_id"]],
            "mode": "available",
            "respect_maintenance_window": False,
        },
    )
    assert dep_cask.status_code == 200

    pending = (
        await client.get("/api/v1/agent/jobs/pending", headers=mac_headers)
    ).json()
    assert len(pending["jobs"]) == 1
    pkg_job = pending["jobs"][0]
    assert pkg_job["package_meta"]["os_family"] == "macos"
    assert pkg_job["package_meta"]["filename"] == "ZoomInstallerIT.pkg"
    assert pkg_job["detection_rule"]["type"] == "pkgutil"
    assert pkg_job["download_url"] == f"/api/v1/agent/packages/{uploaded_pkg_ids[0]}/download"

    assert len(pending["available_catalog"]) == 1
    avail_cask = pending["available_catalog"][0]
    assert avail_cask["os_family"] == "macos"
    assert avail_cask["source_type"] == "brew_cask"
    assert avail_cask["detection_rule"]["type"] == "app_bundle"
    assert avail_cask["download_url"] is None

    cat_canonical = SigningService.canonical_catalog_string(
        package_id=avail_cask["package_id"],
        device_id=avail_cask["device_id"],
        source_type=avail_cask["source_type"],
        package_ref=avail_cask["package_ref"],
        version=avail_cask["version"],
        sha256=avail_cask["sha256"],
        install_cmd=avail_cask["install_cmd"],
        uninstall_cmd=avail_cask["uninstall_cmd"],
        allow_uninstall=avail_cask["allow_uninstall"],
    )
    assert SigningService.verify_canonical_with_b64_pubkey(
        pub_key_b64, cat_canonical, avail_cask["signature_b64"]
    )


@pytest.mark.asyncio
async def test_macos_os_patching_compliance_and_self_update_default_url(
    client: httpx.AsyncClient,
    admin_headers: dict[str, str],
) -> None:
    mac1 = await enroll_test_device(
        client, admin_headers, hostname="mac-mini-01", os_family="macos"
    )
    mac2 = await enroll_test_device(
        client, admin_headers, hostname="mac-studio-02", os_family="macos"
    )
    mac1_headers = {"Authorization": f"Bearer {mac1['device_token']}"}
    mac2_headers = {"Authorization": f"Bearer {mac2['device_token']}"}

    # Report Apple softwareupdate items on mac1; mac2 is clean
    await client.post(
        "/api/v1/agent/inventory",
        headers=mac1_headers,
        json={
            "os_name": "macOS Sequoia",
            "os_version": "15.1",
            "pending_os_updates": [
                {
                    "update_id": "MSU_UPDATE_24B91_patch_15.1.1",
                    "title": "macOS Sequoia 15.1.1",
                    "severity": "critical",
                    "category": "security",
                    "size_bytes": 1_620_000_000,
                    "reboot_behavior": "requires_reboot",
                },
                {
                    "update_id": "Command Line Tools for Xcode-16.1",
                    "title": "Command Line Tools for Xcode 16.1",
                    "severity": "moderate",
                    "category": "developer",
                    "size_bytes": 745_000_000,
                    "reboot_behavior": "no_reboot",
                },
            ],
        },
    )

    # Verify patches endpoint classifies both as os_family == "macos" (NOT "linux")
    mac_patches = (
        await client.get("/api/v1/admin/patches?os_family=macos", headers=admin_headers)
    ).json()
    assert len(mac_patches) == 2
    assert {p["os_family"] for p in mac_patches} == {"macos"}

    lin_patches = (
        await client.get("/api/v1/admin/patches?os_family=linux", headers=admin_headers)
    ).json()
    assert lin_patches == []

    # Compliance: 1 of 2 Macs has pending critical security update -> 50.0%
    dash_before = (
        await client.get("/api/v1/admin/dashboard/summary", headers=admin_headers)
    ).json()
    assert dash_before["pending_critical_patches"] == 1
    assert dash_before["patch_compliance_percent"] == 50.0

    # Exclude Command Line Tools -> must retain os_family == "macos" in PatchPolicy
    excl = await client.post(
        "/api/v1/admin/patches/exclude",
        headers=admin_headers,
        json={"update_ids": ["Command Line Tools for Xcode-16.1"], "excluded": True},
    )
    assert excl.status_code == 200

    # Approve macOS Sequoia 15.1.1 security update
    appr = await client.post(
        "/api/v1/admin/patches/approve",
        headers=admin_headers,
        json={
            "update_ids": ["MSU_UPDATE_24B91_patch_15.1.1"],
            "respect_maintenance_window": False,
        },
    )
    assert appr.status_code == 200
    assert appr.json()["jobs_created"] == 1

    mac1_jobs = (
        await client.get("/api/v1/agent/jobs/pending", headers=mac1_headers)
    ).json()["jobs"]
    assert len(mac1_jobs) == 1
    assert mac1_jobs[0]["job_type"] == "os_patch"
    assert mac1_jobs[0]["update_ids"] == ["MSU_UPDATE_24B91_patch_15.1.1"]

    await client.post(
        f"/api/v1/agent/jobs/{mac1_jobs[0]['job_id']}/result",
        headers=mac1_headers,
        json={"status": "succeeded", "exit_code": 0, "reboot_required": True},
    )

    # Clear pending updates on mac1 and verify excluded policy STILL reports os_family == "macos"
    await client.post(
        "/api/v1/agent/inventory",
        headers=mac1_headers,
        json={"pending_os_updates": [], "reboot_required": True},
    )
    excl_macos = (
        await client.get(
            "/api/v1/admin/patches?os_family=macos&excluded=true", headers=admin_headers
        )
    ).json()
    assert len(excl_macos) == 1
    assert excl_macos[0]["update_id"] == "Command Line Tools for Xcode-16.1"
    assert excl_macos[0]["os_family"] == "macos"

    dash_after = (
        await client.get("/api/v1/admin/dashboard/summary", headers=admin_headers)
    ).json()
    assert dash_after["pending_critical_patches"] == 0
    assert dash_after["patch_compliance_percent"] == 100.0
    assert dash_after["reboot_required_devices"] == 1

    # Trigger macOS Agent Self-Update WITHOUT explicit download_url -> defaults to /dl/agents/macos-universal/swm-agent
    fake_sha = "c" * 64
    su_resp = await client.post(
        "/api/v1/admin/self-update",
        headers=admin_headers,
        json={
            "version": "1.2.1",
            "os_family": "macos",
            "sha256": fake_sha,
            "respect_maintenance_window": False,
        },
    )
    assert su_resp.status_code == 200
    assert su_resp.json()["jobs_created"] == 2

    mac2_jobs = (
        await client.get("/api/v1/agent/jobs/pending", headers=mac2_headers)
    ).json()["jobs"]
    assert len(mac2_jobs) == 1
    su_job = mac2_jobs[0]
    assert su_job["job_type"] == "self_update"
    assert su_job["download_url"] == "/dl/agents/macos-universal/swm-agent"
    assert su_job["command"] == "/dl/agents/macos-universal/swm-agent"
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
        mac2["server_public_key_b64"], canonical, su_job["signature_b64"]
    )


@pytest.mark.asyncio
async def test_macos_artifacts_plists_and_darwin_bootstrap_scripts(
    client: httpx.AsyncClient,
    admin_headers: dict[str, str],
    test_settings: Settings,
) -> None:
    # 1. Stage mock Universal Mach-O binaries in STATIC_AGENTS_DIR/macos-universal
    mac_dir = test_settings.STATIC_AGENTS_DIR / "macos-universal"
    mac_dir.mkdir(parents=True, exist_ok=True)
    fat_agent = b"\xca\xfe\xba\xbe-universal-swm-agent"
    fat_center = b"\xca\xfe\xba\xbe-universal-swm-center"
    (mac_dir / "swm-agent").write_bytes(fat_agent)
    (mac_dir / "swm-center").write_bytes(fat_center)

    meta_resp = await client.get("/api/v1/admin/agent-artifacts", headers=admin_headers)
    assert meta_resp.status_code == 200
    artifacts = {
        f"{a['platform']}/{a['filename']}": a for a in meta_resp.json()["artifacts"]
    }
    assert "macos-universal/swm-agent" in artifacts
    assert "macos-universal/swm-center" in artifacts
    ag_art = artifacts["macos-universal/swm-agent"]
    ct_art = artifacts["macos-universal/swm-center"]
    assert ag_art["os_family"] == "macos"
    assert ag_art["available"] is True
    assert ag_art["sha256"] == hashlib.sha256(fat_agent).hexdigest()
    assert ag_art["download_path"] == "/dl/agents/macos-universal/swm-agent"
    assert ct_art["os_family"] == "macos"
    assert ct_art["available"] is True
    assert ct_art["sha256"] == hashlib.sha256(fat_center).hexdigest()

    dl_bin = await client.get("/dl/agents/macos-universal/swm-agent")
    assert dl_bin.status_code == 200
    assert dl_bin.content == fat_agent

    # 2. Validate agent/packaging/macos/*.plist with plistlib.loads() + pure LF
    daemon_plist_path = MACOS_PACKAGING_DIR / "org.nethserver.swm-agent.plist"
    agent_plist_path = MACOS_PACKAGING_DIR / "org.nethserver.swm-center.plist"
    assert daemon_plist_path.is_file()
    assert agent_plist_path.is_file()

    for p in (daemon_plist_path, agent_plist_path):
        raw = p.read_bytes()
        assert b"\r" not in raw, f"CR/CRLF found in {p}"

    daemon_plist = plistlib.loads(daemon_plist_path.read_bytes())
    assert daemon_plist["Label"] == "org.nethserver.swm-agent"
    assert daemon_plist["ProgramArguments"] == [
        "/usr/local/bin/swm-agent",
        "run",
        "--data-dir",
        "/Library/Application Support/NS8-SoftwareManager",
    ]
    assert daemon_plist["RunAtLoad"] is True
    assert daemon_plist["KeepAlive"] is True

    center_plist = plistlib.loads(agent_plist_path.read_bytes())
    assert center_plist["Label"] == "org.nethserver.swm-center"
    assert center_plist["ProgramArguments"] == [
        "/usr/local/bin/swm-center",
        "gui",
        "--tray-only",
    ]
    assert center_plist["LimitLoadToSessionType"] == "Aqua"
    assert center_plist["RunAtLoad"] is True
    assert center_plist["KeepAlive"] is True

    # 3. Validate /dl/install.sh (headless=0, headless=1, skip_if_enrolled=1) & packaging/install.sh
    sh_ws = (
        await client.get("/dl/install.sh?token=tok-mac-01&headless=0&skip_if_enrolled=1")
    ).text
    sh_hl = (
        await client.get("/dl/install.sh?token=tok-mac-01&headless=1&skip_if_enrolled=1")
    ).text
    pkg_sh = (REPO_ROOT / "agent" / "packaging" / "install.sh").read_text(encoding="utf-8")

    for label, txt in (("dl_ws", sh_ws), ("dl_hl", sh_hl), ("pkg_sh", pkg_sh)):
        assert "\r" not in txt, f"CR found in {label}"
        assert "uname -s" in txt
        assert "Darwin" in txt
        assert "/Library/Application Support/NS8-SoftwareManager" in txt
        assert "/var/run/ns8-swm" in txt
        assert "launchctl print system/org.nethserver.swm-agent" in txt
        assert "/dl/agents/macos-universal/swm-agent" in txt
        assert "xattr -dr com.apple.quarantine" in txt
        assert "/Library/LaunchDaemons/org.nethserver.swm-agent.plist" in txt
        assert "launchctl bootstrap system /Library/LaunchDaemons/org.nethserver.swm-agent.plist" in txt
        # Linux branch must also remain intact
        assert "/etc/ns8-swm" in txt
        assert "swm-agent.service" in txt

    # Workstation-specific Darwin checks
    assert "/dl/agents/macos-universal/swm-center" in sh_ws
    assert "/Applications/NS8 Software Center.app" in sh_ws
    assert "exec /usr/local/bin/swm-center gui" in sh_ws
    assert "/Library/LaunchAgents/org.nethserver.swm-center.plist" in sh_ws
    assert "stat -f %Su /dev/console" in sh_ws
    assert 'launchctl bootstrap "gui/${CONSOLE_UID}"' in sh_ws
    assert 'launchctl kickstart -k "gui/${CONSOLE_UID}/org.nethserver.swm-center"' in sh_ws

    # Parse every XML plist heredoc embedded inside sh_ws via plistlib.loads()
    embedded_plists = re.findall(
        r"(<\?xml version=\"1\.0\" encoding=\"UTF-8\"\?>.*?</plist>)",
        sh_ws,
        flags=re.DOTALL,
    )
    assert len(embedded_plists) == 3  # LaunchDaemon, App Info.plist, LaunchAgent
    for xml_str in embedded_plists:
        parsed = plistlib.loads(xml_str.encode("utf-8"))
        assert isinstance(parsed, dict)

    # Headless Darwin checks
    assert "rm -f /Library/LaunchAgents/org.nethserver.swm-center.plist" in sh_hl
    assert 'rm -rf "/Applications/NS8 Software Center.app"' in sh_hl

    # Optional bash -n syntax validation if bash is available (skip WSL stub if no distro)
    bash_bin = shutil.which("bash")
    if bash_bin and "system32" not in bash_bin.lower():
        for label, txt in (("dl_ws", sh_ws), ("dl_hl", sh_hl), ("pkg_sh", pkg_sh)):
            proc = subprocess.run(
                [bash_bin, "-n"],
                input=txt,
                text=True,
                capture_output=True,
                check=False,
            )
            assert proc.returncode == 0, f"bash -n failed for {label}: {proc.stderr}"

    # 4. Verify CI workflow and Containerfile.app include macOS Universal pipeline
    ci_yml = (REPO_ROOT / ".github" / "workflows" / "build-and-test.yml").read_text(
        encoding="utf-8"
    )
    assert "\r" not in ci_yml
    assert "macos-latest" in ci_yml
    assert "agents-macos-universal" in ci_yml
    assert "aarch64-apple-darwin" in ci_yml
    assert "x86_64-apple-darwin" in ci_yml
    assert "lipo -create" in ci_yml
    assert "prebuilt-agents/macos-universal" in ci_yml

    cfile_app = (REPO_ROOT / "Containerfile.app").read_text(encoding="utf-8")
    assert "\r" not in cfile_app
    assert "/app/agents/macos-universal" in cfile_app
    assert "/app/agents/macos-arm64" in cfile_app
    assert "/app/agents/macos-amd64" in cfile_app
