# NS8-SoftwareManager — Wire, API & Cryptographic Contract (`v1`)

This document defines the exact wire, REST, WebSocket, and Ed25519 cryptographic contract shared across `server/` (`backend`), `agent/` (`swm-common`, `swm-agent`, `swm-center`), and `web/` (Vue 3 Admin Web Console).

---

## 1. Cryptographic Contract (`Ed25519` + `SHA-256`)

### 1.1 Keypair Format & Storage
- Algorithm: **Ed25519** (`cryptography.hazmat.primitives.asymmetric.ed25519` on server; `ed25519-dalek` on Rust agent).
- Storage on server: `{DATA_DIR}/keys/private.key` (32 raw bytes) and `{DATA_DIR}/keys/public.key` (32 raw bytes), auto-generated on first boot if absent.
- `server_public_key_b64`: Standard Base64 (`RFC 4648`, with `=` padding) encoding of the raw 32-byte Ed25519 public key (44 ASCII characters).
- `signature_b64`: Standard Base64 (`RFC 4648`, with `=` padding) encoding of the raw 64-byte Ed25519 signature (88 ASCII characters).
- File hashes (`sha256`): Lowercase 64-character hexadecimal `SHA-256` digest (or `""` when not applicable).

### 1.2 Canonical UTF-8 Strings for Signing & Verification

All fields are interpolated as UTF-8 strings (`None` / `null` becomes `""`). Signatures are computed over `canonical.encode("utf-8")`.

#### `SignedJob` Canonical String
```text
v1|job|{job_id}|{device_id}|{job_type}|{action}|{target_id}|{version}|{sha256_or_empty}|{command_or_empty}|{expires_at_unix}
```
- `job_type`: `"os_patch"` | `"software"` | `"self_update"` | `"scan_updates"`
- `action`: `"install"` | `"update"` | `"uninstall"` | `"scan"`
- `target_id`: Package UUID (for `"software"`), comma-separated update IDs or `"all"` (for `"os_patch"`), `"agent"` (for `"self_update"`), or `"os_updates"` (for `"scan_updates"`)
- `version`: Version string (e.g. `"24.08"`, `"1.0.0"`, or `""`)
- `sha256_or_empty`: Lowercase 64-char hex SHA-256 of binary/installer, or `""`
- `command_or_empty`: Silent install/uninstall argument string, or `""`
- `expires_at_unix`: Integer Unix timestamp in UTC seconds (agent MUST reject if `expires_at_unix < now_unix`)

#### `SignedCatalogItem` Canonical String
```text
v1|catalog|{package_id}|{device_id}|{source_type}|{package_ref}|{version}|{sha256_or_empty}|{install_cmd_or_empty}|{uninstall_cmd_or_empty}|{allow_uninstall_0_or_1}
```
- `source_type`: `"winget"` | `"apt"` | `"dnf"` | `"flatpak"` | `"custom"`
- `allow_uninstall_0_or_1`: `"1"` when `allow_uninstall == true`, `"0"` when `allow_uninstall == false`

---

## 2. Health, Bootstrap & Static Endpoints

- `GET /api/v1/health` -> `200 OK`
  ```json
  {"status": "ok", "version": "1.0.0", "database": "connected"}
  ```
- `GET /dl/install.ps1?token=<token>&headless=<0|1>` -> `200 OK` (`text/plain; charset=utf-8`): PowerShell bootstrap installer configured with server base URL, enrollment token, and headless flag.
- `GET /dl/install.sh?token=<token>&headless=<0|1>` -> `200 OK` (`text/x-shellscript; charset=utf-8`): Bash bootstrap installer configured with server base URL, enrollment token, and headless flag.
- `GET /dl/agents/{platform}/{filename}` -> Serves static agent binaries from `STATIC_AGENTS_DIR/{platform}/{filename}` (`404` if missing).
- Static SPA Fallback: When `WEB_DIST_DIR` exists, serves static assets and falls back to `index.html` for non-`/api/*` and non-`/dl/*` paths.

---

## 3. Admin REST API (`/api/v1/admin/...`)

All endpoints except `/api/v1/admin/auth/login` require header:
`Authorization: Bearer <access_token>` (returns `401 Unauthorized` when missing/invalid).

