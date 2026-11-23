# Umsetzungsplan: NS8-SoftwareManager

## 1. Überblick (Overview)

Wir entwickeln **NS8-SoftwareManager** als vollständige, native Open-Source-Eigenentwicklung für **NethServer 8 (NS8)** zur zentralen Verwaltung von **Betriebssystem-Updates (OS Patching)** und **Software-Installationen/-Updates (inkl. Endbenutzer-„Software Center“)** auf **Windows-** und **Linux-Systemen**. Das System besteht aus einem Rootless-Podman-Modul hinter NS8-Traefik (FastAPI-Backend + PostgreSQL + Vue-3-Admin-Portal + NS8-Cockpit-UI), einem schlanken plattformübergreifenden Hintergrund-Agenten in Rust (`swm-agent` für Server & Workstations) sowie einer unprivilegierten Desktop- & Tray-Anwendung (`swm-center` auf Basis von Tauri v2 + Vue 3) für den Self-Service ohne lokale Administratorrechte.

---

## 2. Architekturentscheidungen & Begründungen (Architecture Decisions)

1. **Komplette Eigenentwicklung im Monorepo (`server/`, `web/`, `agent/`, `imageroot/`, `ui/`):**
   - *Begründung:* Volle Kompatibilität mit NS8 Rootless Podman und Traefik (ausschließlich Port `443` via HTTPS/WSS statt proprietärer NATS-/Depot-Ports), keine Enterprise-Paywalls für Windows-Updates oder Self-Service-Kataloge, und einheitliche Datenmodelle zwischen Server und Agent.

2. **Server-Backend in Python (`FastAPI` + `SQLAlchemy 2 async` + `Alembic` + `PostgreSQL 16`):**
   - *Begründung:* Erstklassige Integration mit NS8 Samba AD / OpenLDAP (`ldap3`), effizientes Streaming großer Installationspakete (`HTTP Range Requests` & Chunked Uploads), schnelle Entwicklungs- und Testzyklen (`pytest`), sowie schlanke Container-Builds.

3. **Client-Agent (`swm-agent`) in Rust (`Cargo`-Workspace):**
   - *Begründung:* Kompiliert zu einem einzelnen, statischen Binary (~8–12 MB) ohne jegliche Laufzeitabhängigkeiten auf Windows (`NT AUTHORITY\SYSTEM` Windows-Service) und Linux (`root` via `systemd`). Ideal für sowohl Headless-Server als auch Desktop-Workstations.

4. **Endbenutzer „Software Center“ (`swm-center`) als lokale Desktop- & Tray-App (Tauri v2 + Vue 3):**
   - *Begründung:* Bietet auf Windows (nutzt vorinstalliertes Edge WebView2) und Linux-Desktops (`webkit2gtk`) ein echtes lokales Programmfenster + System-Tray-Icon + Neustart-Countdown-Dialog – analog zum Microsoft Software Center. Läuft rein mit **Standard-Benutzerrechten** und spricht ausschließlich über eine abgesicherte lokale IPC-Schnittstelle (Windows Named Pipe `\\.\pipe\ns8-swm-ipc` / Linux Unix Socket `/run/ns8-swm/ipc.sock`) mit dem lokalen `swm-agent`. Auf Servern wird nur `swm-agent` (ohne GUI) installiert.

5. **Kryptografische Auftrags- & Paket-Integrität (`Ed25519` + `SHA-256`):**
   - *Begründung:* Da `swm-agent` mit `SYSTEM`/`root`-Rechten läuft, signiert der Server alle Installations-/Update-Manifeste mit einem beim Modulstart erzeugten `Ed25519`-Schlüsselpaar; zusätzlich wird jedes Paket vor Ausführung gegen seinen `SHA-256`-Hash geprüft. Selbst die lokale `swm-center` GUI kann ausschließlich bereits vom Server signierte *Available*-Pakete antriggern – niemals beliebige Befehle übergeben.

