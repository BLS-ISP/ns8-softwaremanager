# NS8-SoftwareManager

**NS8-SoftwareManager** is a native, centralized **Endpoint, Operating System Patch, and Self-Service Software Management module for NethServer 8 (NS8)** designed to manage both **Windows** and **Linux** systems (workstations and headless servers) from a single web console.

Inspired by *Microsoft Endpoint Configuration Manager (MECM) + Software Center*, the module combines an administrative web console for IT teams with a lightweight local **Software Center** on desktop endpoints, allowing standard users to install, update, or (when permitted) uninstall approved applications **without local administrator privileges**.

---

## Key Features

- **Cross-Platform Fleet & Inventory Management:**
  - Automated discovery of hardware (CPU, RAM, disk capacity), OS/kernel/build versions, IP addresses, logged-in desktop users, **Pending Reboot** status, and full installed software inventory (delta-synced).
  - Real-time online presence and live execution log streaming via outbound WebSockets (`WSS` over port `443` behind NS8 Traefik) with automatic HTTPS polling fallback — works across LAN, VPN, and remote/home-office networks with zero inbound firewall ports on clients.
  - Device groups, tags, and phased rollout rings (*Pilot Ring*, *Workstations*, *Servers*).
- **Modern Admin Web Console (`web/`):**
  - **Light & Dark Mode:** Persistent theme switcher (`localStorage` + automatic `prefers-color-scheme` detection) with high-contrast typography, unified brand emblem, and native Windows/Linux SVG badges.
  - **Responsive Navigation & Accessibility:** Collapsible desktop sidebar rail, mobile/tablet navigation drawer, and keyboard accessibility (`Escape` modal dismissal, ARIA labels, focus management).
  - **Safe Confirmations & Live Toasts:** Built-in confirmation dialogs protecting destructive actions (*revoke/delete device*, *delete package*, *remove deployment*, *revoke enrollment token*) paired with non-intrusive toast notifications.
- **Operating System Patch Management:**
  - **Windows (10/11, Server 2019/2022/2025):** Direct integration with the native *Windows Update Agent (WUA)* without requiring WSUS, including severity filtering and centralized KB exclusion lists.
  - **Linux (Debian/Ubuntu/Mint via `apt`, RHEL/Rocky/Alma/Fedora via `dnf`):** Supports security-only updates or full package upgrades in strictly unattended `noninteractive` mode.
  - **Maintenance Windows & Reboot Policies:** Per-group maintenance schedules (including *Catch-up* execution for laptops that were powered off) and granular reboot policies (*Never reboot automatically*, *Reboot during maintenance window*, *User grace period with snooze & countdown*).
- **Hybrid Software Catalog & Custom Package Repository:**
  - **Native Package Managers:** `winget --scope machine` (safely resolved in Windows Session 0 / `NT AUTHORITY\SYSTEM`), `apt`, `dnf`, and `flatpak`.
  - **Built-in NS8 Package Repository:** Chunked uploads for large custom installers (`.msi`, `.exe`, `.zip`, `.deb`, `.rpm`, `.ps1`, `.sh`), resumable `HTTP Range` downloads (`206 Partial Content`), *Detection Rules*, running-process guards (`blocked_processes`), and custom exit-code mapping (`0`, `3010`, `1641`).
  - **Deployment Modes:** *Required* (automated install & 3rd-party version updates), *Available* (end-user self-service), and *Uninstall*.
- **Single-Binary End-User "Software Center" & System Tray (`swm-center`):**
  - **Zero External Assets:** The entire Vue 3 Software Center frontend (`agent/crates/swm-center/ui/dist`) is embedded directly into the `swm-center` binary at compile time (`SWM_CENTER_UI_DIR` remains available as an optional override during development). A standalone `swm-center.exe` or `swm-center` binary runs out of the box without external asset folders.
  - **Native System Tray & Desktop Notifications:** Native Windows System Tray icon (`Shell_NotifyIconW` with right-click context menu for *Open Software Center*, *Check for Updates*, and *Snooze Reboot*) plus native OS notifications (WinRT Toast on Windows, `notify-send` / XDG autostart on Linux) with Light and Dark mode support.
