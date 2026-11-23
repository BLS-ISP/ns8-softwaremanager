# Aufgabenliste (Todo): NS8-SoftwareManager

Diese Datei ist das zentrale Task-Tracking-Dokument gemäß `tasks/plan.md`. Jede Aufgabe ist vertikal geschnitten, in einer fokussierten Sitzung umsetzbar und besitzt überprüfbare Akzeptanz- und Verifikationskriterien.

---

## Phase 1: Fundament, Authentifizierung & Enrollment-Basis

## Task 1: Monorepo-Grundgerüst, LF-Regeln & FastAPI-Datenbank-Kern

**Description:** Legt die Verzeichnisstruktur des Monorepos, `.gitattributes` (strikte LF-Zeilenenden für NS8-Skripte) sowie das Grundgerüst des Python-FastAPI-Servers inklusive asynchroner Datenbank-Anbindung mit Verbindungs-Retry/Backoff, Alembic-Migrationen und `/api/v1/health`-Endpunkt an.

**Acceptance criteria:**
- [x] `.gitattributes` erzwingt `eol=lf` für `imageroot/**`, `*.sh` und `Containerfile*`.
- [x] FastAPI-App startet mit konfigurierbarer `DATABASE_URL` (PostgreSQL im Container, SQLite-Async für schnelle lokale Unit-Tests) inklusive exponentiellem Verbindungs-Backoff beim Start.
- [x] `GET /api/v1/health` liefert HTTP `200` mit DB-Verbindungsstatus und App-Version.

**Verification:**
- [x] Tests pass: `python -m pytest server/tests/test_health.py`
- [x] Build succeeds: `python -m compileall server/app`
- [x] Manual check: Lokaler Start von `uvicorn app.main:app` beantwortet `/api/v1/health` erfolgreich.

**Dependencies:** None

**Files likely touched:**
- `.gitattributes`
- `.gitignore`
- `server/pyproject.toml`
- `server/app/main.py`
- `server/tests/test_health.py`

**Estimated scope:** Medium: 3-5 files

---

## Task 2: Admin-Authentifizierung (Lokaler Admin + NS8 LDAP/AD) & Admin-Web-Shell

**Description:** Implementiert die JWT-basierte Admin-Authentifizierung im Backend (lokaler Fallback-Admin sowie konfigurierbarer LDAP/Active-Directory-Provider für NS8 User-Domains) und erstellt das Vue-3-Admin-Portal (`web/`) mit Login-Maske, geschütztem Router und Basis-Navigation.

**Acceptance criteria:**
- [x] `POST /api/v1/admin/auth/login` authentifiziert sowohl gegen den lokalen Admin-Account als auch (sofern konfiguriert) gegen LDAP/AD und gibt ein signiertes Access-Token zurück.
- [x] Unautorisierte Aufrufe auf `/api/v1/admin/*` werden konsequent mit HTTP `401` abgewiesen.
- [x] Das Vue-3-Frontend (`web/`) bietet Login, Session-Handling, Abmelden und das Basis-Navigationslayout.

**Verification:**
- [x] Tests pass: `python -m pytest server/tests/test_admin_auth.py`
- [x] Build succeeds: `npm --prefix web run build`
- [x] Manual check: Anmeldung im Browser leitet auf das leere Admin-Dashboard weiter; geschützte Routen leiten ohne Token auf `/login`.

**Dependencies:** Task 1

**Files likely touched:**
- `server/app/auth/admin_auth.py`
- `server/app/api/admin_auth.py`
- `server/tests/test_admin_auth.py`
- `web/package.json`
- `web/src/App.vue`

**Estimated scope:** Medium: 3-5 files

---

## Task 3: Ed25519-Signaturkern, Enrollment-Tokens & Bootstrap-Skript-Endpunkte

**Description:** Erzeugt und verwaltet das serverseitige `Ed25519`-Schlüsselpaar zur Signierung aller künftigen Agent-Aufträge, implementiert Enrollment-Tokens (zeitlich begrenzt / nutzungsbegrenzt / Gruppen-Vorbelegung) und stellt fertige Einzeiler-Installationsskripte (`/dl/install.ps1` und `/dl/install.sh`) samt Admin-UI bereit.

**Acceptance criteria:**
- [x] Server generiert beim Erststart automatisch ein persistentes `Ed25519`-Schlüsselpaar und liefert den Public Key für Agenten bereit.
- [x] Admins können über API und Admin-UI Enrollment-Tokens anlegen, widerrufen und sich den fertigen Windows-PowerShell- bzw. Linux-Bash-Einzeiler kopieren.
- [x] Abgelaufene oder widerrufene Enrollment-Tokens werden bei Registrierungsversuchen abgelehnt.

**Verification:**
- [x] Tests pass: `python -m pytest server/tests/test_crypto_and_enrollment.py`
- [x] Build succeeds: `npm --prefix web run build`
- [x] Manual check: Erstellen eines Tokens in der Web-UI zeigt gültige `install.ps1`- und `install.sh`-Befehle mit korrekter Server-URL.

**Dependencies:** Task 2

**Files likely touched:**
- `server/app/crypto/signing.py`
- `server/app/models/enrollment.py`
- `server/app/api/enrollment.py`
- `server/tests/test_crypto_and_enrollment.py`
- `web/src/views/EnrollmentView.vue`

**Estimated scope:** Medium: 3-5 files

---

## Checkpoint: Nach Phase 1 (Tasks 1–3)
- [x] Alle Backend-Tests (`pytest`) laufen fehlerfrei durch
- [x] Admin-Web-UI (`web/`) baut ohne TypeScript-/Vite-Fehler
- [x] Admin-Login, `Ed25519`-Key-Initialisierung und Enrollment-Token-Generierung funktionieren Ende-zu-Ende
- [x] Kurzer Review-Abgleich vor Start der Rust-Agent-Implementierung

---

## Phase 2: Cross-Platform Core-Agent & Flotten-Inventarisierung

## Task 4: Rust-Workspace (`swm-common`, `swm-agent`), Service-Wrapper & Enrollment-Handshake

**Description:** Initialisiert den Rust-Cargo-Workspace (`agent/`), definiert die gemeinsamen Protokoll-Typen und die `Ed25519`-Signaturprüfung in `swm-common` und implementiert in `swm-agent` den Dienst-Lebenszyklus (Windows Service / Linux `systemd` Daemon + CLI-Modus) inklusive sicherem Enrollment-Handshake gegen den Server.

**Acceptance criteria:**
- [x] `swm-common` verifiziert `Ed25519`-Signaturen und `SHA-256`-Prüfsummen deterministisch (inkl. Cross-Language-Testvektor zum Python-Backend).
- [x] `swm-agent enroll --server <url> --token <token>` registriert den Client, speichert das gerätespezifische Zugangs-Token sowie den Server-Public-Key geschützt im Systemverzeichnis (`ProgramData` bzw. `/etc/ns8-swm`).
- [x] Ist serverseitig *Pending Approval* aktiv, wartet der Agent sauber im Status `pending`, bis ein Admin das Gerät freigibt.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml --workspace`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml --bin swm-agent`
- [x] Manual check: Lokaler Agent-Lauf registriert sich erfolgreich gegen das lokale FastAPI-Backend.

**Dependencies:** Task 3

**Files likely touched:**
- `agent/Cargo.toml`
- `agent/crates/swm-common/src/lib.rs`
- `agent/crates/swm-agent/src/main.rs`
- `agent/crates/swm-agent/src/service.rs`
- `server/app/api/agent_enroll.py`

**Estimated scope:** Medium: 3-5 files

---

## Task 5: Echtzeit-Agent-Kanal (`WSS` + HTTPS-Fallback) & Geräte-Freigabe im Admin-Portal

**Description:** Verbindet registrierte Agenten dauerhaft per ausgehendem WebSocket (`wss://<fqdn>/api/v1/agent/ws`) mit automatischem Reconnect-Backoff und HTTPS-Heartbeat-Fallback und stellt die Flotten-Übersicht im Admin-Portal (Online/Offline-Status, Freigeben, Sperren, Löschen) bereit.

**Acceptance criteria:**
- [x] Freigegebene Agenten halten eine authentifizierte WebSocket-Verbindung und wechseln bei Verbindungsabbruch im Admin-Portal zuverlässig auf *Offline*.
- [x] Nicht freigegebene Geräte (*Pending Approval*) können in der Admin-UI mit einem Klick freigegeben oder abgelehnt werden.
- [x] Gesperrte/widerrufene Geräte verlieren sofort den Zugriff auf Agent-Endpunkte und WebSocket.