6. **Hybrides Paket- & Patch-Modell (Geräte- & Gruppen-basiert in v1):**
   - *OS-Updates:* Native Windows Update API (WUA über COM/PowerShell-Host ohne WSUS-Zwang) auf Windows; `apt` (Debian/Ubuntu/Mint) und `dnf` (RHEL/Rocky/Alma/Fedora) auf Linux.
   - *Software-Katalog:* Native Paketmanager (`winget --scope machine` mit automatischer `WindowsApps`-Pfadauflösung unter `SYSTEM`, sowie `apt`, `dnf`, `flatpak`) **plus** eigenes lokales Paket-Repository auf dem NS8-Volume für eigene `.msi`/`.exe`/`.zip`/`.deb`/`.rpm`-Installer inkl. *Detection Rules* und Exit-Code-Mapping (`0`, `3010` = Reboot erforderlich).
   - *Targeting:* Zuweisung in Version 1 an **Geräte und Gerätegruppen** (z. B. *Pilot-Ring*, *Workstations*, *Server*), da dies 90 % aller Szenarien zuverlässig und benutzerunabhängig abdeckt; das Datenmodell hält ein optionales Feld `target_ad_groups` für spätere Erweiterungen frei.

7. **Strikte Einhaltung der NethServer 8 Modul-Regeln:**
   - **Kein** Überschreiben von `imageroot/actions/get-status` (Statusdaten kommen über Core `get-status` + eigene Metadaten über `get-configuration`).
   - Immer `agent.dump_env()` unmittelbar nach `agent.set_env()` aufrufen.
   - Podman-Pod strikt an `127.0.0.1:${TCP_PORT}:8080` binden, Traefik-Route inkl. Hostname-Kollisionsprüfung (`01hostname-validation`) registrieren.
   - `--pull=missing` in Systemd-Units, `pg_isready`-Warteschleife vor App-Start + DB-Retry im Backend, LF-Zeilenenden via `.gitattributes` & `Containerfile`, vollständiges Restic-Backup/Restore/Clone.

---

## 3. Ziel-Projektstruktur (Directory Layout)

