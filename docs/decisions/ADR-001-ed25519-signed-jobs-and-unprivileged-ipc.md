# ADR-001: Cryptographically Signed Jobs (`Ed25519`) and Strict Unprivileged Local IPC

## Status
Accepted

## Date
2026-11-21 (extended in `v1.1.0` & `v1.2.0`)

## Context
An endpoint management agent (`swm-agent`) necessarily executes with highest operating-system privileges (`NT AUTHORITY\SYSTEM` on Windows, `root` on Linux and macOS) to install OS patches and machine-wide software. At the same time, standard desktop users need a self-service **Software Center** (`swm-center`) in their graphical session to install approved applications without holding local administrator rights.

Two critical threat vectors had to be eliminated by design:
1. **Local privilege escalation:** A local user or compromised desktop process attempting to send arbitrary shell commands, file paths, or attacker-controlled URLs to the privileged `swm-agent` service over local IPC.
2. **Transport / database tampering:** Unverified job rows or man-in-the-middle payloads tricking agents into executing unsigned commands or modified binaries.

## Decision
1. **Canonical `Ed25519` Server Signatures on Every Job & Catalog Manifest:**
   - The server generates a persistent `Ed25519` keypair (`{DATA_DIR}/keys/private.key` & `public.key`) backed up by NS8 Restic.
   - Every `SignedJob` (`os_patch`, `software`, `self_update`, `scan_updates`, and `remote_control`) and every `SignedCatalogItem` is signed over a deterministic UTF-8 canonical string including `device_id`, `sha256`, and `expires_at_unix`.
   - `swm-agent` verifies the `Ed25519` signature against its pinned `server_public_key_b64` and rejects expired or tampered payloads before touching the OS.
2. **Strictly Typed Local IPC (`swm-common`) with Server-Side Policy Gates:**
   - `swm-center` runs strictly as the unprivileged desktop user and communicates with `swm-agent` over a local Named Pipe (`\\.\pipe\ns8-swm-ipc` with `PIPE_REJECT_REMOTE_CLIENTS`) or Unix Domain Socket (`/run/ns8-swm/ipc.sock` on Linux, `/var/run/ns8-swm/ipc.sock` on macOS).
   - `IpcRequest` is a closed tagged enum (`ListCatalog`, `GetStatus`, `TriggerPackage`, `TriggerPendingUpdates`, `SyncNow`, `DismissMessage`, `SnoozeReboot`, `ConfirmRebootNow`). Callers can pass only a `package_id` UUID, which `swm-agent` cross-checks against its `Ed25519`-verified catalog and group `DesktopPolicy` (`allow_self_service`, `allow_user_os_updates`).

## Alternatives Considered

### Running `swm-center` Elevated or Using `sudo` / Windows Task Elevation Rules
- **Rejected:** Granting desktop UI processes elevated privileges expands the attack surface to the entire WebView/browser rendering stack and breaks standard-user compliance.

### Relying Solely on HTTPS TLS Without Job-Level Signatures
- **Rejected:** TLS protects transport behind Traefik, but job-level `Ed25519` signatures + `SHA-256` digests enforce end-to-end integrity across reverse proxies, caching layers, and local IPC self-service triggers.

## Consequences
- Standard users can safely install, update, or uninstall approved software without UAC or `sudo` prompts.
- Group restrictions (`allow_self_service = false` Managed Kiosk Mode) cannot be bypassed by crafting raw local IPC calls because `swm-agent` enforces `PermissionDenied` directly in the privileged daemon.
