# Changelog

Alle relevanten Änderungen an **NS8-SoftwareManager** werden in dieser Datei dokumentiert.
Das Format orientiert sich an [Keep a Changelog](https://keepachangelog.com/de/1.1.0/) und [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.3] - 2026-11-23

### Fixed
- **NS8 Cockpit Settings & Subpage Navigation (`ui/`):** Replaced invalid parent router pushes (`/apps/<instance>/settings`) with `@nethserver/ns8-ui-lib`'s official `goToAppPage(instanceName, page)` (`?page=settings`), wired `changeRoute` and initial `?page=` query-parameter handling in `App.vue`, added `initUrlBindingForApp` lifecycle hooks across `Status.vue`, `Settings.vue`, and `About.vue`, and normalized `list-backup-repositories` output parsing.

## [1.0.2] - 2026-11-23

### Fixed
- **NS8 `extract-ui` / `add-module` (`Containerfile`):** Added `ENTRYPOINT ["/"]` to the `FROM scratch` module stage so `podman create` inside NS8 `extract-ui` and `extract-image` succeeds instead of exiting with code `125`.
- **Rootless Podman Host-Loopback & LDAP Proxy (`imageroot/`):** Enabled `--network=slirp4netns:allow_host_loopback=true` on `swm.service`, loaded `agent.env` across systemd units, added `TCP_PORTS` fallback resolution, and mapped local `127.0.0.1` `LdapDiscovery` endpoints to `10.0.2.2` inside the pod.

## [1.0.0] - 2026-11-21

### Added
- **Zentrales FastAPI Management-Backend (`server/`):**
  - Asynchrone Datenbank-Anbindung (`PostgreSQL 16` & `SQLite`) mit exponentiellem Startup-Backoff und aktivem Lifespan-Hintergrund-Scheduler für Wartungsfenster und Online-Status-Bereinigung.
  - JWT-Admin-Authentifizierung mit lokalem Fallback-Administrator sowie direkter NS8-Benutzerdomänen-Anbindung (`Samba Active Directory` / `OpenLDAP`).
  - Persistente `Ed25519`-Auftrags- und Katalogsignierung sowie `SHA-256`-Integritätsprüfung für alle Pakete.
  - Eigenes Software-Paket-Repository mit Chunked-Upload, unterbrechungssicherem `HTTP 206 Range`-Download, *Detection Rules* und Prozess-Sperrprüfung (`blocked_processes`).
  - Ausgehender Echtzeit-WebSocket-Hub (`WSS`) mit Live-Log-Streaming (`stdout`/`stderr`) und Enrollment-Bootstrap-Generator (`/dl/install.ps1` & `/dl/install.sh`).
- **Moderne Admin-Web-Konsole (`web/`):**
  - Deutschsprachige Vue-3-Oberfläche mit Light- & Dark-Mode, Flotten- & Patch-Compliance-Dashboard, Geräteverwaltung (*Pending Approval*, Hardware-/Software-Inventar, *Pending Reboot*), OS-Patch-Steuerung, Software-Katalog, Bereitstellungen (*Pflicht*, *Verfügbar*, *Deinstallieren*), Wartungsfenstern und Live-Job-Logs.
- **Cross-Platform Rust-Agent (`swm-agent`):**
  - Hintergrunddienst für Windows (`NT AUTHORITY\SYSTEM` Windows Service) und Linux (`root` via `systemd`).
  - Native Windows-Update-Steuerung (WUA ohne WSUS-Zwang), Linux-Patching (`apt` & `dnf`), Session-0-sicherer `winget --scope machine`-Runner, Custom-Installer-Runner (`.msi`, `.exe`, `.ps1`, `.deb`, `.rpm`, `.sh`, `flatpak`) sowie signiertes Agent-Self-Update.
- **Single-Binary Endbenutzer „Software Center“ (`swm-center`):**
  - Vollständig ins Binary eingebettete Vue-3-Oberfläche (Light/Dark-Mode) zur Installation freigegebener Software ohne lokale Administratorrechte über abgesichertes lokales IPC (Windows Named Pipe / Linux Unix Socket).
  - Natives Windows-System-Tray-Icon (`Shell_NotifyIconW`), native Desktop-Benachrichtigungen (WinRT Toast / `notify-send`), Desktop-Autostart und interaktiver Neustart-Karenzzeit-Dialog mit Snooze-Countdown.
- **NethServer 8 Rootless Modul & CI/CD (`imageroot/`, `ui/`, `.github/workflows/`):**
  - Vollständiges Rootless-Podman-Modul hinter NS8-Traefik, Cluster-Admin Cockpit-UI (`de`/`en`), Restic-Backup/Restore/Clone sowie automatisierte GitHub-Actions-Pipeline für Tests, `.deb`/`.rpm`/`.exe`-Artefakte und GHCR-Image-Veröffentlichung (`ghcr.io/bls-isp/softwaremanager`).