```text
NS8-SoftwareManager/
├── .gitattributes                 # Erzwingt LF für imageroot/**, *.sh, Containerfile
├── .github/workflows/             # CI/CD: Tests, Cross-Compile Agenten, NS8-Image-Build
├── Containerfile                  # NS8-Modul-Image (imageroot + Cockpit UI Assets)
├── build-images.sh                # Lokales & CI Build-Skript für Modul- & App-Container
├── REQUIREMENTS.md                # Fachliche Anforderungsanalyse & Muss-Funktionen
├── tasks/
│   ├── plan.md                    # Dieser Umsetzungsplan
│   └── todo.md                    # Detaillierte Task-Liste & Checkpoints (Task List Target)
├── server/                        # Zentrales Management-Backend (Python FastAPI)
│   ├── Containerfile.app          # Multi-Stage Build (inkl. Web-UI & vorab gebaute Agenten)
│   ├── pyproject.toml             # Abhängigkeiten & pytest-Konfiguration
│   ├── alembic/                   # Datenbank-Migrationen
│   ├── app/
│   │   ├── api/                   # Getrennte Router: /api/v1/admin/* & /api/v1/agent/*
│   │   ├── auth/                  # JWT, NS8 LDAP/AD-Anbindung, Enrollment- & Device-Tokens
│   │   ├── crypto/                # Ed25519 Manifest-Signierung & SHA-256 Prüfroutinen
│   │   ├── models/                # SQLAlchemy-Modelle (Devices, Groups, Packages, Jobs, Policies)
│   │   ├── schemas/               # Pydantic v2 Schemas
│   │   ├── services/              # Scheduler, Wartungsfenster, Storage (Range/Chunked), WS-Hub
│   │   └── main.py                # FastAPI Entrypoint + DB-Backoff-Check
│   └── tests/                     # Unit- & API-Integrationstests (pytest)
├── web/                           # Dediziertes Admin-Web-Portal (Vue 3 + TS + Vite + Tailwind)
│   ├── src/
│   │   ├── views/                 # Dashboard, Devices, Patching, Catalog, Deployments, Policies, Settings
│   │   ├── components/            # Tabellen, Log-Viewer, Upload-Wizard, Status-Badges
│   │   └── api/                   # Typisierter REST- & WebSocket-Client
│   └── tests/                     # Frontend-Komponententests (Vitest)
├── agent/                         # Cross-Platform Client-Workspace (Rust + Tauri v2)
│   ├── Cargo.toml                 # Workspace: crates/swm-common, crates/swm-agent, crates/swm-center
│   ├── crates/
│   │   ├── swm-common/            # Geteilte DTOs, Signaturprüfung (ed25519-dalek), IPC-Protokoll
│   │   ├── swm-agent/             # Privilegierter Dienst (Windows Service / systemd)
│   │   │   ├── src/inventory/     # HW, OS, Software (HKLM/HKCU, winget, dpkg, rpm, flatpak), Reboot
│   │   │   ├── src/patching/      # Windows WUA Engine + Linux APT/DNF Engine
│   │   │   ├── src/software/      # WinGet (SYSTEM-safe), MSI/EXE/PS1, DEB/RPM/SH, Detection Rules
│   │   │   ├── src/ipc/           # Named Pipe (Windows) & Unix Socket (Linux) für Software Center
│   │   │   └── src/transport/     # WSS-Client, HTTPS-Fallback, Resumable Range Downloader
│   │   └── swm-center/            # Unprivilegierte Desktop-GUI + System-Tray (Tauri v2 + Vue 3)
│   └── packaging/                 # WiX/MSI (Windows), nfpm/.deb/.rpm (Linux), Bootstrap-Skripte
├── imageroot/                     # NethServer 8 Rootless Modul-Steuerung
│   ├── actions/                   # create-module, configure-module, get-configuration, restore, clone, destroy
│   ├── bin/                       # module-dump-state, module-cleanup-state
│   ├── etc/state-include.conf     # Restic Backup-Pfade
│   ├── systemd/user/              # swm.service (Pod), swm-db.service, swm-app.service
│   └── update-module.d/           # Image-Pull & Migrations-Hooks bei Modul-Updates
└── ui/                            # NS8 Cluster-Admin Cockpit UI (Vue 2 + @nethserver/ns8-ui-lib)
    ├── public/i18n/               # Übersetzungen (de, en) inkl. Pflicht-Keys
    └── src/views/                 # Status.vue, Settings.vue, About.vue
```

---

## 4. Abhängigkeitsgraph & Phasen-Reihenfolge

```text
[Phase 1: Repo, Auth, Kryptografie & Enrollment-Basis]
        │
        ▼
[Phase 2: Rust Core-Agent, WSS-Transport, Inventar & Gruppen]
        │
        ├───────────────────────────────────────────┐
        ▼                                           ▼
[Phase 3: Job-Engine, Wartungsfenster &       [Phase 4: Software-Katalog, Custom-Repo,
          OS-Patching (WUA / APT / DNF)]                WinGet/APT/DNF/Flatpak & Rollouts]
        │                                           │
        └─────────────────────┬─────────────────────┘
                              ▼
        [Phase 5: Lokales IPC & Desktop „Software Center“ + Tray]
                              │
                              ▼
        [Phase 6: NS8 Rootless Podman-Modul, Cockpit-UI, Backup & CI/CD]
```

*Parallelisierungspotenzial:* Sobald **Phase 2** (Agent-Kommunikation & Gruppen) sowie **Task 8** (signierte Job-Engine) stehen, können **Phase 3 (OS-Updates)**, **Phase 4 (Software-Verteilung)** und Teile von **Phase 6 (NS8 Cockpit-UI `ui/`)** bei Bedarf auch parallel bearbeitet werden.

