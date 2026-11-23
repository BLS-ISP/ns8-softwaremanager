# NS8-SoftwareManager — Wire, API, IPC & Cryptographic Contract (`v1.2.0`)

This document defines the wire, REST, WebSocket, Local IPC, and `Ed25519` cryptographic contract shared across `server/` (FastAPI), `agent/` (`swm-common`, `swm-agent`, `swm-center`), and `web/` (Vue 3 Admin Web Console).

---

## 1. Cryptographic Contract (`Ed25519` + `SHA-256`)

### 1.1 Keypair Format & Storage
- **Algorithm:** `Ed25519` (`cryptography` on Python server; `ed25519-dalek` on Rust agent).
- **Server storage:** `{DATA_DIR}/keys/private.key` (32 raw bytes) and `{DATA_DIR}/keys/public.key` (32 raw bytes), generated automatically on first boot and included in NS8 Restic backups.
- `server_public_key_b64`: Standard Base64 (`RFC 4648` with `=`) of the 32-byte public key (44 ASCII chars).
- `signature_b64`: Standard Base64 (`RFC 4648` with `=`) of the 64-byte signature (88 ASCII chars).
- `sha256`: Lowercase 64-character hex `SHA-256` digest (or `""` when not applicable).

### 1.2 Canonical UTF-8 Signing Payloads

All fields are serialized as UTF-8 strings (`None` / `null` -> `""`) and signed over `canonical.encode("utf-8")`.

#### `SignedJob` Canonical String
```text
v1|job|{job_id}|{device_id}|{job_type}|{action}|{target_id}|{version}|{sha256_or_empty}|{command_or_empty}|{expires_at_unix}
```
- `job_type`: `"os_patch"` | `"software"` | `"self_update"` | `"scan_updates"` | `"remote_control"`
- `action`:
  - Software / Patch / Self-Update: `"install"` | `"update"` | `"uninstall"` | `"scan"`
  - Real-time Remote Control (`job_type == "remote_control"`):
    - `"force_sync"` (`target_id = "agent"`, `command = ""`) — triggers immediate inventory + catalog sync
    - `"admin_notify"` (`target_id = "desktop"`, `command = JSON {"title": str, "message": str, "urgent": bool}`) — pushes native desktop toast + persistent dismissible banner to `swm-center`
    - `"remote_reboot"` (`target_id = mode`, `command = JSON {"mode": "grace_period" | "immediate" | "cancel", "grace_minutes": int}`) — schedules, forces, or cancels an endpoint reboot
- `expires_at_unix`: UTC Unix timestamp in seconds (`swm-agent` rejects any job with `expires_at_unix < now_unix`).

#### `SignedCatalogItem` Canonical String
```text
v1|catalog|{package_id}|{device_id}|{source_type}|{package_ref}|{version}|{sha256_or_empty}|{install_cmd_or_empty}|{uninstall_cmd_or_empty}|{allow_uninstall_0_or_1}
```
- `os_family`: `"windows"` | `"linux"` | `"macos"`
- `source_type`: `"winget"` | `"apt"` | `"dnf"` | `"flatpak"` | `"brew"` | `"brew_cask"` | `"custom"`
- `allow_uninstall_0_or_1`: `"1"` when `allow_uninstall == true`, else `"0"`

---

## 2. Health, Bootstrap & Static Artifact Endpoints

- `GET /api/v1/health` -> `200 OK`
  ```json
  {"status": "ok", "version": "1.2.0", "database": "connected"}
  ```
- `GET /dl/install.ps1?token=<token>&headless=<0|1>&skip_if_enrolled=<0|1>` (`text/plain; charset=utf-8`):
  - Windows PowerShell bootstrapper (`\r\n` CRLF). Configures Windows SCM crash recovery (`sc.exe failure`), registers & starts `\NS8SoftwareCenterGuardian` (`headless=0`) + Start Menu `.lnk` shortcut, and exits `0` when `skip_if_enrolled=1` and already enrolled.
- `GET /dl/install.sh?token=<token>&headless=<0|1>&skip_if_enrolled=<0|1>` (`text/x-shellscript; charset=utf-8`):
  - Unified Unix Bash bootstrapper (pure `\n` LF) auto-detecting `uname -s`:
    - **`Linux`:** Installs `/usr/local/bin/swm-agent` + `/etc/systemd/system/swm-agent.service` and (`headless=0`) `/etc/systemd/user/ns8-swm-center.service` (`Restart=always`) + XDG desktop entries.
    - **`Darwin` (macOS):** Downloads `macos-universal` binaries, strips Gatekeeper quarantine (`xattr -dr com.apple.quarantine`), installs `/Library/LaunchDaemons/org.nethserver.swm-agent.plist` (`DATA_DIR = /Library/Application Support/NS8-SoftwareManager`), and (`headless=0`) creates `/Applications/NS8 Software Center.app` + `/Library/LaunchAgents/org.nethserver.swm-center.plist` (`LimitLoadToSessionType = Aqua`, `KeepAlive = true`) with immediate `launchctl bootstrap gui/<uid>` startup.
