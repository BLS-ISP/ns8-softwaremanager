# Troubleshooting, Log-Analyse & Betriebshandbuch (`v1.2.0`)

Dieses Nachschlagewerk unterstützt Administratoren und Helpdesk-Techniker bei der schnellen Diagnose und Behebung von Fehlern auf dem **NethServer-8-Modul** sowie auf **Windows-**, **Linux-** und **macOS-Clients**.

---

## 1. Wichtige Pfade, Dienste & Log-Befehle auf einen Blick

### A. NethServer 8 Server (Rootless Podman Modul)
Ersetzen Sie `softwaremanager1` durch den Namen Ihrer Modul-Instanz:

```bash
# Status aller drei Rootless-Container-Units (Pod, PostgreSQL, FastAPI-App) prüfen:
runagent -m softwaremanager1 systemctl --user status swm.service swm-db.service swm-app.service

# Live-Logs des Management-Servers verfolgen:
runagent -m softwaremanager1 journalctl --user -u swm-app.service -f

# Datenbank-Logs prüfen:
runagent -m softwaremanager1 journalctl --user -u swm-db.service -n 100

# Aktive Umgebungsvariablen (FQDN, Port, LDAP-Konfiguration) einsehen:
runagent -m softwaremanager1 cat environment
```

### B. Verwaltete Endgeräte (Windows, Linux & macOS)

| Ressource | Windows (`PowerShell Admin`) | Linux (`root`) | macOS (`sudo`) |
|---|---|---|---|
| **Dienst-Status prüfen** | `Get-Service NS8SoftwareManagerAgent` | `systemctl status swm-agent.service` | `sudo launchctl print system/org.nethserver.swm-agent` |
| **Dienst neu starten** | `Restart-Service NS8SoftwareManagerAgent` | `sudo systemctl restart swm-agent.service` | `sudo launchctl kickstart -k system/org.nethserver.swm-agent` |
| **Konfiguration & Status** | `C:\ProgramData\NS8-SoftwareManager\config.json` & `state.json` | `/etc/ns8-swm/config.json` & `state.json` | `/Library/Application Support/NS8-SoftwareManager/config.json` & `state.json` |
| **Lokaler IPC-Endpunkt** | `\\.\pipe\ns8-swm-ipc` | `/run/ns8-swm/ipc.sock` | `/var/run/ns8-swm/ipc.sock` |
| **Manueller Test-Zyklus (Live-Konsolenausgabe)** | `Stop-Service NS8SoftwareManagerAgent; & 'C:\Program Files\NS8-SoftwareManager\swm-agent.exe' run --once; Start-Service NS8SoftwareManagerAgent` | `sudo swm-agent run --once` | `sudo /usr/local/bin/swm-agent run --once` |
| **IPC-Status als Standardnutzer abfragen** | `& 'C:\Program Files\NS8-SoftwareManager\swm-center.exe' status` | `swm-center status` | `/usr/local/bin/swm-center status` |

---

## 2. Typische Fehlerszenarien & Lösungen

### Fall 1: Client erscheint nach der Installation nicht unter „Geräte & Flotte“
1. **Erreichbarkeit & TLS prüfen:** Testen Sie vom betroffenen Client aus den Health-Endpunkt:
   - Windows: `Invoke-RestMethod https://swm.firma.de/api/v1/health`
   - Linux / macOS: `curl -fsSL https://swm.firma.de/api/v1/health`
   - Erscheint ein Zertifikatsfehler, prüfen Sie im NS8 Cluster-Admin, ob **Let's Encrypt** für den FQDN aktiv ist bzw. ob Ihre interne Root-CA auf dem Client installiert wurde.
2. **Enrollment-Token prüfen:** Ist das Token unter **Client-Rollout & Agent** abgelaufen, widerrufen oder wurde `max_uses` erreicht? Erstellen Sie ggf. ein frisches Token.

### Fall 2: Client ist sichtbar, erhält aber keine Pakete oder Jobs
- Prüfen Sie in **Geräte & Flotte** die Spalte **Verbindung & Freigabe**: Steht das Gerät auf **Wartet auf Freigabe** (`pending`), darf es aus Sicherheitsgründen noch keine internen Pakete abrufen. Klicken Sie auf **Freigeben**.
- Prüfen Sie unter **Aufträge & Wartung**, ob der Auftrag auf **Wartet auf Wartungsfenster** steht (`respect_maintenance_window = true`). Für sofortige Tests deaktivieren Sie beim Anlegen der Bereitstellung die Option *Wartungsfenster beachten*.

