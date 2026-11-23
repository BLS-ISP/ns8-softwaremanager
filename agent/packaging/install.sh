#!/usr/bin/env bash
set -euo pipefail

SERVER_URL=""
TOKEN=""
HEADLESS=0
SKIP_IF_ENROLLED=0

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
        --skip-if-enrolled)
            SKIP_IF_ENROLLED=1
            shift
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 2
            ;;
    esac
done

if [[ -z "$SERVER_URL" || -z "$TOKEN" ]]; then
    echo "Usage: install.sh --server <https://swm.example.org> --token <token> [--headless] [--skip-if-enrolled]" >&2
    exit 2
fi

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    echo "[NS8-SWM] Error: install.sh must be executed as root (use sudo)." >&2
    exit 1
fi

BASE_URL="${SERVER_URL%/}"
INSTALL_DIR="/usr/local/bin"
OS_KERNEL="$(uname -s)"
UNAME_ARCH="$(uname -m)"
case "$UNAME_ARCH" in
    x86_64|amd64) ARCH="amd64" ;;
    aarch64|arm64) ARCH="arm64" ;;
    *)
        echo "[NS8-SWM] Unsupported architecture: $UNAME_ARCH" >&2
        exit 1
        ;;
esac

if [[ "$OS_KERNEL" == "Darwin" ]]; then
    DATA_DIR="/Library/Application Support/NS8-SoftwareManager"
    SOCK_DIR="/var/run/ns8-swm"

    if [[ "$SKIP_IF_ENROLLED" -eq 1 ]] && [ -f "${DATA_DIR}/config.json" ] && launchctl print system/org.nethserver.swm-agent >/dev/null 2>&1; then
        echo "[NS8-SWM] macOS endpoint already enrolled and org.nethserver.swm-agent active; skipping."
        exit 0
    fi

    mkdir -p "${INSTALL_DIR}" "${DATA_DIR}" "${SOCK_DIR}" /Library/LaunchDaemons /Library/LaunchAgents
    chmod 0755 "${INSTALL_DIR}" "${DATA_DIR}" "${SOCK_DIR}"

    echo "[NS8-SWM] Downloading macOS Universal swm-agent from $BASE_URL ..."
    if ! curl -fsSL "$BASE_URL/dl/agents/macos-universal/swm-agent" -o "${INSTALL_DIR}/swm-agent.tmp" 2>/dev/null; then
        if ! curl -fsSL "$BASE_URL/dl/agents/macos-$ARCH/swm-agent" -o "${INSTALL_DIR}/swm-agent.tmp" 2>/dev/null; then
            curl -fsSL "$BASE_URL/dl/agents/macos/swm-agent" -o "${INSTALL_DIR}/swm-agent.tmp"
        fi
    fi
    chmod 0755 "${INSTALL_DIR}/swm-agent.tmp"
    mv -f "${INSTALL_DIR}/swm-agent.tmp" "${INSTALL_DIR}/swm-agent"
    xattr -dr com.apple.quarantine "${INSTALL_DIR}/swm-agent" 2>/dev/null || true

    echo "[NS8-SWM] Enrolling macOS endpoint..."
    "${INSTALL_DIR}/swm-agent" enroll --server "$BASE_URL" --token "$TOKEN" --data-dir "${DATA_DIR}"

    cat >/Library/LaunchDaemons/org.nethserver.swm-agent.plist <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>org.nethserver.swm-agent</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/swm-agent</string>
        <string>run</string>
        <string>--data-dir</string>
        <string>/Library/Application Support/NS8-SoftwareManager</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/var/log/ns8-swm-agent.log</string>
    <key>StandardErrorPath</key>
    <string>/var/log/ns8-swm-agent.err.log</string>
</dict>
</plist>
EOF
    chown root:wheel /Library/LaunchDaemons/org.nethserver.swm-agent.plist 2>/dev/null || true
    chmod 0644 /Library/LaunchDaemons/org.nethserver.swm-agent.plist
    launchctl bootout system /Library/LaunchDaemons/org.nethserver.swm-agent.plist 2>/dev/null || true
    launchctl bootstrap system /Library/LaunchDaemons/org.nethserver.swm-agent.plist

    if [[ "$HEADLESS" -eq 0 ]]; then
        echo "[NS8-SWM] Workstation mode: installing macOS swm-center desktop & tray client..."
        if curl -fsSL "$BASE_URL/dl/agents/macos-universal/swm-center" -o "${INSTALL_DIR}/swm-center.tmp" 2>/dev/null || \
           curl -fsSL "$BASE_URL/dl/agents/macos-$ARCH/swm-center" -o "${INSTALL_DIR}/swm-center.tmp" 2>/dev/null || \
           curl -fsSL "$BASE_URL/dl/agents/macos/swm-center" -o "${INSTALL_DIR}/swm-center.tmp" 2>/dev/null; then
            chmod 0755 "${INSTALL_DIR}/swm-center.tmp"
            mv -f "${INSTALL_DIR}/swm-center.tmp" "${INSTALL_DIR}/swm-center"
            xattr -dr com.apple.quarantine "${INSTALL_DIR}/swm-center" 2>/dev/null || true

            APP_BUNDLE="/Applications/NS8 Software Center.app"
            mkdir -p "${APP_BUNDLE}/Contents/MacOS"
            cat >"${APP_BUNDLE}/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>NS8 Software Center</string>
    <key>CFBundleDisplayName</key>
    <string>NS8 Software Center</string>
    <key>CFBundleIdentifier</key>
    <string>org.nethserver.swm-center</string>
    <key>CFBundleVersion</key>
    <string>1.2.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.2.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>NS8 Software Center</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF
            cat >"${APP_BUNDLE}/Contents/MacOS/NS8 Software Center" <<'EOF'