---

## 5. Phasen & Aufgabenübersicht (Details in `tasks/todo.md`)

Alle Einzelaufgaben mit Akzeptanzkriterien, Prüfkommandos und betroffenen Dateien sind in `tasks/todo.md` gepflegt:

### Phase 1: Fundament, Authentifizierung & Enrollment-Basis
- **Task 1:** Monorepo-Grundgerüst, `.gitattributes` (LF), FastAPI-Basis, Datenbank-Anbindung mit Backoff & Alembic-Setup
- **Task 2:** Admin-Authentifizierung (Lokaler Admin + NS8 LDAP/AD-Provider) & Vue-3-Admin-Grundlayout mit Login
- **Task 3:** Ed25519-Manifest-Signierung, Enrollment-Tokens & dynamische Bootstrap-Installer-Endpunkte (`install.ps1` / `install.sh`)
- **Checkpoint 1:** Server startet sauber, Admin-Login & Token-/Bootstrap-Generierung funktionieren Ende-zu-Ende

### Phase 2: Cross-Platform Core-Agent & Flotten-Inventarisierung
- **Task 4:** Rust-Workspace (`swm-common`, `swm-agent`), Dienst-Integration (Windows Service / `systemd`) & Enrollment-Handshake
- **Task 5:** Echtzeit-Agent-Kanal (`WSS` + HTTPS-Fallback) & Geräte-Übersicht inkl. Freigabe-Workflow (*Pending Approval*) im Admin-Portal
- **Task 6:** Hardware-, Betriebssystem- & Pending-Reboot-Inventar (Windows & Linux) Ende-zu-Ende bis zur Gerätedetail-Ansicht
- **Task 7:** Software-Inventarisierung (`HKLM`/`HKCU`/WinGet & `dpkg`/`rpm`/`flatpak`) sowie Gerätegruppen & Tags
- **Checkpoint 2:** Windows- und Linux-Agenten registrieren sich, melden Live-Status & vollständiges Inventar

### Phase 3: Job-Engine, Wartungsfenster & OS-Update-Management
- **Task 8:** Signierte Job-Engine, Live-Log-Streaming (`stdout`/`stderr`/Exit-Code) & Wartungsfenster-Scheduler (inkl. Catch-up)
- **Task 9:** Windows OS-Update-Engine im Agenten (native WUA-API Scan/Install + KB-Ausschluss) & Admin-Freigabeansicht
- **Task 10:** Linux OS-Update-Engine (`apt` & `dnf` Sicherheits- vs. Voll-Updates) & plattformübergreifendes Patch-Compliance-Dashboard
- **Task 11:** Gesteuerte Neustart-Richtlinien (Kein Reboot / Im Wartungsfenster / Benutzer-Karenzzeit mit Deadline)
- **Checkpoint 3:** OS-Patching für Windows & Linux läuft kontrolliert innerhalb definierter Wartungsfenster

### Phase 4: Software-Katalog, Eigenes Paket-Repository & Software-Verteilung
- **Task 12:** Zentraler Software-Katalog für native Paketmanager (`winget`, `apt`, `dnf`, `flatpak`) in Backend & Admin-UI
- **Task 13:** Eigenes NS8-Paket-Repository (Chunked Upload, `SHA-256`, HTTP Range Download, *Detection Rules* & Exit-Codes `0`/`3010`)
- **Task 14:** Windows Software-Runner im Agenten (`SYSTEM`-sicherer WinGet-Resolver + `.msi`/`.exe`/`.ps1`-Runner + Prozess-Sperrprüfung)
- **Task 15:** Linux Software-Runner (`apt`/`dnf`/`flatpak` + `.deb`/`.rpm`/`.sh`) & Bereitstellungs-Zuweisungen (*Pflicht* vs. *Verfügbar* + Deinstallation & Drittanbieter-Updates)
- **Checkpoint 4:** Pflicht-Rollouts und Drittanbieter-Updates funktionieren für Katalog- & Custom-Pakete auf Windows und Linux

