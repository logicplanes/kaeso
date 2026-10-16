---
title: Selbst hosten
description: Eine Kaeso-Installation betreiben, konfigurieren und pflegen.
order: 2
---

Eine Kaeso-Installation ist eine Gruppe von Containern unter einer Domain. Die Compose-Datei, die jedem Kaeso-Release beiliegt, startet sie alle; der [Schnellstart](../get-started/quick-start.md) ist der schnellste Weg zu einer laufenden Installation.

| Dienst | Image | Was er tut | Erreichbar von |
| --- | --- | --- | --- |
| `caddy` | `caddy:2.11.7-alpine` | Terminiert TLS für die Domain, mit einem Zertifikat, das er selbst holt und erneuert, und leitet Anfragen weiter | dem Internet, Ports 80 und 443 |
| `gateway` | `ghcr.io/logicplanes/kaeso-gateway` | Stellt die Modell-API (`/v1`) und die Platform API (`/platform`) bereit | Caddy |
| `console` | `ghcr.io/logicplanes/kaeso-console` | Stellt die Weboberfläche bereit (jeder andere Pfad) | Caddy |
| `postgres` | `postgres:18.6-alpine` | PostgreSQL: Benutzer, Organisationen, Schlüssel, verschlüsselte Anbieter-Schlüssel, Kataloge, Limits, Nutzung, Audit-Logs | nur dem Gateway |
| `clickhouse` | `clickhouse/clickhouse-server:26.3.48.2-alpine` | ClickHouse: Anfrageprotokolle und Nutzungsaggregate | nur dem Gateway |
| `valkey` | `valkey/valkey:9.1.2-alpine` | Valkey: Raten- und Budgetzähler und gemeinsame Caches | nur dem Gateway |

Die Gateway- und Konsolen-Images tragen die Version des Releases, dessen `compose.yaml` verwendet wird. Die Datenbanken liegen in einem internen Netzwerk, das von außen nicht erreichbar ist, und ihre Daten in benannten Docker-Volumes (`postgres`, `clickhouse`, `valkey` und `caddy_data` für die Zertifikate). Den Metrik-Listener des Gateways, Port 9100, leitet Caddy nicht weiter. Kein Container kann Rechte hinzugewinnen, Gateway und Konsole laufen ohne Linux-Capabilities, und das Dateisystem des Gateways ist schreibgeschützt; Caddy behält nur das Recht, die Ports 80 und 443 zu belegen, und sendet `Strict-Transport-Security`. Das Gateway meldet sich bei PostgreSQL als `kaeso` an, dem die Kaeso-Datenbank gehört, der aber kein Superuser ist; der ClickHouse-Benutzer `kaeso` kann keine Benutzer und Berechtigungen verwalten; Valkey verlangt von jeder Verbindung sein Passwort. Die Images von Drittanbietern sind mit Version und Digest festgelegt.

## Mehr als ein Gateway

Gateway-Instanzen haben keinen eigenen Zustand. Um mehrere zu betreiben, sie hinter denselben Proxy stellen und auf dieselbe PostgreSQL-, ClickHouse- und Valkey-Instanz verweisen: Caches werden instanzübergreifend über Valkey invalidiert, Hintergrundjobs laufen unter PostgreSQL-Locks jeweils auf einer Instanz, und jede Instanz puffert und schreibt ihre eigenen Zeilen der Anfrageprotokolle und ihre eigene Nutzung.

## Seiten

| Seite | Was sie behandelt |
| --- | --- |
| [Anforderungen](requirements.md) | Servergröße, Festplatte für Anfrageprotokolle, Netzwerk |
| [Installation](installation.md) | Die Dateien einer Installation, was beim ersten Start geschieht, lokale Änderungen, die Kommandozeile |
| [Konfiguration](configuration.md) | Wie Einstellungen gelesen werden, jede `KAESO_*`-Einstellung und die Drosselung von API-Schlüsseln |
| [Domains und TLS](domains-and-tls.md) | Die Domain, Zertifikate, ein eigener Reverse Proxy, lange Streams |
| [E-Mail](email.md) | SMTP, was Kaeso sendet, Sprachen, Testen |
| [Upgrades](upgrades.md) | Der Wechsel auf ein neues Release |
| [Backups und Hauptschlüssel](backups.md) | Was gesichert wird, Wiederherstellen, Rotieren des Hauptschlüssels |
| [Zustand und Metriken](monitoring.md) | Health-Checks, Prometheus-Metriken und Logs |
| [Fehlerbehebung](troubleshooting.md) | Häufige Probleme und ihre Lösung |