### 3.1 Auth & Dashboard
- `POST /api/v1/admin/auth/login`
  - Request: `{"username": "admin", "password": "...", "auth_source": "local" | "ldap"}`
  - Response (`200`):
    ```json
    {
      "access_token": "eyJ...",
      "token_type": "bearer",
      "user": {"id": "uuid", "username": "admin", "role": "admin", "auth_source": "local"}
    }
    ```
- `GET /api/v1/admin/auth/me`
  - Response (`200`): `{"id": "uuid", "username": "admin", "role": "admin", "auth_source": "local"}`
- `GET /api/v1/admin/dashboard/summary`
  - Response (`200`):
    ```json
    {
      "total_devices": 12,
      "online_devices": 9,
      "pending_approval_devices": 1,
      "reboot_required_devices": 2,
      "patch_compliance_percent": 83.3,
      "pending_critical_patches": 4,
      "recent_jobs": [],
      "os_breakdown": {"windows": 7, "linux": 5}
    }
    ```

### 3.2 Enrollment Tokens
- `GET /api/v1/admin/enrollment-tokens` -> `list[EnrollmentToken]`
- `POST /api/v1/admin/enrollment-tokens`
  - Request:
    ```json
    {
      "name": "Workstations Rollout",
      "expires_in_hours": 72,
      "max_uses": 100,
      "auto_approve": false,
      "group_id": null
    }
    ```
  - Response (`200` / `201`, status `200` returned for uniform handling):
    ```json
    {
      "id": "uuid",
      "name": "Workstations Rollout",
      "token": "swm_tok_...",
      "expires_at": "2026-11-23T20:00:00Z",
      "max_uses": 100,
      "uses_count": 0,
      "auto_approve": false,
      "group_id": null,
      "revoked": false,
      "created_at": "2026-11-20T20:00:00Z",
      "install_ps1_cmd": "irm 'http://localhost:8080/dl/install.ps1?token=swm_tok_...&headless=0' | iex",
      "install_sh_cmd": "curl -fsSL 'http://localhost:8080/dl/install.sh?token=swm_tok_...&headless=0' | sudo bash"
    }
    ```
- `DELETE /api/v1/admin/enrollment-tokens/{id}` -> `{"status": "deleted", "id": "..."}`

### 3.3 Devices
- `GET /api/v1/admin/devices?status=&group_id=&os_family=&search=` -> `list[DeviceSummary]`
  ```json
  {
    "id": "uuid",
    "hostname": "ws-pc01",
    "domain": "WORKGROUP",
    "os_family": "windows",
    "os_name": "Windows 11 Pro",
    "os_version": "23H2",
    "kernel_or_build": "22631.4317",
    "arch": "amd64",
    "agent_version": "1.0.0",
    "machine_uid": "uid-123",
    "status": "approved",
    "is_online": true,
    "reboot_required": false,
    "ip_addresses": ["192.168.1.50"],
    "logged_in_users": ["jdoe"],
    "group_ids": ["grp-uuid"],
    "groups": [{"id": "grp-uuid", "name": "Workstations"}],
    "tags": ["office"],
    "pending_updates_count": 2,
    "installed_software_count": 45,
    "last_seen_at": "2026-11-20T20:05:00Z",
    "created_at": "2026-11-20T20:00:00Z"
  }
  ```
- `GET /api/v1/admin/devices/{id}` -> `DeviceDetail` (all `DeviceSummary` fields + `hardware`, `installed_software`, `pending_os_updates`, `recent_jobs`)
- `POST /api/v1/admin/devices/{id}/approve` -> `DeviceSummary`
- `POST /api/v1/admin/devices/{id}/revoke` -> `DeviceSummary`
- `PATCH /api/v1/admin/devices/{id}` (`{"group_ids": [...], "tags": [...]}`) -> `DeviceSummary`
- `DELETE /api/v1/admin/devices/{id}` -> `{"status": "deleted", "id": "..."}`
- `POST /api/v1/admin/devices/{id}/scan-updates` -> `Job`

