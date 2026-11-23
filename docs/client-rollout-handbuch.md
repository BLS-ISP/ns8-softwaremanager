# Praxishandbuch Client-Rollout & Massenbereitstellung (`v1.2.0`)

Dieses Handbuch zeigt Schritt für Schritt, wie Sie den **NS8-SoftwareManager Agent (`swm-agent`)** und das **NS8 Software Center (`swm-center`)** sowohl auf einzelnen Testgeräten als auch vollautomatisch auf hunderten **Windows-**, **Linux-** und **macOS-Rechnern** ausrollen.

---

## Inhaltsverzeichnis

1. [Vorbereitung: Netzwerk & Enrollment-Token anlegen](#1-vorbereitung-netzwerk--enrollment-token-anlegen)
2. [Szenario A: Einzelplatz-Schnellinstallation (Windows, Linux & macOS)](#2-szenario-a-einzelplatz-schnellinstallation-windows-linux--macos)
3. [Szenario B: Windows-Flotte über Active Directory GPO](#3-szenario-b-windows-flotte-über-active-directory-gpo)
4. [Szenario C: Windows & macOS über Microsoft Intune](#4-szenario-c-windows--macos-über-microsoft-intune)
5. [Szenario D: macOS-Flotte über Jamf Pro, Kandji oder Mosyle](#5-szenario-d-macos-flotte-über-jamf-pro-kandji-oder-mosyle)
6. [Szenario E: Linux-Flotte über Ansible, Cloud-Init & `.deb`/`.rpm`](#6-szenario-e-linux-flotte-über-ansible-cloud-init--debrpm)
7. [Szenario F: Gold-Images & VM-Vorlagen (Wichtige Klon-Regel!)](#7-szenario-f-gold-images--vm-vorlagen-wichtige-klon-regel)
8. [Saubere Deinstallation eines Clients](#8-saubere-deinstallation-eines-clients)

---

## 1. Vorbereitung: Netzwerk & Enrollment-Token anlegen

### Netzwerk-Voraussetzungen
- Verwaltete Clients benötigen **ausschließlich ausgehenden HTTPS/WSS-Zugriff (TCP Port 443)** zur konfigurierten Server-URL (z. B. `https://swm.firma.de`).
- Auf den Clients müssen **keinerlei eingehende Ports** (kein SSH, kein WinRM, kein SMB) geöffnet werden.
- Das TLS-Zertifikat des Servers muss auf den Clients vertrauenswürdig sein (automatisch erfüllt bei **Let's Encrypt**; bei eigener interner CA muss das Root-Zertifikat auf den Clients ausgerollt sein).

### Enrollment-Token erstellen
1. Öffnen Sie in der Web-Konsole **Client-Rollout & Agent**.
2. Wählen Sie oben rechts den Zieltyp:
   - **Desktop (+ Software Center)** (`headless=0`): Für Arbeitsplatzrechner, Laptops, iMacs & MacBooks. Installiert sowohl den Hintergrunddienst als auch das gehärtete Endbenutzer-Software-Center.
   - **Headless-Server** (`headless=1`): Für Windows- und Linux-Server ohne grafische Benutzersitzung. Installiert ausschließlich `swm-agent`.
3. Klicken Sie auf **Neues Token erstellen** (oder beim Erststart auf *1-Klick-Schnellstart-Token*):
   - Für einen automatisierten **GPO-, Ansible- oder MDM-Massenrollout** empfehlen sich: eine passende **Ziel-Gerätegruppe** (z. B. *Büro-Workstations*), eine ausreichende Nutzungsanzahl (z. B. `500`) und **Auto-Approve = Aktiv**, damit neue Firmenrechner sofort verwaltet werden.

---

## 2. Szenario A: Einzelplatz-Schnellinstallation (Windows, Linux & macOS)

Wählen Sie in Ihrer Token-Karte den ersten Reiter **Einzelplatz / Schnelltest (1-Zeiler)**:

### Windows 10 / 11 / Server (PowerShell „Als Administrator ausführen“)
```powershell
irm 'https://swm.firma.de/dl/install.ps1?token=<IHR_TOKEN>&headless=0' | iex
```
**Was dabei automatisch passiert:**
1. Legt `C:\Program Files\NS8-SoftwareManager` und `C:\ProgramData\NS8-SoftwareManager` (geschützte ACLs) an.
2. Lädt `swm-agent.exe` und (bei `headless=0`) `swm-center.exe` herunter.
3. Registriert das Gerät am Server, richtet den Dienst `NS8SoftwareManagerAgent` inkl. automatischer Windows-SCM-Crash-Recovery (`5s/10s/30s`) ein und startet ihn.
4. Richtet bei `headless=0` die Startmenü-Verknüpfung **NS8 Software Center**, den Autostart sowie den geplanten System-Task `\NS8SoftwareCenterGuardian` ein und startet das Tray-Icon sofort in der laufenden Benutzersitzung (kein Ab-/Anmelden nötig).

### Linux & macOS Universal (Terminal mit `sudo`)
Derselbe Befehl funktioniert sowohl unter **Linux** als auch unter **macOS (Apple Silicon M1–M4 & Intel)**, da `/dl/install.sh` das Betriebssystem per `uname -s` automatisch erkennt:

```bash
curl -fsSL 'https://swm.firma.de/dl/install.sh?token=<IHR_TOKEN>&headless=0' | sudo bash
```
- **Unter Linux:** Richtet `/usr/local/bin/swm-agent`, `/etc/systemd/system/swm-agent.service` und (bei `headless=0`) `/etc/systemd/user/ns8-swm-center.service` (`Restart=always`) + Anwendungsmenü-Eintrag ein.
- **Unter macOS:** Lädt die Universal Binaries (`arm64` + `x86_64`), entfernt etwaige Gatekeeper-Quarantäne-Flags (`xattr -dr com.apple.quarantine`), richtet `/Library/LaunchDaemons/org.nethserver.swm-agent.plist` sowie (bei `headless=0`) `/Applications/NS8 Software Center.app` + `/Library/LaunchAgents/org.nethserver.swm-center.plist` ein und startet die App sofort in der aktiven macOS-Benutzersitzung.

---

## 3. Szenario B: Windows-Flotte über Active Directory GPO

Für Domänen-PCs erzeugt der Reiter **Windows Flotte (AD-GPO / Intune)** ein idempotentes Startskript (`install-swm-gpo.ps1`). Ist der Agent auf einem PC bereits installiert, beendet sich das Skript innerhalb von Millisekunden, ohne erneut Dateien herunterzuladen (`skip_if_enrolled=1`).

### Schritt-für-Schritt in der Gruppenrichtlinienverwaltung (`gpmc.msc`)
1. Laden Sie im Reiter **Windows Flotte (AD-GPO / Intune)** die Datei **`install-swm-gpo.ps1`** herunter.
2. Öffnen Sie auf Ihrem Domänencontroller / Admin-PC `gpmc.msc`, erstellen Sie ein neues GPO (z. B. `Rollout - NS8 SoftwareManager Agent`) und verknüpfen Sie es mit der gewünschten Computer-OU (z. B. `OU=Workstations`).
3. Bearbeiten Sie das GPO und navigieren Sie zu:
   **Computerkonfiguration -> Richtlinien -> Windows-Einstellungen -> Skripts (Start/Herunterfahren) -> Starten** *(wichtig: Computer-Startskript, NICHT Benutzer-Anmeldeskript, damit es unter `NT AUTHORITY\SYSTEM` läuft!)*.
4. Wechseln Sie oben auf den Reiter **PowerShell-Skripts**, klicken Sie auf **Dateien anzeigen…**, kopieren Sie `install-swm-gpo.ps1` in diesen SYSVOL-Ordner und fügen Sie es über **Hinzufügen…** ohne weitere Parameter hinzu.
5. Beim nächsten Neustart (oder Test per `gpupdate /force` + Reboot) registrieren sich alle Domänen-PCs automatisch.

---

## 4. Szenario C: Windows & macOS über Microsoft Intune

### Windows über Intune
1. Öffnen Sie das **Microsoft Intune Admin Center -> Devices -> Windows -> Scripts and remediations -> Platform scripts -> Add**.
2. Laden Sie `install-swm-gpo.ps1` hoch und setzen Sie exakt diese drei Optionen:
   - **Run this script using the logged on credentials:** `No` *(führt als `SYSTEM` aus)*
   - **Enforce script signature check:** `No`
   - **Run script in 64 bit PowerShell host:** `Yes`
3. Weisen Sie das Skript Ihrer Gerätegruppe zu.

### macOS über Intune
1. Laden Sie im Reiter **macOS MDM (Jamf / Intune)** die Datei **`install-swm-mdm-macos.sh`** herunter (garantiert reine Unix-LF-Zeilenumbrüche).
2. Öffnen Sie **Devices -> macOS -> Shell scripts -> Add**.
3. Laden Sie `install-swm-mdm-macos.sh` hoch und konfigurieren Sie:
   - **Run script as signed-in user:** `No` *(führt als `root` aus)*
   - **Script frequency:** *Every 1 week* (dank Idempotenz-Prüfung auf `config.json` + `launchctl` wirkt dies zugleich als Selbstheilungs-Check).

---

## 5. Szenario D: macOS-Flotte über Jamf Pro, Kandji oder Mosyle

1. Laden Sie im Reiter **macOS MDM (Jamf / Intune)** die Datei **`install-swm-mdm-macos.sh`** herunter.
2. **In Jamf Pro:**
   - Navigieren Sie zu **Settings -> Computer Management -> Scripts -> + New**, vergeben Sie den Namen `Install NS8-SoftwareManager Agent` und fügen Sie den Inhalt von `install-swm-mdm-macos.sh` ein.
   - Erstellen Sie unter **Computers -> Policies -> + New** eine Richtlinie:
     - **Trigger:** `Enrollment Complete` + `Recurring Check-in`
     - **Execution Frequency:** `Once per computer` (oder `Once every week`, da das Skript idempotent prüft, ob `org.nethserver.swm-agent` bereits aktiv ist).
     - **Scope:** Ziel-Smart-Group Ihrer Macs (sowohl Intel- als auch Apple-Silicon-Macs nutzen dasselbe Universal-Skript).
3. **Hinweis zu macOS Hintergrundobjekten:** Unter macOS Ventura/Sonoma/Sequoia informiert macOS den Benutzer beim ersten Anlegen von `/Library/LaunchDaemons/org.nethserver.swm-agent.plist` über ein neues Hintergrundobjekt. In verwalteten MDM-Umgebungen können Sie optional ein *Service Management – Managed Login Items* Konfigurationsprofil für das Label-Präfix `org.nethserver.` ausrollen, um diesen Hinweis stummzuschalten.

---

## 6. Szenario E: Linux-Flotte über Ansible, Cloud-Init & `.deb`/`.rpm`

### Mit dem generierten Ansible-Playbook
1. Laden Sie im Reiter **Linux Flotte (Ansible / Cloud-Init)** die Datei **`deploy-swm-agent.yml`** herunter.
2. Führen Sie das Playbook gegen Ihr Ansible-Inventar aus:
   ```bash
   ansible-playbook -i inventory.ini deploy-swm-agent.yml --become
   ```

### Alternative Installation über `.deb`- oder `.rpm`-Paket
Jeder GitHub-Actions-Release-Lauf erzeugt fertige Linux-Pakete (`ns8-swm-agent_1.2.0_amd64.deb` und `.rpm`):

```bash
# Debian / Ubuntu / Linux Mint:
sudo apt install ./ns8-swm-agent_1.2.0_amd64.deb
sudo swm-agent enroll --server https://swm.firma.de --token <IHR_TOKEN>
sudo systemctl enable --now swm-agent.service

# RHEL / Rocky / AlmaLinux / Fedora:
sudo dnf install ./ns8-swm-agent-1.2.0-1.x86_64.rpm
sudo swm-agent enroll --server https://swm.firma.de --token <IHR_TOKEN>
sudo systemctl enable --now swm-agent.service
```

---

## 7. Szenario F: Gold-Images & VM-Vorlagen (Wichtige Klon-Regel!)

> **WICHTIG:** Führen Sie `swm-agent enroll` **niemals** innerhalb eines Master-Images / VM-Templates vor dem Versiegeln (`sysprep` / Klonen) aus! Andernfalls erhalten alle daraus geklonten VMs dieselbe `config.json` (`device_id` + `device_token`) und überschreiben sich gegenseitig in der Konsole.

### Korrektes Vorgehen für Master-Templates
1. **Im Master-Image nur die Binaries ablegen** (ohne `enroll` und ohne `config.json`):
   - Windows: `swm-agent.exe` & `swm-center.exe` nach `C:\Program Files\NS8-SoftwareManager\` kopieren.
   - Linux / macOS: Binaries nach `/usr/local/bin/` kopieren (oder `.deb`/`.rpm` ohne `enroll` installieren).
2. **Erst beim ersten Start des geklonten Systems** (*FirstBoot* / `SetupComplete.cmd` / `cloud-init` `runcmd`) das idempotente Bootstrap-Skript bzw. `swm-agent enroll` ausführen — so erhält jeder Klon seine eigene Hardware-UUID und sein eigenes Geräte-Token.

---

## 8. Saubere Deinstallation eines Clients

Falls ein Gerät dauerhaft außer Betrieb genommen wird, widerrufen/löschen Sie es in der Konsole unter **Geräte & Flotte** und entfernen Sie den Dienst auf dem Endgerät:

### Windows (PowerShell als Administrator)
```powershell
Unregister-ScheduledTask -TaskName 'NS8SoftwareCenterGuardian' -Confirm:$false -ErrorAction SilentlyContinue
Stop-Process -Name 'swm-center' -Force -ErrorAction SilentlyContinue
& 'C:\Program Files\NS8-SoftwareManager\swm-agent.exe' service-uninstall
Remove-Item -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' -Name 'NS8SoftwareCenter' -ErrorAction SilentlyContinue
Remove-Item -LiteralPath "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\NS8 Software Center.lnk" -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath 'C:\Program Files\NS8-SoftwareManager', 'C:\ProgramData\NS8-SoftwareManager' -Recurse -Force
```

### Linux (`root`)
```bash
sudo systemctl disable --now swm-agent.service
sudo pkill -x swm-center || true
sudo rm -f /etc/systemd/system/swm-agent.service \
           /etc/systemd/user/default.target.wants/ns8-swm-center.service \
           /etc/systemd/user/ns8-swm-center.service \
           /etc/xdg/autostart/ns8-swm-center.desktop \
           /usr/share/applications/ns8-swm-center.desktop \
           /usr/local/bin/swm-agent /usr/local/bin/swm-center
sudo rm -rf /etc/ns8-swm /run/ns8-swm
sudo systemctl daemon-reload
```

### macOS (`root`)
```bash
CONSOLE_UID=$(stat -f %u /dev/console 2>/dev/null || true)
[ -n "${CONSOLE_UID}" ] && sudo launchctl bootout "gui/${CONSOLE_UID}" /Library/LaunchAgents/org.nethserver.swm-center.plist 2>/dev/null || true
sudo launchctl bootout system /Library/LaunchDaemons/org.nethserver.swm-agent.plist 2>/dev/null || true
sudo pkill -x swm-center || true
sudo rm -rf "/Library/LaunchDaemons/org.nethserver.swm-agent.plist" \
            "/Library/LaunchAgents/org.nethserver.swm-center.plist" \
            "/Applications/NS8 Software Center.app" \
            "/Library/Application Support/NS8-SoftwareManager" \
            /var/run/ns8-swm /usr/local/bin/swm-agent /usr/local/bin/swm-center
```