**Verification:**
- [x] Tests pass: `python -m pytest server/tests/test_agent_ws.py` und `cargo test --manifest-path agent/Cargo.toml -p swm-agent`
- [x] Build succeeds: `npm --prefix web run build`
- [x] Manual check: Starten und Stoppen von `swm-agent` aktualisiert die Status-Anzeige in `DevicesView.vue` live.

**Dependencies:** Task 4

**Files likely touched:**
- `server/app/services/ws_hub.py`
- `server/app/api/agent_ws.py`
- `agent/crates/swm-agent/src/transport/ws_client.rs`
- `web/src/views/DevicesView.vue`
- `server/tests/test_agent_ws.py`

**Estimated scope:** Medium: 3-5 files

---

## Task 6: Hardware-, OS- & Pending-Reboot-Inventar Ende-zu-Ende

**Description:** Erfasst auf Windows und Linux die System- und Hardware-Daten (Hostname, OS-Edition/Version/Build/Kernel, CPU, RAM, Laufwerksbelegung, IPs, angemeldete Benutzer) sowie den **Pending-Reboot-Status** und zeigt alle Details in der Gerätedetail-Ansicht des Admin-Portals an.

**Acceptance criteria:**
- [x] Windows-Erkennung prüft `Component Based Servicing\RebootPending`, `WindowsUpdate\Auto Update\RebootRequired` und `PendingFileRenameOperations`.
- [x] Linux-Erkennung liest `/etc/os-release`, Laufwerks-/Speicherwerte sowie `/var/run/reboot-required` (Debian/Ubuntu) bzw. `needs-restarting -r` (RHEL/Rocky/Alma/Fedora).
- [x] Backend speichert Inventar-Snapshots und zeigt Hardware, OS und ein deutliches „Neustart erforderlich“-Badge in der Gerätedetail-Ansicht an.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-agent inventory` und `python -m pytest server/tests/test_inventory.py`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml --bin swm-agent` und `npm --prefix web run build`
- [x] Manual check: Gerätedetailseite im Browser zeigt echte lokale Systemwerte des Test-Clients.

**Dependencies:** Task 5

**Files likely touched:**
- `agent/crates/swm-agent/src/inventory/system.rs`
- `agent/crates/swm-agent/src/inventory/reboot.rs`
- `server/app/api/agent_inventory.py`
- `web/src/views/DeviceDetailView.vue`
- `server/tests/test_inventory.py`

**Estimated scope:** Medium: 3-5 files

---

## Task 7: Software-Inventarisierung & Gerätegruppen / Tags

**Description:** Erweitert den Agenten um das Auslesen aller installierten Anwendungen (Windows: `HKLM`/`HKCU` Uninstall-Registry + `winget`; Linux: `dpkg-query`, `rpm -qa`, `flatpak list`) und implementiert im Server und Admin-Portal Gerätegruppen (z. B. *Pilot-Ring*, *Workstations*, *Server*) und Tags.

**Acceptance criteria:**
- [x] Agent überträgt installierte Software inklusive Name, Version, Herausgeber, Quelle (`msi`/`registry`/`winget`/`apt`/`dnf`/`flatpak`) und Scope (`machine`/`user`) per Delta-/Hash-Abgleich (nur bei Änderung).
- [x] Admins können Gruppen und Tags anlegen, Geräte zuordnen und im Inventar nach bestimmter Software oder Version über alle Rechner hinweg suchen.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-agent software_inv` und `python -m pytest server/tests/test_groups_and_software_inv.py`
- [x] Build succeeds: `npm --prefix web run build`
- [x] Manual check: Gerätedetailseite listet installierte Programme durchsuchbar auf; Geräte lassen sich Gruppen zuweisen.

**Dependencies:** Task 6

**Files likely touched:**
- `agent/crates/swm-agent/src/inventory/software.rs`
- `server/app/models/groups.py`
- `server/app/api/admin_groups.py`
- `web/src/views/GroupsView.vue`
- `server/tests/test_groups_and_software_inv.py`

**Estimated scope:** Medium: 3-5 files

---

## Checkpoint: Nach Phase 2 (Tasks 4–7)
- [x] `cargo test --workspace` und `pytest` laufen vollständig grün
- [x] Agent registriert sich sicher, hält WSS-Verbindung und meldet Hardware-, OS-, Reboot- und Software-Inventar
- [x] Gerätegruppen und Freigabe-Workflow sind in der Admin-UI voll bedienbar

---

## Phase 3: Job-Engine, Wartungsfenster & OS-Update-Management

## Task 8: Signierte Job-Engine, Live-Log-Streaming & Wartungsfenster-Scheduler

**Description:** Baut die zentrale Auftrags-Engine auf: Der Server erzeugt `Ed25519`-signierte Jobs, prüft vor Auslieferung definierte **Wartungsfenster** pro Gruppe (inkl. *Catch-up* für ausgeschaltete Geräte), und der Agent verifiziert die Signatur, führt Aufträge sequenziell aus und streamt `stdout`/`stderr` sowie den Exit-Code zurück.

**Acceptance criteria:**
- [x] Agent lehnt jeden Job mit ungültiger oder manipulierter `Ed25519`-Signatur strikt ab und protokolliert einen Sicherheitsfehler.
- [x] Wartungsfenster (Wochentage, Start-/Endzeit, Zeitzone, *Catch-up*-Flag) halten geplante Jobs außerhalb des Fensters zurück und lösen sie im Fenster bzw. beim nächsten Online-Kontakt (falls *Catch-up* aktiv) aus.
- [x] Admin-UI zeigt eine Auftrags-Übersicht inkl. detaillierter Log-Ausgabe (`stdout`/`stderr`, Laufzeit, Exit-Code) pro Gerät.

**Verification:**
- [x] Tests pass: `python -m pytest server/tests/test_jobs_and_maintenance.py` und `cargo test --manifest-path agent/Cargo.toml -p swm-agent job_runner`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml --bin swm-agent` und `npm --prefix web run build`
- [x] Manual check: Manipulierter Test-Job wird vom Agenten abgewiesen; gültiger Job liefert Live-Logs in der Admin-UI.

**Dependencies:** Task 7

**Files likely touched:**
- `server/app/models/jobs_and_windows.py`
- `server/app/services/scheduler.py`
- `agent/crates/swm-agent/src/jobs/runner.rs`
- `web/src/views/JobsAndWindowsView.vue`
- `server/tests/test_jobs_and_maintenance.py`

**Estimated scope:** Medium: 3-5 files

---

## Task 9: Windows OS-Update-Engine (Native WUA-API) & KB-Steuerung

**Description:** Implementiert im Windows-Teil des `swm-agent` den Scanner und Installer für Betriebssystem-Updates über die native **Windows Update Agent (WUA)** Schnittstelle (`Microsoft.Update.Session` ohne WSUS-Zwang) inklusive Kategorie-Filterung (Security, Critical, Optional/Drivers) und KB-Ausschlussliste im Admin-Portal.

**Acceptance criteria:**
- [x] Windows-Agent ermittelt ausstehende Updates mit KB-Nummer, Titel, Kategorie, Schweregrad und Größe und meldet sie an den Server.
- [x] Vom Admin zurückgestellte/gesperrte KB-Nummern werden bei Update-Läufen übersprungen.
- [x] Freigegebene Windows-Updates werden im Hintergrund heruntergeladen und installiert; Rückgabecodes inklusive Reboot-Anforderung werden exakt an den Job-Status gemeldet.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-agent windows_wua` und `python -m pytest server/tests/test_os_patching_win.py`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml --bin swm-agent`
- [x] Manual check: Auslösen eines Update-Scans unter Windows listet reale oder gemockte WUA-Updates im Admin-Portal auf.

**Dependencies:** Task 8

**Files likely touched:**
- `agent/crates/swm-agent/src/patching/windows_wua.rs`
- `server/app/models/os_patches.py`
- `server/app/api/admin_patches.py`
- `server/tests/test_os_patching_win.py`

**Estimated scope:** Medium: 3-5 files

---

## Task 10: Linux OS-Update-Engine (`apt` & `dnf`) & Patch-Compliance-Dashboard

**Description:** Implementiert das Scannen und unbeaufsichtigte Installieren von Linux-Systemupdates für **APT** (`Debian/Ubuntu/Mint`) und **DNF** (`RHEL/Rocky/Alma/Fedora`) – wahlweise *nur Sicherheitsupdates* oder *alle Paketupdates* – und ergänzt das plattformübergreifende **Compliance-Dashboard** im Admin-Portal.