#!/bin/sh
exec /usr/local/bin/swm-center gui "$@"
EOF
            chmod 0644 "${APP_BUNDLE}/Contents/Info.plist"
            chmod 0755 "${APP_BUNDLE}/Contents/MacOS/NS8 Software Center"
            xattr -dr com.apple.quarantine "${APP_BUNDLE}" 2>/dev/null || true

            cat >/Library/LaunchAgents/org.nethserver.swm-center.plist <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>org.nethserver.swm-center</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/swm-center</string>
        <string>gui</string>
        <string>--tray-only</string>
    </array>
    <key>LimitLoadToSessionType</key>
    <string>Aqua</string>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
EOF
            chown root:wheel /Library/LaunchAgents/org.nethserver.swm-center.plist 2>/dev/null || true
            chmod 0644 /Library/LaunchAgents/org.nethserver.swm-center.plist

            CONSOLE_USER=$(stat -f %Su /dev/console 2>/dev/null || true)
            if [[ -n "$CONSOLE_USER" && "$CONSOLE_USER" != "root" && "$CONSOLE_USER" != "loginwindow" && "$CONSOLE_USER" != "_mbsetupuser" ]]; then
                CONSOLE_UID=$(id -u "$CONSOLE_USER" 2>/dev/null || true)
                if [[ -n "$CONSOLE_UID" ]]; then
                    launchctl bootout "gui/${CONSOLE_UID}" /Library/LaunchAgents/org.nethserver.swm-center.plist 2>/dev/null || true
                    launchctl bootstrap "gui/${CONSOLE_UID}" /Library/LaunchAgents/org.nethserver.swm-center.plist 2>/dev/null || \
                        launchctl kickstart -k "gui/${CONSOLE_UID}/org.nethserver.swm-center" 2>/dev/null || true
                fi
            fi
        fi
    else
        CONSOLE_USER=$(stat -f %Su /dev/console 2>/dev/null || true)
        if [[ -n "$CONSOLE_USER" && "$CONSOLE_USER" != "root" && "$CONSOLE_USER" != "loginwindow" ]]; then
            CONSOLE_UID=$(id -u "$CONSOLE_USER" 2>/dev/null || true)
            if [[ -n "$CONSOLE_UID" ]]; then
                launchctl bootout "gui/${CONSOLE_UID}" /Library/LaunchAgents/org.nethserver.swm-center.plist 2>/dev/null || true
            fi
        fi
        rm -f /Library/LaunchAgents/org.nethserver.swm-center.plist
        rm -rf "/Applications/NS8 Software Center.app"
    fi

    echo "[NS8-SWM] macOS installation completed successfully."
    exit 0
fi

DATA_DIR="/etc/ns8-swm"
if [[ "$SKIP_IF_ENROLLED" -eq 1 ]] && [ -f "${DATA_DIR}/config.json" ] && systemctl is-active --quiet swm-agent.service; then
    echo "[NS8-SWM] Endpoint already enrolled and swm-agent.service active; skipping."
    exit 0
fi

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
        install -d -m 0755 /etc/systemd/user /etc/systemd/user/default.target.wants
        cat >/etc/systemd/user/ns8-swm-center.service <<'EOF'
[Unit]
Description=NS8 Software Center Desktop Tray & Self-Service Agent Bridge
After=graphical-session.target

[Service]
Type=simple
ExecStart=/usr/local/bin/swm-center gui --tray-only
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
EOF
        chmod 0644 /etc/systemd/user/ns8-swm-center.service
        ln -sf /etc/systemd/user/ns8-swm-center.service /etc/systemd/user/default.target.wants/ns8-swm-center.service
    fi
else
    rm -f /etc/xdg/autostart/swm-center.desktop /usr/share/applications/swm-center.desktop
    rm -f /etc/systemd/user/default.target.wants/ns8-swm-center.service /etc/systemd/user/ns8-swm-center.service
fi

echo "[NS8-SWM] Enrolling endpoint..."
/usr/local/bin/swm-agent enroll --server "$BASE_URL" --token "$TOKEN" --data-dir /etc/ns8-swm

echo "[NS8-SWM] Installing and enabling systemd service..."
/usr/local/bin/swm-agent service-install --data-dir /etc/ns8-swm
systemctl enable --now swm-agent.service

echo "[NS8-SWM] Installation completed successfully."