- `GET /dl/agents/{platform}/{filename}` -> Serves bundled client binaries (`windows`, `linux-amd64`, `linux`, `macos-universal`, `macos`, `macos-arm64`, `macos-amd64`).

---

## 3. Admin REST API (`/api/v1/admin/...`)

All endpoints except `/api/v1/admin/auth/login` require `Authorization: Bearer <jwt>`.

### 3.1 Auth, Dashboard & Bundled Agent Artifacts
- `POST /api/v1/admin/auth/login` (`{"username": "...", "password": "...", "auth_source": "local" | "ldap"}`)
- `GET /api/v1/admin/auth/me`
- `GET /api/v1/admin/dashboard/summary`
  - Includes 3-way `os_breakdown`: `{"windows": 7, "linux": 5, "macos": 3}`
- `GET /api/v1/admin/agent-artifacts`
  - Returns live availability, `size_bytes`, `sha256`, and `download_url` for all bundled `KNOWN_AGENT_ARTIFACTS` (`windows`, `linux-amd64`, `macos-universal`).

### 3.2 Enrollment Tokens
- `GET /api/v1/admin/enrollment-tokens`
- `POST /api/v1/admin/enrollment-tokens`
- `DELETE /api/v1/admin/enrollment-tokens/{id}`

### 3.3 Devices, Real-Time Remote Controls & Bulk Fleet Actions
- `GET /api/v1/admin/devices?status=&group_id=&os_family=&search=` -> `list[DeviceSummary]` (includes `center_running: bool` and `os_family: "windows" | "linux" | "macos"`)
- `GET /api/v1/admin/devices/{id}` -> `DeviceDetail`
- `POST /api/v1/admin/devices/{id}/approve` | `/revoke`
- `PATCH /api/v1/admin/devices/{id}` (`{"group_ids": [...], "tags": [...]}`)
- `DELETE /api/v1/admin/devices/{id}`
- `POST /api/v1/admin/devices/{id}/scan-updates` -> Creates & pushes signed `scan_updates` job
- `POST /api/v1/admin/devices/{id}/sync` -> Creates & pushes signed `remote_control` / `force_sync` job
- `POST /api/v1/admin/devices/{id}/notify` (`{"title": "...", "message": "...", "urgent": false}`) -> Creates & pushes signed `remote_control` / `admin_notify` job
- `POST /api/v1/admin/devices/{id}/reboot` (`{"mode": "grace_period" | "immediate" | "cancel", "grace_minutes": 60}`) -> Creates & pushes signed `remote_control` / `remote_reboot` job
- `POST /api/v1/admin/devices/bulk-action`
  - Request:
    ```json
    {
      "device_ids": ["dev-1", "dev-2"],
      "action": "approve" | "sync" | "scan_updates" | "assign_group" | "notify",
      "group_id": "optional-grp-uuid",
      "title": "Optional notification title",
      "message": "Optional notification body",
      "urgent": false
    }
    ```

### 3.4 Groups, Desktop Policies & Maintenance Windows
- `GET /api/v1/admin/groups` | `POST /api/v1/admin/groups` | `PATCH /api/v1/admin/groups/{id}` | `DELETE /api/v1/admin/groups/{id}`
  - Group payload includes both reboot and desktop hardening policies:
    ```json
    {
      "name": "Office Workstations",
      "description": "Managed desktop endpoints",
      "reboot_policy": "never" | "maintenance_window" | "grace_period",
      "grace_period_minutes": 240,
      "enforce_tray": true,
      "allow_self_service": true,
      "allow_user_os_updates": true,
      "silent_notifications": false
    }
    ```
  - **Multi-Group Resolution Rule:** When a device belongs to multiple groups, `enforce_tray` and `silent_notifications` resolve restrictively via logical `OR` (any `true` wins), while `allow_self_service` and `allow_user_os_updates` resolve restrictively via logical `AND` (any `false` locks self-service into Managed Kiosk Mode). Updating a group immediately pushes `catalog_updated` over active WebSockets.
- `GET /api/v1/admin/maintenance-windows` | `POST` | `DELETE /{id}`

### 3.5 OS Patching (`WUA`, `apt`/`dnf`, `softwareupdate`)
- `GET /api/v1/admin/patches` -> `list[PatchSummary]` (`os_family: "windows" | "linux" | "macos"`)
- `POST /api/v1/admin/patches/approve`
- `POST /api/v1/admin/patches/exclude`