### Fall 3: Gerät wird trotz laufendem Dienst als „Offline“ angezeigt
- Der Echtzeit-Status nutzt ausgehende **WebSockets (`wss://swm.firma.de/api/v1/agent/ws`)**.
- Sitzt vor Ihrem NethServer 8 oder im Firmennetz ein zusätzlicher externer Reverse-Proxy / SSL-Inspection-Proxy, stellen Sie sicher, dass dieser `Connection: Upgrade` und `Upgrade: websocket` auf Pfad `/api/v1/agent/ws` durchlässt.
- *Gut zu wissen:* Selbst wenn ein restriktives Gast-WLAN WebSockets blockiert, fällt `swm-agent` automatisch auf reguläres HTTPS-Polling zurück — Inventar, Patches und Installationen funktionieren trotzdem weiter.

### Fall 4: Software-Job bleibt auf `waiting_for_process` oder schlägt fehl
1. **`waiting_for_process`:** Im Katalog-Paket ist unter `blocked_processes` ein Prozess hinterlegt (z. B. `firefox.exe`), der auf dem Zielgerät aktuell geöffnet ist. Sobald der Benutzer das Programm schließt, installiert der Agent beim nächsten Zyklus automatisch weiter.
2. **Exit-Code `1618` (Windows MSI):** Eine andere `msiexec.exe`-Installation läuft im selben Moment noch im Hintergrund. Der Agent protokolliert `stdout`/`stderr` vollständig unter **Aufträge & Wartung -> Job-Details**.
3. **Exit-Code `3010` / `1641`:** **Kein Fehler!** Diese Windows-Installer-Codes bedeuten *„Erfolgreich installiert, Neustart erforderlich“* und werden standardmäßig automatisch als `succeeded` + `reboot_required = true` gewertet.
4. **Systemzeituhr des Clients verstellt (`SECURITY REJECTION: job expired`):** Jeder `Ed25519`-signierte Auftrag besitzt einen Ablaufzeitstempel (`expires_at_unix`). Geht die Uhr eines Clients um mehr als 24 Stunden falsch, lehnt `swm-agent` Aufträge zum Schutz vor Replay-Angriffen ab -> NTP-Zeitsynchronisation auf dem Client aktivieren.

### Fall 5: `WinGet` (Windows) oder `Homebrew` (macOS) findet Paket nicht
- **Windows (`WinGet`):** Auf ganz frisch installierten Windows-10/11-LTSC- oder Windows-Server-Systemen ist Microsofts *App Installer* (`Microsoft.DesktopAppInstaller`) manchmal noch nicht vorinstalliert. Nutzen Sie auf solchen Servern direkte `.msi`- oder `.exe`-Pakete über das eingebaute NS8-Paket-Repository.
- **macOS (`Homebrew`):** `brew`- und `brew_cask`-Pakete setzen voraus, dass Homebrew (`/opt/homebrew/bin/brew` auf Apple Silicon bzw. `/usr/local/bin/brew` auf Intel) auf dem Mac vorhanden ist und einem regulären lokalen Benutzer gehört (niemals `root`). Für Macs komplett ohne Homebrew nutzen Sie im Katalog einfach direkte **`.pkg`- oder `.dmg`-Pakete** — diese benötigen nur macOS-Bordmittel (`installer` / `hdiutil`).

### Fall 6: Domänen-Login (LDAP / Active Directory) an der Web-Konsole schlägt fehl
1. Öffnen Sie **NS8 Cluster-Admin -> SoftwareManager -> Einstellungen** und prüfen Sie, ob die gewünschte Benutzerdomäne ausgewählt ist, und speichern Sie die Einstellungen ggf. einmal neu (aktualisiert die `10.0.2.2` `ldapproxy`-Bindung im Pod).
2. Wählen Sie auf der Login-Seite von `https://swm.firma.de` explizit den Modus **Domänen-Login (LDAP / AD)** und geben Sie den Benutzernamen ohne `@domain`-Suffix ein (z. B. `mmustermann`).
3. Der **Lokale Administrator** steht jederzeit unabhängig von der Erreichbarkeit des Domänencontrollers als Fallback zur Verfügung.

### Fall 7: `NS8 Software Center` Tray-Icon öffnet sich auf einer Workstation nicht
- Prüfen Sie in der Gerätedetail-Ansicht, ob das Gerät einer Gruppe mit deaktiviertem `enforce_tray` angehört oder mit `headless=1` installiert wurde.
- Wurde ein PC ursprünglich versehentlich mit `headless=1` registriert, führen Sie einfach einmalig den Bootstrap-Befehl mit `headless=0` erneut aus — die bestehende Geräte-ID bleibt dabei erhalten, und Startmenü-Verknüpfung sowie Session-Watchdog werden nachgerüstet.
