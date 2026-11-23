# NS8-SoftwareManager

**NS8-SoftwareManager** (`v1.2.0`) is a native, centralized **Endpoint, Operating System Patch, and Self-Service Software Management module for NethServer 8 (NS8)** built to manage **Windows**, **Linux**, and **macOS (Apple Silicon `arm64` & Intel `x86_64` Universal)** workstations and headless servers from one web console.

Inspired by *Microsoft Endpoint Configuration Manager (MECM) + Software Center* and *Intune Company Portal*, it pairs a real-time administrative console with a single-binary, tamper-hardened **NS8 Software Center** on desktop endpoints — enabling standard users to install or update approved applications **without local administrator privileges**, while giving IT teams full control over OS patching, maintenance windows, kiosk restrictions, and remote fleet actions.

---

## Key Capabilities

### 1. Cross-Platform Fleet & Real-Time Control (`Windows`, `Linux` & `macOS`)
- **Automated Hardware & Software Inventory:** Discovers CPU, RAM, disk usage, OS/kernel/build versions (`WUA`/Registry, `/etc/os-release`, `sw_vers` + `IOPlatformUUID`), IP addresses, logged-in desktop users, live **Software Center Tray** status (`center_running`), **Pending Reboot** state, and delta-synced installed software (`winget`/Registry, `dpkg`/`rpm`/`flatpak`, `/Applications/*.app` `Info.plist`, `pkgutil`, and Homebrew).
- **Outbound-Only Architecture (`HTTPS / WSS 443`):** Endpoints connect strictly outbound behind NS8 Traefik with automatic Let's Encrypt TLS — zero inbound firewall ports required on LAN, VPN, or remote/home-office devices.
- **Real-Time `Ed25519`-Signed Remote Actions & Bulk Fleet Toolbar:**
  - Instant 1-click **Force Sync** (`force_sync`), **Desktop Admin Notifications** (`admin_notify` with native WinRT / `notify-send` / `osascript` toasts + dismissible banner), and **Remote Reboot Control** (`grace_period`, `immediate`, or `cancel`).
  - Multi-select **Bulk Fleet Bar** (`bulk-action`) for batch approval, group assignment, update scans, immediate sync, and broadcast desktop messages.

### 2. Operating System Patch Management
- **Windows (10/11, Server 2019/2022/2025):** Direct integration with the native *Windows Update Agent (WUA)* without requiring WSUS, including severity filtering and centralized KB exclusion lists.
- **Linux (Debian/Ubuntu/Mint via `apt`, RHEL/Rocky/Alma/Fedora via `dnf`):** Security-only updates or full package upgrades in strictly unattended `noninteractive` mode (`force-confdef` / `force-confold`).
- **macOS (Sonoma / Sequoia & newer — Apple Silicon & Intel):** Native integration with Apple's `softwareupdate -l` and `softwareupdate -i`, including label exclusions, live log streaming, and restart detection (`Action: restart`).
- **Maintenance Windows & Reboot Policies:** Per-group schedules with *Catch-up* execution for laptops that were offline, paired with configurable reboot policies (*Never reboot automatically*, *Reboot during maintenance window*, *User grace period with snooze & deadline countdown*).

### 3. Hybrid Software Catalog & Custom Package Repository
- **Native Package Managers:**
  - **Windows:** `winget --scope machine` (safely resolved for Windows Session 0 / `NT AUTHORITY\SYSTEM`).
  - **Linux:** `apt`, `dnf`, and `flatpak`.
  - **macOS:** **Homebrew Formulae (`brew`)** and **Homebrew Casks (`brew_cask`)** (automatically executed under the non-root console/Homebrew owner via `sudo -u <owner> -H` with `NONINTERACTIVE=1`).
