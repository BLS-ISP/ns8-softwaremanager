from __future__ import annotations

import re
from pathlib import Path

from fastapi import APIRouter, HTTPException, Query, Request, status
from fastapi.responses import FileResponse, PlainTextResponse

from app.config import Settings
from app.schemas import HealthResponse

router = APIRouter()

SAFE_TOKEN_RE = re.compile(r"^[A-Za-z0-9._-]+$")
SAFE_PATH_SEGMENT_RE = re.compile(r"^[A-Za-z0-9._-]+$")


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


@router.get("/dl/install.ps1", response_class=PlainTextResponse, tags=["bootstrap"])
async def download_install_ps1(
    request: Request,
    token: str = Query(..., min_length=1),
    headless: int = Query(default=0, ge=0, le=1),
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
            '        Invoke-WebRequest -UseBasicParsing -Uri "$ServerUrl/dl/agents/windows/swm-center.exe" -OutFile "$InstallDir\\swm-center.exe"',
            "    } catch {",
            '        Write-Warning "Optional Software Center GUI binary not present on server yet; continuing headless-safe."',
            "    }",
            '    if (Test-Path "$InstallDir\\swm-center.exe") {',
            '        New-Item -Path "HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" -Force | Out-Null',
            '        Set-ItemProperty -Path "HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" -Name "NS8SoftwareCenter" -Value \'"C:\\Program Files\\NS8-SoftwareManager\\swm-center.exe" gui --tray-only\'',
            "    }",
            "}",
        ]
    else:
        center_ps1_lines = [
            "if ($Headless -eq 1) {",
            '    if ($null -ne (Get-ItemProperty -Path "HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" -Name "NS8SoftwareCenter" -ErrorAction SilentlyContinue)) {',
            '        Remove-ItemProperty -Path "HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" -Name "NS8SoftwareCenter" -ErrorAction SilentlyContinue',
            "    }",
            "}",
        ]

    ps1_lines = [
        "# NS8-SoftwareManager Windows Bootstrap Installer",
        '$ErrorActionPreference = "Stop"',
        f'$ServerUrl = "{base_url}"',
        f'$EnrollmentToken = "{token}"',
        f"$Headless = {headless}",
        '$InstallDir = "C:\\Program Files\\NS8-SoftwareManager"',
        '$DataDir = "C:\\ProgramData\\NS8-SoftwareManager"',
        "",
        "New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null",
        "New-Item -ItemType Directory -Force -Path $DataDir | Out-Null",
        "",
        'Write-Host "[NS8-SWM] Downloading swm-agent.exe from $ServerUrl..."',
        'Invoke-WebRequest -UseBasicParsing -Uri "$ServerUrl/dl/agents/windows/swm-agent.exe" -OutFile "$InstallDir\\swm-agent.exe"',
        "",
        *center_ps1_lines,
        "",
        'Write-Host "[NS8-SWM] Enrolling device with NS8-SoftwareManager..."',
        '& "$InstallDir\\swm-agent.exe" enroll --server "$ServerUrl" --token "$EnrollmentToken" --data-dir "$DataDir"',
        "",
        'Write-Host "[NS8-SWM] Registering and starting Windows Service..."',
        '& "$InstallDir\\swm-agent.exe" service-install --data-dir "$DataDir"',
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
) -> PlainTextResponse:
    if not SAFE_TOKEN_RE.match(token):
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Invalid enrollment token format",
        )
    settings: Settings = request.app.state.settings
    base_url = resolve_public_base_url(request, settings)

    if headless == 0:
        center_sh_lines = [
            'if [ "${HEADLESS}" -eq 0 ]; then',
            '    if curl -fsSL "${SERVER_URL}/dl/agents/linux-${ARCH}/swm-center" -o "${INSTALL_DIR}/swm-center" 2>/dev/null || \\',
            '       curl -fsSL "${SERVER_URL}/dl/agents/linux/swm-center" -o "${INSTALL_DIR}/swm-center" 2>/dev/null; then',
            '        chmod 0755 "${INSTALL_DIR}/swm-center"',
            "        mkdir -p /etc/xdg/autostart",
            "        cat >/etc/xdg/autostart/ns8-swm-center.desktop <<'EOF'",
            "[Desktop Entry]",
            "Type=Application",
            "Name=NS8 Software Center",
            "Exec=/usr/local/bin/swm-center gui --tray-only",
            "Terminal=false",
            "X-GNOME-Autostart-enabled=true",
            "EOF",
            "        chmod 0644 /etc/xdg/autostart/ns8-swm-center.desktop",
            "    fi",
            "fi",
        ]
    else:
        center_sh_lines = [
            'if [ "${HEADLESS}" -eq 1 ]; then',
            "    rm -f /etc/xdg/autostart/ns8-swm-center.desktop",
            "fi",
        ]

    sh_lines = [
        "#!/usr/bin/env bash",
        "# NS8-SoftwareManager Linux Bootstrap Installer",
        "set -euo pipefail",
        "",
        f'SERVER_URL="{base_url}"',
        f'ENROLLMENT_TOKEN="{token}"',
        f"HEADLESS={headless}",
        'INSTALL_DIR="/usr/local/bin"',
        'DATA_DIR="/etc/ns8-swm"',
        "",
        "ARCH_RAW=$(uname -m)",
        'case "${ARCH_RAW}" in',
        '    x86_64|amd64) ARCH="amd64" ;;',
        '    aarch64|arm64) ARCH="arm64" ;;',
        '    *) ARCH="${ARCH_RAW}" ;;',
        "esac",
        "",
        'mkdir -p "${DATA_DIR}" /run/ns8-swm',
        'chmod 0755 "${DATA_DIR}" /run/ns8-swm',
        "",
        'echo "[NS8-SWM] Downloading swm-agent (${ARCH}) from ${SERVER_URL}..."',
        'if ! curl -fsSL "${SERVER_URL}/dl/agents/linux-${ARCH}/swm-agent" -o "${INSTALL_DIR}/swm-agent"; then',
        '    curl -fsSL "${SERVER_URL}/dl/agents/linux/swm-agent" -o "${INSTALL_DIR}/swm-agent"',
        "fi",
        'chmod 0755 "${INSTALL_DIR}/swm-agent"',
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
