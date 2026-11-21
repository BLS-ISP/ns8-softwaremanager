# NS8-SoftwareManager – Recherche, Konzept & Erste Muss-Funktionen

## 1. Zielsetzung & Vision

Ziel ist der Aufbau eines zentralen **Endpoint-, Patch- und Software-Management-Dienstes für NethServer 8 (NS8)**, mit dem sich sowohl **Windows-** als auch **Linux-Systeme** (Workstations & Server) aus einer einzigen Oberfläche heraus verwalten lassen.

Als Vorbild dient insbesondere das Prinzip von **Microsoft Endpoint Configuration Manager (MECM) + Software Center** bzw. **Intune Company Portal**:
- **Für IT-Administratoren:** Zentrale Steuerung von Betriebssystem-Updates, Software-Rollouts, Wartungsfenstern, Neustart-Richtlinien und Compliance-Überwachung.
- **Für Endanwender:** Ein lokales **„Software Center“ (Self-Service App-Katalog)** auf dem Desktop, über das freigegebene Programme **ohne lokale Administratorrechte** per Klick installiert oder aktualisiert werden können – inklusive transparenter Update- und Neustart-Hinweise statt überraschender Zwangs-Reboots.

---

## 2. Marktanalyse & Einordnung bestehender Lösungen

Vor einer Umsetzung lohnt der Blick auf bestehende Systeme, um bewährte Konzepte zu übernehmen und typische Schwächen zu vermeiden:

| Lösung | Stärken | Schwächen / Hürden im NS8-Kontext |
|---|---|---|
| **Microsoft MECM (Software Center) / Intune** | Goldstandard beim Self-Service-Katalog (*Verfügbar* vs. *Pflicht*), Wartungsfenstern, Erkennungsregeln (Detection Rules) und Reboot-Steuerung. | Proprietär, teuer, stark Windows-zentriert, keine schlanke On-Prem-Lösung für gemischte KMU-Umgebungen auf Linux/NS8. |
| **WAPT / opsi** | Sehr ausgereifte europäische Systeme für Software-Verteilung inkl. Self-Service-Kiosk und eigenem Paket-Repository. | Schwergewichtige Server-Architektur; viele relevante Features (z. B. Linux-Clients, Windows-Update-Steuerung, AD-Self-Service) sind bei WAPT/opsi teils kostenpflichtigen Enterprise-Editionen vorbehalten. |
| **OpenUEM** | Moderner Stack (Go, PostgreSQL), nutzt clever native Paketmanager (**WinGet** unter Windows, **Flatpak/apt/dnf** unter Linux). | Nutzt NATS mit eigener mTLS/OCSP-Infrastruktur auf Sonderports (unpraktisch hinter Rootless NS8-Traefik) und bietet aktuell kein klassisches Enduser-„Software Center“ auf dem Desktop. |
| **PatchMon** | Extrem schlank (Go-Agent, Outbound-Only HTTPS), stark bei plattformübergreifendem OS-Patching (Linux & Windows) und Compliance. | Reines Admin-Patch-Tool – kein Software-Katalog und kein Self-Service für Endbenutzer. |
| **Tactical RMM / MeshCentral** | Mächtiges Remote-Management, Skripting und Remote-Desktop. | Komplexer Multi-Container-Stack, eingeschränkte Open-Source-Lizenz (bei Tactical RMM teils kostenpflichtige Code-Signierung) und kein Enduser-Software-Center. |

### Erkenntnis für NS8-SoftwareManager
Eine **maßgeschneiderte NS8-Eigenentwicklung** kombiniert die besten Konzepte dieser Welten optimal für NethServer 8:
1. **Schlanker, einzelner Agent (z. B. in Go oder Rust)** ohne Laufzeitabhängigkeiten (.NET/Python/Java auf Clients entfällt).
2. **Reine HTTPS/WebSocket-Kommunikation über Port 443** – passt zu 100 % hinter den Rootless-Traefik-Proxy von NS8 (inkl. Homeoffice-/VPN-/Roadwarrior-Fähigkeit ohne Portfreigaben auf den Clients).
3. **Hybrides Paketmodell:** Nutzung vorhandener Paketquellen (`winget`, `apt`, `dnf`, `flatpak`) für Standard-Apps **plus** eigenes lokales Paket-Repository auf dem NS8-Server für individuelle `.msi`/`.exe`/`.deb`/`.rpm`-Installer und Skripte.
4. **Modulare Trennung auf dem Client:** Hintergrund-Dienst (`SYSTEM` / `root`) für Server & Workstations + optionale Desktop-App („Software Center“ & Tray-Benachrichtigung) für Arbeitsplatzrechner.