### 3.4 Groups & Maintenance Windows
- `GET /api/v1/admin/groups` -> `list[Group]`
- `POST /api/v1/admin/groups`
  - Request: `{"name": "Servers", "description": "Production servers", "reboot_policy": "never" | "maintenance_window" | "grace_period", "grace_period_minutes": 240}`
  - Response: `{"id": "uuid", "name": "Servers", "description": "...", "reboot_policy": "never", "grace_period_minutes": 240, "device_count": 0, "created_at": "..."}`
- `PATCH /api/v1/admin/groups/{id}` -> `Group`
- `DELETE /api/v1/admin/groups/{id}` -> `{"status": "deleted", "id": "..."}`
- `GET /api/v1/admin/maintenance-windows` -> `list[MaintenanceWindow]`
- `POST /api/v1/admin/maintenance-windows`
  - Request: `{"group_id": "uuid", "name": "Sunday Night", "days_of_week": [6], "start_time": "02:00", "end_time": "04:00", "timezone": "Europe/Berlin", "catch_up": true}`
  - (`days_of_week`: `0 = Monday .. 6 = Sunday`)
  - Response: `{"id": "uuid", "group_id": "uuid", "group_name": "Servers", "name": "Sunday Night", "days_of_week": [6], "start_time": "02:00", "end_time": "04:00", "timezone": "Europe/Berlin", "catch_up": true, "created_at": "..."}`
- `DELETE /api/v1/admin/maintenance-windows/{id}` -> `{"status": "deleted", "id": "..."}`

### 3.5 OS Patching
- `GET /api/v1/admin/patches` -> `list[PatchSummary]`
  ```json
  {
    "update_id": "KB5044285",
    "title": "2024-10 Cumulative Security Update",
    "os_family": "windows",
    "severity": "critical",
    "category": "security",
    "size_bytes": 524288000,
    "affected_device_count": 3,
    "affected_device_ids": ["dev-1", "dev-2", "dev-3"],
    "approved": false,
    "excluded": false,
    "last_seen_at": "2026-11-20T20:05:00Z"
  }
  ```
- `POST /api/v1/admin/patches/approve`
  - Request: `{"update_ids": ["KB5044285"], "group_ids": [], "respect_maintenance_window": true}`
  - Response: `{"approved_update_ids": ["KB5044285"], "jobs_created": 3, "job_ids": ["..."]}`
- `POST /api/v1/admin/patches/exclude`
  - Request: `{"update_ids": ["KB5034441"], "excluded": true}`
  - Response: `{"update_ids": ["KB5034441"], "excluded": true}`

### 3.6 Software Catalog & Chunked Package Repository
- `GET /api/v1/admin/catalog` -> `list[CatalogItem]`
- `POST /api/v1/admin/catalog`
  - Request:
    ```json
    {
      "name": "7-Zip",
      "version": "24.08",
      "vendor": "Igor Pavlov",
      "description": "File archiver",
      "category": "Utilities",
      "icon_url": "",
      "os_family": "windows",
      "source_type": "winget",
      "package_ref": "7zip.7zip",
      "install_args": "",
      "uninstall_args": "",
      "blocked_processes": ["7zFM.exe"],
      "allow_uninstall": true,
      "return_codes": {"0": "success", "3010": "reboot_required"},
      "detection_rule": {"type": "file", "path": "C:\\Program Files\\7-Zip\\7z.exe", "version": "24.08"}
    }
    ```
  - Response: `CatalogItem` (includes `id`, `filename`, `file_size`, `sha256`, `created_at`, `updated_at`)
- `PATCH /api/v1/admin/catalog/{id}` -> `CatalogItem`
- `DELETE /api/v1/admin/catalog/{id}` -> `{"status": "deleted", "id": "..."}`
- `POST /api/v1/admin/catalog/{id}/upload` -> Multipart form (`file`) -> returns updated `CatalogItem` with `sha256` & `file_size`.
- `POST /api/v1/admin/catalog/{id}/upload-chunk` -> Multipart form (`chunk_index`: int `0..total_chunks-1`, `total_chunks`: int, `file`: UploadFile, optional `filename`: str)
  - Response:
    ```json
    {
      "package_id": "uuid",
      "chunk_index": 0,
      "total_chunks": 1,
      "completed": true,
      "sha256": "e3b0c442...",
      "file_size": 1048576,
      "catalog_item": { /* CatalogItem when completed == true */ }
    }
    ```