### Phase 5: Endbenutzer „Software Center“ (Desktop-GUI & Tray)
- **Task 16:** Abgesicherter lokaler IPC-Server im `swm-agent` (Named Pipe unter Windows / Unix Socket unter Linux mit strikter Privilegientrennung)
- **Task 17:** `swm-center` Desktop-App (Tauri v2 + Vue 3): Self-Service App-Store (*Installieren*, *Aktualisieren*, *Deinstallieren*), Fortschritt & Updates-Tab
- **Task 18:** System-Tray-Modus, Desktop-Benachrichtigungen & interaktiver Neustart-Dialog („Jetzt neu starten“ / „Später erinnern“ mit Countdown)
- **Checkpoint 5:** Standardbenutzer ohne Admin-Rechte können freigegebene Software installieren und Neustarts kontrolliert steuern

### Phase 6: NethServer 8 Modul-Integration, Cockpit-UI, Backup & Release-Pipeline
- **Task 19:** NS8 `imageroot` Kern-Actions (`create`, `configure` mit `01hostname-validation` & `agent.dump_env()`, `get-configuration`, `destroy`) & Systemd-Pod-Units (`127.0.0.1:${TCP_PORT}`, `pg_isready`, `--pull=missing`)
- **Task 20:** NS8 Restic-Backup, Restore, Clone & Update-Hooks (`state-include.conf`, `module-dump-state`, `restore-module`, `clone-module`, `update-module.d`)
- **Task 21:** NS8 Cluster-Admin Cockpit-UI (`ui/` mit Vue 2, `@nethserver/ns8-ui-lib`, Carbon-Icons & vollständiger `de`/`en` Lokalisierung)
- **Task 22:** Multi-Stage `Containerfile`, `build-images.sh`, Client-Installer-Paketierung (`.msi`/`.deb`/`.rpm`) & GitHub Actions CI/CD inkl. E2E-Smoke-Test
- **Checkpoint 6:** Produktionsreifes, vollständig getestetes NethServer 8 Modul

---

## 6. Risiken & Gegenmaßnahmen (Risks and Mitigations)

| Risiko | Auswirkung | Gegenmaßnahme |
|---|---|---|
| **`winget.exe` schlägt unter `NT AUTHORITY\SYSTEM` (Session 0) fehl** | Hoch | Dedizierter Resolver sucht die aktuelle Binärdatei unter `C:\Program Files\WindowsApps\Microsoft.DesktopAppInstaller_*_x64__8wekyb3d8bbwe\winget.exe`, erzwingt `--scope machine --silent --accept-package-agreements --accept-source-agreements`, überwacht Prozess-Timeouts gegen versteckte GUI-Dialoge und bietet direkte `.msi`/`.exe`-Pakete als 100 % deterministischen Fallback. |
| **Kompromittierung der Schnittstelle zwischen unprivilegierter GUI und `SYSTEM`/`root`-Dienst** | Kritisch | Die lokale IPC-Schnittstelle akzeptiert keinerlei Befehlszeilen oder URLs vom Benutzer, sondern ausschließlich Paket-/Job-IDs, deren `Ed25519`-Serversignatur der `swm-agent` selbst gegen den hinterlegten Server-Public-Key verifiziert. |
| **Abbruch großer Downloads (> 1 GB) über VPN / langsames WAN hinter Traefik** | Mittel | Backend liefert Pakete mit `Accept-Ranges: bytes` aus; `swm-agent` lädt große Dateien in Chunks in ein Staging-Verzeichnis, setzt unterbrochene Downloads fort und prüft vor Ausführung den `SHA-256`-Gesamthash. |
| **Datenbank beim Pod-Start noch nicht bereit oder CRLF-Fehler in NS8-Skripten** | Hoch | Kombination aus `ExecStartPre=... pg_isready` in `swm-app.service` + exponentiellem Verbindungs-Backoff im FastAPI-Start + automatischer CRLF-Bereinigung (`sed -i 's/\r$//'`) und `chmod +x` im `Containerfile`. |
| **Unbeabsichtigter Neustart von Produktiv-Servern oder laufender Benutzerarbeit** | Hoch | Standard-Richtlinie für neue Gruppen ist *„Kein automatischer Neustart“*; automatische Reboots greifen nur bei explizit aktivierter Richtlinie innerhalb des aktiven Wartungsfensters bzw. nach Ablauf der Benutzer-Karenzzeit. |