---

## 3. Erste Muss-Funktionen (MVP / Kernanforderungen)

Die folgenden Funktionen bilden das Fundament (**Must-Have**), damit der Dienst im Alltag produktiv für Windows- und Linux-Rechner eingesetzt werden kann.

### M1 – Architektur, Sicherheit & Client-Onboarding
- **Plattformübergreifender Hintergrund-Agent:**
  - **Windows (10/11, Server 2019/2022/2025):** Läuft als nativer Windows-Dienst unter `NT AUTHORITY\SYSTEM`.
  - **Linux (Debian/Ubuntu, RHEL/Rocky/Alma/Fedora):** Läuft als `systemd`-Service unter `root`.
- **Outbound-Only Kommunikation (HTTPS / WSS über Port 443):**
  - Keine offenen eingehenden Ports (kein SSH, kein WinRM, kein SMB) auf den verwalteten Rechnern erforderlich.
  - Nahtloser Betrieb durch NS8-Traefik (automatisches Let's Encrypt TLS) – funktioniert im LAN genauso wie im Homeoffice.
- **Einfaches & sicheres Enrollment (Registrierung):**
  - Einmal- oder zeitbegrenzte Enrollment-Tokens (z. B. für Massen-Rollout per Windows-GPO/MSI-Parameter oder Einzeiler-Installationsskript unter Linux).
  - Optionale **Admin-Freigabe (Pending Approval)** für neue Geräte, bevor diese Befehle oder interne Pakete erhalten.
  - Nach Registrierung: Eindeutiges Geräte-Zertifikat bzw. kryptografisches Geräte-Token pro Client.
- **Manipulationsschutz & Integrität:**
  - Verpflichtende `SHA-256`-Prüfsummenvalidierung für alle vom Server geladenen Installer und Skripte vor der Ausführung.
  - Strikte Trennung zwischen Agent-API und Admin-API.
- **Agent-Self-Update:**
  - Zentral ausrollbare Aktualisierung des Client-Agenten direkt über das Verwaltungsportal.

---

### M2 – Geräteverwaltung & Inventarisierung
- **System- & Hardware-Basisdaten:**
  - Hostname, Domain/Workgroup, Betriebssystem + Edition + Kernel/Build-Nummer, Architektur (`amd64`/`arm64`), CPU, RAM, freier Plattenplatz, lokale IP-Adressen, MAC-Adresse, Seriennummer/UUID.
  - Echtzeit-Online-Status (Heartbeat / WebSocket), Zeitpunkt des letzten Kontakts sowie aktuell angemeldete Desktop-Benutzer.
- **Software-Inventar:**
  - Automatische Erfassung aller installierten Programme inkl. Versionsnummer und Installations-Scope (Windows: Registry `HKLM`/`HKCU` & WinGet; Linux: `dpkg`/`rpm` & `flatpak`).
- **Update- & Neustart-Status:**
  - Übersicht ausstehender OS-Updates (gruppiert nach Sicherheits-/Kritikalitätsstufe).
  - Zuverlässige Erkennung von **„Neustart erforderlich“ (Pending Reboot)** (Windows: Component Based Servicing / Windows Update Reboot-Keys; Linux: `/var/run/reboot-required` bzw. `needs-restarting -r`).
- **Gruppen & Tags:**
  - Statische (und perspektivisch dynamische) Gerätegruppen, z. B. *Workstations*, *Server*, *Test-Ring (Pilot)*, *Buchhaltung*, um Rollouts gestuft auszuführen.

---

### M3 – Betriebssystem-Update-Management (OS Patching)
- **Windows OS-Updates (ohne WSUS-Zwang):**
  - Direkte Ansteuerung der nativen **Windows Update Agent (WUA) API** durch den Agenten.
  - Scannen, Auflisten und gezieltes Installieren von Sicherheitsupdates, kumulativen Updates, Definitionen und optionalen Updates/Treibern.
  - Möglichkeit, einzelne fehlerhafte Updates (KB-Nummern) zentral zurückzustellen/auszuschließen.
- **Linux OS-Updates:**
  - Native Unterstützung für **APT** (`Debian`, `Ubuntu`, `Linux Mint`) und **DNF** (`RHEL`, `Rocky`, `AlmaLinux`, `Fedora`).
  - Unterscheidung zwischen reinen **Sicherheitsupdates** und vollständigen Paket-Upgrades im unbeaufsichtigten Modus (`noninteractive`, Konfigurationsdateien sicher beibehalten via `force-confdef`/`force-confold`).
- **Wartungsfenster (Maintenance Windows) & Zeitpläne:**
  - Definition fester Zeitfenster pro Gruppe (z. B. *Server: So 02:00–04:00 Uhr*, *Clients: Mi ab 12:00 Uhr*), außerhalb derer keine automatischen Installationen oder Neustarts stattfinden dürfen.
  - **Catch-up-Logik:** War ein Laptop während des Wartungsfensters ausgeschaltet, wird der Auftrag gemäß Richtlinie beim nächsten Start nachgeholt.
- **Gesteuerte Neustart-Richtlinien (Reboot Policies):**
  - *Kein automatischer Neustart* (nur Flag setzen / Admin entscheidet – wichtig für kritische Server).
  - *Automatischer Neustart im Wartungsfenster*.
  - *Benutzergesteuerter Neustart mit Karenzzeit (Grace Period)* auf Arbeitsplatzrechnern (z. B. Benachrichtigung mit Verschiebe-Möglichkeit bis zu einer festgelegten Deadline).

---

### M4 – Software-Verteilung & Paket-Katalog (Software Deployment)
- **Zentraler Software-Katalog mit zwei Paket-Typen:**
  1. **Native Paketmanager-Pakete:**
     - **Windows:** Anbindung an **WinGet** (System-/Machine-Scope) für tausende fertige Standardprogramme.
     - **Linux:** Anbindung an **APT / DNF** sowie **Flatpak (Flathub)** für Desktop-Anwendungen.
  2. **Eigene Pakete (Lokales NS8-Paket-Repository):**
     - Upload eigener Installationsdateien (`.msi`, `.exe`, `.zip`-Archive mit Begleitdateien, `.deb`, `.rpm`, `.ps1`, `.sh`) direkt auf den NS8-Server.
     - Hinterlegung von: Name, Icon, Beschreibung, Version, Kategorie, Installationsbefehl (Silent-Flags wie `/qn`, `/S`), Deinstallationsbefehl, erlaubten Rückgabewerten (z. B. `0` = Erfolg, `3010` = Erfolg + Reboot nötig).
     - **Erkennungsregeln (Detection Rules):** Prüfung per Dateipfad/Version, Registry-Key oder Paketname, ob die Software bereits in der Zielversion vorhanden ist (verhindert unnötige Neuinstallationen und erkennt Erfolg zuverlässig).
- **Bereitstellungs-Modi (Zuweisung an Gruppen oder Einzelgeräte):**
  - **Pflicht (Required):** Software wird automatisch im Hintergrund installiert bzw. aktuell gehalten (oder per *Uninstall*-Auftrag entfernt).
  - **Verfügbar (Available / Self-Service):** Software erscheint im **Software Center** des Endbenutzers und kann dort bei Bedarf selbstständig ohne lokale Admin-Rechte installiert werden.
- **Drittanbieter-Software-Updates (3rd-Party Patching):**
  - Erkennung veralteter Katalog-Software auf den Clients und automatische oder freigegebene Aktualisierung (inkl. Prüfung/Hinweis, falls die zu aktualisierende Anwendung gerade geöffnet ist).

---

### M5 – Endbenutzer „Software Center“ (Client-GUI & Tray)
*(Optionales Modul bei der Agent-Installation: Aktiv auf Windows-/Linux-Desktops, deaktiviert auf Headless-Servern)*
- **Übersichtlicher Self-Service App-Store:**
  - Darstellung aller für diesen Rechner (bzw. Benutzer) freigegebenen Programme mit Firmen-Branding, Icon, Beschreibung, Version und Such-/Kategoriefilter.
  - Buttons für **Installieren**, **Aktualisieren** und (sofern vom Admin für das Paket erlaubt) **Deinstallieren**.
- **Reiter „Updates“ & „Verlauf“:**
  - Anzeige anstehender OS- und Software-Updates, sodass Anwender Updates auch proaktiv vor dem Pflichttermin (z. B. in der Mittagspause) selbst starten können.
  - Live-Statusanzeige (*Wird heruntergeladen…*, *Wird installiert…*, *Erfolgreich installiert*, *Neustart ausstehend*).
- **System-Tray-Integration & Benachrichtigungen:**
  - Dezente Desktop-Benachrichtigungen bei neuen Pflicht-Updates oder erforderlichem Neustart.
  - Klarer Neustart-Dialog („Neustart jetzt durchführen“ / „In 1 Stunde erinnern“ / Countdown bei ablaufender Deadline) statt unerwartetem Datenverlust.
- **Sichere lokale Architektur (Privilege Separation):**
  - Die grafische Software-Center-App läuft ausschließlich mit normalen **Benutzerrechten** und kommuniziert lokal über eine abgesicherte Schnittstelle (Named Pipe unter Windows / Unix Domain Socket unter Linux) mit dem privilegierten Agent-Dienst, der nur vom Server signierte/freigegebene Katalog-Aktionen zulässt.

---

### M6 – NethServer 8 Integration, Admin-Konsole & Monitoring
- **NS8-Modul-Konformität (Rootless Podman):**
  - Betrieb als Rootless Podman-Pod (Backend + Datenbank wie PostgreSQL + Paket-Storage-Volume) streng gebunden an `127.0.0.1:${TCP_PORT}` hinter NS8-Traefik.
  - **NS8 Cockpit-Modul-UI (`@nethserver/ns8-ui-lib`):** Konfiguration von FQDN/Hostname, Let's Encrypt, HTTP-zu-HTTPS, Anbindung an NS8 User-Domain (Samba AD / OpenLDAP), SMTP-Einstellungen für Alarme sowie Speicherort/Quotas.
  - **NS8 Backup, Restore & Clone:** Vollständige Integration in das NS8-Restic-Backup (`state-include.conf`, Datenbank-Dump in `module-dump-state` sowie Sicherung des eigenen Software-Katalogs und der PKI-/Enrollment-Schlüssel).
- **Dedizierte Web-Admin-Oberfläche (unter eigener Subdomain, z. B. `swm.firma.de`):**
  - **Compliance-Dashboard:** Sofortüberblick über Online-/Offline-Geräte, Patch-Quote, kritische fehlende Sicherheitsupdates, Geräte mit ausstehendem Neustart und fehlgeschlagene Jobs.
  - **Auftrags- & Log-Zentrale:** Detaillierte Ausführungsprotokolle pro Gerät und Job (Start-/Endzeit, Exit-Code, vollständiger `stdout`/`stderr`-Log-Auszug zur schnellen Fehleranalyse bei zickigen Installern).
  - **Benutzer- & Rollenanbindung:** Login für Administratoren über NS8-AD/LDAP (oder lokalen Fallback-Admin).

---

## 4. Wichtige technische Besonderheiten & Fallstricke (aus der Recherche)

Bei der späteren Architekturplanung müssen insbesondere vier bekannte Praxis-Hürden direkt berücksichtigt werden:

1. **Das `WinGet`-Problem unter `NT AUTHORITY\SYSTEM` (Windows):**
   - `winget.exe` ist in Windows als MSIX-App (`Microsoft.DesktopAppInstaller`) verpackt und verhält sich im `SYSTEM`-Kontext (Session 0) anders als im Benutzerkontext: Es liegt nicht im Standard-`PATH` von `SYSTEM`, findet ohne explizites `--scope machine` benutzerbezogene Apps (`HKCU`) nicht und kann auf ganz frischen Windows-Installationen ohne VCLibs/UI.Xaml fehlschlagen.
   - *Lösung im Agenten:* Automatische Pfadauflösung des `DesktopAppInstaller`-Ordners unter `C:\Program Files\WindowsApps\`, erzwungener `--scope machine --silent --accept-package-agreements --accept-source-agreements` Parameter, Timeout-Überwachung gegen hängende GUI-Dialoge in Session 0 sowie nativer Fallback auf `msiexec` / Custom-Installer.
2. **Große Installationspakete & Bandbreite:**
   - Eigene Pakete (z. B. Office-Suiten, Treiberpakete, Branchensoftware > 500 MB) benötigen **HTTP Range Requests (Resumable Downloads)** beim Download durch den Agenten sowie Streaming-Uploads (Chunked Upload), damit Traefik-Timeouts und abgebrochene WLAN-/VPN-Verbindungen problemlos abgefangen werden.
3. **Blockierte Updates durch geöffnete Programme:**
   - Ein Browser- oder Office-Update schlägt fehl oder schießt dem Nutzer das offene Programm ab, wenn es blind im Hintergrund läuft.
   - *Lösung:* Option pro Paket „Nur installieren, wenn Prozess `xyz` nicht läuft“ bzw. Hinweis über das Software-Center („Bitte Anwendung schließen, um Update abzuschließen“).
4. **Hochsicherheits-Charakter des Dienstes:**
   - Da der Agent `SYSTEM`- bzw. `root`-Rechte auf allen angeschlossenen Rechnern besitzt, entspricht die Kompromittierung des Servers einer vollständigen Domänen-Übernahme.
   - *Konsequenz:* Strikte Hash-/Signaturprüfung, Audit-Log für alle Admin-Aktionen, keine ungeprüfte Ausführung beliebiger Befehle über die Client-GUI und saubere TLS-Zertifikatsprüfung im Agenten.

---

## 5. Abgrenzung: Was gehört NICHT in Version 1 (Spätere Ausbaustufen / Nice-to-Have)

Um das Projekt schlank und sauber umsetzbar zu halten, sollten folgende Funktionen bewusst auf **Phase 2+** verschoben werden:
- **Bare-Metal OS-Deployment (PXE-Boot / Imaging):** Windows-/Linux-Neuinstallation über Netzwerk (komplexe DHCP/TFTP-Anforderungen passen schlecht in Rootless-Container).
- **Remote-Desktop / Live-Bildschirmsteuerung:** Hierfür existieren auf NS8 bereits spezialisierte Module wie `ns8-meshcentral` oder RustDesk (ggf. später per Verlinkung integrierbar).
- **Genehmigungs-Workflows (Approval Requests):** „Benutzer beantragt kostenpflichtige Software im Software Center -> Vorgesetzter genehmigt per Mail -> Installation startet“.
- **Dynamische AD-Benutzergruppen-Zuweisung (User-Targeting statt Device-Targeting):** In Version 1 reicht die Zuweisung an **Geräte & Gerätegruppen** (deckt ~90 % aller Anwendungsfälle deutlich robuster ab); benutzerabhängige Kataloge für wechselnde Benutzer am selben PC können in Phase 2 folgen.
- **macOS-Agent:** Fokus in v1 klar auf **Windows + Linux**; die Agent-Architektur sollte jedoch so modular sein, dass macOS (`brew`/`pkg`) später ergänzt werden kann.
- **CVE-/Schwachstellen-Datenbank-Abgleich:** Abgleich des Software-Inventars gegen NVD/CVE-Feeds.

---

## 6. Offene Richtungsentscheidungen für den nächsten Schritt

Bevor wir in die detaillierte Spezifikation und Architekturplanung einsteigen, sollten wir folgende Punkte kurz klären:

1. **Eigenentwicklung vs. Integration:** Möchtest du diesen Dienst komplett als **eigenes, modernes NS8-System** (eigener Server + eigener schlanker Windows/Linux-Agent + eigenes Software Center) von Grund auf entwickeln?
2. **Form des „Software Centers“ auf den Clients:**
   - **Variante A (Nativ / Desktop-Fenster + Tray):** Eine kleine lokale Desktop-App (z. B. via Tauri/WebView oder Go/Wails) mit System-Tray-Icon auf Windows & Linux – fühlt sich 1:1 wie das Microsoft Software Center an.
   - **Variante B (Tray-Helper + Browser-Portal):** Kleines Tray-Icon für Neustart-/Update-Meldungen; ein Klick auf „Software Center öffnen“ öffnet das Self-Service-Portal im Browser und triggert den lokalen Agenten.
3. **Unterstützte Linux-Familien zum Start:** Reichen für v1 **Debian/Ubuntu (`apt`)** und **RHEL/Rocky/Alma/Fedora (`dnf`)** (+ optional `Flatpak` für Desktop-Apps), oder hast du noch SUSE (`zypper`) / Arch (`pacman`) im Einsatz?
4. **Benutzer- vs. Geräte-Zuweisung:** Reicht es für den Start, Software und Updates **Geräten und Gerätegruppen** zuzuweisen, oder brauchst du sofort im MVP auch eine Zuweisung nach **angemeldetem AD-Benutzer / AD-Gruppe**?