**Acceptance criteria:**
- [x] APT-Runner nutzt `DEBIAN_FRONTEND=noninteractive` mit `-o Dpkg::Options::="--force-confdef" -o Dpkg::Options::="--force-confold"`, sodass Konfigurationsabfragen niemals hängenbleiben.
- [x] DNF-Runner unterstützt sowohl `dnf upgrade --security -y` als auch vollständige Paket-Upgrades und erkennt Kernel-/Core-Updates sauber.
- [x] Das Admin-Dashboard und die Patch-Ansicht zeigen Compliance-Quoten, fehlende kritische Updates und Filter für Windows & Linux in einer gemeinsamen Ansicht.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-agent linux_patch` und `python -m pytest server/tests/test_os_patching_linux.py`
- [x] Build succeeds: `npm --prefix web run build`
- [x] Manual check: Dashboard visualisiert Patch-Status, Compliance-Rate und offene Sicherheitsupdates korrekt.

**Dependencies:** Task 9

**Files likely touched:**
- `agent/crates/swm-agent/src/patching/linux_pkg.rs`
- `web/src/views/DashboardView.vue`
- `web/src/views/PatchManagementView.vue`
- `server/tests/test_os_patching_linux.py`

**Estimated scope:** Medium: 3-5 files

---

## Task 11: Gesteuerte Neustart-Richtlinien (Reboot Policies) & Agent-Self-Update

**Description:** Setzt die konfigurierbaren Neustart-Richtlinien pro Gruppe um (*Nie automatisch neu starten*, *Automatisch im Wartungsfenster*, *Benutzer-Karenzzeit mit Verschiebe-Option & Deadline*) und implementiert den signierten **Agent-Self-Update**-Mechanismus mit Rollback-Schutz.

**Acceptance criteria:**
- [x] Server-Gruppen mit Richtlinie *Kein automatischer Neustart* werden nach Updates niemals automatisch neu gestartet, sondern nur als *Reboot Pending* markiert.
- [x] Bei aktiver Karenzzeit-Richtlinie speichert der Agent die Reboot-Deadline persistent (überlebt Neustarts/Abmeldungen) und bereitet Status-Events für das lokale Software Center vor.
- [x] Ein Agent-Self-Update prüft `SHA-256` + `Ed25519`-Signatur des neuen Agent-Binaries und ersetzt den Dienst atomar.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-agent reboot_policy` und `python -m pytest server/tests/test_reboot_and_selfupdate.py`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml --bin swm-agent` und `npm --prefix web run build`
- [x] Manual check: Neustart-Richtlinie lässt sich pro Gruppe konfigurieren und wird im Agent-Zustand korrekt abgebildet.

**Dependencies:** Task 10

**Files likely touched:**
- `agent/crates/swm-agent/src/patching/reboot_manager.rs`
- `agent/crates/swm-agent/src/self_update.rs`
- `server/app/api/admin_policies.py`
- `web/src/views/PoliciesView.vue`
- `server/tests/test_reboot_and_selfupdate.py`

**Estimated scope:** Medium: 3-5 files

---

## Checkpoint: Nach Phase 3 (Tasks 8–11)
- [x] Alle Backend-, Agent- und Frontend-Tests grün
- [x] Signierte Job-Ausführung, Wartungsfenster, Windows-/Linux-OS-Patching und Reboot-Richtlinien greifen sauber ineinander
- [x] Kein automatischer Neustart außerhalb erlaubter Richtlinien möglich

---

## Phase 4: Software-Katalog, Eigenes Paket-Repository & Software-Verteilung

## Task 12: Software-Katalog für Native Paketmanager (`winget`, `apt`, `dnf`, `flatpak`)

**Description:** Erstellt das Datenmodell, die Admin-API und die Katalog-Verwaltungsoberfläche für Software-Pakete aus nativen Paketmanagern (`winget`, `apt`, `dnf`, `flatpak`) inklusive Metadaten (Anzeigename, Icon, Beschreibung, Kategorie, Paket-ID, Ziel-Plattform, optionale Parameter).

**Acceptance criteria:**
- [x] Admins können Katalog-Einträge für Windows (`winget`) und Linux (`apt`, `dnf`, `flatpak`) mit Icon, Beschreibung und Kategorie anlegen, bearbeiten und archivieren.
- [x] Katalog-Einträge unterstützen die Angabe gesperrter Prozessnamen (z. B. `firefox.exe`), die vor einem Update geschlossen sein müssen.

**Verification:**
- [x] Tests pass: `python -m pytest server/tests/test_catalog_native.py`
- [x] Build succeeds: `npm --prefix web run build`
- [x] Manual check: Anlegen und Filtern von Windows- und Linux-Katalogpaketen in `CatalogView.vue`.

**Dependencies:** Task 8

**Files likely touched:**
- `server/app/models/catalog.py`
- `server/app/api/admin_catalog.py`
- `web/src/views/CatalogView.vue`
- `server/tests/test_catalog_native.py`

**Estimated scope:** Medium: 3-5 files

---

## Task 13: Eigenes NS8-Paket-Repository (Chunked Upload, `SHA-256`, Range-Download & Detection Rules)

**Description:** Erweitert den Software-Katalog um den Upload eigener Installationsdateien (`.msi`, `.exe`, `.zip`, `.deb`, `.rpm`, `.ps1`, `.sh`) mit Chunked-Upload, automatischer `SHA-256`-Berechnung, `HTTP Range`-Download-Endpunkt für Agenten, konfigurierbaren **Detection Rules** und Exit-Code-Mapping (`0`, `3010`, `1641`).

**Acceptance criteria:**
- [x] Große Pakete werden per Streaming/Chunks ohne RAM-Spitzen hochgeladen und mit berechnetem `SHA-256`-Hash im persistenten Storage abgelegt.
- [x] `GET /api/v1/agent/packages/{id}/download` unterstützt `Range: bytes=start-end` (`206 Partial Content`) für unterbrechungssichere Downloads über VPN/WAN.
- [x] Pro Paket lassen sich *Detection Rules* (Registry-Key/Version, Dateipfad/Version oder Paketname) sowie Silent-Install-/Uninstall-Befehle hinterlegen.

**Verification:**
- [x] Tests pass: `python -m pytest server/tests/test_custom_repo_and_ranges.py` und `cargo test --manifest-path agent/Cargo.toml -p swm-agent downloader`
- [x] Build succeeds: `npm --prefix web run build`
- [x] Manual check: Upload eines Test-Installers im Upload-Wizard erzeugt korrekten `SHA-256`-Hash und erlaubt Range-Downloads.

**Dependencies:** Task 12

**Files likely touched:**
- `server/app/services/package_storage.py`
- `server/app/api/agent_packages.py`
- `agent/crates/swm-agent/src/transport/downloader.rs`
- `web/src/components/PackageUploadModal.vue`
- `server/tests/test_custom_repo_and_ranges.py`

**Estimated scope:** Medium: 3-5 files

---

## Task 14: Windows Software-Runner (`SYSTEM`-sicherer WinGet-Resolver, MSI/EXE/PS1 & Detection Rules)

**Description:** Implementiert die Software-Installation, Aktualisierung und Deinstallation unter Windows im `swm-agent`: automatische `winget.exe`-Auflösung unter `NT AUTHORITY\SYSTEM` (`--scope machine --silent`), Ausführung eigener `.msi`/`.exe`/`.ps1`-Pakete (inkl. `.zip`-Entpacken), Auswertung der *Detection Rules* und Prüfung laufender Prozesse.

**Acceptance criteria:**
- [x] WinGet-Runner findet `winget.exe` unter `C:\Program Files\WindowsApps\Microsoft.DesktopAppInstaller_*` auch aus Session 0 heraus, erzwingt `--scope machine` und bricht hängende Prozesse nach konfigurierbarem Timeout sauber ab.
- [x] Custom-Runner prüft vor der Installation die *Detection Rule* (überspringt, falls bereits in Zielversion installiert), verifiziert `SHA-256`, führt den Silent-Befehl aus und wertet Exit-Code `3010` als *Erfolg + Neustart erforderlich* aus.
- [x] Läuft ein als konfliktbehaftet markierter Prozess, meldet der Runner deterministisch den Status *Wartet auf Schließen der Anwendung*.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-agent windows_software`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml --bin swm-agent`
- [x] Manual check: Testlauf der Detection-Rules und eines harmlosen Test-Skripts/Pakets unter Windows meldet korrekten Status und Exit-Code.

**Dependencies:** Task 13

**Files likely touched:**
- `agent/crates/swm-agent/src/software/winget.rs`
- `agent/crates/swm-agent/src/software/windows_installer.rs`
- `agent/crates/swm-agent/src/software/detection.rs`
- `agent/crates/swm-agent/src/software/mod.rs`

**Estimated scope:** Medium: 3-5 files

---

## Task 15: Linux Software-Runner (`apt`/`dnf`/`flatpak`/Custom) & Deployment-Zuweisungen (*Pflicht* vs. *Verfügbar*)

**Description:** Implementiert die Linux-Software-Runner (`apt`, `dnf`, `flatpak --system -y` sowie eigene `.deb`/`.rpm`/`.sh`-Pakete) und vervollständigt im Server & Admin-Portal die **Bereitstellungs-Zuweisungen (Deployments)** an Gruppen/Geräte in den Modi **Pflicht (Required)**, **Verfügbar (Available / Self-Service)** und **Deinstallieren (Uninstall)** inkl. Drittanbieter-Update-Erkennung.

**Acceptance criteria:**
- [x] *Required*-Zuweisungen rollen Software automatisch (bzw. im zugewiesenen Wartungsfenster) auf Zielgruppen aus und halten sie bei neuen Katalogversionen aktuell.
- [x] *Available*-Zuweisungen installieren noch nichts automatisch, sondern synchronisieren ein vom Server `Ed25519`-signiertes Self-Service-Manifest an die Ziel-Agenten.
- [x] Die Deployment-Ansicht im Admin-Portal zeigt pro Software-Rollout den Fortschrittsbalken (*Installiert*, *Ausstehend*, *Fehlgeschlagen*) über alle Zielgeräte.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-agent linux_software` und `python -m pytest server/tests/test_deployments.py`
- [x] Build succeeds: `npm --prefix web run build`
- [x] Manual check: Anlegen einer *Pflicht*- und einer *Verfügbar*-Bereitstellung in `DeploymentsView.vue` erzeugt passende signierte Manifeste für den Agenten.