---

## 7. Offene Punkte / Getroffene Standard-Annahmen

Sofern du keine Änderung wünschst, geht dieser Plan von folgenden Standard-Entscheidungen aus (entsprechend unseren vorherigen Eckpunkten):
1. **Tech-Stack:** Python `FastAPI` + `PostgreSQL` für den NS8-Server, `Vue 3` für das Admin-Web-Portal, `Rust` für den `swm-agent` und `Tauri v2 + Vue 3` für das lokale Desktop-Software-Center (`swm-center`).
2. **Linux-Familien zum Start:** `Debian/Ubuntu/Mint (apt)` + `RHEL/Rocky/Alma/Fedora (dnf)` + `Flatpak` (Modularer Trait-Aufbau im Agenten erlaubt später leichtes Nachrüsten von `zypper`/`pacman`).
3. **Zuweisungs-Ebene in v1:** Zuweisung von Paketen und Update-Richtlinien an **Geräte & Gerätegruppen** (mit vorbereitetem DB-Schema für spätere AD-Benutzergruppen-Filterung).


---

# Erweiterungsplan Phase 7 (v1.1.0): Gehärteter Desktop-Client & Erweiterte Admin-Kontrollen

## Überblick
Diese Erweiterung schließt alle Manipulationslücken der Endbenutzer-App (`swm-center`) im verwalteten Unternehmensbetrieb (Entfernen der Beenden-Möglichkeit, Single-Instance-Sperre, automatischer Watchdog-Respawn bei Task-Manager-Kill, Sofortstart ohne Neuanmeldung sowie Windows-SCM-Crash-Recovery) und gibt Administratoren leistungsstarke Echtzeit-Werkzeuge an die Hand (Gruppenrichtlinien für den Desktop-Client, signierte 1-Klick-Fernbefehle für Live-Sync, Toast-Benutzerhinweise & Reboot-Steuerung, Bulk-Flottenaktionen sowie ein integriertes Helpdesk-Diagnosepanel im Client).

## Architekturentscheidungen (Phase 7)
- **Serverseitig erzwungene Desktop-Richtlinien statt reiner UI-Ausblendung:** Flags wie `enforce_tray`, `allow_self_service` und `allow_user_os_updates` werden nicht nur in der Vue-UI des `swm-center` ausgewertet, sondern direkt im privilegierten IPC-Handler von `swm-agent` geprüft (`PermissionDenied`, falls Self-Service für die Gruppe deaktiviert ist).
- **Zweistufiger Session-Keep-Alive (OS-Task + Agent-Guardian):** Unter Windows kombiniert die Lösung einen systemweiten geplanten Task (`\NS8SoftwareCenterGuardian` für `S-1-5-32-545`, `MultipleInstances=IgnoreNew`) mit einer periodischen Prozessprüfung im `swm-agent`, die den Task bei beendetem `swm-center.exe` in aktiven Benutzersitzungen automatisch neu anstößt. Unter Linux übernimmt eine globale `systemd`-User-Unit (`/etc/systemd/user/ns8-swm-center.service`) mit `Restart=always` (`RestartSec=5`) den automatischen Neustart.
- **Unterbrechungsfreie Live-Datenbankmigration (`ensure_schema_upgrades`):** Da produktive NS8-Instanzen bestehende PostgreSQL-Volumes nutzen, prüft der Server beim Start fehlende Spalten per SQLAlchemy-Inspektor und ergänzt sie idempotent (`ALTER TABLE ... ADD COLUMN`), ohne bestehende Daten zu gefährden.
- **Kryptografisch signierte Fernbefehle:** Alle neuen Sofortaktionen (`force_sync`, `admin_notify`, `remote_reboot`) nutzen dieselbe kanonische `Ed25519`-Job-Signatur (`SignedJob`) wie Software- und Patch-Aufträge — der Agent führt keinen unautorisierten Befehl aus.


