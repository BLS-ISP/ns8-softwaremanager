# Changelog

Alle relevanten Änderungen an **NS8-SoftwareManager** werden in dieser Datei dokumentiert.
Das Format orientiert sich an [Keep a Changelog](https://keepachangelog.com/de/1.1.0/) und [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.6] - 2026-11-23

### Fixed
- **Reiner NethServer-8-Cockpit-Stil ohne CSS-Überlagerung (`ui/`):**
  - Doppelten Import von `carbon-components.min.css`, den fehlerhaften `top: 3rem`-Abstand der Seitenleiste im Iframe sowie das `:active`-Light-Theme-Binding auf `<cv-side-nav-link>` entfernt, sodass `/cluster-admin/css/core.css` unverfälscht greift (kein weißer Balken über dem Instanznamen und kein weiß-auf-weißer aktiver Menüpunkt mehr).
  - `Status.vue`, `Settings.vue` und `About.vue` vollständig auf native NS8-Standardkomponenten (`cv-grid`, `NsInfoCard`, `NsTag`, `NsInlineNotification`, `cv-structured-list`, `NsEmptyState`) analog zu den übrigen BLS-ISP-NS8-Modulen umgestellt.

## [1.0.5] - 2026-11-23

### Added
- **Geführter Client-Rollout & Massenbereitstellungs-Assistent (`web/`, `server/`):**
  - Überarbeitete *Client-Rollout & Agent-Installation*-Ansicht mit Architektur-Erklärung der beiden Client-Komponenten (`swm-agent` Systemdienst + `swm-center` Endbenutzer-„NS8 Software Center“ ohne lokale Admin-Rechte), 1-Klick-Schnellstart-Token sowie dedizierten Tabs für **Einzelplatz-Einzeiler**, **Active Directory GPO / Intune (`install-swm-gpo.ps1`)**, **Ansible (`deploy-swm-agent.yml`)** und **Gold-Image-CLI**.
  - Neuer Endpunkt `GET /api/v1/admin/agent-artifacts` liefert Live-Dateigrößen und `SHA-256`-Prüfsummen aller Client-Binaries für Direkt-Downloads und die automatische Übernahme beim Agent-Self-Update.
  - Idempotenter Rollout-Parameter (`skip_if_enrolled=1`) sowie automatische Erstellung von Windows-Startmenü- (`NS8 Software Center.lnk`) und Linux-Desktop-Verknüpfungen (`/usr/share/applications/ns8-swm-center.desktop`) in `/dl/install.ps1` und `/dl/install.sh`.
  - First-Run-Onboarding-Leitfaden direkt auf dem Dashboard, solange noch keine Endpunkte angebunden sind.

### Fixed
- **NS8 Cluster-Admin Cockpit-UI (`ui/`):**
  - Behoben: Leerer Inhaltsbereich und abgeschnittener Titel in der Karte *Endpunkt & Portal-Routing* auf `Status.vue` (verursacht durch nicht-reaktives `$slots.content` in Vue 2 `NsInfoCard` bei asynchron geladener Konfiguration).
  - Behoben: Zweispaltiges Layout und Größen-/Zeitstempel-Formatierung für *Container-Images* (String- & Byte-Werte aus Podman) und *Persistente Volumes* sowie direkter Client-Onboarding-Leitfaden mit Server-URL und Binary-Links auf der NS8-Statusseite.

## [1.0.4] - 2026-11-23

### Fixed
- **NS8 `agent.ldapproxy` Integration (`imageroot/`):** Replaced non-existent `LdapDiscovery` imports with NS8's official `from agent.ldapproxy import Ldapproxy` across `get-configuration`, `configure-module`, `restore-module`, and `clone-module`, properly resolving string domain lists from `Ldapproxy().get_domains_list()` via `get_domain(name)` and mapping local `127.0.0.1` `ldapproxy` endpoints to `10.0.2.2` with plaintext LDAP (`LDAP_TLS="false"`).
- **NS8 OpenLDAP (`rfc2307`) Login & Service Lifecycle (`server/`, `imageroot/`):** Added anonymous subtree DN lookup for OpenLDAP user domains without a dedicated bind DN, prevented redundant double-restarts on initial `configure-module`, guarded Restic `module-dump-state` when `swm-db.service` is unconfigured, and staged prebuilt Linux binaries into `Containerfile.app` in CI.

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
