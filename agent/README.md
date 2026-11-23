# NS8-SoftwareManager — Endpoint Agent & Software Center (`agent/`)

This Rust workspace builds the two client-side binaries deployed to managed **Windows**, **Linux**, and **macOS Universal (`arm64` + `x86_64`)** endpoints:

1. **`swm-agent` (`crates/swm-agent`):** Privileged background daemon (`NT AUTHORITY\SYSTEM` on Windows, `root` via `systemd` on Linux, `root` via `launchd` on macOS) responsible for inventory, OS patching, software installation, `Ed25519` job verification, and the user-session watchdog.
2. **`swm-center` (`crates/swm-center`):** Unprivileged single-binary desktop & System Tray client (**NS8 Software Center**) with its compiled Vue 3 SPA (`crates/swm-center/ui/dist`) embedded directly into the executable at compile time.
3. **`swm-common` (`crates/swm-common`):** Shared wire DTOs, `Ed25519` (`ed25519-dalek`) + `SHA-256` verification, SemVer comparison, and the typed local IPC protocol.

---

## Critical Build Order (Embedded UI)

`swm-center/build.rs` embeds every file inside `crates/swm-center/ui/dist/` at compile time. Always build the Vue 3 UI **before** invoking `cargo test` or `cargo build`:

```bash
npm --prefix agent/crates/swm-center/ui install
npm --prefix agent/crates/swm-center/ui test -- --run
npm --prefix agent/crates/swm-center/ui run build

cargo test --manifest-path agent/Cargo.toml --workspace
cargo build --release --manifest-path agent/Cargo.toml --workspace
```

### Building macOS Universal Binaries (`arm64` + `x86_64`)
Because the workspace uses pure-Rust TLS (`rustls` — zero C OpenSSL dependencies), any macOS host with Xcode Command Line Tools can produce Universal Binaries via `lipo`:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --manifest-path agent/Cargo.toml --workspace --target aarch64-apple-darwin
cargo build --release --manifest-path agent/Cargo.toml --workspace --target x86_64-apple-darwin

lipo -create \
  agent/target/aarch64-apple-darwin/release/swm-agent \
  agent/target/x86_64-apple-darwin/release/swm-agent \
  -output swm-agent
lipo -archs swm-agent   # -> x86_64 arm64
```

---

## Platform Matrix

| Aspect | Windows (`x86_64`) | Linux (`x86_64`) | macOS Universal (`arm64` + `x86_64`) |
|---|---|---|---|
| **Agent Service** | Windows Service `NS8SoftwareManagerAgent` (`sc.exe failure` crash recovery) | `/etc/systemd/system/swm-agent.service` (`Restart=always`) | `/Library/LaunchDaemons/org.nethserver.swm-agent.plist` (`KeepAlive=true`) |
| **Default `--data-dir`** | `C:\ProgramData\NS8-SoftwareManager` | `/etc/ns8-swm` | `/Library/Application Support/NS8-SoftwareManager` |
| **Local IPC Endpoint** | `\\.\pipe\ns8-swm-ipc` | `/run/ns8-swm/ipc.sock` | `/var/run/ns8-swm/ipc.sock` |
| **Center Session Guard** | `%LOCALAPPDATA%\NS8-SoftwareManager\center-session.json` | `${XDG_RUNTIME_DIR:-/tmp}/ns8-swm-center-<uid>.json` | `~/Library/Application Support/NS8-SoftwareManager/center-session.json` |
| **Center Watchdog** | ScheduledTask `\NS8SoftwareCenterGuardian` + `swm-agent` respawn | `/etc/systemd/user/ns8-swm-center.service` (`Restart=always`) | `/Library/LaunchAgents/org.nethserver.swm-center.plist` (`Aqua`, `KeepAlive=true`) + `launchctl kickstart` |
| **OS Patch Engine** | Native Windows Update Agent (`Microsoft.Update.Session`) | `apt` (`noninteractive`) & `dnf` | Apple `/usr/sbin/softwareupdate -l` / `-i` |
| **Package Sources** | `winget --scope machine`, `.msi`, `.exe`, `.zip`, `.ps1` | `apt`, `dnf`, `flatpak`, `.deb`, `.rpm`, `.sh` | `brew`, `brew_cask` (non-root via `sudo -u <owner> -H`), `.pkg`, `.dmg`, `.app.zip`, `.sh` |
| **Desktop Notifications** | WinRT `ToastGeneric` | `notify-send` | `/usr/bin/osascript` Notification Center |

---

## CLI Reference

### `swm-agent`
```bash
# Enroll endpoint against NS8 server:
swm-agent enroll --server https://swm.example.com --token <TOKEN> [--data-dir <DIR>] [--wait-approval]

# Run foreground daemon (or a single deterministic cycle with --once):
swm-agent run [--once] [--poll-interval-secs 30] [--data-dir <DIR>]

# Install or remove OS service (Windows SCM / systemd / launchd):
swm-agent service-install [--data-dir <DIR>]
swm-agent service-uninstall
```

Environment overrides:
- `SWM_DATA_DIR` — Override persistent config/state directory
- `SWM_IPC_ADDR` — Override local IPC socket/pipe (or `127.0.0.1:<port>` in integration tests)
- `SWM_DISABLE_WATCHDOG=1` — Disable `swm-center` respawn checks during automated tests

### `swm-center`
```bash
# Start local UI bridge + System Tray (or hand off to existing user-session instance):
swm-center gui [--tray-only] [--port 0] [--no-open]

# CLI IPC helpers (no admin privileges required):
swm-center status
swm-center catalog
swm-center trigger --package-id <UUID> --action install|update|uninstall
swm-center trigger-updates
swm-center snooze --minutes 60
swm-center confirm-reboot
```