**Dependencies:** Task 14

**Files likely touched:**
- `agent/crates/swm-agent/src/software/linux_installer.rs`
- `server/app/models/deployments.py`
- `server/app/api/admin_deployments.py`
- `web/src/views/DeploymentsView.vue`
- `server/tests/test_deployments.py`

**Estimated scope:** Medium: 3-5 files

---

## Checkpoint: Nach Phase 4 (Tasks 12–15)
- [x] Katalog-Pakete und eigene hochgeladene Installer werden sauber signiert, per Range-Download geladen und auf Windows & Linux ausgeführt
- [x] *Required*-Rollouts, *Uninstall*-Aufträge und *Available*-Manifest-Synchronisation sind vollständig getestet

---

## Phase 5: Endbenutzer „Software Center“ (Desktop-GUI & Tray)

## Task 16: Abgesicherter lokaler IPC-Server im `swm-agent` (Privilege Separation)

**Description:** Implementiert im `swm-agent` den lokalen IPC-Endpunkt (Windows Named Pipe `\\.\pipe\ns8-swm-ipc` mit ACL für authentifizierte lokale interaktive Benutzer / Linux Unix Domain Socket `/run/ns8-swm/ipc.sock`), über den das unprivilegierte Software Center den Katalog abfragen und ausschließlich serverseitig freigegebene Aktionen auslösen kann.

**Acceptance criteria:**
- [x] Unprivilegierte lokale Benutzerprozesse können über IPC den freigegebenen *Available*-Katalog, den Live-Installationsfortschritt und ausstehende Updates lesen.
- [x] Über IPC lassen sich ausschließlich Aktionen anhand einer bereits lokal im `swm-agent` hinterlegten, `Ed25519`-signierten Paket-/Update-ID starten (Installieren, Aktualisieren, Deinstallieren sofern im Manifest erlaubt, Reboot-Snooze, Reboot-Jetzt) – freie Kommandoübergabe ist strukturell unmöglich.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-agent ipc`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml --bin swm-agent`
- [x] Manual check: IPC-Integrationstest sendet nicht-freigegebene Paket-ID und erhält deterministisch `PermissionDenied`.

**Dependencies:** Task 15

**Files likely touched:**
- `agent/crates/swm-common/src/ipc.rs`
- `agent/crates/swm-agent/src/ipc/mod.rs`
- `agent/crates/swm-agent/src/ipc/windows_pipe.rs`
- `agent/crates/swm-agent/src/ipc/unix_socket.rs`

**Estimated scope:** Medium: 3-5 files

---

## Task 17: `swm-center` Desktop-Anwendung (Tauri v2 + Vue 3 Self-Service App-Store)

**Description:** Erstellt die plattformübergreifende Endbenutzer-Desktop-App `swm-center` (Tauri v2 + Vue 3) im Stil des Microsoft Software Centers mit den Ansichten **Anwendungen** (Suche, Kategorien, 1-Klick *Installieren* / *Aktualisieren* / *Deinstallieren*), **Updates** (proaktives Anstoßen ausstehender OS- & App-Updates vor der Deadline) und **Aktivitäten / Verlauf**.

**Acceptance criteria:**
- [x] `swm-center` startet komplett ohne Administrator-/Root-Rechte im normalen Benutzerkontext und lädt den Katalog über den lokalen IPC-Kanal von `swm-agent`.
- [x] Benutzer sehen beim Klick auf *Installieren* Live-Statuswechsel (*Wird heruntergeladen*, *Wird installiert*, *Installiert* bzw. verständliche Fehlermeldung z. B. „Bitte erst Anwendung schließen“).
- [x] Ist Deinstallation vom Admin für ein Paket deaktiviert, ist der *Deinstallieren*-Button im Software Center ausgeblendet/gesperrt.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-center`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml -p swm-center`
- [x] Manual check: Starten von `swm-center` neben lokal laufendem `swm-agent` zeigt freigegebene Katalog-Apps und löst Test-Installation per Klick aus.

**Dependencies:** Task 16

**Files likely touched:**
- `agent/crates/swm-center/Cargo.toml`
- `agent/crates/swm-center/tauri.conf.json`
- `agent/crates/swm-center/src/main.rs`
- `agent/crates/swm-center/ui/src/App.vue`
- `agent/crates/swm-center/ui/src/views/StoreView.vue`

**Estimated scope:** Medium: 3-5 files

---

## Task 18: System-Tray-Integration, Benachrichtigungen & Neustart-Countdown-Dialog

**Description:** Ergänzt `swm-center` um den dauerhaften Hintergrundbetrieb im System-Tray (Autostart bei Benutzeranmeldung), dezente Desktop-Benachrichtigungen bei neuen Pflicht-Updates sowie den interaktiven **Neustart-Dialog** mit Verschiebe-Funktion („Später erinnern“) und sichtbarem Countdown vor Ablauf der Karenzzeit.

**Acceptance criteria:**
- [x] Schließen des Hauptfensters minimiert `swm-center` in den System-Tray (mit Schnellmenü: *Software Center öffnen*, *Nach Updates suchen*, *Status*).
- [x] Fordert ein installiertes Update einen Neustart an, erscheint eine Benachrichtigung bzw. das Reboot-Dialogfenster mit Auswahl *Jetzt neu starten* oder *Erinnern in 1h / 4h* (solange die Admin-Deadline noch nicht erreicht ist).
- [x] Auf Headless-Systemen ohne Desktop-Session wird `swm-center` nicht benötigt; `swm-agent` arbeitet eigenständig weiter.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml --workspace`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml -p swm-center`
- [x] Manual check: Simuliertes `RebootRequired`-Event vom Agenten öffnet den Reboot-Countdown-Dialog und übermittelt „Snooze“ sauber zurück an `swm-agent`.

**Dependencies:** Task 17

**Files likely touched:**
- `agent/crates/swm-center/src/tray.rs`
- `agent/crates/swm-center/ui/src/components/RebootModal.vue`
- `agent/crates/swm-center/ui/src/views/UpdatesView.vue`

**Estimated scope:** Medium: 3-5 files

---

## Checkpoint: Nach Phase 5 (Tasks 16–18)
- [x] Kompletter Endbenutzer-Workflow (Tray -> Software Center öffnen -> App ohne Admin-Rechte installieren -> Reboot verschieben/bestätigen) funktioniert Ende-zu-Ende
- [x] Sicherheitsprüfung bestätigt: Kein unprivilegierter IPC-Aufruf kann nicht-signierte Befehle ausführen

---

## Phase 6: NethServer 8 Modul-Integration, Cockpit-UI, Backup & Release-Pipeline

## Task 19: NS8 `imageroot` Actions & Rootless Systemd Podman-Units

**Description:** Erstellt die vollständige NethServer 8 `imageroot/`-Struktur (`create-module`, `configure-module` inkl. `01hostname-validation` & `agent.dump_env()`, `get-configuration`, `destroy-module`) sowie die Rootless-Systemd-Units (`swm.service`, `swm-db.service`, `swm-app.service`) streng nach den NS8-Architekturregeln.

**Acceptance criteria:**
- [x] **Kein** eigenes `imageroot/actions/get-status/` vorhanden (nutzt NS8 Core `get-status` + eigene Einstellungen über `get-configuration`).
- [x] `20initialize` und `20configure` rufen nach allen `agent.set_env()`-Aufrufen zwingend `agent.dump_env()` auf; `01hostname-validation` prüft `agent.http_route_in_use()` und beendet bei Konflikt mit Exit-Code `2`.
- [x] Podman bindet ausschließlich an `127.0.0.1:${TCP_PORT}:8080`, registriert die Traefik-Route (`lets_encrypt`, `http2https`), nutzt `--pull=missing` und wartet in `ExecStartPre` per `pg_isready` auf PostgreSQL.