- **Full NethServer 8 Integration:**
  - Rootless Podman pod (`swm.service`, `swm-db.service` running PostgreSQL 16, `swm-app.service`) bound strictly to `127.0.0.1:${TCP_PORT}:8080` behind NS8 Traefik (with automatic Let's Encrypt TLS).
  - Official NS8 Cluster-Admin Cockpit UI (`en` & `de`), optional integration with NS8 User Domains (Samba Active Directory / OpenLDAP), and full Restic backup, restore, and clone support.

---

## Architecture & Directory Layout

```text
NS8-SoftwareManager/
├── Containerfile                  # Rootless NS8 module image (imageroot + Cockpit UI)
├── Containerfile.app              # Multi-stage app image (FastAPI + Admin SPA + bundled Linux/Win agents)
├── build-images.sh                # Dynamic build & push script (auto-resolves GHCR namespace)
├── CONTRACT.md                    # Canonical REST API, WebSocket, and Ed25519 signature specification
├── REQUIREMENTS.md                # Functional requirements and architectural design notes
├── server/                        # Central Python FastAPI backend (SQLAlchemy 2 async + scheduler)
├── web/                           # Dedicated Admin Web Console (Vue 3 + TypeScript + Vite + TailwindCSS)
├── agent/                         # Cross-platform Rust workspace
│   ├── crates/
│   │   ├── swm-common/            # Shared DTOs, Ed25519/SHA-256 verification & local IPC protocol
│   │   ├── swm-agent/             # Privileged background service (Windows Service / systemd)
│   │   └── swm-center/            # Single-binary desktop & tray app ("Software Center" + embedded Vue 3 UI)
│   └── packaging/                 # Bootstrap scripts, systemd units, XDG autostart, WiX (.wxs) & nFPM (.deb/.rpm)
├── imageroot/                     # NS8 module actions, systemd user units & Restic backup hooks
├── ui/                            # NS8 Cluster-Admin Cockpit UI (Vue 2 + @nethserver/ns8-ui-lib)
└── prebuilt-agents/windows/       # Staging directory for precompiled Windows binaries (.exe) during image builds
```

### Security Model

1. **Cryptographic Job & Catalog Signing (`Ed25519`):** On first startup, the server generates a persistent `Ed25519` keypair (`/data/keys/`). Every job (`SignedJob`) and self-service catalog manifest (`SignedCatalogItem`) is canonically signed by the server and verified by `swm-agent` against the pinned server public key and expiration timestamp (`expires_at_unix`) before execution.
2. **Mandatory `SHA-256` Integrity Verification:** Every custom installer archive or agent self-update binary downloaded from the server is verified against its expected `SHA-256` digest prior to execution.
3. **Strict Privilege Separation (Local IPC):** `swm-center` runs strictly as an unprivileged standard user process and communicates with `swm-agent` over a local IPC channel (Windows Named Pipe `\\.\pipe\ns8-swm-ipc` with SDDL ACLs & `PIPE_REJECT_REMOTE_CLIENTS`, or Linux Unix Domain Socket `/run/ns8-swm/ipc.sock`). Arbitrary shell commands or external URLs cannot be passed over IPC — callers can only trigger `package_id`s already present in `swm-agent`'s `Ed25519`-verified catalog.

---

## Installation on NethServer 8

### 1. Install the Module on Your NS8 Cluster

Run as `root` on the NS8 leader or target node:

```bash
# Latest build:
add-module ghcr.io/bls-isp/softwaremanager:latest 1

# Specific release tag:
add-module ghcr.io/bls-isp/softwaremanager:v1.0.0 1
```

> **Note for Private GHCR Packages:** If the container packages under `ghcr.io/bls-isp` are set to private visibility, either switch `softwaremanager` and `softwaremanager-app` to *Public* in GitHub *Package settings*, or authenticate Podman on the NS8 node beforehand via `podman login ghcr.io`.

### 2. Configure the Instance in NS8 Cluster-Admin

1. Open **NS8 Cluster-Admin**, select the installed **SoftwareManager** instance, and navigate to **Settings**.
2. Enter the **SoftwareManager Hostname (FQDN)** (e.g. `swm.example.com`) and optionally enable **Let's Encrypt Certificate** and **HTTP to HTTPS Redirect**.
3. Optionally select an **NS8 User Domain (Active Directory / OpenLDAP)** for centralized administrator authentication and configure the local fallback admin credentials if needed.
4. Click **Save configuration**, then launch the **SoftwareManager Admin Console** (`https://swm.example.com`).

### 3. Backup, Restore & Clone

The module integrates natively with NS8 Restic cluster backups (`imageroot/etc/state-include.conf` & `module-dump-state`), backing up a consistent PostgreSQL database dump, module environment state, `Ed25519` signing keys, and all uploaded custom software packages stored in the `swm-app-data` volume.

---

## Client Onboarding (Windows & Linux)

1. In the **SoftwareManager Admin Console**, open **Client-Enrollment** and generate an enrollment token (optionally scoped with expiration time, maximum uses, target device group, and auto-approval vs. *Pending Approval*).
2. Run the generated one-line bootstrap command on the target endpoint with Administrator / `root` privileges:

### Windows (Elevated PowerShell)

Workstation (installs the `swm-agent.exe` Windows Service + single-binary System Tray **Software Center** `swm-center.exe` in Windows startup):

```powershell
irm "https://swm.example.com/dl/install.ps1?token=<ENROLLMENT_TOKEN>&headless=0" | iex
```

Windows Server (headless — installs `swm-agent.exe` background service only):

```powershell
irm "https://swm.example.com/dl/install.ps1?token=<ENROLLMENT_TOKEN>&headless=1" | iex
```

### Linux (Debian/Ubuntu/Mint & RHEL/Rocky/Alma/Fedora as `root`)

Linux Desktop (installs `swm-agent`, `swm-center`, and XDG desktop autostart):

```bash
curl -fsSL "https://swm.example.com/dl/install.sh?token=<ENROLLMENT_TOKEN>&headless=0" | sudo bash
```

Linux Server (headless — installs `swm-agent.service` systemd daemon only):

```bash
curl -fsSL "https://swm.example.com/dl/install.sh?token=<ENROLLMENT_TOKEN>&headless=1" | sudo bash
```

Alternatively, every GitHub Actions workflow run produces ready-to-install **`.deb` and `.rpm` client packages** (`ns8-swm-agent`) via `nFPM` (`agent/packaging/nfpm.yaml`) alongside standalone Windows and Linux release binaries.

---

## Automated CI/CD & Release Workflow (`.github/workflows/build-and-test.yml`)

The repository includes an automated GitHub Actions pipeline that runs tests, builds cross-platform client binaries, and publishes NS8 container images:

1. **Dynamic GHCR Namespace (`REPOBASE`):**
   `build-images.sh` automatically resolves `REPOBASE="${REPOBASE:-ghcr.io/${GITHUB_REPOSITORY_OWNER:-bls-isp}}"` (normalized to lowercase), publishing images to `ghcr.io/bls-isp/softwaremanager` and `ghcr.io/bls-isp/softwaremanager-app` (or automatically adapting to forks).
2. **Guaranteed Embedded-UI Build Order:**
   Both the CI matrix and the multi-stage `Containerfile.app` build `agent/crates/swm-center/ui` first and stage `ui/dist` prior to running `cargo build --release --workspace`, guaranteeing that `swm-center` embeds the latest compiled Vue 3 assets.
3. **Client Artifacts (`.deb`, `.rpm`, `.exe`):**
   Every CI run compiles `swm-agent` and `swm-center` for Linux (`x86_64`) and Windows (`x86_64`), builds `.deb` and `.rpm` packages via `nFPM`, and uploads them as workflow artifacts (`linux-client-packages-deb-rpm`, `agents-linux-amd64`, `agents-windows-amd64`).
4. **Automatic GHCR Publishing on Branch Pushes & Release Tags (`v*`):**
   Pushing to `main` or pushing a semantic version tag (`v*`) automatically authenticates to `ghcr.io` with `GITHUB_TOKEN` and publishes both the version/branch tag and `:latest`:

   ```bash
   git tag -a v1.0.0 -m "Release v1.0.0"
   git push origin v1.0.0
   ```

---

## Local Development, Testing & Image Builds

### 1. Backend & NS8 Compliance Tests (`Python 3.12+`)

```bash
python -m pip install -r server/requirements.txt
python -m pytest server/tests -v
python -m compileall server/app imageroot
```

Run the backend locally in development mode:

```bash
cd server
uvicorn app.main:app --reload --host 127.0.0.1 --port 8080
```

### 2. Admin Web Console (`Node.js 20+`)

```bash
npm --prefix web install
npm --prefix web test -- --run
npm --prefix web run build
```

### 3. NS8 Cluster-Admin Cockpit UI (`Node.js 20+`)

```bash
npm --prefix ui install --legacy-peer-deps
npm --prefix ui run build
```

### 4. Rust Agent & Software Center (`Rust 1.85+` & `Node.js 20+`)

```bash
npm --prefix agent/crates/swm-center/ui install
npm --prefix agent/crates/swm-center/ui test -- --run
npm --prefix agent/crates/swm-center/ui run build

cargo test --manifest-path agent/Cargo.toml --workspace
cargo build --release --manifest-path agent/Cargo.toml --workspace
```

### 5. Build Container Images Locally (`Podman` / `Docker`)

To optionally bundle precompiled Windows binaries (`swm-agent.exe`, `swm-center.exe`) during a local image build, copy them into `prebuilt-agents/windows/` beforehand (in GitHub Actions this is handled automatically from the `agents-windows-amd64` artifact):

```bash
REPOBASE=ghcr.io/bls-isp IMAGETAG=latest bash ./build-images.sh
```

This produces both container images:
- `ghcr.io/bls-isp/softwaremanager-app:latest` (FastAPI server + compiled Vue 3 Admin SPA + Linux & Windows client binaries)
- `ghcr.io/bls-isp/softwaremanager:latest` (Rootless NS8 module image containing `imageroot/` and compiled Cockpit UI `ui/`)

---

## License

Licensed under the **GPL-3.0-or-later** license.
