# NS8-SoftwareManager – Konzept, Anforderungen & Umsetzungsstand (`v1.2.0`)

## 1. Zielsetzung & Vision

**NS8-SoftwareManager** ist der zentrale **Endpoint-, Betriebssystem-Patch- und Self-Service-Software-Management-Dienst für NethServer 8 (NS8)**, mit dem sich **Windows-**, **Linux-** und **macOS-Systeme** (Workstations, Laptops & Headless-Server) aus einer einzigen Oberfläche heraus verwalten lassen.

Als Vorbild dient das Prinzip von **Microsoft Endpoint Configuration Manager (MECM) + Software Center** bzw. **Intune Company Portal**:
- **Für IT-Administratoren:** Zentrale Steuerung von Betriebssystem-Updates, Software-Rollouts, Wartungsfenstern, Neustart- und Kiosk-Richtlinien, Echtzeit-Fernbefehlen (Live-Sync, Desktop-Nachrichten, Remote-Reboot) sowie Flotten-Compliance.
- **Für Endanwender:** Ein lokales, gegen versehentliches Beenden gehärtetes **„NS8 Software Center“** auf dem Desktop, über das freigegebene Programme **ohne lokale Administratorrechte** per Klick installiert oder aktualisiert werden können – inklusive transparenter Update-/Neustart-Hinweise und integriertem Helpdesk-Diagnosepanel.

---

## 2. Marktanalyse & Architekturentscheidung für NethServer 8

| Lösung | Stärken | Hürden im NS8-Kontext |
|---|---|---|
| **Microsoft MECM / Intune** | Goldstandard beim Self-Service-Katalog (*Verfügbar* vs. *Pflicht*), Wartungsfenstern, Detection Rules und Reboot-Steuerung. | Proprietär, laufende Lizenzkosten, keine schlanke On-Prem-Lösung auf NethServer 8. |
| **WAPT / opsi** | Ausgereifte Software-Verteilung inkl. Self-Service-Kiosk und Paket-Repository. | Schwergewichtige Server-Architektur; Linux-/macOS-Clients oder Update-Steuerung teils kostenpflichtigen Enterprise-Editionen vorbehalten. |
| **OpenUEM** | Nutzt native Paketmanager (`winget`, `apt`, `dnf`, `flatpak`). | NATS + eigene mTLS/OCSP-Ports passen schlecht hinter Rootless NS8-Traefik; kein klassisches Enduser-„Software Center“. |
| **PatchMon** | Schlanker Outbound-Agent, stark bei OS-Patching. | Reines Admin-Patch-Tool ohne Software-Katalog und ohne Endbenutzer-Self-Service. |

