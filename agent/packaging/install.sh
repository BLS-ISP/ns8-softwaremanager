#!/usr/bin/env bash
set -euo pipefail

SERVER_URL=""
TOKEN=""
HEADLESS=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --server)
            SERVER_URL="$2"
            shift 2
            ;;
        --token)
            TOKEN="$2"
            shift 2
            ;;
        --headless)
            HEADLESS=1
            shift
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 2
            ;;
    esac
done

if [[ -z "$SERVER_URL" || -z "$TOKEN" ]]; then
    echo "Usage: install.sh --server <https://swm.example.org> --token <token> [--headless]" >&2
    exit 2
fi

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    echo "[NS8-SWM] Error: install.sh must be executed as root (use sudo)." >&2
    exit 1
fi

BASE_URL="${SERVER_URL%/}"
UNAME_ARCH="$(uname -m)"
case "$UNAME_ARCH" in
    x86_64|amd64) ARCH="amd64" ;;
    aarch64|arm64) ARCH="arm64" ;;
    *)
        echo "[NS8-SWM] Unsupported architecture: $UNAME_ARCH" >&2
        exit 1
        ;;
esac

install -d -m 0700 /etc/ns8-swm
install -d -m 0755 /run/ns8-swm /usr/local/bin

echo "[NS8-SWM] Downloading swm-agent ($ARCH) from $BASE_URL ..."
if ! curl -fsSL "$BASE_URL/dl/agents/linux-$ARCH/swm-agent" -o /usr/local/bin/swm-agent 2>/dev/null; then
    curl -fsSL "$BASE_URL/dl/agents/linux/swm-agent" -o /usr/local/bin/swm-agent
fi
chmod 0755 /usr/local/bin/swm-agent

if [[ "$HEADLESS" -eq 0 ]]; then
    echo "[NS8-SWM] Workstation mode: installing single-file swm-center desktop & tray client..."
    if curl -fsSL "$BASE_URL/dl/agents/linux-$ARCH/swm-center" -o /usr/local/bin/swm-center 2>/dev/null || \
       curl -fsSL "$BASE_URL/dl/agents/linux/swm-center" -o /usr/local/bin/swm-center 2>/dev/null; then
        chmod 0755 /usr/local/bin/swm-center
        install -d -m 0755 /etc/xdg/autostart /usr/share/applications
        cat >/etc/xdg/autostart/swm-center.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=NS8 Software Center
GenericName=Software Center
Comment=Installieren und aktualisieren Sie freigegebene Unternehmensanwendungen
Comment[en]=Install and update approved enterprise applications
Exec=/usr/local/bin/swm-center gui --tray-only
Icon=system-software-install
Terminal=false
Categories=System;PackageManager;
X-GNOME-Autostart-enabled=true
EOF
        chmod 0644 /etc/xdg/autostart/swm-center.desktop
        cat >/usr/share/applications/swm-center.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=NS8 Software Center
GenericName=Software Center
Comment=Installieren und aktualisieren Sie freigegebene Unternehmensanwendungen
Comment[en]=Install and update approved enterprise applications
Exec=/usr/local/bin/swm-center gui
Icon=system-software-install
Terminal=false
Categories=System;PackageManager;
EOF
        chmod 0644 /usr/share/applications/swm-center.desktop
    fi
else
    rm -f /etc/xdg/autostart/swm-center.desktop
fi

echo "[NS8-SWM] Enrolling endpoint..."
/usr/local/bin/swm-agent --data-dir /etc/ns8-swm enroll --server "$BASE_URL" --token "$TOKEN"

echo "[NS8-SWM] Installing and enabling systemd service..."
/usr/local/bin/swm-agent --data-dir /etc/ns8-swm service-install
systemctl enable --now swm-agent.service

echo "[NS8-SWM] Installation completed successfully."
