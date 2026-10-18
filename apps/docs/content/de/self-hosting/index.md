---
title: Selbst hosten
description: Eine Kaeso-Installation betreiben, konfigurieren und pflegen.
order: 5
---

Eine Kaeso-Installation besteht aus sechs Containern hinter einer Domain; gestartet werden sie von der `compose.yaml`, die jedem Release beiliegt. Der [Schnellstart](../get-started/quick-start.md) bringt eine Installation zum Laufen, diese Seiten behandeln alles Weitere.

| Dienst | Image | Aufgabe | Erreichbar von |
| --- | --- | --- | --- |
| `caddy` | `caddy:2.11.7-alpine` | Terminiert TLS mit einem Zertifikat, das er selbst bezieht und erneuert, und leitet Anfragen weiter | Dem Internet, Ports 80 und 443 |
| `gateway` | `ghcr.io/logicplanes/kaeso-gateway` | Stellt die Modell-API (`/v1`) und die Platform API (`/platform`) bereit | Caddy |
| `console` | `ghcr.io/logicplanes/kaeso-console` | Stellt die Weboberfläche bereit (alle übrigen Pfade) | Caddy |
| `postgres` | `postgres:18.6-alpine` | Benutzer, Organisationen, Schlüssel, verschlüsselte Anbieter-Schlüssel, Kataloge, Limits, Nutzung, Audit-Logs | Nur dem Gateway |
| `clickhouse` | `clickhouse/clickhouse-server:26.3.48.2-alpine` | Anfrageprotokolle und Nutzungsaggregate | Nur dem Gateway |
| `valkey` | `valkey/valkey:9.1.2-alpine` | Raten- und Budgetzähler, gemeinsame Caches | Nur dem Gateway |

Die Images von Gateway und Konsole tragen die Version des Releases, dessen `compose.yaml` läuft; die Images von Drittanbietern sind mit Version und Digest festgelegt.

## Isolation

- Die Datenbanken liegen in einem internen Netzwerk ohne Weg nach außen. Ihre Daten liegen in den benannten Volumes `postgres`, `clickhouse` und `valkey`; Caddy hält seine Zertifikate in `caddy_data` und `caddy_config`.
- Kein Container kann Rechte hinzugewinnen. Gateway und Konsole laufen ohne Linux-Capabilities, das Dateisystem des Gateways ist schreibgeschützt. Caddy behält nur das Recht, die Ports 80 und 443 zu belegen, und sendet `Strict-Transport-Security`.
- Das Gateway meldet sich bei PostgreSQL als `kaeso` an; diesem Benutzer gehört die Kaeso-Datenbank, ein Superuser ist er nicht. Der ClickHouse-Benutzer `kaeso` kann keine Benutzer und Berechtigungen verwalten. Valkey verlangt von jeder Verbindung sein Passwort.
- Den Metrik-Port des Gateways, 9100, leitet Caddy nicht weiter.

## Mehr als ein Gateway

Gateway-Instanzen halten keinen eigenen Zustand. Für mehrere Instanzen diese hinter denselben Proxy stellen und auf dieselben PostgreSQL-, ClickHouse- und Valkey-Dienste verweisen. Caches werden instanzübergreifend über Valkey invalidiert, Hintergrundjobs laufen unter PostgreSQL-Locks jeweils auf einer Instanz, und jede Instanz puffert und schreibt ihre eigenen Zeilen der Anfrageprotokolle und ihre eigene Nutzung.

## Seiten

| Seite | Inhalt |
| --- | --- |
| [Anforderungen](requirements.md) | Servergröße, Netzwerk, Speicherplatz für Anfrageprotokolle |
| [Installation](installation.md) | Die Dateien einer Installation, der erste Start, lokale Änderungen, die Kommandozeile |
| [Konfiguration](configuration.md) | Wie Einstellungen gelesen werden, jede `KAESO_*`-Einstellung, die Drosselung von API-Schlüsseln |
| [Domains und TLS](domains-and-tls.md) | Die Domain, Zertifikate, ein eigener Reverse Proxy, lange Streams |
| [E-Mail](email.md) | SMTP, was Kaeso in welcher Sprache sendet, Testen |
| [Anmeldung und Single Sign-on](sign-in.md) | Anmeldemethoden, Installationen nur mit Single Sign-on, wieder hineinkommen |
| [Upgrades](upgrades.md) | Der Wechsel auf ein neues Release |
| [Backups und Hauptschlüssel](backups.md) | Was gesichert wird, Wiederherstellen, Rotieren des Hauptschlüssels |
| [Zustand und Metriken](monitoring.md) | Health-Checks, Prometheus-Metriken, Logs |
| [Fehlerbehebung](troubleshooting.md) | Häufige Probleme und ihre Lösung |
