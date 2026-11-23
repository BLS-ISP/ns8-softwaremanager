from __future__ import annotations

import hashlib
import re
from pathlib import Path
from typing import Any

from fastapi import APIRouter, Depends, HTTPException, Query, Request, status
from fastapi.responses import FileResponse, PlainTextResponse

from app.auth import get_current_admin
from app.config import Settings
from app.models import AdminUser
from app.schemas import HealthResponse

router = APIRouter()

SAFE_TOKEN_RE = re.compile(r"^[A-Za-z0-9._-]+$")
SAFE_PATH_SEGMENT_RE = re.compile(r"^[A-Za-z0-9._-]+$")

KNOWN_AGENT_ARTIFACTS: tuple[dict[str, str], ...] = (
    {
        "platform": "windows",
        "os_family": "windows",
        "filename": "swm-agent.exe",
        "role": "agent",
        "label": "Windows Management-Agent (System-Dienst)",
    },
    {
        "platform": "windows",
        "os_family": "windows",
        "filename": "swm-center.exe",
        "role": "center",
        "label": "Windows Software Center (Endbenutzer-App & Tray)",
    },
    {
        "platform": "linux-amd64",
        "os_family": "linux",
        "filename": "swm-agent",
        "role": "agent",
        "label": "Linux Management-Agent (x86_64 systemd-Dienst)",
    },
    {
        "platform": "linux-amd64",
        "os_family": "linux",
        "filename": "swm-center",
        "role": "center",
        "label": "Linux Software Center (x86_64 Desktop-App)",
    },
    {
        "platform": "macos-universal",
        "os_family": "macos",
        "filename": "swm-agent",
        "role": "agent",
        "label": "macOS Management-Agent (Universal Binary: Apple Silicon & Intel, launchd)",
    },
    {
        "platform": "macos-universal",
        "os_family": "macos",
        "filename": "swm-center",
        "role": "center",
        "label": "macOS Software Center (Universal Binary: Desktop-App & Tray)",
    },
)


def resolve_public_base_url(request: Request, settings: Settings) -> str:
    if settings.PUBLIC_BASE_URL:
        return settings.PUBLIC_BASE_URL.rstrip("/")
    proto = request.headers.get("x-forwarded-proto")
    host = request.headers.get("x-forwarded-host") or request.headers.get("host")
    if proto and host:
        return f"{proto.split(',')[0].strip()}://{host.split(',')[0].strip()}".rstrip("/")
    return str(request.base_url).rstrip("/")


@router.get("/api/v1/health", response_model=HealthResponse, tags=["health"])
async def health_check(request: Request) -> HealthResponse:
    settings: Settings = request.app.state.settings
    db_ok = await request.app.state.db.ping()
    if not db_ok:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail="Database connection unavailable",
        )
    return HealthResponse(
        status="ok",
        version=settings.APP_VERSION,
        database="connected",
    )


@router.get("/api/v1/admin/agent-artifacts", tags=["bootstrap"])
async def list_agent_artifacts(
    request: Request,
    _admin: AdminUser = Depends(get_current_admin),
) -> dict[str, Any]:
    settings: Settings = request.app.state.settings
    base_url = resolve_public_base_url(request, settings)
    base_dir = settings.STATIC_AGENTS_DIR.resolve()

    items: list[dict[str, Any]] = []
    for spec in KNOWN_AGENT_ARTIFACTS:
        platform = spec["platform"]
        filename = spec["filename"]
        candidate = (base_dir / platform / filename).resolve()
        available = str(candidate).startswith(str(base_dir)) and candidate.is_file()
        size_bytes = 0
        sha256_hex = ""
        if available:
            size_bytes = candidate.stat().st_size
            digest = hashlib.sha256()
            with candidate.open("rb") as fh:
                for chunk in iter(lambda: fh.read(65536), b""):
                    digest.update(chunk)
            sha256_hex = digest.hexdigest()
        items.append(
            {
                "platform": platform,
                "os_family": spec["os_family"],
                "filename": filename,
                "role": spec["role"],
                "label": spec["label"],
                "available": available,
                "size_bytes": size_bytes,
                "sha256": sha256_hex,
                "download_path": f"/dl/agents/{platform}/{filename}",
                "download_url": f"{base_url}/dl/agents/{platform}/{filename}",
            }
        )

    return {
        "server_url": base_url,
        "version": settings.APP_VERSION,
        "artifacts": items,
    }


