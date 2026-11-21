# NS8-SoftwareManager

**NS8-SoftwareManager** ist ein natives, zentrales **Endpoint-, Betriebssystem-Patch- und Self-Service-Software-Management-Modul für NethServer 8 (NS8)** zur einheitlichen Verwaltung von **Windows-** und **Linux-Systemen** (Workstations & Server).

Inspiriert von *Microsoft Endpoint Configuration Manager (MECM) + Software Center* kombiniert das Modul eine zentrale Web-Verwaltungskonsole für IT-Administratoren mit einem lokalen **Software Center** auf den Endgeräten, über das freigegebene Anwendungen **ohne lokale Administratorrechte** installiert, aktualisiert oder (sofern erlaubt) deinstalliert werden können.

---

## Funktionsumfang

- **Plattformübergreifendes Flotten- & Inventar-Management:**
  - Automatische Erfassung von Hardware (CPU, RAM, Datenträger), Betriebssystem/Kernel/Build, IP-Adressen, angemeldeten Benutzern, **Pending-Reboot-Status** sowie sämtlicher installierter Software (Delta-Synchronisation).
  - Echtzeit-Online-Status und Live-Log-Streaming über ausgehende WebSockets (`WSS` über Port `443` hinter NS8-Traefik) inklusive automatischem HTTPS-Polling-Fallback – ideal für LAN, VPN und Homeoffice ohne eingehende Firewall-Freigaben auf den Clients.
  - Gerätegruppen, Tags und mehrstufige Rollouts (*Pilot-Ring*, *Workstations*, *Server*).
- **Moderne Admin-Web-Konsole (`web/`):**
  - **Light- & Dark-Mode:** Umschaltbares helles und dunkles Erscheinungsbild (persistent in `localStorage` gespeichert + automatische `prefers-color-scheme`-Erkennung) mit kontraststarker Typografie, einheitlichem Markenlogo und nativen Windows-/Linux-Plattform-Badges.
  - **Responsive Navigation & Barrierefreiheit:** Einklappbare Seitenleiste am Desktop, responsiver Navigations-Drawer für Mobilgeräte/Tablets, vollständige Tastaturbedienbarkeit (`Escape` schließt Dialoge, `aria-label` & Fokus-Management).
  - **Sichere Bestätigungsdialoge & Live-Toasts:** Schutz vor versehentlichen destruktiven Aktionen (*Gerät sperren/löschen*, *Paket löschen*, *Bereitstellung aufheben*, *Token widerrufen*) über einen zentralen Bestätigungsdialog sowie nicht-intrusive Toast-Benachrichtigungen.
- **Betriebssystem-Update-Management (OS Patching):**
  - **Windows (10/11, Server 2019/2022/2025):** Direkte Ansteuerung des nativen *Windows Update Agent (WUA)* ohne WSUS-Zwang inkl. Schweregrad-Filter und zentraler KB-Ausschlussliste.
  - **Linux (Debian/Ubuntu/Mint via `apt`, RHEL/Rocky/Alma/Fedora via `dnf`):** Wahlweise reine Sicherheitsupdates oder vollständige Paket-Upgrades im garantiert unbeaufsichtigten Modus (`noninteractive`).
  - **Wartungsfenster & Neustart-Richtlinien:** Zeitfenster pro Gruppe (inkl. *Catch-up* für ausgeschaltete Laptops) sowie gesteuerte Reboot-Policies (*Nie automatisch neu starten*, *Automatisch im Wartungsfenster*, *Benutzer-Karenzzeit mit Snooze & Countdown*).
- **Hybrider Software-Katalog & Eigenes Paket-Repository:**
  - **Native Paketmanager:** `winget --scope machine` (Session-0/`SYSTEM`-sicher aufgelöst), `apt`, `dnf` und `flatpak`.
  - **Eigenes NS8-Paket-Repository:** Chunked-Upload für große eigene Installer (`.msi`, `.exe`, `.zip`, `.deb`, `.rpm`, `.ps1`, `.sh`), unterbrechungssicherer `HTTP Range`-Download (`206 Partial Content`), *Detection Rules*, Prozess-Sperrprüfung (`blocked_processes`) und Exit-Code-Mapping (`0`, `3010`, `1641`).
  - **Bereitstellungs-Modi:** *Pflicht (Required)*, *Verfügbar (Available / Self-Service)* und *Deinstallieren (Uninstall)* inkl. automatischer Drittanbieter-Updates.