### Umgesetzte NS8-Architektur
1. **Reiner Rust-Client ohne Laufzeitabhängigkeiten (`swm-agent` + Single-Binary `swm-center`):** Kompiliert mit `rustls` (keine OpenSSL-Abhängigkeit) für Windows (`x86_64`), Linux (`x86_64`) und **macOS Universal (`arm64` Apple Silicon + `x86_64` Intel via `lipo`)**.
2. **Reine Outbound-Kommunikation über Port 443 (`HTTPS / WSS`):** Funktioniert ohne eingehende Firewall-Freigaben hinter NS8-Traefik (automatisches Let's Encrypt TLS) im Büro, VPN und Homeoffice.
3. **Hybrides Paketmodell:** Native Paketmanager (`winget`, `apt`, `dnf`, `flatpak`, `brew`, `brew_cask`) kombiniert mit einem eigenen chunked NS8-Paket-Repository (`.msi`, `.exe`, `.zip`, `.ps1`, `.deb`, `.rpm`, `.pkg`, `.dmg`, `.sh`) inkl. `HTTP 206 Range`-Downloads.
4. **Zwei-Oberflächen-Prinzip:** Reines NS8-Cockpit-Modul (`ui/`, `core.css`, `DE`/`EN`) für Modul-Routing, LDAP/AD-Bindung und Status, plus eigenständige Vue-3-Admin-Konsole (`web/`) unter eigener Subdomain (z. B. `https://swm.firma.de`).

---

## 3. Umgesetzter Funktionsumfang (Stand `v1.2.0`)

### M1 – Plattformübergreifender Agent, Sicherheit & Massen-Rollout (Erledigt)
- **Hintergrund-Dienste mit Selbstheilung:**
  - **Windows (10/11, Server 2019/2022/2025):** Dienst `NS8SoftwareManagerAgent` (`NT AUTHORITY\SYSTEM`) mit automatischer Windows-SCM-Crash-Recovery (`5s/10s/30s`).
  - **Linux (Debian/Ubuntu/Mint, RHEL/Rocky/Alma/Fedora):** `systemd`-Unit `swm-agent.service` (`Restart=always`), zusätzlich als `.deb`- und `.rpm`-Paket (`nFPM`).
  - **macOS (Apple Silicon `M1–M4` & Intel):** `LaunchDaemon` `/Library/LaunchDaemons/org.nethserver.swm-agent.plist` (`KeepAlive = true`, Gatekeeper-Quarantäne wird beim Root-Rollout automatisch bereinigt).
- **Geführter 5-Wege-Rollout-Assistent (`EnrollmentView.vue`):**
  - Zeit- und nutzungsbegrenzte Enrollment-Tokens (*Auto-Approve* oder *Pending Approval*).
  - Fertige Skript-Generatoren (`skip_if_enrolled=1`) für **Einzelplatz-Einzeiler**, **Active Directory GPO / Intune (`install-swm-gpo.ps1`)**, **Ansible / Cloud-Init (`deploy-swm-agent.yml`)**, **macOS MDM für Jamf / Intune / Mosyle (`install-swm-mdm-macos.sh`)** und **Gold-Image CLI**.
- **Kryptografisches Sicherheitsmodell:**
  - Kanonische `Ed25519`-Signaturprüfung (`SignedJob` & `SignedCatalogItem`) + `SHA-256`-Prüfsummenvalidierung + Binärheader-Prüfung (`MZ` / `ELF` / `Mach-O Universal`) beim zentralen Agent-Self-Update.

### M2 – Inventarisierung, Gruppen & Echtzeit-Flottensteuerung (Erledigt)
- **Vollständiges System- & Software-Inventar:** Hardware, OS/Build, IP-Adressen, angemeldete Benutzer, Live-Tray-Status (`center_running`), `Pending Reboot` und Delta-Software-Inventar über Windows, Linux und macOS.
- **Echtzeit-Fernbefehle & Bulk-Steuerung:**
  - `Ed25519`-signierte Sofortjobs über WebSocket für **Sofort-Sync** (`force_sync`), **Desktop-Benachrichtigungen** (`admin_notify` mit Toast + quittierbarem Banner) und **Remote-Reboot** (`grace_period`, `immediate`, `cancel`).
  - Mehrfachauswahl-Aktionsleiste (`bulk-action`) für Sammel-Freigabe, Sync, Update-Scan, Gruppenzuweisung und Rundnachrichten.

### M3 – Betriebssystem-Patch-Management (Erledigt)
- **Windows:** Native **WUA-API** ohne WSUS-Zwang inkl. KB-Ausschlussliste.
- **Linux:** Unbeaufsichtigtes Patching via **APT** und **DNF** (*Sicherheitsupdates* oder *Alle Pakete*).
- **macOS:** Offizielle Apple-System- und Sicherheitsupdates via **`softwareupdate -l` / `softwareupdate -i`** inkl. Label-Ausschlussliste und Neustart-Erkennung.
- **Wartungsfenster & Reboot-Richtlinien:** Zeitfenster pro Gruppe inkl. *Catch-up* für Laptops sowie Gruppenrichtlinien (*Nie*, *Im Wartungsfenster*, *Benutzer-Karenzzeit mit Snooze & Countdown*).

### M4 – Software-Katalog, Paket-Repository & Detection Rules (Erledigt)
- **Quellen:** `winget --scope machine`, `apt`, `dnf`, `flatpak`, `brew` & `brew_cask` (ausgeführt unter dem unprivilegierten Homebrew-Eigentümer) sowie eigenes NS8-Repository (`.msi`, `.exe`, `.zip`, `.ps1`, `.deb`, `.rpm`, `.pkg`, `.dmg`, `.sh`).
- **Erkennungsregeln & Prozessschutz:** `file`, `registry`, `package`, `app_bundle` (`/Applications/*.app` `Info.plist`), `pkgutil`, `brew`/`brew_cask` sowie `blocked_processes`.

### M5 – Gehärtetes Endbenutzer „NS8 Software Center“ (`swm-center`) (Erledigt)
- **Single-File-Binary** mit eingebetteter Vue-3-UI (Light & Dark Mode) und 4 Reitern (*Katalog*, *Updates*, *Verlauf*, *System & Diagnose*).
- **Manipulationsschutz im Unternehmensbetrieb:**
  - `enforce_tray = true` (Standard) blendet „Beenden“ im Tray aus; Single-Instance-Guard verhindert doppelte Tray-Prozesse.
  - Automatischer Session-Watchdog startet `swm-center` nach Prozess-Kill neu (Windows `\NS8SoftwareCenterGuardian`, Linux `systemd --user`, macOS Aqua `LaunchAgent` + `launchctl kickstart`).
  - Gruppenbasierter **Managed-Kiosk-Modus** (`allow_self_service = false`, `allow_user_os_updates = false`, `silent_notifications = true`), serverseitig an der lokalen IPC-Schnittstelle von `swm-agent` durchgesetzt.

---

## 4. Ausblick / Optionale spätere Erweiterungen (Backlog)

Folgende Themen sind bewusst nicht Teil von `v1.2.0` und können bei Bedarf in künftigen Releases ergänzt werden:
- **Mehrsprachigkeit (`i18n` `DE`/`EN`/weitere) für `web/` und `swm-center`:** Das NS8-Cockpit-Modul (`ui/`) ist bereits zweisprachig (`DE`/`EN`); Admin-Konsole und Desktop-Client können optional um einen Sprachumschalter ergänzt werden.
- **Dynamische AD-/LDAP-Benutzergruppen-Zuweisung (User-Targeting):** Ergänzend zur bestehenden Geräte- und Gerätegruppen-Zuweisung.
- **Genehmigungs-Workflows (Approval Requests):** Anforderungs- und Freigabeprozess für lizenzpflichtige Software im Software Center.
- **CVE-/Schwachstellen-Abgleich:** Automatischer Abgleich des Software-Inventars gegen NVD/OSV-Feeds.
- **Apple Developer ID Notarizing:** Optionale Integration von Apple-Signaturzertifikaten in GitHub Secrets für manuelle Doppelklick-Installationen außerhalb des Root-/MDM-Bootstraps.