- **Built-in NS8 Package Repository:** Chunked uploads for large custom installers (`.msi`, `.exe`, `.zip`, `.ps1`, `.deb`, `.rpm`, `.pkg`, `.dmg`, `.app.zip`, `.sh`), resumable `HTTP 206 Range` downloads, `SHA-256` verification, running-process guards (`blocked_processes`), custom exit-code mapping (`0`, `3010`, `1641`), and cross-platform *Detection Rules* (`file`, `registry`, `package`, `app_bundle`, `pkgutil`, `brew`, `brew_cask`).
- **Deployment Modes:** *Required* (automated background install & 3rd-party updates), *Available* (end-user self-service), and *Uninstall*.

### 4. Tamper-Hardened Single-Binary "NS8 Software Center" (`swm-center`)
- **Zero External Assets:** The compiled Vue 3 SPA (`agent/crates/swm-center/ui/dist`) is embedded directly into the `swm-center` binary at compile time.
- **Hardened Tray & Session Watchdog (Self-Healing):**
  - When `enforce_tray` is active (default), the System Tray **Quit** option is removed and `QuitTray` events are ignored.
  - **Single-Instance Session Guard:** Launching `swm-center gui` a second time detects the active user-session instance (`center-session.json` + `/api/ping`) and brings the existing window to the foreground (`/api/open-window`) instead of spawning duplicate tray icons.
  - **Two-Tier Recovery:** Windows SCM crash recovery (`sc.exe failure`) + `\NS8SoftwareCenterGuardian` ScheduledTask respawned by `swm-agent` if killed in Task Manager; Linux global `/etc/systemd/user/ns8-swm-center.service` (`Restart=always`); macOS `/Library/LaunchDaemons/org.nethserver.swm-agent.plist` + `/Library/LaunchAgents/org.nethserver.swm-center.plist` (`KeepAlive = true`, `Aqua` session) + `launchctl kickstart` watchdog.
- **Per-Group Desktop Policies & Helpdesk Diagnostics:**
  - Group-level controls for `enforce_tray`, `allow_self_service` (*Managed Kiosk Mode*, enforced server-side at the `swm-agent` IPC boundary), `allow_user_os_updates`, and `silent_notifications`.
  - Built-in **System & Diagnostics** tab with live server sync and 1-click diagnostic report copy for IT support.

### 5. Guided Mass Rollout & Full NethServer 8 Integration
- **5-Tab Client Rollout Assistant (`EnrollmentView.vue`):** Generates ready-to-run **Quick 1-Liners**, idempotent **Active Directory GPO / Microsoft Intune** startup scripts (`install-swm-gpo.ps1`), **Ansible / Cloud-Init** playbooks (`deploy-swm-agent.yml`), **macOS MDM** root scripts for Jamf Pro / Intune / Mosyle (`install-swm-mdm-macos.sh`), and **Gold-Image CLI** commands, plus live `SHA-256` binary checksums (`GET /api/v1/admin/agent-artifacts`).
- **Native Rootless NS8 Module:** Rootless Podman pod (`swm.service`, `swm-db.service` PostgreSQL 16, `swm-app.service`) bound to `127.0.0.1:${TCP_PORT}:8080`, pure NS8 `core.css` Cluster-Admin Cockpit UI (`ui/`, `en` & `de`), NS8 `Ldapproxy` user-domain authentication (Samba AD & OpenLDAP), zero-downtime live DB schema migration (`ensure_schema_upgrades()`), and full Restic backup, restore, and clone support.

---

## Architecture & Directory Layout