- **Single-Binary Endbenutzer „Software Center“ & System-Tray (`swm-center`):**
  - **Einzeldatei-Bereitstellung (Zero External Assets):** Die komplette Vue-3-Oberfläche des Software Centers (`agent/crates/swm-center/ui/dist`) wird zur Compile-Zeit direkt in das `swm-center`-Binary eingebettet (`SWM_CENTER_UI_DIR` bleibt optional für Live-Entwicklung nutzbar). Dadurch lässt sich `swm-center.exe` bzw. `swm-center` ohne externe Asset-Ordner auf jedem Arbeitsplatz ausführen.
  - **Natives System-Tray & Desktop-Benachrichtigungen:** Echtes Windows-System-Tray-Icon (`Shell_NotifyIconW` mit Rechtsklick-Schnellmenü für *Software Center öffnen*, *Nach Updates suchen* und *Neustart verschieben*) sowie native Betriebssystem-Benachrichtigungen (WinRT Toast unter Windows, `notify-send` / XDG-Autostart unter Linux) inklusive Light-/Dark-Mode.
- **Vollständige NethServer-8-Integration:**
  - Rootless Podman-Pod (`swm.service`, `swm-db.service` mit PostgreSQL 16, `swm-app.service`) strikt gebunden an `127.0.0.1:${TCP_PORT}:8080` hinter NS8-Traefik (inkl. Let's Encrypt).
  - Offizielle NS8 Cluster-Admin Cockpit-UI (`de` & `en`), optionale Anbindung an NS8-Benutzerdomänen (Samba Active Directory / OpenLDAP) sowie Restic-Backup, Restore und Clone.

---

## Architektur & Projektstruktur

```text
NS8-SoftwareManager/
├── Containerfile                  # Rootless NS8-Modul-Image (imageroot + Cockpit-UI)
├── Containerfile.app              # Multi-Stage App-Image (FastAPI + Admin-SPA + eingebettete Linux/Win-Agenten)
├── build-images.sh                # Dynamisches Build- & Push-Skript (automatischer GHCR-Namespace)
├── CONTRACT.md                    # Verbindlicher API-, WebSocket- & Ed25519-Signaturvertrag
├── REQUIREMENTS.md                # Fachliche Anforderungsanalyse & Architekturkonzept
├── server/                        # Zentrales Python FastAPI Backend (SQLAlchemy 2 async + Scheduler)
├── web/                           # Dediziertes Admin-Web-Portal (Vue 3 + TypeScript + Vite + TailwindCSS)
├── agent/                         # Plattformübergreifender Rust-Workspace
│   ├── crates/
│   │   ├── swm-common/            # Gemeinsame DTOs, Ed25519/SHA-256-Prüfung & lokales IPC-Protokoll
│   │   ├── swm-agent/             # Privilegierter Hintergrunddienst (Windows Service / systemd)
│   │   └── swm-center/            # Single-Binary Desktop- & Tray-App („Software Center“ + eingebettete Vue-3-UI)
│   └── packaging/                 # Bootstrap-Skripte, systemd-Units, XDG-Autostart, WiX (.wxs) & nFPM (.deb/.rpm)
├── imageroot/                     # NS8 Modul-Actions, Systemd-User-Units & Restic-Backup-Hooks
├── ui/                            # NS8 Cluster-Admin Cockpit-UI (Vue 2 + @nethserver/ns8-ui-lib)
└── prebuilt-agents/windows/       # Staging-Ordner für vorab gebaute Windows-Binaries (.exe) im Image-Build
```

### Sicherheitsmodell

1. **Kryptografische Auftrags- & Katalog-Signierung (`Ed25519`):** Der Server erzeugt beim Erststart ein persistentes `Ed25519`-Schlüsselpaar (`/data/keys/`). Jeder Auftrag (`SignedJob`) und jeder Self-Service-Katalogeintrag (`SignedCatalogItem`) wird kanonisch signiert und vom `swm-agent` vor jeder Ausführung gegen den hinterlegten Server-Public-Key sowie das Ablaufdatum (`expires_at_unix`) geprüft.
2. **Verpflichtende `SHA-256`-Prüfung:** Jedes vom Server geladene Installationspaket oder Agent-Update wird vor der Ausführung vollständig gegen seinen `SHA-256`-Hash verifiziert.
3. **Strikte Privilegientrennung (Local IPC):** `swm-center` läuft ausschließlich mit normalen Benutzerrechten und spricht über eine abgesicherte lokale Schnittstelle (Windows Named Pipe `\\.\pipe\ns8-swm-ipc` bzw. Linux Unix Socket `/run/ns8-swm/ipc.sock`) mit `swm-agent`. Freie Befehlszeilen oder fremde URLs können strukturell nicht übergeben werden – erlaubt ist nur das Auslösen bereits vom Server `Ed25519`-signierter Paket-IDs.

---

## Installation auf NethServer 8 (inkl. eigener GHCR-Namespace)

### 1. Modul auf dem NS8-Cluster installieren

Auf dem NS8-Leader- oder Ziel-Knoten als `root` (Standard-Namespace oder eigener GitHub-Account / eigene Organisation in Kleinbuchstaben):

```bash
# Standard-Installation (BLS-ISP GHCR-Repository):
add-module ghcr.io/bls-isp/softwaremanager:latest 1

# Oder eine gezielte Release-Version:
add-module ghcr.io/bls-isp/softwaremanager:v1.0.0 1
```

### 2. Instanz im NS8 Cluster-Admin konfigurieren

1. Im **NS8 Cluster-Admin** die installierte **SoftwareManager**-Instanz öffnen und zu **Einstellungen** wechseln.
2. **SoftwareManager-Hostname (FQDN)** eintragen (z. B. `swm.firma.de`) sowie optional **Let's-Encrypt-Zertifikat** und **HTTP-zu-HTTPS-Weiterleitung** aktivieren.
3. Optional eine **NS8-Benutzerdomäne (Active Directory / OpenLDAP)** für die zentrale Administrator-Anmeldung auswählen und bei Bedarf das Passwort des lokalen Fallback-Administrators setzen.
4. Auf **Konfiguration speichern** klicken und anschließend über **SoftwareManager-Admin-Konsole öffnen** (`https://swm.firma.de`) das zentrale Verwaltungsportal starten.

### 3. Backup, Restore & Clone

Das Modul integriert sich vollständig in das NS8-Restic-Backup (`imageroot/etc/state-include.conf` & `module-dump-state`): Gesichert werden der konsistente PostgreSQL-Datenbank-Dump, die Modul-Umgebungsvariablen, die `Ed25519`-Signaturschlüssel sowie alle eigenen hochgeladenen Software-Pakete im Volume `swm-app-data`.

---

## Client-Onboarding (Windows & Linux)

1. In der **SoftwareManager-Admin-Konsole** unter **Client-Enrollment** ein neues Enrollment-Token anlegen (optional mit Ablaufzeit, maximaler Nutzungsanzahl, Zielgruppe und direkter Auto-Freigabe oder *Pending Approval*).
2. Den angezeigten Bootstrap-Einzeiler auf dem Zielsystem mit Administrator- bzw. `root`-Rechten ausführen:

### Windows (PowerShell als Administrator)

Workstation (inkl. Windows-Dienst `swm-agent.exe` + Single-Binary System-Tray **Software Center** `swm-center.exe` im Autostart):

```powershell
irm "https://swm.firma.de/dl/install.ps1?token=<ENROLLMENT_TOKEN>&headless=0" | iex
```

Windows-Server (Headless – nur Hintergrunddienst `swm-agent.exe`):

```powershell
irm "https://swm.firma.de/dl/install.ps1?token=<ENROLLMENT_TOKEN>&headless=1" | iex
```

### Linux (Debian/Ubuntu/Mint & RHEL/Rocky/Alma/Fedora als `root`)

Linux-Desktop (inkl. `swm-center` & XDG-Autostart):

```bash
curl -fsSL "https://swm.firma.de/dl/install.sh?token=<ENROLLMENT_TOKEN>&headless=0" | sudo bash
```

Linux-Server (Headless – nur `systemd`-Dienst `swm-agent.service`):

```bash
curl -fsSL "https://swm.firma.de/dl/install.sh?token=<ENROLLMENT_TOKEN>&headless=1" | sudo bash
```

Alternativ erzeugt die GitHub-Actions-Pipeline bei jedem Lauf fertige **`.deb`- und `.rpm`-Client-Pakete** (`ns8-swm-agent`) über `nFPM` (`agent/packaging/nfpm.yaml`) sowie kompilierte Windows-Binaries als Download-Artefakte.

---

## Automatisierter CI/CD- & Release-Tag-Workflow (`.github/workflows/build-and-test.yml`)

Das Projekt bringt eine vollständige GitHub-Actions-Pipeline mit, die ohne manuelle Skript-Anpassungen in jedem GitHub-Account oder Organisations-Fork funktioniert:

1. **Dynamischer GHCR-Namespace (`REPOBASE`):**
   `build-images.sh` ermittelt automatisch `REPOBASE="${REPOBASE:-ghcr.io/${GITHUB_REPOSITORY_OWNER:-bls-isp}}"` (automatisch in Kleinbuchstaben konvertiert). Das Repository `github.com/BLS-ISP/ns8-softwaremanager` publiziert seine Images direkt nach `ghcr.io/bls-isp/softwaremanager` und `ghcr.io/bls-isp/softwaremanager-app`.
2. **Garantierte Embedded-UI-Reihenfolge:**
   Sowohl in der CI-Matrix als auch im mehrstufigen `Containerfile.app` wird zuerst `agent/crates/swm-center/ui` gebaut und vor `cargo build --release --workspace` nach `agent/crates/swm-center/ui/dist` kopiert, sodass `swm-center` garantiert die aktuelle Vue-3-Oberfläche einbettet (zusätzlich liegt `/app/agents/linux/swm-center-ui` als Backup im App-Container).
3. **Client-Artefakte (`.deb`, `.rpm`, `.exe`):**
   Jeder Workflow-Lauf kompiliert `swm-agent` und `swm-center` für Linux (`x86_64`) und Windows (`x86_64`), baut über `nFPM` installierbare `.deb`- und `.rpm`-Pakete und stellt alle Pakete als GitHub-Actions-Artefakte bereit (`linux-client-packages-deb-rpm`, `agents-linux-amd64`, `agents-windows-amd64`).
4. **Automatische GHCR-Veröffentlichung per Versions-Tag (`v*`):**
   Wird ein Versions-Tag der Form `v*` gepusht, meldet sich der Job `package-ns8-images` automatisch mit `GITHUB_TOKEN` an `ghcr.io` an und veröffentlicht sowohl den Versions-Tag (z. B. `:v1.0.0`) als auch `:latest`:

   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

   Anschließend ist das Modul sofort auf jedem NS8-Cluster installierbar via:

   ```bash
   add-module ghcr.io/bls-isp/softwaremanager:v1.0.0 1
   # bzw.
   add-module ghcr.io/bls-isp/softwaremanager:latest 1
   ```

---

## Lokale Entwicklung, Tests & Image-Build

### 1. Backend & NS8-Regel-Tests (`Python 3.12+`)

```bash
python -m pip install -r server/requirements.txt
python -m pytest server/tests -v
python -m compileall server/app imageroot
```

Lokaler Entwicklungsstart des Servers:

```bash
cd server
uvicorn app.main:app --reload --host 127.0.0.1 --port 8080
```

### 2. Admin-Web-Konsole (`Node.js 20+`)

```bash
npm --prefix web install
npm --prefix web test -- --run
npm --prefix web run build
```

### 3. NS8 Cluster-Admin Cockpit-UI (`Node.js 20+`)

```bash
npm --prefix ui install --legacy-peer-deps
npm --prefix ui run build
```

### 4. Rust-Agent & Software Center (`Rust 1.85+` & `Node.js 20+`)

```bash
npm --prefix agent/crates/swm-center/ui install
npm --prefix agent/crates/swm-center/ui test -- --run
npm --prefix agent/crates/swm-center/ui run build

cargo test --manifest-path agent/Cargo.toml --workspace
cargo build --release --manifest-path agent/Cargo.toml --workspace
```

### 5. Container-Images lokal bauen (`Podman` / `Docker`)

Um bei einem lokalen Build optional auch vorab kompilierte Windows-Binaries (`swm-agent.exe`, `swm-center.exe`) in das App-Image einzubetten, diese zuvor nach `prebuilt-agents/windows/` kopieren (in GitHub Actions geschieht dies automatisch aus dem `agents-windows-amd64`-Artefakt):

```bash
REPOBASE=ghcr.io/mein-namespace IMAGETAG=latest bash ./build-images.sh
```

Dies erzeugt beide Container-Images:
- `ghcr.io/mein-namespace/softwaremanager-app:latest` (FastAPI-Server + gebaute Vue-3-Admin-Konsole + Linux- & Windows-Client-Binaries)
- `ghcr.io/mein-namespace/softwaremanager:latest` (Rootless NS8-Modul-Image mit `imageroot/` und gebauter Cockpit-UI `ui/`)

---

## Lizenz

Veröffentlicht unter der **GPL-3.0-or-later** Lizenz.