@router.get("/dl/install.ps1", response_class=PlainTextResponse, tags=["bootstrap"])
async def download_install_ps1(
    request: Request,
    token: str = Query(..., min_length=1),
    headless: int = Query(default=0, ge=0, le=1),
    skip_if_enrolled: int = Query(default=0, ge=0, le=1),
) -> PlainTextResponse:
    if not SAFE_TOKEN_RE.match(token):
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Invalid enrollment token format",
        )
    settings: Settings = request.app.state.settings
    base_url = resolve_public_base_url(request, settings)

    if headless == 0:
        center_ps1_lines = [
            "if ($Headless -eq 0) {",
            "    try {",
            '        Get-Process -Name "swm-center" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue',
            '        Invoke-WebRequest -UseBasicParsing -Uri "$ServerUrl/dl/agents/windows/swm-center.exe" -OutFile "$InstallDir\\swm-center.exe"',
            "    } catch {",
            '        Write-Warning "Optional Software Center GUI binary not present on server yet; continuing headless-safe."',
            "    }",
            '    if (Test-Path "$InstallDir\\swm-center.exe") {',
            '        New-Item -Path "HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" -Force | Out-Null',
            '        Set-ItemProperty -Path "HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" -Name "NS8SoftwareCenter" -Value \'"C:\\Program Files\\NS8-SoftwareManager\\swm-center.exe" gui --tray-only\'',
            "        try {",
            '            $GuardianAction = New-ScheduledTaskAction -Execute "$InstallDir\\swm-center.exe" -Argument "gui --tray-only" -WorkingDirectory $InstallDir',
            "            $GuardianTrigger = New-ScheduledTaskTrigger -AtLogOn",
            '            $GuardianPrincipal = New-ScheduledTaskPrincipal -GroupId "S-1-5-32-545" -RunLevel Limited',
            "            $GuardianSettings = New-ScheduledTaskSettingsSet -MultipleInstances IgnoreNew -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero)",
            '            Register-ScheduledTask -TaskName "NS8SoftwareCenterGuardian" -Action $GuardianAction -Trigger $GuardianTrigger -Principal $GuardianPrincipal -Settings $GuardianSettings -Force | Out-Null',
            '            Start-ScheduledTask -TaskName "NS8SoftwareCenterGuardian" -ErrorAction SilentlyContinue',
            "        } catch {",
            '            Write-Warning "Could not register NS8SoftwareCenterGuardian scheduled task: $_"',
            "        }",
            "        try {",
            '            $StartMenuDir = Join-Path $env:ProgramData "Microsoft\\Windows\\Start Menu\\Programs"',
            '            $ShortcutPath = Join-Path $StartMenuDir "NS8 Software Center.lnk"',
            '            $WshShell = New-Object -ComObject WScript.Shell',
            '            $Shortcut = $WshShell.CreateShortcut($ShortcutPath)',
            '            $Shortcut.TargetPath = "$InstallDir\\swm-center.exe"',
            '            $Shortcut.Arguments = "gui"',
            '            $Shortcut.WorkingDirectory = $InstallDir',
            '            $Shortcut.Description = "Self-Service Software Center (NS8 SoftwareManager)"',
            "            $Shortcut.Save()",
            "        } catch {",
            '            Write-Warning "Could not create Start Menu shortcut: $_"',
            "        }",
            "    }",
            "}",
        ]
    else:
        center_ps1_lines = [
            "if ($Headless -eq 1) {",
            '    if ($null -ne (Get-ItemProperty -Path "HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" -Name "NS8SoftwareCenter" -ErrorAction SilentlyContinue)) {',
            '        Remove-ItemProperty -Path "HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" -Name "NS8SoftwareCenter" -ErrorAction SilentlyContinue',
            "    }",
            '    Unregister-ScheduledTask -TaskName "NS8SoftwareCenterGuardian" -Confirm:$false -ErrorAction SilentlyContinue',
            '    $ShortcutPath = Join-Path $env:ProgramData "Microsoft\\Windows\\Start Menu\\Programs\\NS8 Software Center.lnk"',
            '    if (Test-Path $ShortcutPath) { Remove-Item -Path $ShortcutPath -Force -ErrorAction SilentlyContinue }',
            "}",
        ]

    ps1_lines = [
        "# NS8-SoftwareManager Windows Bootstrap Installer (Interactive / AD GPO / Intune)",
        '$ErrorActionPreference = "Stop"',
        "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12",
        f'$ServerUrl = "{base_url}"',
        f'$EnrollmentToken = "{token}"',
        f"$Headless = {headless}",
        f"$SkipIfEnrolled = {skip_if_enrolled}",
        '$InstallDir = "C:\\Program Files\\NS8-SoftwareManager"',
        '$DataDir = "C:\\ProgramData\\NS8-SoftwareManager"',
        '$ServiceName = "NS8SoftwareManagerAgent"',
        "",
        'if ($SkipIfEnrolled -eq 1 -and (Test-Path "$DataDir\\config.json")) {',
        '    $ExistingSvc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue',
        "    if ($null -ne $ExistingSvc) {",
        '        if ($ExistingSvc.Status -ne "Running") { Start-Service -Name $ServiceName -ErrorAction SilentlyContinue }',
        '        Write-Host "[NS8-SWM] Endpoint already enrolled and service registered; skipping GPO re-enrollment."',
        "        exit 0",
        "    }",
        "}",
        "",
        "New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null",
        "New-Item -ItemType Directory -Force -Path $DataDir | Out-Null",
        "",
        'if ($null -ne (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue)) {',
        '    Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue',
        "}",
        "",
        'Write-Host "[NS8-SWM] Downloading swm-agent.exe from $ServerUrl..."',
        'Invoke-WebRequest -UseBasicParsing -Uri "$ServerUrl/dl/agents/windows/swm-agent.exe" -OutFile "$InstallDir\\swm-agent.exe"',
        "",
        *center_ps1_lines,
        "",
        'Write-Host "[NS8-SWM] Enrolling device with NS8-SoftwareManager..."',
        '& "$InstallDir\\swm-agent.exe" enroll --server "$ServerUrl" --token "$EnrollmentToken" --data-dir "$DataDir"',
        "",
        'Write-Host "[NS8-SWM] Registering and starting Windows Service ($ServiceName)..."',
        '& "$InstallDir\\swm-agent.exe" service-install --data-dir "$DataDir"',
        "& sc.exe failure $ServiceName reset= 86400 actions= restart/5000/restart/10000/restart/30000 | Out-Null",
        "& sc.exe failureflag $ServiceName 1 | Out-Null",
        'Write-Host "[NS8-SWM] Enrollment and service setup completed."',
        "",
    ]
    return PlainTextResponse(
        content="\r\n".join(ps1_lines),
        media_type="text/plain; charset=utf-8",
    )