### 3.7 Deployments, Jobs & Self-Update
- `GET /api/v1/admin/deployments` -> `list[Deployment]`
- `POST /api/v1/admin/deployments`
  - Request:
    ```json
    {
      "package_id": "uuid",
      "target_group_ids": ["grp-uuid"],
      "target_device_ids": [],
      "mode": "required" | "available" | "uninstall",
      "respect_maintenance_window": true
    }
    ```
  - Response: `Deployment` (includes `id`, `package_id`, `package_name`, `package_version`, `os_family`, `target_group_ids`, `target_device_ids`, `mode`, `respect_maintenance_window`, `stats`: `{"total", "installed", "pending", "failed"}`, `created_at`)
- `DELETE /api/v1/admin/deployments/{id}` -> `{"status": "deleted", "id": "..."}`
- `GET /api/v1/admin/jobs` -> `list[Job]`
- `GET /api/v1/admin/jobs/{id}` -> `Job` (`id`, `device_id`, `device_hostname`, `job_type`, `action`, `target_id`, `version`, `status`, `exit_code`, `reboot_required`, `stdout`, `stderr`, `respect_maintenance_window`, `created_at`, `started_at`, `completed_at`)
- `POST /api/v1/admin/self-update`
  - Request: `{"version": "1.1.0", "os_family": "windows", "sha256": "...", "download_url": "/dl/agents/windows/swm-agent.exe", "target_group_ids": [], "target_device_ids": [], "respect_maintenance_window": true}`
  - Response: `{"jobs_created": 2, "job_ids": ["..."]}`

---

## 4. Agent API (`/api/v1/agent/...`)

All endpoints except `/api/v1/agent/enroll` and `WS /api/v1/agent/ws` require header:
`Authorization: Bearer <device_token>`.

- `POST /api/v1/agent/enroll`
  - Request:
    ```json
    {
      "enrollment_token": "swm_tok_...",
      "hostname": "ws-pc01",
      "os_family": "windows",
      "os_name": "Windows 11 Pro",
      "os_version": "23H2",
      "arch": "amd64",
      "agent_version": "1.0.0",
      "machine_uid": "machine-uuid-123"
    }
    ```
  - Response (`200`):
    ```json
    {
      "device_id": "uuid",
      "device_token": "swm_dev_...",
      "status": "pending",
      "server_public_key_b64": "base64..."
    }
    ```
- `GET /api/v1/agent/status`
  - Returns `403` if `revoked`, `200` if `pending` or `approved`:
    ```json
    {
      "device_id": "uuid",
      "status": "approved",
      "reboot_policy": "never",
      "grace_period_minutes": 240,
      "in_maintenance_window": true,
      "excluded_updates": ["KB5034441"],
      "server_public_key_b64": "base64..."
    }
    ```
- `POST /api/v1/agent/inventory` (requires `approved`, else `403`)
  - Request:
    ```json
    {
      "hostname": "ws-pc01",
      "domain": "WORKGROUP",
      "os_family": "windows",
      "os_name": "Windows 11 Pro",
      "os_version": "23H2",
      "kernel_or_build": "22631.4317",
      "arch": "amd64",
      "agent_version": "1.0.0",
      "hardware": {
        "cpu_model": "AMD Ryzen 7",
        "cpu_cores": 8,
        "ram_total_mb": 16384,
        "ram_free_mb": 8192,
        "disks": [{"mount": "C:\\", "total_gb": 512.0, "free_gb": 210.5}]
      },
      "ip_addresses": ["192.168.1.50"],
      "logged_in_users": ["jdoe"],
      "reboot_required": false,
      "software_hash": null,
      "installed_software": [
        {
          "name": "7-Zip",
          "version": "24.07",
          "publisher": "Igor Pavlov",
          "source": "registry",
          "scope": "machine",
          "package_ref": "7zip.7zip"
        }
      ],
      "pending_os_updates": [
        {
          "update_id": "KB5044285",
          "title": "2024-10 Cumulative Security Update",
          "severity": "critical",
          "category": "security",
          "size_bytes": 524288000,
          "reboot_behavior": "may_require_reboot"
        }
      ]
    }
    ```
  - Response (`200`): `{"status": "ok", "enqueued_jobs": 0, "software_updated": true}`