### 3.6 Software Catalog, Detection Rules & Deployments
- `GET /api/v1/admin/catalog` | `POST` | `PATCH /{id}` | `DELETE /{id}`
- `POST /api/v1/admin/catalog/{id}/upload` & `/upload-chunk` (accepts `.msi`, `.exe`, `.zip`, `.ps1`, `.deb`, `.rpm`, `.pkg`, `.dmg`, `.sh`)
- **Supported `detection_rule` Schemas:**
  - File check: `{"type": "file", "path": "...", "min_version": "optional"}`
  - Windows Registry: `{"type": "registry", "key": "HKLM\\...", "value": "DisplayVersion", "min_version": "optional"}`
  - Native Package (`winget` / `dpkg` / `rpm` / `flatpak`): `{"type": "package", "package": "...", "min_version": "optional"}`
  - macOS App Bundle: `{"type": "app_bundle", "path": "/Applications/Firefox.app", "bundle_id": "optional", "min_version": "optional"}`
  - macOS Installer Receipt: `{"type": "pkgutil", "package_id": "org.vendor.pkg", "min_version": "optional"}`
  - Homebrew: `{"type": "brew" | "brew_cask", "package": "firefox", "min_version": "optional"}`
- `GET /api/v1/admin/deployments` | `POST` | `DELETE /{id}`
- `GET /api/v1/admin/jobs` | `GET /api/v1/admin/jobs/{id}`
- `POST /api/v1/admin/self-update` (`os_family: "windows" | "linux" | "macos" | null`; default macOS URL: `/dl/agents/macos-universal/swm-agent`)

---

## 4. Agent Wire API (`/api/v1/agent/...`)

- `POST /api/v1/agent/enroll` (`os_family` normalizes `"windows"`, `"linux"`, and `"mac"`/`"macos"`/`"darwin"`/`"osx"` -> `"macos"`)
- `GET /api/v1/agent/status`
  - Includes resolved `desktop_policy`:
    ```json
    {
      "device_id": "uuid",
      "status": "approved",
      "reboot_policy": "grace_period",
      "grace_period_minutes": 240,
      "in_maintenance_window": false,
      "excluded_updates": [],
      "server_public_key_b64": "base64...",
      "desktop_policy": {
        "enforce_tray": true,
        "allow_self_service": true,
        "allow_user_os_updates": true,
        "silent_notifications": false
      }
    }
    ```
- `POST /api/v1/agent/inventory` (includes `center_running: bool` alongside hardware, OS, delta software hash, and pending OS updates)
- `GET /api/v1/agent/jobs/pending`
- `POST /api/v1/agent/jobs/{job_id}/result`
- `POST /api/v1/agent/self-service/trigger`
- `GET /api/v1/agent/packages/{package_id}/download` (`HTTP 206 Partial Content` + `X-Package-SHA256`)
- `WS /api/v1/agent/ws?token=<device_token>` (`heartbeat`, `log_chunk` <-> `heartbeat_ack`, `job_available`, `catalog_updated`, `revoked`)

---

## 5. Local Endpoint IPC & Single-Instance Protocol (`swm-common` / `swm-center`)

### 5.1 Privileged IPC Transport (`swm-center` -> `swm-agent`)
- **Framing:** Newline-delimited UTF-8 JSON (`MAX_IPC_FRAME_BYTES = 1 MiB`).
- **Default Platform Endpoints (`swm_common::default_ipc_address()`):**
  - **Windows:** `\\.\pipe\ns8-swm-ipc` (SDDL ACL allowing standard users read/write while `PIPE_REJECT_REMOTE_CLIENTS` blocks SMB network access)
  - **Linux:** `/run/ns8-swm/ipc.sock`
  - **macOS:** `/var/run/ns8-swm/ipc.sock`
- **Typed `IpcRequest` Variants:**
  - `{"command": "list_catalog"}`
  - `{"command": "get_status"}` -> returns `CenterStatusSnapshot` (`hostname`, `device_id`, `server_url`, `ws_connected`, `agent_version`, `desktop_policy`, `admin_messages`, `reboot_state`, `pending_updates`, `recent_history`)
  - `{"command": "trigger_package", "package_id": "<uuid>", "action": "install" | "update" | "uninstall"}` -> rejected with `PermissionDenied` if `!desktop_policy.allow_self_service`
  - `{"command": "trigger_pending_updates"}` -> rejected with `PermissionDenied` if `!desktop_policy.allow_user_os_updates`
  - `{"command": "sync_now"}` -> wakes `swm-agent` for an immediate server sync cycle
  - `{"command": "dismiss_message", "message_id": "<id>"}` -> removes acknowledged admin desktop message
  - `{"command": "snooze_reboot", "minutes": 60}` | `{"command": "confirm_reboot_now"}`

### 5.2 Single-Instance Desktop Session Guard (`swm-center`)
- Session lock file `center-session.json` (`{"pid": u32, "port": u16, "url": "http://127.0.0.1:<port>"}`):
  - **Windows:** `%LOCALAPPDATA%\NS8-SoftwareManager\center-session.json`
  - **macOS:** `~/Library/Application Support/NS8-SoftwareManager/center-session.json`
  - **Linux:** `${XDG_RUNTIME_DIR:-/tmp}/ns8-swm-center-<uid>.json`
- On startup, a second `swm-center gui` invocation probes `GET /api/ping` (`300 ms` timeout); if alive and `--tray-only` is not set, it calls `POST /api/open-window` on the running instance and exits `0`.