@router.get("/dl/install.sh", response_class=PlainTextResponse, tags=["bootstrap"])
async def download_install_sh(
    request: Request,
    token: str = Query(..., min_length=1),
    headless: int = Query(default=0, ge=0, le=1),
    skip_if_enrolled: int = Query(default=0, ge=0, le=1),
) -> PlainTextResponse:
    if not SAFE_TOKEN_RE.match(token):
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Invalid enrollment token format",
        )
    settings: Settings = request.app.state.settings
    base_url = resolve_public_base_url(request, settings)

    if headless == 0:
        darwin_center_sh_lines = [
            '    if [ "${HEADLESS}" -eq 0 ]; then',
            '        if curl -fsSL "${SERVER_URL}/dl/agents/macos-universal/swm-center" -o "${INSTALL_DIR}/swm-center.tmp" 2>/dev/null || \\',
            '           curl -fsSL "${SERVER_URL}/dl/agents/macos-${ARCH}/swm-center" -o "${INSTALL_DIR}/swm-center.tmp" 2>/dev/null || \\',
            '           curl -fsSL "${SERVER_URL}/dl/agents/macos/swm-center" -o "${INSTALL_DIR}/swm-center.tmp" 2>/dev/null; then',
            '            chmod 0755 "${INSTALL_DIR}/swm-center.tmp"',
            '            mv -f "${INSTALL_DIR}/swm-center.tmp" "${INSTALL_DIR}/swm-center"',
            '            xattr -dr com.apple.quarantine "${INSTALL_DIR}/swm-center" 2>/dev/null || true',
            '            APP_BUNDLE="/Applications/NS8 Software Center.app"',
            '            mkdir -p "${APP_BUNDLE}/Contents/MacOS"',
            '            cat >"${APP_BUNDLE}/Contents/Info.plist" <<\'EOF\'',
            '<?xml version="1.0" encoding="UTF-8"?>',
            '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">',
            '<plist version="1.0">',
            "<dict>",
            "    <key>CFBundleName</key>",
            "    <string>NS8 Software Center</string>",
            "    <key>CFBundleDisplayName</key>",
            "    <string>NS8 Software Center</string>",
            "    <key>CFBundleIdentifier</key>",
            "    <string>org.nethserver.swm-center</string>",
            "    <key>CFBundleVersion</key>",
            "    <string>1.2.0</string>",
            "    <key>CFBundleShortVersionString</key>",
            "    <string>1.2.0</string>",
            "    <key>CFBundlePackageType</key>",
            "    <string>APPL</string>",
            "    <key>CFBundleExecutable</key>",
            "    <string>NS8 Software Center</string>",
            "    <key>LSMinimumSystemVersion</key>",
            "    <string>12.0</string>",
            "    <key>NSHighResolutionCapable</key>",
            "    <true/>",
            "</dict>",
            "</plist>",
            "EOF",
            '            cat >"${APP_BUNDLE}/Contents/MacOS/NS8 Software Center" <<\'EOF\'',
            "#!/bin/sh",
            'exec /usr/local/bin/swm-center gui "$@"',
            "EOF",
            '            chmod 0644 "${APP_BUNDLE}/Contents/Info.plist"',
            '            chmod 0755 "${APP_BUNDLE}/Contents/MacOS/NS8 Software Center"',
            '            xattr -dr com.apple.quarantine "${APP_BUNDLE}" 2>/dev/null || true',
            "            cat >/Library/LaunchAgents/org.nethserver.swm-center.plist <<'EOF'",
            '<?xml version="1.0" encoding="UTF-8"?>',
            '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">',
            '<plist version="1.0">',
            "<dict>",
            "    <key>Label</key>",
            "    <string>org.nethserver.swm-center</string>",
            "    <key>ProgramArguments</key>",
            "    <array>",
            "        <string>/usr/local/bin/swm-center</string>",
            "        <string>gui</string>",
            "        <string>--tray-only</string>",
            "    </array>",
            "    <key>LimitLoadToSessionType</key>",
            "    <string>Aqua</string>",
            "    <key>RunAtLoad</key>",
            "    <true/>",
            "    <key>KeepAlive</key>",
            "    <true/>",
            "</dict>",
            "</plist>",
            "EOF",
            "            chown root:wheel /Library/LaunchAgents/org.nethserver.swm-center.plist 2>/dev/null || true",
            "            chmod 0644 /Library/LaunchAgents/org.nethserver.swm-center.plist",
            "            CONSOLE_USER=$(stat -f %Su /dev/console 2>/dev/null || true)",
            '            if [ -n "${CONSOLE_USER}" ] && [ "${CONSOLE_USER}" != "root" ] && [ "${CONSOLE_USER}" != "loginwindow" ] && [ "${CONSOLE_USER}" != "_mbsetupuser" ]; then',
            '                CONSOLE_UID=$(id -u "${CONSOLE_USER}" 2>/dev/null || true)',
            '                if [ -n "${CONSOLE_UID}" ]; then',
            '                    launchctl bootout "gui/${CONSOLE_UID}" /Library/LaunchAgents/org.nethserver.swm-center.plist 2>/dev/null || true',
            '                    launchctl bootstrap "gui/${CONSOLE_UID}" /Library/LaunchAgents/org.nethserver.swm-center.plist 2>/dev/null || \\',
            '                        launchctl kickstart -k "gui/${CONSOLE_UID}/org.nethserver.swm-center" 2>/dev/null || true',
            "                fi",
            "            fi",
            "        fi",
            "    fi",
        ]
        center_sh_lines = [
            'if [ "${HEADLESS}" -eq 0 ]; then',
            '    if curl -fsSL "${SERVER_URL}/dl/agents/linux-${ARCH}/swm-center" -o "${INSTALL_DIR}/swm-center.tmp" 2>/dev/null || \\',
            '       curl -fsSL "${SERVER_URL}/dl/agents/linux/swm-center" -o "${INSTALL_DIR}/swm-center.tmp" 2>/dev/null; then',
            '        chmod 0755 "${INSTALL_DIR}/swm-center.tmp"',
            '        mv -f "${INSTALL_DIR}/swm-center.tmp" "${INSTALL_DIR}/swm-center"',
            "        mkdir -p /etc/xdg/autostart /usr/share/applications",
            "        cat >/etc/xdg/autostart/ns8-swm-center.desktop <<'EOF'",
            "[Desktop Entry]",
            "Type=Application",
            "Name=NS8 Software Center",
            "Exec=/usr/local/bin/swm-center gui --tray-only",
            "Terminal=false",
            "X-GNOME-Autostart-enabled=true",
            "EOF",
            "        chmod 0644 /etc/xdg/autostart/ns8-swm-center.desktop",
            "        cat >/usr/share/applications/ns8-swm-center.desktop <<'EOF'",
            "[Desktop Entry]",
            "Type=Application",
            "Name=NS8 Software Center",
            "Comment=Self-Service Software Installation (NS8 SoftwareManager)",
            "Exec=/usr/local/bin/swm-center gui",
            "Terminal=false",
            "Categories=System;PackageManager;",
            "EOF",
            "        chmod 0644 /usr/share/applications/ns8-swm-center.desktop",
            "        mkdir -p /etc/systemd/user/default.target.wants",
            "        cat >/etc/systemd/user/ns8-swm-center.service <<'EOF'",
            "[Unit]",
            "Description=NS8 Software Center Desktop Tray & Self-Service Agent Bridge",
            "After=graphical-session.target",
            "",
            "[Service]",
            "Type=simple",
            "ExecStart=/usr/local/bin/swm-center gui --tray-only",
            "Restart=always",
            "RestartSec=5",
            "",
            "[Install]",
            "WantedBy=default.target",
            "EOF",
            "        chmod 0644 /etc/systemd/user/ns8-swm-center.service",
            "        ln -sf /etc/systemd/user/ns8-swm-center.service /etc/systemd/user/default.target.wants/ns8-swm-center.service",
            "    fi",
            "fi",
        ]
    else:
        darwin_center_sh_lines = [
            '    if [ "${HEADLESS}" -eq 1 ]; then',
            "        CONSOLE_USER=$(stat -f %Su /dev/console 2>/dev/null || true)",
            '        if [ -n "${CONSOLE_USER}" ] && [ "${CONSOLE_USER}" != "root" ] && [ "${CONSOLE_USER}" != "loginwindow" ]; then',
            '            CONSOLE_UID=$(id -u "${CONSOLE_USER}" 2>/dev/null || true)',
            '            if [ -n "${CONSOLE_UID}" ]; then',
            '                launchctl bootout "gui/${CONSOLE_UID}" /Library/LaunchAgents/org.nethserver.swm-center.plist 2>/dev/null || true',
            "            fi",
            "        fi",
            "        rm -f /Library/LaunchAgents/org.nethserver.swm-center.plist",
            '        rm -rf "/Applications/NS8 Software Center.app"',
            "    fi",
        ]
        center_sh_lines = [
            'if [ "${HEADLESS}" -eq 1 ]; then',
            "    rm -f /etc/xdg/autostart/ns8-swm-center.desktop /usr/share/applications/ns8-swm-center.desktop",
            "    rm -f /etc/systemd/user/default.target.wants/ns8-swm-center.service /etc/systemd/user/ns8-swm-center.service",
            "fi",
        ]

    sh_lines = [
        "#!/usr/bin/env bash",
        "# NS8-SoftwareManager Unix Bootstrap Installer (Linux systemd & macOS launchd / MDM / Ansible)",
        "set -euo pipefail",
        "",
        f'SERVER_URL="{base_url}"',
        f'ENROLLMENT_TOKEN="{token}"',
        f"HEADLESS={headless}",
        f"SKIP_IF_ENROLLED={skip_if_enrolled}",
        'INSTALL_DIR="/usr/local/bin"',
        "OS_KERNEL=$(uname -s)",
        "",
        "ARCH_RAW=$(uname -m)",
        'case "${ARCH_RAW}" in',
        '    x86_64|amd64) ARCH="amd64" ;;',
        '    aarch64|arm64) ARCH="arm64" ;;',
        '    *) ARCH="${ARCH_RAW}" ;;',
        "esac",
        "",
        'if [ "${OS_KERNEL}" = "Darwin" ]; then',
        '    DATA_DIR="/Library/Application Support/NS8-SoftwareManager"',
        '    SOCK_DIR="/var/run/ns8-swm"',
        "",
        '    if [ "${SKIP_IF_ENROLLED}" -eq 1 ] && [ -f "${DATA_DIR}/config.json" ] && launchctl print system/org.nethserver.swm-agent >/dev/null 2>&1; then',
        '        echo "[NS8-SWM] macOS endpoint already enrolled and org.nethserver.swm-agent active; skipping."',
        "        exit 0",
        "    fi",
        "",
        '    mkdir -p "${INSTALL_DIR}" "${DATA_DIR}" "${SOCK_DIR}" /Library/LaunchDaemons /Library/LaunchAgents',
        '    chmod 0755 "${INSTALL_DIR}" "${DATA_DIR}" "${SOCK_DIR}"',
        "",
        '    echo "[NS8-SWM] Downloading swm-agent (macOS Universal) from ${SERVER_URL}..."',
        '    if ! curl -fsSL "${SERVER_URL}/dl/agents/macos-universal/swm-agent" -o "${INSTALL_DIR}/swm-agent.tmp" 2>/dev/null; then',
        '        if ! curl -fsSL "${SERVER_URL}/dl/agents/macos-${ARCH}/swm-agent" -o "${INSTALL_DIR}/swm-agent.tmp" 2>/dev/null; then',
        '            curl -fsSL "${SERVER_URL}/dl/agents/macos/swm-agent" -o "${INSTALL_DIR}/swm-agent.tmp"',
        "        fi",
        "    fi",
        '    chmod 0755 "${INSTALL_DIR}/swm-agent.tmp"',
        '    mv -f "${INSTALL_DIR}/swm-agent.tmp" "${INSTALL_DIR}/swm-agent"',
        '    xattr -dr com.apple.quarantine "${INSTALL_DIR}/swm-agent" 2>/dev/null || true',
        "",
        '    echo "[NS8-SWM] Enrolling macOS device..."',
        '    "${INSTALL_DIR}/swm-agent" enroll --server "${SERVER_URL}" --token "${ENROLLMENT_TOKEN}" --data-dir "${DATA_DIR}"',
        "",
        "    cat >/Library/LaunchDaemons/org.nethserver.swm-agent.plist <<'EOF'",
        '<?xml version="1.0" encoding="UTF-8"?>',
        '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">',
        '<plist version="1.0">',
        "<dict>",
        "    <key>Label</key>",
        "    <string>org.nethserver.swm-agent</string>",
        "    <key>ProgramArguments</key>",
        "    <array>",
        "        <string>/usr/local/bin/swm-agent</string>",
        "        <string>run</string>",
        "        <string>--data-dir</string>",
        "        <string>/Library/Application Support/NS8-SoftwareManager</string>",
        "    </array>",
        "    <key>RunAtLoad</key>",
        "    <true/>",
        "    <key>KeepAlive</key>",
        "    <true/>",
        "    <key>StandardOutPath</key>",
        "    <string>/var/log/ns8-swm-agent.log</string>",
        "    <key>StandardErrorPath</key>",
        "    <string>/var/log/ns8-swm-agent.err.log</string>",
        "</dict>",
        "</plist>",
        "EOF",
        "    chown root:wheel /Library/LaunchDaemons/org.nethserver.swm-agent.plist 2>/dev/null || true",
        "    chmod 0644 /Library/LaunchDaemons/org.nethserver.swm-agent.plist",
        "    launchctl bootout system /Library/LaunchDaemons/org.nethserver.swm-agent.plist 2>/dev/null || true",
        "    launchctl bootstrap system /Library/LaunchDaemons/org.nethserver.swm-agent.plist",
        "",
        *darwin_center_sh_lines,
        "",
        '    echo "[NS8-SWM] macOS agent installed and started via launchd."',
        "    exit 0",
        "fi",
        "",
        'DATA_DIR="/etc/ns8-swm"',
        "",
        'if [ "${SKIP_IF_ENROLLED}" -eq 1 ] && [ -f "${DATA_DIR}/config.json" ] && systemctl is-active --quiet swm-agent.service; then',
        '    echo "[NS8-SWM] Endpoint already enrolled and swm-agent.service active; skipping."',
        "    exit 0",
        "fi",
        "",
        'mkdir -p "${DATA_DIR}" /run/ns8-swm',
        'chmod 0755 "${DATA_DIR}" /run/ns8-swm',
        "",
        'echo "[NS8-SWM] Downloading swm-agent (${ARCH}) from ${SERVER_URL}..."',
        'if ! curl -fsSL "${SERVER_URL}/dl/agents/linux-${ARCH}/swm-agent" -o "${INSTALL_DIR}/swm-agent.tmp"; then',
        '    curl -fsSL "${SERVER_URL}/dl/agents/linux/swm-agent" -o "${INSTALL_DIR}/swm-agent.tmp"',
        "fi",
        'chmod 0755 "${INSTALL_DIR}/swm-agent.tmp"',
        'mv -f "${INSTALL_DIR}/swm-agent.tmp" "${INSTALL_DIR}/swm-agent"',
        "",
        *center_sh_lines,
        "",
        'echo "[NS8-SWM] Enrolling device..."',
        '"${INSTALL_DIR}/swm-agent" enroll --server "${SERVER_URL}" --token "${ENROLLMENT_TOKEN}" --data-dir "${DATA_DIR}"',
        "",
        'cat >/etc/systemd/system/swm-agent.service <<EOF',
        "[Unit]",
        "Description=NS8 SoftwareManager Endpoint Agent",
        "After=network-online.target",
        "Wants=network-online.target",
        "",
        "[Service]",
        "Type=simple",
        "ExecStart=/usr/local/bin/swm-agent run --data-dir /etc/ns8-swm",
        "Restart=always",
        "RestartSec=10",
        "",
        "[Install]",
        "WantedBy=multi-user.target",
        "EOF",
        "",
        "systemctl daemon-reload",
        "systemctl enable --now swm-agent.service",
        'echo "[NS8-SWM] Linux agent installed and started."',
        "",
    ]
    return PlainTextResponse(
        content="\n".join(sh_lines),
        media_type="text/x-shellscript; charset=utf-8",
    )


@router.get("/dl/agents/{platform}/{filename}", tags=["bootstrap"])
async def download_agent_binary(
    platform: str,
    filename: str,
    request: Request,
) -> FileResponse:
    if not SAFE_PATH_SEGMENT_RE.match(platform) or not SAFE_PATH_SEGMENT_RE.match(
        filename
    ):
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Invalid platform or filename",
        )
    settings: Settings = request.app.state.settings
    base_dir = settings.STATIC_AGENTS_DIR.resolve()
    candidate: Path = (base_dir / platform / filename).resolve()
    if not str(candidate).startswith(str(base_dir)) or not candidate.is_file():
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Agent artifact not found",
        )
    return FileResponse(
        path=candidate,
        filename=filename,
        media_type="application/octet-stream",
    )