**Verification:**
- [x] Tests pass: `python -m pytest server/tests/test_ns8_imageroot_rules.py` (statischer Architektur- & Regel-Test für alle NS8-Vorgaben)
- [x] Build succeeds: `python -m compileall imageroot`
- [x] Manual check: Prüfung aller Shebangs, LF-Zeilenenden und Action-JSON-Schemas (`validate-input.json`, `validate-output.json`).

**Dependencies:** Task 3

**Files likely touched:**
- `imageroot/actions/create-module/20initialize`
- `imageroot/actions/configure-module/01hostname-validation`
- `imageroot/actions/configure-module/20configure`
- `imageroot/actions/get-configuration/20read`
- `imageroot/systemd/user/swm-app.service`

**Estimated scope:** Medium: 3-5 files

---

## Task 20: NS8 Backup, Restore, Clone & Update-Hooks

**Description:** Implementiert die vollständige Datensicherungs- und Wiederherstellungsunterstützung für das NS8-Restic-Cluster-Backup (`imageroot/etc/state-include.conf`, `module-dump-state` mit `pg_dump`, `module-cleanup-state`, `restore-module/`, `clone-module/` sowie `update-module.d/`).

**Acceptance criteria:**
- [x] `module-dump-state` erstellt konsistent einen PostgreSQL-Dump, und `state-include.conf` sichert Datenbank-Dump, `Ed25519`-Signaturschlüssel sowie das eigene Software-Paket-Volume.
- [x] `restore-module/06copyenv` ruft `agent.dump_env()` auf und `restore-module/50configure` spielt Datenbank, Schlüssel und Traefik-Route sauber wieder ein; `clone-module` vergibt konfliktfreie Umgebungswerte.
- [x] `imageroot/update-module.d/20pull_images` zieht aktualisierte Container-Images bei Modul-Updates sauber nach.

**Verification:**
- [x] Tests pass: `python -m pytest server/tests/test_ns8_imageroot_rules.py`
- [x] Build succeeds: `python -m compileall imageroot`
- [x] Manual check: Alle Pfade in `state-include.conf` stimmen exakt mit den Volume-/Umgebungspfaden in `swm-db.service` und `swm-app.service` überein.

**Dependencies:** Task 19

**Files likely touched:**
- `imageroot/etc/state-include.conf`
- `imageroot/bin/module-dump-state`
- `imageroot/actions/restore-module/06copyenv`
- `imageroot/actions/restore-module/50configure`
- `imageroot/update-module.d/20pull_images`

**Estimated scope:** Medium: 3-5 files

---

## Task 21: NS8 Cluster-Admin Cockpit-UI (`ui/` mit Vue 2 & `@nethserver/ns8-ui-lib`)