- `GET /api/v1/agent/jobs/pending` (requires `approved`, else `403`)
  - Response (`200`):
    ```json
    {
      "jobs": [
        {
          "job_id": "uuid",
          "device_id": "uuid",
          "job_type": "software",
          "action": "install",
          "target_id": "pkg-uuid",
          "version": "24.08",
          "sha256": "e3b0c442...",
          "command": "/qn /norestart",
          "expires_at_unix": 1763750000,
          "signature_b64": "base64...",
          "download_url": "/api/v1/agent/packages/pkg-uuid/download",
          "update_ids": [],
          "security_only": false,
          "package_meta": {
            "package_id": "pkg-uuid",
            "name": "7-Zip",
            "vendor": "Igor Pavlov",
            "os_family": "windows",
            "source_type": "custom",
            "package_ref": "7z2408-x64.msi",
            "filename": "7z2408-x64.msi",
            "file_size": 1843200,
            "install_args": "/qn /norestart",
            "uninstall_args": "/x {GUID} /qn /norestart"
          },
          "detection_rule": {"type": "file", "path": "C:\\Program Files\\7-Zip\\7z.exe", "version": "24.08"},
          "blocked_processes": ["7zFM.exe"],
          "return_codes": {"0": "success", "3010": "reboot_required"}
        }
      ],
      "available_catalog": [
        {
          "package_id": "pkg-uuid",
          "device_id": "uuid",
          "name": "Mozilla Firefox",
          "vendor": "Mozilla",
          "description": "Web browser",
          "category": "Browser",
          "icon_url": "",
          "os_family": "windows",
          "source_type": "winget",
          "package_ref": "Mozilla.Firefox",
          "version": "132.0",
          "sha256": "",
          "file_size": 0,
          "download_url": null,
          "install_cmd": "",
          "uninstall_cmd": "",
          "allow_uninstall": true,
          "installed_version": null,
          "status": "available",
          "blocked_processes": ["firefox.exe"],
          "detection_rule": {},
          "return_codes": {"0": "success", "3010": "reboot_required"},
          "signature_b64": "base64..."
        }
      ]
    }
    ```
- `POST /api/v1/agent/jobs/{job_id}/result`
  - Request: `{"status": "succeeded" | "failed" | "running" | "waiting_for_process", "exit_code": 0, "reboot_required": false, "stdout": "...", "stderr": "..."}`
  - Response (`200`): `{"status": "ok", "job_id": "uuid", "job_status": "succeeded"}`
- `POST /api/v1/agent/self-service/trigger`
  - Request: `{"package_id": "pkg-uuid", "action": "install" | "update" | "uninstall"}`
  - Response (`200`): `SignedJob` object (same JSON structure as an element of `jobs` in `/api/v1/agent/jobs/pending`). Returns `403` if package is not assigned to this device or if `action == "uninstall"` when `allow_uninstall == false`.
- `GET /api/v1/agent/packages/{package_id}/download`
  - Supports `Range: bytes=<start>-<end>` and `Range: bytes=<start>-`.
  - Headers: `Accept-Ranges: bytes`, `X-Package-SHA256: <sha256>`, `Content-Range: bytes <start>-<end>/<total>` (with `206 Partial Content` when `Range` header is sent, `200 OK` otherwise).
- `WS /api/v1/agent/ws?token=<device_token>`
  - Agent -> Server frames:
    - `{"type": "heartbeat", "agent_version": "1.0.0", "reboot_required": false}`
    - `{"type": "log_chunk", "job_id": "uuid", "stream": "stdout" | "stderr", "chunk": "..."}`
  - Server -> Agent frames:
    - `{"type": "heartbeat_ack", "server_time": "2026-11-20T20:05:00Z"}`
    - `{"type": "job_available", "job_id": "uuid"}`
    - `{"type": "catalog_updated"}`
    - `{"type": "revoked"}` (immediately followed by server closing WebSocket with code `1008`)