```text
NS8-SoftwareManager/
├── Containerfile                  # Rootless NS8 module image (imageroot + Cockpit UI)
├── Containerfile.app              # Multi-stage app image (FastAPI + Admin SPA + Win/Linux/macOS Universal binaries)
├── build-images.sh                # Dynamic Podman build & GHCR push script
├── CONTRACT.md                    # Canonical REST, WebSocket, Local IPC & Ed25519 cryptographic specification
├── REQUIREMENTS.md                # Product concept, requirements & implementation status (German)
├── CHANGELOG.md                   # Release history (Keep a Changelog / SemVer)
├── docs/decisions/                # Architecture Decision Records (ADRs)
├── server/                        # Central Python FastAPI backend (SQLAlchemy 2 async, scheduler & signing)
├── web/                           # Standalone Admin Web Console (Vue 3 + TypeScript + Vite + TailwindCSS)
├── ui/                            # NS8 Cluster-Admin Cockpit UI (Vue 2 + @nethserver/ns8-ui-lib, en/de)
├── imageroot/                     # NS8 module actions, systemd user units & Restic backup/restore hooks
└── agent/                         # Cross-platform Rust workspace (Windows, Linux & macOS Universal)
    ├── crates/
    │   ├── swm-common/            # Shared DTOs, Ed25519/SHA-256 verification & typed local IPC protocol
    │   ├── swm-agent/             # Privileged daemon (Windows Service / systemd / macOS LaunchDaemon)
    │   └── swm-center/            # Hardened desktop & tray client ("NS8 Software Center" + embedded Vue 3 UI)
    └── packaging/                 # Bootstrap scripts, systemd units, macOS launchd plists, WiX (.wxs) & nFPM (.deb/.rpm)
```

Component-level developer guides and architectural rationales:
- [server/README.md](server/README.md) — FastAPI backend, configuration, live schema upgrades, and `pytest` suites
- [agent/README.md](agent/README.md) — Rust workspace, embedded UI build order, platform matrix, and CLI reference
- [web/README.md](web/README.md) — Dual-UI boundary (`web/` standalone console vs. `ui/` NS8 Cockpit module)
- [CONTRACT.md](CONTRACT.md) — Complete REST, WebSocket, Local IPC, and `Ed25519` wire specification
- [docs/decisions/](docs/decisions/) — Architecture Decision Records (`ADR-001` through `ADR-003`)

---

## Security Model

1. **Cryptographic Job & Catalog Signing (`Ed25519`):** On first startup, the server generates a persistent `Ed25519` keypair (`/data/keys/`). Every job (`SignedJob` — including software deployments, OS patches, self-updates, and real-time `remote_control` actions) and every self-service catalog manifest (`SignedCatalogItem`) is canonically signed and verified by `swm-agent` against the pinned server public key and expiration timestamp (`expires_at_unix`).
2. **Mandatory `SHA-256` & Binary Header Verification:** Downloaded installers and agent self-update binaries are verified against their expected `SHA-256` digest (plus PE `MZ` / ELF / Mach-O Universal `0xCAFEBABE` header validation during self-updates) before execution.
3. **Strict Privilege Separation & Server-Side IPC Policy Gates:** `swm-center` runs strictly as an unprivileged desktop user and talks to `swm-agent` over a typed local IPC channel (`\\.\pipe\ns8-swm-ipc` on Windows, `/run/ns8-swm/ipc.sock` on Linux, `/var/run/ns8-swm/ipc.sock` on macOS). Raw shell commands or URLs cannot cross IPC, and `swm-agent` enforces `allow_self_service` and `allow_user_os_updates` policies directly at the privileged IPC boundary.

---

## Installation & Updating on NethServer 8

### Option A: Via NS8 Software Center (`ns8-nethforge` Repository)
Add the `BLS-ISP` repository in **NS8 Cluster-Admin -> Settings -> Software repositories** (`https://raw.githubusercontent.com/BLS-ISP/ns8-nethforge/repomd/ns8/updates/`), then install or update **SoftwareManager** directly from the **NS8 Software Center**.

### Option B: Via NS8 CLI (`add-module` / `update-module`)
Run as `root` on the NS8 leader node (note that NS8 module tags use strict SemVer without a `v` prefix):

```bash
# Fresh installation (release 1.2.0):
add-module ghcr.io/bls-isp/softwaremanager:1.2.0 1

# In-place update of an existing instance (e.g. softwaremanager1):
api-cli run update-module --data '{"module_url":"ghcr.io/bls-isp/softwaremanager:1.2.0","instances":["softwaremanager1"],"force":true}'
```