---

# Erweiterungsplan Phase 8 (v1.2.0): Vollständiger macOS-Support (Universal Binary, `launchd`, `softwareupdate` & Homebrew)

## Überblick
Diese Phase erweitert **NS8-SoftwareManager** um vollwertigen First-Class-Support für **macOS** (Apple Silicon `M1–M4 / arm64` und Intel `x86_64` über ein kombiniertes **Universal Binary**). Sowohl der privilegierte Hintergrunddienst (`swm-agent`), das gehärtete Endbenutzer-„NS8 Software Center“ (`swm-center`), das FastAPI-Backend inkl. Bootstrap-Installer und Katalog, die Admin-Web-Konsole (`web/`), die NS8-Cockpit-Statusseite (`ui/`) als auch die GitHub-Actions-CI/CD-Pipeline (`macos-latest` + `lipo`) unterstützen danach Windows, Linux und macOS einheitlich über denselben `Ed25519`-signierten Sicherheits- und Steuerungskanal.

## Architekturentscheidungen (Phase 8)
- **Echtes macOS Universal Binary (`lipo`: `arm64` + `x86_64`):** Da unser Rust-Workspace reines `rustls` ohne C-OpenSSL-Abhängigkeiten nutzt, kompiliert der GitHub-Actions-Runner (`macos-latest`) sowohl `aarch64-apple-darwin` als auch `x86_64-apple-darwin` und vereint beide per `lipo -create` zu je einem Universal Binary (`swm-agent` & `swm-center`). Vorteil: Ein einziges Artefakt und eine einzige `SHA-256`-Prüfsumme decken gemischte Intel- und Apple-Silicon-Flotten beim Rollout und beim zentralen Agent-Self-Update ab.
- **Plattformgerechter Unix-IPC-Pfad (`/var/run/ns8-swm/ipc.sock`):** Da macOS kein `/run`-Verzeichnis besitzt, nutzt `#[cfg(target_os = "macos")]` `/var/run/ns8-swm/ipc.sock` (unter Linux bleibt `/run/ns8-swm/ipc.sock` für volle Rückwärtskompatibilität erhalten).
- **Zweistufige `launchd`-Selbstheilung (`LaunchDaemon` + `LaunchAgent`):**
  - `swm-agent` läuft als Root-Dienst über `/Library/LaunchDaemons/org.nethserver.swm-agent.plist` (`RunAtLoad = true`, `KeepAlive = true`, Datenverzeichnis `/Library/Application Support/NS8-SoftwareManager`).
  - `swm-center` wird als App-Bundle unter `/Applications/NS8 Software Center.app` abgelegt und über `/Library/LaunchAgents/org.nethserver.swm-center.plist` (`LimitLoadToSessionType = Aqua`, `KeepAlive = true`) in jeder grafischen Benutzersitzung gestartet; zusätzlich stößt der `swm-agent`-Session-Watchdog bei aktivem `enforce_tray` einen abgebrochenen Agent per `launchctl kickstart -k gui/<uid>/org.nethserver.swm-center` erneut an.
