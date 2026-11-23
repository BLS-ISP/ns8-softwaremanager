# NS8-SoftwareManager – Handbücher & Dokumentations-Zentrale (`v1.2.0`)

Hier finden Sie sämtliche **Benutzer-, Rollout- und Betriebshandbücher** sowie die **technische Entwickler- und Architekturdokumentation** für **NS8-SoftwareManager**.

---

## 1. Praxis- & Benutzerhandbücher (Deutsch)

| Handbuch | Zielgruppe | Inhalt |
|---|---|---|
| **[Administrator-Handbuch](admin-handbuch.md)** | IT-Administratoren & Systemhäuser | Komplette Einrichtung auf NethServer 8, Gruppen-/Ring-Aufbau, Wartungsfenster, Echtzeit-Flottensteuerung (Sync, Desktop-Nachrichten, Remote-Reboot, Sammelaktionen), OS-Patching (`WUA`, `APT`/`DNF`, `softwareupdate`), Paket-Katalog (`WinGet`, `Homebrew`, `.msi`/`.pkg`/`.dmg`), Kiosk-Härtung & Agent-Self-Update |
| **[Praxishandbuch Client-Rollout](client-rollout-handbuch.md)** | Rollout-Techniker & DevOps | Schritt-für-Schritt-Anleitungen für Einzelplatz-Installation, **Active Directory GPO (`gpmc.msc`)**, **Microsoft Intune** (Windows & macOS), **Jamf Pro / Kandji / Mosyle**, **Ansible & `.deb`/`.rpm`**, **VM-Vorlagen / Gold-Images** (Klon-Regel!) sowie saubere Client-Deinstallation |
| **[Endbenutzer-Handbuch: NS8 Software Center](benutzerhandbuch-software-center.md)** | Mitarbeiterinnen & Mitarbeiter *(Intranet-Vorlage)* | Leicht verständliche Anleitung ohne Fachbegriffe: Wo finde ich das Software Center (Windows, Mac, Linux)? Programme ohne Admin-Rechte installieren & aktualisieren, Neustart-Erinnerungen verschieben (*Snooze*), IT-Hinweise lesen & 1-Klick-Diagnosebericht für den Support kopieren |
| **[Troubleshooting, Log-Analyse & Betrieb](troubleshooting-und-betrieb.md)** | Helpdesk & 2nd-Level-Support | Übersichts-Tabelle aller Dienst-, Konfigurations-, Socket- und Log-Pfade auf NS8, Windows, Linux & macOS sowie konkrete Lösungen für häufige Fehlerbilder (Pending Approval, WebSockets/Proxy, Exit-Codes `1618`/`3010`, Zeitsynchronisation, WinGet/Homebrew, LDAP/AD) |

---

## 2. Technische Referenz & Architektur (Englisch / Deutsch)

| Dokument | Inhalt |
|---|---|
| **[../README.md](../README.md)** | Projektübersicht, Sicherheitsmodell, NS8-Installation (`ns8-nethforge` & CLI) und Build-Befehle |
| **[../CONTRACT.md](../CONTRACT.md)** | Kanonische Spezifikation für REST-API, WebSockets, lokales IPC (`swm-common`) und `Ed25519`-Signaturen |
| **[../REQUIREMENTS.md](../REQUIREMENTS.md)** | Produktvision, Marktanalyse, umgesetzter Funktionsumfang (`v1.2.0`) und Backlog |
| **[../CHANGELOG.md](../CHANGELOG.md)** | Vollständige Versionshistorie (`1.0.0` bis `1.2.0`) |
| **[../agent/README.md](../agent/README.md)** | Rust-Workspace (`swm-agent`, `swm-center`, `swm-common`), Plattformmatrix & CLI-Referenz |
| **[../server/README.md](../server/README.md)** | FastAPI-Backend, Live-Datenbankmigration & NS8-`Ldapproxy`-Anbindung |
| **[../web/README.md](../web/README.md)** & **[../ui/README.md](../ui/README.md)** | Eigenständige Vue-3-Konsole (`web/`) vs. natives NS8-Cockpit-Modul (`ui/`) |

---

## 3. Architecture Decision Records (`decisions/`)

- **[ADR-001](decisions/ADR-001-ed25519-signed-jobs-and-unprivileged-ipc.md):** Kryptografisch signierte Aufträge (`Ed25519`) und strikte lokale IPC-Privilegientrennung
- **[ADR-002](decisions/ADR-002-dual-ui-ns8-cockpit-and-standalone-console.md):** Zwei-UI-Architektur — reines NS8-`core.css`-Modul (`ui/`) + eigenständige Admin-Web-Konsole (`web/`)
- **[ADR-003](decisions/ADR-003-hardened-desktop-client-and-macos-universal-binaries.md):** Zweistufiger Session-Watchdog, nicht-privilegierte Homebrew-Ausführung & macOS Universal Binaries (`lipo`)