### Instance Configuration
1. Open **NS8 Cluster-Admin -> SoftwareManager -> Settings**.
2. Set the **SoftwareManager Hostname (FQDN)** (e.g. `swm.example.com`) and enable **Let's Encrypt** and **HTTP -> HTTPS** as needed.
3. Optionally bind an **NS8 User Domain** (*Samba Active Directory* or *OpenLDAP*) for administrator login alongside the local fallback admin account.
4. Click **Save** and open `https://swm.example.com`.

---

## Client Onboarding (Windows, Linux & macOS)

Create an enrollment token in the Admin Console under **Client-Rollout & Agent** (`/enrollment`), then run the generated bootstrapper (or download the pre-built **GPO**, **Ansible**, or **macOS MDM** script from the rollout tabs):

### Windows (`NT AUTHORITY\SYSTEM` / Elevated PowerShell)
```powershell
# Workstation (swm-agent service + hardened swm-center tray & Start Menu shortcut):
irm "https://swm.example.com/dl/install.ps1?token=<TOKEN>&headless=0" | iex

# Headless Windows Server (swm-agent service only, idempotent for GPO with skip_if_enrolled=1):
irm "https://swm.example.com/dl/install.ps1?token=<TOKEN>&headless=1&skip_if_enrolled=1" | iex
```

### Linux & macOS Universal (`uname -s` Auto-Detection: `Linux` & `Darwin`)
```bash
# Desktop / MacBook / iMac (installs agent daemon + NS8 Software Center + session watchdog):
curl -fsSL "https://swm.example.com/dl/install.sh?token=<TOKEN>&headless=0" | sudo bash

# Headless Server / Idempotent Ansible or Jamf/Intune MDM execution:
curl -fsSL "https://swm.example.com/dl/install.sh?token=<TOKEN>&headless=1&skip_if_enrolled=1" | sudo bash
```

- **On Linux:** Configures `/etc/systemd/system/swm-agent.service` and (when `headless=0`) `/etc/systemd/user/ns8-swm-center.service` (`Restart=always`) + desktop launchers. Ready-to-install `.deb` and `.rpm` packages (`ns8-swm-agent`) are also built on every CI run.
- **On macOS (Apple Silicon `arm64` & Intel `x86_64`):** Downloads the `lipo` Universal Binaries, clears Gatekeeper quarantine (`xattr -dr com.apple.quarantine`), registers `/Library/LaunchDaemons/org.nethserver.swm-agent.plist`, and (when `headless=0`) installs `/Applications/NS8 Software Center.app` + `/Library/LaunchAgents/org.nethserver.swm-center.plist` and starts it immediately in the active Aqua session.

---

## Local Development & Verification Commands

| Component | Test & Build Commands |
|---|---|
| **1. FastAPI Backend & NS8 Rules** (`Python 3.12+`) | `pip install -r server/requirements.txt`<br>`python -m pytest server/tests -v` |
| **2. Admin Web Console** (`Node.js 20+`) | `npm --prefix web install`<br>`npm --prefix web test -- --run && npm --prefix web run build` |
| **3. NS8 Cockpit UI** (`Node.js 20+`) | `npm --prefix ui install --legacy-peer-deps`<br>`npm --prefix ui run build` |
| **4. Software Center UI + Rust Workspace** (`Rust 1.85+`) | `npm --prefix agent/crates/swm-center/ui install`<br>`npm --prefix agent/crates/swm-center/ui test -- --run`<br>`npm --prefix agent/crates/swm-center/ui run build` *(must run before Cargo!)*<br>`cargo test --manifest-path agent/Cargo.toml --workspace`<br>`cargo build --release --manifest-path agent/Cargo.toml --workspace` |
| **5. Container Images** (`Podman`) | `REPOBASE=ghcr.io/bls-isp IMAGETAG=1.2.0 bash ./build-images.sh` |

> **Release Tagging Note:** GitHub Actions strips the leading `v` from `v*` git tags (`v1.2.0` -> GHCR image tag `1.2.0`) for compatibility with `ns8-nethforge` (`createrepo.py`). Always push the annotated git tag (`git push origin v1.2.0`) before pushing `main`.

---

## License

Licensed under the **GPL-3.0-or-later** license. See [LICENSE](LICENSE).