- **Sichere Homebrew-Ausführung ohne Root-Konflikt:** Da Homebrew die Ausführung als `root` verweigert, ermittelt `swm-agent` für `brew` / `brew_cask` automatisch das installierte Binary (`/opt/homebrew/bin/brew` bzw. `/usr/local/bin/brew`) sowie dessen unprivilegierten Eigentümer (`stat -f %Su`) und führt Paketbefehle per `sudo -u <owner> -H env NONINTERACTIVE=1 HOMEBREW_NO_AUTO_UPDATE=1` aus.
- **Gatekeeper-freier Enterprise-Rollout ohne Notarizing-Zwang:** Da das Bootstrap-Skript (`/dl/install.sh`) per `sudo` oder MDM-Root-Agent (Jamf, Intune, Mosyle) ausgeführt wird, entfernt es beim Installieren gezielt etwaige Quarantäne-Attribute (`xattr -dr com.apple.quarantine`) auf `/usr/local/bin/swm-*` und `/Applications/NS8 Software Center.app`.

## Aufgabenübersicht (Phase 8)
- **Task 29:** Backend-, Bootstrap- & IPC-Vertrag für `macos` (`server/`, `/dl/install.sh`, `swm-common`)
- **Task 30:** macOS-Systeminventar, `launchd`-Dienststeuerung & Session-Watchdog in `swm-agent`
- **Task 31:** macOS OS-Patching über Apples `softwareupdate` in `swm-agent` & Server
- **Checkpoint (Tasks 29–31):** macOS-Agent kompiliert, inventarisiert, verwaltet `launchd` und scannt/installiert macOS-Systemupdates
- **Task 32:** macOS Software-Deployment (`.pkg`, `.dmg`, `.app.zip`, `.sh` & Homebrew Formulae/Casks) + Detection Rules
- **Task 33:** Gehärtetes `swm-center` auf macOS (App-Bundle, `open`-Integration, `osascript`-Benachrichtigungen & Session-Pfad)
- **Task 34:** Admin-Web-Konsole (`web/`), NS8 Cockpit-UI (`ui/`) & Universal-Binary CI/CD (`macos-latest` + `lipo`)
- **Finaler Checkpoint:** Alle Backend-, Frontend- und Rust-Tests grün (inkl. nativem macOS-CI-Testlauf) und Release `v1.2.0` bereit

## Risiken & Gegenmaßnahmen (Phase 8)
| Risiko | Auswirkung | Gegenmaßnahme |
|---|---|---|
| **Homebrew verweigert Ausführung unter `root` (`LaunchDaemon`)** | Hoch | Automatische Ermittlung des Besitzers der Homebrew-Installation (`/opt/homebrew` bzw. `/usr/local/Homebrew`) oder des aktiven Konsolenbenutzers und Ausführung via `sudo -u <user> -H` mit `NONINTERACTIVE=1`. |
| **Fehlendes `/run`-Verzeichnis unter macOS bricht lokalen IPC-Socket** | Kritisch | `default_ipc_address()` liefert unter `target_os = "macos"` `/var/run/ns8-swm/ipc.sock`; `daemon.rs` legt `/var/run/ns8-swm` mit `0755` und Socket `0666` an. |
| **Unterschiedliches Ausgabeformat von `softwareupdate -l` zwischen macOS-Versionen** | Mittel | Robuster zweistufiger Parser für sowohl das moderne Schlüssel-Wert-Format (`* Label:` / `Title: ..., Version: ..., Action: restart`) als auch ältere Zeilenformate, abgesichert durch Unit-Tests mit realen macOS-Fixtures. |
| **Hängende `.dmg`-Mounts bei fehlerhaften Installern** | Mittel | Mount immer read-only und unsichtbar (`hdiutil attach -nobrowse -readonly -mountpoint <tempdir>`) mit garantiertem `hdiutil detach -force` im Cleanup-Pfad (Drop/Finally). |
