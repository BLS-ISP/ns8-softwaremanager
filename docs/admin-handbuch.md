# Administrator-Handbuch – NS8-SoftwareManager (`v1.2.0`)

Dieses Handbuch führt IT-Administratoren Schritt für Schritt durch die Einrichtung, den täglichen Betrieb, das Patch-Management, die Paketverteilung und die Echtzeit-Flottensteuerung von **NS8-SoftwareManager** für **Windows**, **Linux** und **macOS**.

> **Weitere Handbücher im Ordner `docs/`:**
> - [Übersicht aller Handbücher](README.md)
> - [Praxishandbuch Client-Rollout (Einzelplatz, GPO, Intune, Jamf/MDM, Ansible & Gold-Images)](client-rollout-handbuch.md)
> - [Endbenutzer-Handbuch „NS8 Software Center“ (Vorlage für Mitarbeiter / Intranet)](benutzerhandbuch-software-center.md)
> - [Troubleshooting, Log-Analyse & Betriebshandbuch](troubleshooting-und-betrieb.md)

---

## Inhaltsverzeichnis

1. [Architektur im Überblick: Zwei Oberflächen & zwei Client-Komponenten](#1-architektur-im-überblick-zwei-oberflächen--zwei-client-komponenten)
2. [Ersteinrichtung auf NethServer 8](#2-ersteinrichtung-auf-nethserver-8)
3. [Grundstruktur aufbauen: Gerätegruppen, Ringe & Wartungsfenster](#3-grundstruktur-aufbauen-gerätegruppen-ringe--wartungsfenster)
4. [Geräteverwaltung & Echtzeit-Flottensteuerung](#4-geräteverwaltung--echtzeit-flottensteuerung)
5. [Betriebssystem-Patching (Windows, Linux & macOS)](#5-betriebssystem-patching-windows-linux--macos)
6. [Software-Katalog & Eigene Pakete bereitstellen](#6-software-katalog--eigene-pakete-bereitstellen)
7. [Desktop-Client-Härtung, Kiosk-Modus & Agent-Self-Update](#7-desktop-client-härtung-kiosk-modus--agent-self-update)
8. [Datensicherung, Wiederherstellung & Modul-Updates](#8-datensicherung-wiederherstellung--modul-updates)

---

## 1. Architektur im Überblick: Zwei Oberflächen & zwei Client-Komponenten

Um Berechtigungen sauber zu trennen, besitzt NS8-SoftwareManager **zwei Web-Oberflächen** und **zwei Client-Programme**:

| Komponente | Wer nutzt sie? | Aufgabe |
|---|---|---|
| **NS8 Cluster-Admin Modul** (`/cluster-admin/#/apps/softwaremanager1`) | NS8-Cluster-Root-Admin | Einmalige Grundeinrichtung: Subdomain (FQDN), Let's Encrypt, Anbindung an NS8 Active Directory / OpenLDAP, Container-Status & Restic-Backup. |
| **SoftwareManager Web-Konsole** (`https://swm.ihre-firma.de`) | IT-Administratoren & Helpdesk | Täglicher Betrieb: Dashboard, Geräte-Flotte, Client-Rollout, OS-Patching, Software-Katalog, Bereitstellungen, Wartungsfenster und Richtlinien. |
| **Management-Agent (`swm-agent`)** | Hintergrunddienst (`SYSTEM` / `root`) | Verbindet sich **rein ausgehend über Port 443 (`HTTPS/WSS`)** mit dem Server, prüft `Ed25519`-Signaturen und führt Inventar, Patches und Installationen aus. |
| **Endbenutzer-App (`swm-center`)** | Angemeldete Mitarbeiter (ohne Admin-Rechte) | Lokales **„NS8 Software Center“** + System-Tray für Self-Service-Installationen, IT-Mitteilungen und gesteuerte Neustart-Countdowns. |

---

## 2. Ersteinrichtung auf NethServer 8

### Schritt 1: Modul installieren
Entweder direkt im **NS8 Software Center** (über das Repository `ns8-nethforge`) oder per SSH auf dem NS8-Leader-Node:

```bash
add-module ghcr.io/bls-isp/softwaremanager:1.2.0 1
```

### Schritt 2: Instanz im NS8-Cockpit konfigurieren
1. Öffnen Sie im **NS8 Cluster-Admin** links unter **Applikationen** Ihre neue Instanz (z. B. `softwaremanager1`) und wechseln Sie auf **Einstellungen**.
2. **Hostname (FQDN):** Tragen Sie eine extern wie intern auflösbare Subdomain ein (z. B. `swm.firma.de`), damit auch Laptops im Homeoffice den Server erreichen.
3. **Let's Encrypt & HTTP -> HTTPS:** Aktivieren Sie beide Schalter (empfohlen, da alle Agenten strikt TLS-Zertifikate prüfen).
4. **Benutzerdomäne (LDAP / Active Directory):** Wählen Sie optional Ihre NS8-Domäne (z. B. `ad.firma.de`) aus, damit sich Administratoren mit ihrem Domänenkonto anmelden können.
5. **Lokaler Fallback-Administrator:** Ändern Sie das initiale Passwort des lokalen Notfall-Kontos (`admin`) auf ein starkes Passwort und klicken Sie auf **Einstellungen speichern**.

### Schritt 3: Anmeldung an der Web-Konsole
1. Klicken Sie oben rechts auf **SoftwareManager-Konsole öffnen** (`https://swm.firma.de`).
2. Wählen Sie auf der Anmeldeseite zwischen **Domänen-Login (LDAP / AD)** und **Lokaler Administrator** und melden Sie sich an.
3. Solange noch keine Endgeräte registriert sind, begrüßt Sie auf dem **Dashboard** automatisch der Schnellstart-Assistent zur Anbindung Ihres ersten Clients.

---

## 3. Grundstruktur aufbauen: Gerätegruppen, Ringe & Wartungsfenster

Legen Sie vor dem ersten großen Rollout Ihre Gruppenstruktur an — so steuern Sie Neustart-Verhalten, Kiosk-Sperren und Wartungsfenster zentral pro Gerätekategorie.

### Empfohlenes Gruppen- & Ring-Modell (Menüpunkt: *Gerätegruppen*)
Klicken Sie unter **Gerätegruppen** auf **Neue Gruppe erstellen**. Bewährt hat sich folgende Aufteilung:

| Gruppe | Empfohlene Neustart-Richtlinie | Einsatzzweck |
|---|---|---|
| **1. Pilot-Ring (IT & Testgeräte)** | *Benutzer-Karenzzeit (120 Min.)* | Erhält neue OS-Updates und Katalog-Versionen 3–5 Tage vor der Gesamtflotte. |
| **2. Büro-Workstations & Laptops** | *Benutzer-Karenzzeit (240 Min.)* | Mitarbeiter-PCs & Macs: Updates laufen im Hintergrund; ist ein Reboot nötig, erhält der Nutzer einen verschiebbaren Countdown. |
| **3. Empfang / Kiosk / Schulung** | *Automatisch im Wartungsfenster* | Gesperrte Terminals ohne eigene Software-Auswahl (*Managed-Kiosk-Modus*). |
| **4. Produktiv-Server (Headless)** | *Nie automatisch neu starten* (oder enges Nacht-Wartungsfenster) | Verhindert ungeplante Reboots im Tagesbetrieb; Server werden bei Bedarf nur als *Neustart erforderlich* markiert. |

### Wartungsfenster anlegen (Menüpunkt: *Aufträge & Wartung*)
1. Wechseln Sie zu **Aufträge & Wartung** und klicken Sie auf **Wartungsfenster anlegen**.
2. Wählen Sie die Zielgruppe (z. B. *Produktiv-Server*), Wochentage (z. B. *Sonntag*), Zeitraum (`02:00` bis `04:00`) und Zeitzone (`Europe/Berlin`).
3. **Option „Nachholen (Catch-up)“:**
   - **Aktivieren bei Laptops/Workstations:** War ein Notebook während des Fensters ausgeschaltet, holt der Agent ausstehende Pflichtaufträge beim nächsten Einschalten automatisch nach.
   - **Deaktivieren bei kritischen Servern:** Verhindert, dass ein verspätet gestarteter Job am Montagmorgen um 08:30 Uhr anläuft.

---

## 4. Geräteverwaltung & Echtzeit-Flottensteuerung

Unter **Geräte & Flotte** sehen Sie sämtliche Windows-, Linux- und macOS-Systeme mit Echtzeit-Verbindungsstatus (`Online` über WebSocket).

### Neue Geräte prüfen & freigeben (*Pending Approval*)
Sofern beim Enrollment-Token **Auto-Approve** nicht aktiviert war, landen neue Rechner zunächst im Sicherheitsstatus **Wartet auf Freigabe**:
- Das Gerät kann in diesem Zustand noch **keine** internen Softwarepakete herunterladen.
- Prüfen Sie Hostname, IP-Adresse und Betriebssystem und klicken Sie auf **Freigeben** (oder wählen Sie mehrere Geräte per Checkbox aus -> **Freigeben**).

### Einzelgerät-Steuerung (Gerätedetail-Ansicht)
Ein Klick auf das Augen-Symbol (oder den Hostnamen) öffnet die Detailansicht eines Rechners:
- **Reiter Übersicht / Hardware / Installierte Software / Offene OS-Updates / Job-Historie:** Vollständige Bestandsdaten inkl. Anzeige, ob das **NS8 Software Center** aktuell in einer Benutzersitzung aktiv ist (`Software Center: Aktiv in Benutzersitzung` vs. `Kein aktives Tray / Headless`).
- **Direkt-Aktionen (oben rechts):**
  1. **Jetzt synchronisieren (`Sync`):** Weckt den Agenten sofort über WebSocket für einen frischen Inventar- und Katalog-Abgleich.
  2. **Nachricht senden:** Öffnet einen Dialog für Titel, Nachrichtentext und Option *Dringend*. Die Meldung erscheint innerhalb von Sekunden als natives Desktop-Popup (Windows Toast / Linux Notification / macOS Mitteilungszentrale) **und** als quittierbares Hinweisbanner oben im Software Center des Mitarbeiters.
  3. **Neustart steuern:** Erlaubt drei signierte Modi:
     - *Karenzzeit & Countdown starten* (`15`, `60` oder `240` Minuten Vorlaufzeit für den Benutzer),
     - *Ausstehenden Neustart abbrechen (`cancel`)* (falls ein geplanter Reboot gestoppt werden soll),
     - *Sofortiger Zwangsneustart (`immediate`)* (nur für unbesetzte Systeme / Notfälle).

### Flotten-Mehrfachauswahl (*Bulk-Steuerung*)
Setzen Sie in der Gerätetabelle links Häkchen bei beliebig vielen Rechnern, um die schwebende **Flotten-Sammelsteuerung** einzublenden:
- **Freigeben:** Alle markierten *Pending*-Geräte in einem Schritt zulassen.
- **Jetzt synchronisieren:** Sofort-Sync auf allen ausgewählten Systemen auslösen.
- **Update-Scan:** Frischen Betriebssystem-Update-Scan (`WUA`, `APT`/`DNF` bzw. `softwareupdate`) starten.
- **Gruppe zuweisen:** Ausgewählte Rechner gesammelt einer Gruppe zuordnen.
- **Nachricht senden:** Gleichzeitige Wartungsankündigung an alle markierten Arbeitsplätze senden (z. B. *„Bitte speichern Sie Ihre Arbeit bis 17:00 Uhr wegen Server-Wartung“*).

---

## 5. Betriebssystem-Patching (Windows, Linux & macOS)

Unter **OS-Patching** laufen alle gemeldeten Betriebssystem-Updates der drei Plattformen zusammen:

| Plattform | Verwendete Engine auf dem Client | Besonderheiten |
|---|---|---|
| **Windows** | Native *Windows Update Agent (WUA)* API | Kein eigener WSUS-Server nötig; erkennt KB-Nummern, Sicherheitsstufe & Reboot-Bedarf. |
| **Linux** | `apt` (Debian/Ubuntu/Mint) & `dnf` (RHEL/Rocky/Alma/Fedora) | Läuft strikt unbeaufsichtigt (`DEBIAN_FRONTEND=noninteractive`, bestehende Konfigurationsdateien bleiben geschützt). |
| **macOS** | Apple `/usr/sbin/softwareupdate` | Erkennt macOS-Punktupdates, Rapid Security Responses, Safari- und Xcode-CLT-Updates inkl. `Action: restart`. |

### Typischer Patch-Workflow
1. **Filtern:** Filtern Sie oben nach Plattform (*Alle*, *Nur Windows*, *Nur Linux*, *Nur macOS*) oder nach Schweregrad (*Kritisch / Sicherheit*).
2. **Updates freigeben & ausrollen:** Markieren Sie die gewünschten Updates und klicken Sie auf **Freigeben & Installieren** (wahlweise beschränkt auf Ihren *Pilot-Ring* und unter Beachtung des Wartungsfensters).
3. **Problematische Updates ausschließen (Sperrliste):** Ist z. B. ein fehlerhaftes Windows-KB oder Treiberupdate bekannt, klicken Sie auf **Ausschließen**. Gesperrte Update-IDs werden von allen Agenten auch bei automatischen Läufen übersprungen, bis Sie die Sperre wieder aufheben.

---

## 6. Software-Katalog & Eigene Pakete bereitstellen

Öffnen Sie **Software-Katalog** und klicken Sie auf **Paket hinzufügen / Upload**. Der Assistent ist in drei übersichtliche Reiter gegliedert:

### Reiter 1: Basisdaten & Paket-Quelle
Wählen Sie zuerst die **Ziel-Plattform** und anschließend die **Paket-Quelle**:

- **Windows:**
  - *WinGet (Offizielles Microsoft-Repository):* Geben Sie die exakte WinGet-Paket-ID an (z. B. `Mozilla.Firefox`, `7zip.7zip`, `VideoLAN.VLC`). Der Agent löst `winget.exe` automatisch für den Maschinen-Scope (`--scope machine`) auf.
  - *Eigenes Paket (NS8-Repository):* Für eigene `.msi`-, `.exe`-, `.zip`- (mehrteilige Setups mit Lizenz-/Konfigurationsdateien) oder `.ps1`-Installer.
- **Linux:**
  - *APT*, *DNF* oder *Flatpak (Flathub)* über den Paketnamen/App-ID (z. B. `org.gimp.GIMP`), oder *Eigenes Paket* (`.deb`, `.rpm`, `.sh`).
- **macOS (Apple Silicon & Intel):**
  - *Homebrew Cask (Desktop-App `.app`):* Cask-Token angeben (z. B. `firefox`, `visual-studio-code`, `libreoffice`).
  - *Homebrew Formula (CLI-Werkzeug):* Formula-Name angeben (z. B. `jq`, `wget`, `htop`). *(Hinweis: `swm-agent` führt Homebrew automatisch unter dem unprivilegierten Konsolen-/Homebrew-Eigentümer aus, niemals unsicher als `root`.)*
  - *Eigenes macOS-Paket:* Upload von `.pkg` (wird via `installer -pkg ... -target /` installiert), `.dmg` (wird unsichtbar read-only gemountet, enthaltene `.pkg` installiert bzw. `.app` nach `/Applications` kopiert und wieder ausgehängt), `.app.zip` oder `.sh`.

### Reiter 2: Parameter, Rückgabecodes & Prozess-Sperre
- **Installations-/Deinstallations-Parameter:** Bei `.msi` z. B. `/qn /norestart` (Standard wird sinnvoll vorbelegt); bei `.exe`-Installern je nach Hersteller `/S`, `/VERYSILENT /NORESTART` oder `/quiet`.
- **Blockierende Prozesse (`blocked_processes`):** Tragen Sie z. B. `firefox.exe` oder `WINWORD.EXE` ein, wenn ein Update nicht in die laufende Anwendung hineinlaufen darf. Läuft der Prozess, wartet der Auftrag sauber (`waiting_for_process`), statt die offene Arbeit des Nutzers abzuschießen.
- **Deinstallation im Software Center erlauben:** Steuert, ob Mitarbeiter im Self-Service einen *Deinstallieren*-Button sehen.

### Reiter 3: Erkennungsregel (*Detection Rule*) & Datei-Upload
Erkennungsregeln prüfen vor **und** nach der Installation, ob die Software bereits in mindestens der gewünschten Zielversion vorhanden ist:
- **Windows:** Dateipfad (z. B. `C:\Program Files\7-Zip\7z.exe`) oder Registry-Schlüssel (`HKLM\SOFTWARE\...`).
- **Linux:** Paketprüfung (`dpkg`/`rpm`/`flatpak`) oder Dateipfad (`/usr/bin/...`).
- **macOS:** Geführte Vorlagen für **App-Bundle** (`/Applications/Firefox.app` -> prüft `CFBundleShortVersionString` in `Info.plist`) oder **Installer-Receipt (`pkgutil`)** (`org.mozilla.firefox.pkg`).
- **Chunked Datei-Upload:** Auch sehr große Pakete (> 1 GB) werden in Teilstücken hochgeladen, serverseitig mit einer `SHA-256`-Prüfsumme versehen und von den Clients unterbrechungssicher per `HTTP 206 Range` geladen.

### Bereitstellung zuweisen (Menüpunkt: *Bereitstellungen*)
Ein Katalog-Eintrag allein installiert noch nichts. Wechseln Sie zu **Bereitstellungen -> Neue Bereitstellung**:
1. Wählen Sie das Paket sowie Ziel-Gruppen oder Einzelgeräte.
2. Wählen Sie den **Modus**:
   - **Pflicht (`Required`):** Wird vollautomatisch installiert und bei Versions-Erhöhung im Katalog automatisch aktualisiert.
   - **Verfügbar (`Available`):** Erscheint zur freiwilligen 1-Klick-Installation im **NS8 Software Center** der Mitarbeiter.
   - **Deinstallieren (`Uninstall`):** Entfernt das Paket automatisiert von den Zielgeräten.

---

## 7. Desktop-Client-Härtung, Kiosk-Modus & Agent-Self-Update

Unter **Richtlinien & Agent** steuern Sie für jede Gerätegruppe vier Schutzmechanismen für das **NS8 Software Center**:

1. **Tray-Icon erzwingen & Beenden sperren (`enforce_tray`, Standard: Aktiv):**
   - Blendet den Menüpunkt *„Beenden“* im System-Tray aus.
   - Beendet ein Benutzer `swm-center` im Task-Manager / Aktivitätsanzeige, startet der zweistufige Session-Watchdog den Client automatisch wieder neu.
2. **Self-Service-Installationen erlauben (`allow_self_service`):**
   - Deaktivieren Sie diese Option für Kiosk-, Schulungs- oder Empfangsrechner (**Managed-Kiosk-Modus**): Benutzer sehen installierte Programme und Neustart-Hinweise, können aber selbst keine Installationen anstoßen (wird zusätzlich hart im privilegierten `swm-agent` über IPC blockiert).
3. **Manuelle OS-Updates durch Nutzer erlauben (`allow_user_os_updates`):**
   - Steuert, ob Endanwender ausstehende System-Updates vorab selbst starten dürfen.
4. **Stiller Modus (`silent_notifications`):**
   - Unterdrückt Routine-Popups bei Hintergrund-Installationen und zeigt nur noch dringende Admin-Mitteilungen sowie kritische Neustart-Warnungen (`< 15 Min.`) an.

> **Wichtig bei Mehrfach-Gruppenmitgliedschaft:** Ist ein Rechner in mehreren Gruppen, gewinnt immer die **restriktivere** Einstellung (`enforce_tray = Ja`, `allow_self_service = Nein`).

### Zentrales Agent-Self-Update ausrollen
Nach einem Modul-Update auf dem NethServer (z. B. von `1.1.0` auf `1.2.0`) aktualisieren Sie bestehende Clients ohne erneuten Turnschuh-Einsatz:
1. Wählen Sie unter **Richtlinien & Agent -> Zentrales Agent-Self-Update** die Ziel-Plattform (*Windows*, *Linux* oder *macOS Universal*).
2. Download-Pfad und die exakte **`SHA-256`-Prüfsumme** des im Container mitgelieferten Agent-Binaries werden automatisch eingetragen.
3. Klicken Sie auf **Self-Update ausrollen** — jeder Agent lädt das neue Binary, prüft `Ed25519`-Signatur, `SHA-256`-Hash und Binärheader und tauscht seinen Dienst atomar aus.

---

## 8. Datensicherung, Wiederherstellung & Modul-Updates

- **NS8 Restic Backup:** Sobald Sie im NS8 Cluster-Admin unter **Datensicherung** einen Backup-Zeitplan eingerichtet haben, sichert `softwaremanager` automatisch einen konsistenten PostgreSQL-Datenbank-Dump, die Modul-Konfiguration, das `Ed25519`-Schlüsselpaar (`/data/keys/`) sowie sämtliche hochgeladenen Softwarepakete (`/data/packages/`).
- **In-Place Modul-Update:** Bestehende NS8-Instanzen können jederzeit über das NS8 *Software Center* oder per CLI aktualisiert werden; neue Datenbankspalten werden beim Start automatisch und verlustfrei ergänzt (`ensure_schema_upgrades()`).
