# ADR-003: Two-Tier Desktop Client Hardening and macOS Universal Binaries (`lipo`)

## Status
Accepted

## Date
2026-11-23 (`v1.1.0` & `v1.2.0`)

## Context
In managed enterprise fleets, two operational challenges arise with desktop companion apps (`swm-center`) and mixed OS architectures:
1. **User termination & duplicate tray instances:** Standard users may accidentally close the System Tray app (missing critical reboot deadlines or IT notifications), kill it in Task Manager / Activity Monitor, or launch it repeatedly from the Start Menu / `/Applications`, creating duplicate tray icons.
2. **macOS architecture & root-daemon constraints:** Modern Mac fleets mix Apple Silicon (`arm64` M1–M4) and Intel (`x86_64`) hardware, `/run` does not exist on macOS, and **Homebrew explicitly refuses to execute as `root`**, even though `swm-agent` runs as a `root` `LaunchDaemon`.

## Decision
1. **Hardened Tray + Single-Instance Guard + Two-Tier Session Watchdog:**
   - When a device's group policy sets `enforce_tray = true` (default), `swm-center` omits the *Quit* tray menu item and ignores `QuitTray` events.
   - `swm-center` writes a per-user session file (`center-session.json`) and exposes `GET /api/ping` and `POST /api/open-window` on loopback; any second `swm-center gui` invocation focuses the running window and exits `0`.
   - OS session managers (`\NS8SoftwareCenterGuardian` ScheduledTask on Windows, `/etc/systemd/user/ns8-swm-center.service` with `Restart=always` on Linux, and `/Library/LaunchAgents/org.nethserver.swm-center.plist` with `KeepAlive=true` on macOS) are paired with an active `swm-agent` watchdog that respawns `swm-center` if `enforce_tray` is active and a desktop user is logged in.
2. **Single `lipo` Universal Binary (`arm64` + `x86_64`) for macOS:**
   - Because `swm-agent` and `swm-center` rely on pure-Rust `rustls` (zero C OpenSSL linkage), GitHub Actions (`macos-latest`) compiles both `aarch64-apple-darwin` and `x86_64-apple-darwin` and merges them via `lipo -create` into Universal Binaries served at `/dl/agents/macos-universal/`.
3. **Non-Root Homebrew Execution from `LaunchDaemon`:**
   - For `brew` and `brew_cask` jobs/inventory, `swm-agent` locates `/opt/homebrew/bin/brew` or `/usr/local/bin/brew`, inspects its non-root owner (`stat -f %Su`, falling back to `/dev/console` owner, strictly refusing `root`), and invokes `sudo -u <owner> -H env NONINTERACTIVE=1 HOMEBREW_NO_AUTO_UPDATE=1`.

## Alternatives Considered

### Separate `macos-arm64` and `macos-amd64` Rollouts Without `lipo`
- **Rejected:** Forces admins to track two separate `SHA-256` hashes during centralized Agent-Self-Updates across mixed Intel + Apple Silicon fleets. A Universal Binary allows one artifact and one `SHA-256` for all Macs.

### Running `brew` Directly as `root`
- **Rejected:** Fails immediately on macOS (`Running Homebrew as root is extremely dangerous and no longer supported`) and corrupts `/opt/homebrew` permissions.

## Consequences
- `swm-center` stays reliably active across Windows, Linux, and macOS user sessions without spawning duplicate instances, and reports live `center_running` telemetry to administrators.
- Mixed Apple Silicon and Intel Mac fleets share a single bootstrapper, MDM script (`install-swm-mdm-macos.sh`), and self-update `SHA-256` digest.