**Description:** Erstellt die offizielle NethServer 8 Modul-Oberfläche für das Cluster-Admin-Cockpit (`ui/`) mit `Status.vue`, `Settings.vue` (FQDN, Let's Encrypt, HTTP->HTTPS, NS8 User-Domain/LDAP-Auswahl, Admin-Link zum SoftwareManager-Portal) und `About.vue` inklusive vollständiger deutscher und englischer Übersetzung (`de`/`en`).

**Acceptance criteria:**
- [x] Nutzt strikt die korrekten `TaskService`-Methoden (`createErrorNotificationForApp(err, msg)` – **kein** nicht-existierendes `createSuccessNotificationForApp`).
- [x] Alle im `<template>` genutzten `@carbon/icons-vue`-Komponenten sind in `components` bzw. `data()` registriert.
- [x] `ui/src/main.js` lädt sowohl `de` als auch `en` (`fallbackLocale: "en"`), und `public/i18n/{de,en}/translation.json` enthält alle NS8-Pflicht-Keys (`action.list-installed-modules`, `task.cannot_create_task`, `error.cannot_retrieve_installed_modules`, `error.cannot_retrieve_module_info`).

**Verification:**
- [x] Tests pass: `python -m pytest server/tests/test_ns8_ui_rules.py` (prüft i18n-Pflichtkeys, Icon-Registrierung und verbotene API-Aufrufe)
- [x] Build succeeds: `npm --prefix ui install && npm --prefix ui run build`
- [x] Manual check: Produktions-Build unter `ui/dist/` wird fehlerfrei erzeugt.

**Dependencies:** Task 19

**Files likely touched:**
- `ui/package.json`
- `ui/src/main.js`
- `ui/src/views/Settings.vue`
- `ui/public/i18n/de/translation.json`
- `ui/public/i18n/en/translation.json`

**Estimated scope:** Medium: 3-5 files

---

## Task 22: Multi-Stage `Containerfile`, Client-Paketierung (`.msi`/`.deb`/`.rpm`), CI/CD & E2E-Smoke-Test

**Description:** Verbindet alle Komponenten im Root-`Containerfile` (inkl. `org.nethserver.images`, CRLF-Strip & `chmod +x`), `server/Containerfile.app` und `build-images.sh`, erstellt die Paketierungs-Definitionen für Windows (`.msi` / Setup-Skript) & Linux (`.deb` / `.rpm`) sowie die GitHub-Actions-Pipeline und einen automatisierten Ende-zu-Ende-Integrations-Smoke-Test.

**Acceptance criteria:**
- [x] Root-`Containerfile` deklariert `org.nethserver.images` (inkl. `postgres:16-alpine` und App-Image), entfernt `\r`-Zeichen in `/imageroot` und setzt Ausführungsrechte (`chmod +x`) auf alle Actions, Hooks und Binaries.
- [x] Client-Installer-Skripte und Paket-Konfigurationen installieren wahlweise nur `swm-agent` (`--headless` für Server) oder `swm-agent` + `swm-center` (für Workstations) und hinterlegen die gebauten Artefakte direkt zum Download im App-Container.
- [x] Automatisierter E2E-Smoke-Test spielt den kompletten Lebenszyklus durch: *Token erstellen -> Agent registrieren & freigeben -> Inventar senden -> Software-Paket hochladen & ausrollen -> Self-Service-Trigger via IPC -> Erfolgs-Log im Server*.

**Verification:**
- [x] Tests pass: `python -m pytest server/tests/` und `cargo test --manifest-path agent/Cargo.toml --workspace`
- [x] Build succeeds: `npm --prefix web run build` und `npm --prefix ui run build`
- [x] Manual check: GitHub Actions Workflow-Validierung und vollständiger Durchlauf des E2E-Smoke-Tests.

**Dependencies:** Task 18, Task 20, Task 21

**Files likely touched:**
- `Containerfile`
- `server/Containerfile.app`
- `build-images.sh`
- `agent/packaging/install.ps1`
- `.github/workflows/build-and-test.yml`

**Estimated scope:** Medium: 3-5 files

---

## Finaler Checkpoint: Projektabschluss (Nach Task 22)
- [x] Alle 22 Aufgaben erfüllt und abgehakt
- [x] Alle Backend-Tests (`pytest`), Rust-Workspace-Tests (`cargo test`), Admin-UI-Builds (`web/`) und NS8-Cockpit-UI-Builds (`ui/`) laufen fehlerfrei
- [x] Alle 8 NS8-Architekturregeln und 3 NS8-UI-Regeln sind automatisiert verifiziert
- [x] Bereit für Deployment und Test auf einem NethServer-8-Cluster (`add-module`)


---

## Phase 7: Gehärteter Desktop-Client (`swm-center`) & Erweiterte Admin-Kontrollen

## Task 23: Gruppenbasierte Desktop-Client-Richtlinien & Live-DB-Schema-Upgrade

**Description:** Erweitert Gerätegruppen um zentrale Steuerungsrichtlinien für den Desktop-Client (`enforce_tray`, `allow_self_service`, `allow_user_os_updates`, `silent_notifications`), rüstet eine automatische, zerstörungsfreie Spalten-Migration beim Serverstart für bestehende PostgreSQL-/SQLite-Instanzen nach und liefert die effektiven Richtlinien an `swm-agent` und `swm-center` aus.

**Acceptance criteria:**
- [x] Bestehende PostgreSQL- und SQLite-Datenbanken erhalten neue Spalten auf `groups` und `devices` (`center_running`) beim Start idempotent per `ensure_schema_upgrades()` ohne Datenverlust.
- [x] `GET /api/v1/agent/status` liefert die effektive `desktop_policy` der Gerätegruppen (`enforce_tray` gewinnt restriktiv, `allow_self_service` / `allow_user_os_updates` konfigurierbar).
- [x] `PoliciesView.vue` erlaubt das komfortable Speichern der Desktop-Client-Richtlinien pro Gerätegruppe.

**Verification:**
- [x] Tests pass: `python -m pytest server/tests/test_desktop_hardening_and_controls.py`
- [x] Build succeeds: `npm --prefix web run build`
- [x] Manual check: Ändern einer Gruppenrichtlinie in `PoliciesView.vue` aktualisiert sofort das `/api/v1/agent/status`-Payload.

**Dependencies:** None

**Files likely touched:**
- `server/app/models.py`
- `server/app/db.py`
- `server/app/schemas.py`
- `server/app/api/agent_api.py`
- `web/src/views/PoliciesView.vue`

**Estimated scope:** Medium: 3-5 files

---

## Task 24: `swm-center` Härtung (Tray-Beenden-Sperre, Single-Instance-Lock & Live-Sync)

**Description:** Härtet den Rust-Desktop-Client `swm-center`: Entfernt/blockiert im verwalteten Modus (`enforce_tray = true`) das Beenden über das System-Tray-Menü, verhindert doppelte Tray-Prozesse durch eine Single-Instance-Session-Koordination (erneuter Start öffnet das bestehende Fenster statt einer zweiten Instanz) und ergänzt den IPC-Befehl `SyncNow` im Tray-Menü.

**Acceptance criteria:**
- [x] Bei `enforce_tray = true` enthält das Tray-Kontextmenü keinen „Beenden“-Eintrag (`IDM_QUIT`), und ein `QuitTray`-Action-Event beendet den Prozess nicht.
- [x] Wird `swm-center gui` ein zweites Mal in derselben Benutzersitzung aufgerufen (z. B. über Startmenü oder Watchdog), erkennt es die laufende Instanz über die lokale Session-Datei/Port-Prüfung, öffnet bei nicht-gesetztem `--tray-only` das bestehende UI-Fenster und beendet den Zweitprozess sauber mit Exit `0`.
- [x] Neuer IPC-Request `IpcRequest::SyncNow` löst im `swm-agent` sofort einen Synchronisationszyklus mit dem NS8-Server aus.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-center -p swm-common`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml --workspace`
- [x] Manual check: Zweiter Aufruf von `swm-center gui` erzeugt kein zweites Tray-Icon.

**Dependencies:** Task 23

**Files likely touched:**
- `agent/crates/swm-common/src/dto.rs`
- `agent/crates/swm-common/src/ipc.rs`
- `agent/crates/swm-center/src/tray.rs`
- `agent/crates/swm-center/src/main.rs`
- `agent/crates/swm-agent/src/ipc/mod.rs`

**Estimated scope:** Medium: 3-5 files

---

## Task 25: Persistenter Session-Watchdog, Sofortstart & Dienst-Selbstheilung (Windows & Linux)

**Description:** Stellt sicher, dass weder `swm-agent` noch `swm-center` dauerhaft ausfallen oder vom Benutzer abgeschossen bleiben: Konfiguriert Windows-SCM-Crash-Recovery für `NS8SoftwareManagerAgent`, richtet unter Windows den geplanten System-Task `\NS8SoftwareCenterGuardian` inkl. Sofortstart während `install.ps1` und Watchdog-Prüfung durch `swm-agent` ein, installiert unter Linux eine globale `systemd`-User-Unit mit `Restart=always` und meldet `center_running` im Inventar.

**Acceptance criteria:**
- [x] `service-install` und `install.ps1` konfigurieren Windows-Service-Recovery (`restart/5000/restart/10000/restart/30000`) für `NS8SoftwareManagerAgent`.
- [x] `install.ps1` (`headless=0`) registriert und startet `\NS8SoftwareCenterGuardian` sofort in aktiven Benutzersitzungen; beendet ein Nutzer `swm-center.exe` im Task-Manager, startet der Agent-Watchdog den Task bei `enforce_tray = true` automatisch neu.
- [x] `install.sh` (`headless=0`) und `nfpm.yaml` richten `/etc/systemd/user/ns8-swm-center.service` mit `Restart=always` und `RestartSec=5` ein; `InventoryReport` überträgt `center_running: bool`.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-agent` und `python -m pytest server/tests/test_scheduler_and_autostart.py`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml --workspace`
- [x] Manual check: PowerShell-AST-Parser-Test für `install.ps1` und Bash-Syntaxprüfung für `install.sh` laufen fehlerfrei.

**Dependencies:** Task 24

**Files likely touched:**
- `agent/crates/swm-agent/src/service.rs`
- `agent/crates/swm-agent/src/daemon.rs`
- `agent/crates/swm-agent/src/inventory/system.rs`
- `server/app/api/health_and_dl.py`
- `agent/packaging/nfpm.yaml`

**Estimated scope:** Medium: 3-5 files

---

## Checkpoint: Nach Tasks 23–25 (Client-Härtung komplett)
- [x] `cargo test --workspace` und `pytest` laufen vollständig grün
- [x] `swm-center` lässt sich bei aktivem `enforce_tray` nicht über das Tray beenden und wird nach Prozess-Kill automatisch neu gestartet
- [x] Bestehende Datenbanken migrieren beim Start automatisch ohne Fehler

---

## Task 26: Signierte Admin-Sofortbefehle (`force_sync`, `admin_notify`, `remote_reboot`)

**Description:** Erweitert Server und `swm-agent` um drei signierte Echtzeit-Steuerungsbefehle für Administratoren: sofortiger Inventar-/Katalog-Sync (`force_sync`), direkte Desktop-Benachrichtigungen an angemeldete Benutzer (`admin_notify` mit Titel, Nachricht & Dringlichkeit) sowie gesteuerte Remote-Reboots (`grace_period`, `immediate` oder `cancel`).

**Acceptance criteria:**
- [x] Server-Endpunkte `POST /api/v1/admin/devices/{id}/sync`, `/notify`, `/reboot` sowie Bulk-Endpunkt `POST /api/v1/admin/devices/bulk-action` erzeugen `Ed25519`-signierte Sofortjobs und pushen sie unmittelbar über den aktiven WebSocket.
- [x] `swm-agent` verifiziert die `Ed25519`-Signatur jedes Steuerungsjobs; bei `admin_notify` speichert er die Nachricht für `swm-center`, welches sofort ein natives Toast-Popup (WinRT / `notify-send`) und ein Hinweisbanner anzeigt.
- [x] `remote_reboot` erlaubt sowohl das Setzen einer Karenzzeit-Deadline als auch das Stornieren (`cancel`) eines ausstehenden Agent-Reboots.

**Verification:**
- [x] Tests pass: `python -m pytest server/tests/test_desktop_hardening_and_controls.py` und `cargo test --manifest-path agent/Cargo.toml -p swm-agent`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml --workspace`
- [x] Manual check: Auslösen von `admin_notify` über die API erzeugt ein verifiziertes Benachrichtigungs-Event im `CenterStatusSnapshot`.

**Dependencies:** Task 25

**Files likely touched:**
- `server/app/api/admin_devices.py`
- `agent/crates/swm-agent/src/jobs/runner.rs`
- `agent/crates/swm-center/src/tray.rs`
- `server/tests/test_desktop_hardening_and_controls.py`

**Estimated scope:** Medium: 3-5 files

---

## Task 27: Flotten-Mehrfachauswahl & Remote-Action-Leiste in der Admin-Konsole (`web/`)

**Description:** Baut die neuen Administrator-Werkzeuge übersichtlich in `DevicesView.vue` und `DeviceDetailView.vue` ein: Mehrfachauswahl-Toolbar (Bulk-Freigabe, Bulk-Sync, Bulk-Update-Scan, Gruppen-Zuweisung, Rundnachricht an ausgewählte PCs), Live-Anzeige des Tray-Status (`Software Center aktiv`) sowie 1-Klick-Dialoge für Benutzer-Nachrichten und Neustart-Steuerung.

**Acceptance criteria:**
- [x] In `DevicesView.vue` lassen sich mehrere Geräte per Checkbox auswählen und gesammelt synchronisieren, freigeben, scannen oder mit einer Desktop-Benachrichtigung versehen.
- [x] `DeviceDetailView.vue` zeigt den Live-Status von `swm-center` (`Tray aktiv` / `Keine Benutzersitzung`) sowie Direkt-Buttons für *Jetzt synchronisieren*, *Nachricht senden* und *Neustart steuern*.
- [x] Alle neuen Modals und Aktionen funktionieren barrierefrei im Light- und Dark-Mode.

**Verification:**
- [x] Tests pass: `npm --prefix web test -- --run`
- [x] Build succeeds: `npm --prefix web run build`
- [x] Manual check: Visuelle Prüfung von Bulk-Leiste, Nachrichten-Modal und Gerätedetails.

**Dependencies:** Task 26

**Files likely touched:**
- `web/src/api/types.ts`
- `web/src/api/client.ts`
- `web/src/views/DevicesView.vue`
- `web/src/views/DeviceDetailView.vue`
- `web/tests/dashboard-and-devices.spec.ts`

**Estimated scope:** Medium: 3-5 files

---

## Task 28: „System & Diagnose“-Tab + Kiosk-/Richtlinien-Durchsetzung in `swm-center` UI

**Description:** Erweitert die eingebettete Vue-3-Oberfläche des Desktop-Clients (`swm-center/ui`) um ein prominentes Admin-Mitteilungsbanner, die strikte Beachtung von `allow_self_service` / `allow_user_os_updates` (inkl. serverseitiger IPC-Sperre in `swm-agent`) sowie einen neuen Tab **„System & Diagnose“** für schnellen Helpdesk-Support.

**Acceptance criteria:**
- [x] Ist `allow_self_service = false`, lehnt `swm-agent` `TriggerPackage` über IPC mit `PermissionDenied` ab und `swm-center` zeigt einen klaren Hinweis (*„Durch IT-Richtlinie zentral verwaltet“*).
- [x] Neuer Tab **„System & Diagnose“** in `swm-center` zeigt Hostname, Geräte-ID, Server-URL, WebSocket-Status, Agent-Version, aktive Gruppenrichtlinien sowie Buttons für *„Jetzt mit Server synchronisieren“* und *„Diagnosebericht kopieren“*.
- [x] Vom Administrator gesendete Nachrichten (`admin_notify`) erscheinen prominent und quittierbar am oberen Rand des Software Centers.

**Verification:**
- [x] Tests pass: `npm --prefix agent/crates/swm-center/ui test -- --run` und `cargo test --manifest-path agent/Cargo.toml --workspace`
- [x] Build succeeds: `npm --prefix agent/crates/swm-center/ui run build && cargo build --manifest-path agent/Cargo.toml -p swm-center`
- [x] Manual check: Alle Backend-, Web-, Cockpit- und Rust-Tests laufen grün; Release-Bereitschaft geprüft.

**Dependencies:** Task 24, Task 26

**Files likely touched:**
- `agent/crates/swm-agent/src/ipc/mod.rs`
- `agent/crates/swm-center/src/bridge.rs`
- `agent/crates/swm-center/ui/src/App.vue`
- `agent/crates/swm-center/ui/src/__tests__/app.spec.ts`

**Estimated scope:** Medium: 3-5 files

---

## Finaler Checkpoint: Abschluss Phase 7
- [x] Alle 6 Aufgaben (Tasks 23–28) erfüllt und abgehakt
- [x] `pytest`, `vitest` (`web` & `swm-center/ui`) und `cargo test --workspace` laufen zu 100 % grün
- [x] Bereit für Release `v1.1.0` auf GitHub/GHCR und `ns8-nethforge`


---

## Phase 8: Vollständiger macOS-Support (`v1.2.0` — Universal Binary, `launchd`, `softwareupdate` & Homebrew)

## Task 29: Backend-, Bootstrap- & IPC-Vertrag für `macos` (`server/`, `/dl/install.sh`, `swm-common`)

**Description:** Erweitert alle FastAPI-Schemas, Katalog-/Patch-/Self-Update-Endpunkte, Artefakt-Metadaten und das Unix-Bootstrap-Skript (`/dl/install.sh`) um `os_family = "macos"` (`macos-universal` sowie Aliase `macos-arm64` / `macos-amd64`) und stellt den Standard-IPC-Socket unter macOS in `swm-common` auf `/var/run/ns8-swm/ipc.sock` um.

**Acceptance criteria:**
- [x] `AgentEnrollRequest`, `CatalogItemCreate`/`Update`, `SelfUpdateRequest`, `OsBreakdownOut` und `KNOWN_AGENT_ARTIFACTS` unterstützen `os_family = "macos"` sowie die Katalog-Quelltypen `brew` und `brew_cask`.
- [x] `GET /dl/install.sh` erkennt `uname -s == "Darwin"` automatisch, lädt die `macos-universal`-Binaries, entfernt Quarantäne-Attribute (`xattr -dr com.apple.quarantine`), richtet `/Library/LaunchDaemons/org.nethserver.swm-agent.plist` sowie bei `headless=0` `/Applications/NS8 Software Center.app` und `/Library/LaunchAgents/org.nethserver.swm-center.plist` inkl. Sofortstart in der aktiven Benutzersitzung ein.
- [x] `swm-common::default_ipc_address()` liefert unter `target_os = "macos"` `/var/run/ns8-swm/ipc.sock` (Linux behält `/run/ns8-swm/ipc.sock`).

**Verification:**
- [x] Tests pass: `python -m pytest server/tests/test_macos_support.py` und `cargo test --manifest-path agent/Cargo.toml -p swm-common`
- [x] Build succeeds: `cargo check --manifest-path agent/Cargo.toml --workspace`
- [x] Manual check: Abruf von `/dl/install.sh?token=test&headless=0` enthält saubere LF-Zeilenenden und gültige Bash-Syntax (`bash -n`) für Linux- und Darwin-Zweig.

**Dependencies:** None

**Files likely touched:**
- `server/app/schemas/api_schemas.py`
- `server/app/api/admin_auth.py`
- `server/app/api/admin_catalog_deployments.py`
- `server/app/api/health_and_dl.py`
- `agent/crates/swm-common/src/ipc.rs`

**Estimated scope:** Medium: 3-5 files

---

## Task 30: macOS-Systeminventar, `launchd`-Dienststeuerung & Session-Watchdog in `swm-agent`

**Description:** Rüstet `swm-agent` für den nativen Betrieb als macOS-Root-Daemon aus: Erkennung von macOS-Version/Build (`sw_vers`), Hardware-UUID (`IOPlatformUUID`), installiertem Software-Bestand (`/Applications/*.app`, `pkgutil`, `brew`), `launchd`-Service-Installation/-Deinstallation sowie Session-Watchdog für `swm-center`.

**Acceptance criteria:**
- [x] `detect_os_family()` liefert `"macos"` auf `target_os = "macos"`, liest Produktname/-version über `sw_vers`, ermittelt `IOPlatformUUID` als stabile `machine_uid` und nutzt `/Library/Application Support/NS8-SoftwareManager` als Standard-Datenverzeichnis.
- [x] Software-Inventar unter macOS erfasst `/Applications/*.app/Contents/Info.plist`, `pkgutil --pkgs` sowie installierte Homebrew-Formulae und -Casks (`brew list --versions` / `--cask`).
- [x] `service-install` / `service-uninstall` verwalten unter macOS `/Library/LaunchDaemons/org.nethserver.swm-agent.plist` (`KeepAlive = true`), und der Session-Watchdog startet `gui/<uid>/org.nethserver.swm-center` neu, falls `enforce_tray` aktiv ist und `swm-center` beendet wurde.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-agent`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml -p swm-agent`
- [x] Manual check: Unit-Tests für `sw_vers`-, `Info.plist`-, `brew list`-Parser und `launchd`-Plist-Generierung bestehen plattformübergreifend.

**Dependencies:** Task 29

**Files likely touched:**
- `agent/crates/swm-agent/src/inventory/system.rs`
- `agent/crates/swm-agent/src/inventory/software.rs`
- `agent/crates/swm-agent/src/inventory/reboot.rs`
- `agent/crates/swm-agent/src/service.rs`
- `agent/crates/swm-agent/src/daemon.rs`

**Estimated scope:** Medium: 3-5 files

---

## Task 31: macOS OS-Patching über Apples `softwareupdate` in `swm-agent` & Server

**Description:** Implementiert das Scannen und unbeaufsichtigte Installieren offizieller Apple-Betriebssystem- und Sicherheitsupdates über `softwareupdate -l` und `softwareupdate -i` inklusive Ausschlusslisten-Filterung und Reboot-Erkennung.

**Acceptance criteria:**
- [x] Neues Modul `patching/macos_update.rs` parst `softwareupdate -l` (modernes Label-/Key-Value-Format sowie klassisches Format) zuverlässig in `PendingOsUpdate`-Einträge (ID/Label, Titel, Version, Größe, Kategorie, `reboot_required`).
- [x] Vom Administrator ausgeschlossene Update-IDs werden beim macOS-Patch-Lauf übersprungen; freigegebene Updates werden per `softwareupdate -i <label>` mit Live-Log-Streaming installiert und melden Neustart-Anforderungen (`Action: restart` / `restart`) an den `RebootManager`.
- [x] `server/app/api/admin_patches.py` ordnet macOS-Updates sauber `os_family = "macos"` zu und berücksichtigt Macs in der Patch-Compliance-Quote.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-agent macos_update` und `python -m pytest server/tests/test_macos_support.py`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml -p swm-agent`
- [x] Manual check: Fixture-Tests mit realen `softwareupdate -l`-Ausgaben (inkl. macOS Sonoma/Sequoia Security Responses & OS-Upgrades) laufen grün.

**Dependencies:** Task 30

**Files likely touched:**
- `agent/crates/swm-agent/src/patching/mod.rs`
- `agent/crates/swm-agent/src/patching/macos_update.rs`
- `agent/crates/swm-agent/src/jobs/runner.rs`
- `server/app/api/admin_patches.py`

**Estimated scope:** Medium: 3-5 files

---

## Checkpoint: Nach Tasks 29–31 (macOS-Kernagent & OS-Patching komplett)
- [x] `pytest` und `cargo test --workspace` laufen grün
- [x] `swm-agent` unterstützt Enrollment, Inventar, `launchd`, IPC und `softwareupdate` unter macOS
- [x] `/dl/install.sh` deckt Linux und macOS (Darwin) ab

---

## Task 32: macOS Software-Deployment (`.pkg`, `.dmg`, `.app.zip`, `.sh` & Homebrew) + Detection Rules

**Description:** Baut den macOS-Software-Installer und die Detection-Engine in `swm-agent`: unterstützt eigene Pakete (`.pkg`, `.dmg` inkl. automatischem Mount/Copy/Unmount, `.zip` App-Bundles, `.sh`) sowie **Homebrew Formulae (`brew`) & Casks (`brew_cask`)** unter dem unprivilegierten Homebrew-Eigentümer.

**Acceptance criteria:**
- [x] `macos_installer.rs` installiert `.pkg` via `installer -pkg ... -target /`, mountet `.dmg`-Images unsichtbar/read-only (`hdiutil attach -nobrowse -readonly`), installiert enthaltene `.pkg` oder kopiert `.app` nach `/Applications` (inkl. `xattr -dr com.apple.quarantine`) und räumt den Mountpoint garantiert per `hdiutil detach -force` auf.
- [x] Für `source_type` `brew` und `brew_cask` ermittelt `swm-agent` den nicht-`root` Besitzer von `/opt/homebrew` bzw. `/usr/local/Homebrew` und führt `install`, `upgrade` und `uninstall` sicher per `sudo -u <owner> -H env NONINTERACTIVE=1 HOMEBREW_NO_AUTO_UPDATE=1` aus.
- [x] `detection.rs` prüft macOS-Erkennungsregeln vom Typ `app_bundle` (`/Applications/<App>.app/Contents/Info.plist` inkl. `min_version`), `pkgutil` (`pkgutil --pkg-info`), `brew` / `brew_cask` und `file`.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-agent macos_installer` und `cargo test --manifest-path agent/Cargo.toml -p swm-agent detection`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml -p swm-agent`
- [x] Manual check: Kommandopläne für `.pkg`, `.dmg`, `.app.zip`, `brew` und `brew_cask` sowie Versionsvergleiche in `Info.plist` sind per Unit-Test abgesichert.

**Dependencies:** Task 30

**Files likely touched:**
- `agent/crates/swm-agent/src/software/mod.rs`
- `agent/crates/swm-agent/src/software/macos_installer.rs`
- `agent/crates/swm-agent/src/software/detection.rs`
- `agent/crates/swm-agent/src/jobs/runner.rs`

**Estimated scope:** Medium: 3-5 files

---

## Task 33: Gehärtetes `swm-center` auf macOS (App-Bundle, `open`, Native Notifications & Packaging)

**Description:** Passt das Single-Binary Endbenutzer-„NS8 Software Center“ (`swm-center`) und die Packaging-Vorlagen an macOS an: Fensteröffnung über `/usr/bin/open`, native macOS-Mitteilungen über `osascript`, macOS-konformer Session-Lock-Pfad sowie fertige `LaunchDaemon`-/ `LaunchAgent`-Plists.

**Acceptance criteria:**
- [x] `launch_desktop_window()` öffnet das lokale Software-Center-Fenster unter macOS zuverlässig über `/usr/bin/open` (ohne `DISPLAY`/`WAYLAND_DISPLAY`-Abhängigkeit).
- [x] `dispatch_native_notification()` sendet unter macOS echte Desktop-Mitteilungen über `osascript` (mit sauberem Escaping und Berücksichtigung von `silent_notifications` / dringenden Admin-Hinweisen).
- [x] `default_session_file_path()` nutzt unter macOS `~/Library/Application Support/NS8-SoftwareManager/center-session.json` für den Single-Instance-Guard; Packaging-Vorlagen unter `agent/packaging/macos/` liegen mit reinen LF-Zeilenenden vor.

**Verification:**
- [x] Tests pass: `cargo test --manifest-path agent/Cargo.toml -p swm-center`
- [x] Build succeeds: `cargo build --manifest-path agent/Cargo.toml -p swm-center`
- [x] Manual check: AppleScript-Escaping-Tests und Packaging-Invarianten-Tests bestehen zu 100 %.

**Dependencies:** Task 29

**Files likely touched:**
- `agent/crates/swm-center/src/tray.rs`
- `agent/crates/swm-center/src/main.rs`
- `agent/crates/swm-center/src/bridge.rs`
- `agent/packaging/macos/org.nethserver.swm-agent.plist`
- `agent/packaging/macos/org.nethserver.swm-center.plist`

**Estimated scope:** Medium: 3-5 files

---

## Task 34: Admin-Web-Konsole (`web/`), NS8 Cockpit-UI (`ui/`) & Universal-Binary CI/CD (`macos-latest` + `lipo`)

**Description:** Integriert macOS durchgängig in die Admin-Web-Konsole (`OsBadge`, Dashboard-Statistik, Geräte- & Patch-Filter, Katalog-Modal für `.pkg`/`.dmg`/`Homebrew`, Agent-Self-Update sowie macOS-MDM-Tab für Jamf/Intune/Mosyle im Rollout-Assistenten), ergänzt den Download-Link auf der NS8-Statusseite und erweitert GitHub Actions + `Containerfile.app` um den Bau und das Einbetten der macOS-Universal-Binaries.

**Acceptance criteria:**
- [x] `web/` unterstützt `macos` in `OsBadge.vue` (eigenes Apple/macOS-Icon), `DashboardView.vue`, `DevicesView.vue`, `PatchManagementView.vue`, `CatalogView.vue`, `PackageUploadModal.vue` (`brew`, `brew_cask`, `.pkg`, `.dmg`, `.zip` + `app_bundle`/`pkgutil`-Erkennungsregeln), `PoliciesView.vue` und `EnrollmentView.vue` (inkl. MDM-Skript-Generator für Jamf/Intune/Mosyle).
- [x] `ui/src/views/Status.vue` (+ `de`/`en` Sprachdateien) bietet den Direkt-Download für macOS Universal unter Wahrung aller 5 NS8-Cockpit-UI-Regeln.
- [x] `.github/workflows/build-and-test.yml` baut auf `macos-latest` beide Targets (`aarch64-apple-darwin` & `x86_64-apple-darwin`), führt `cargo test --workspace` nativ auf macOS aus, erzeugt per `lipo -create` Universal Binaries und bettet sie über `Containerfile.app` unter `/app/agents/macos-universal` (sowie `macos`, `macos-arm64`, `macos-amd64`) ein.

**Verification:**
- [x] Tests pass: `npm --prefix web test -- --run` und `python -m pytest server/tests -v`
- [x] Build succeeds: `npm --prefix web run build` und `npm --prefix ui run build`
- [x] Manual check: Alle CI-Workflows, Containerfiles und UI-Ansichten im Light- & Dark-Mode geprüft.

**Dependencies:** Task 29, Task 31, Task 32, Task 33

**Files likely touched:**
- `web/src/components/OsBadge.vue`
- `web/src/components/PackageUploadModal.vue`
- `web/src/views/EnrollmentView.vue`
- `web/src/views/DashboardView.vue`
- `.github/workflows/build-and-test.yml`
- `Containerfile.app`

**Estimated scope:** Large: 6-8 files

---

## Finaler Checkpoint: Abschluss Phase 8 (`v1.2.0`)
- [x] Alle 6 Aufgaben (Tasks 29–34) erfüllt und abgehakt
- [x] `pytest`, `vitest` (`web` & `swm-center/ui`), `ui`-Build und `cargo test --workspace` laufen lokal und in GitHub Actions (`ubuntu-latest`, `windows-latest`, `macos-latest`) zu 100 % grün
- [ ] Release `v1.2.0` veröffentlicht auf GHCR (`ghcr.io/bls-isp/softwaremanager:1.2.0`) und indiziert in `ns8-nethforge`
