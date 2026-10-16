---
title: Installation
description: Die Dateien einer Installation, ihr erster Start, lokale Änderungen und die Kommandozeile.
order: 2
---

Der [Schnellstart](../get-started/quick-start.md) ist die Kurzfassung dieser Seite.

## Die Dateien

Eine Installation ist ein Ordner, den nur der eigene Benutzer lesen kann, mit diesen Dateien:

| Datei | Was sie enthält |
| --- | --- |
| `compose.yaml` | Die Dienste, aus dem Kaeso-Release. Für ein Upgrade durch die Datei des neuen Releases ersetzen. |
| `.env` | Die eigenen Einstellungen: die Domain, die Datenbankpasswörter, E-Mail. Docker Compose liest sie aus demselben Ordner. |
| `master-keys` | Die Master-Schlüssel, dem Gateway als Docker-Secret `master_keys` übergeben. |
| `compose.override.yaml` | Optional: eigene Änderungen an den Diensten, die erhalten bleiben, wenn `compose.yaml` ersetzt wird. |

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
printf 'KAESO_DOMAIN=kaeso.example.com\nPOSTGRES_PASSWORD=%s\nCLICKHOUSE_PASSWORD=%s\n' "$(openssl rand -hex 24)" "$(openssl rand -hex 24)" > .env
docker run --rm ghcr.io/logicplanes/kaeso-gateway:latest master-key generate > master-keys
chmod 644 master-keys
```

Um ein bestimmtes Release zu installieren, stattdessen dessen Datei herunterladen: `https://github.com/logicplanes/kaeso/releases/download/v0.1.0/compose.yaml`. Die Datei im Repository, [`deploy/compose.yaml`](https://github.com/logicplanes/kaeso/blob/dev/deploy/compose.yaml), ist die, die das nächste Release mitbringt.

Das Gateway läuft in seinem Container als Benutzer ohne Sonderrechte und liest `master-keys` von dort, daher ist die Datei selbst lesbar; der Ordner hält sie vor anderen Benutzern des Servers verschlossen. `.env` und `master-keys` getrennt von den Datenbank-Backups sichern; siehe [Backups und Master-Schlüssel](backups.md).

## Die Datei .env

`compose.yaml` liest diese Variablen aus `.env`:

| Variable | Erforderlich | Was sie festlegt |
| --- | --- | --- |
| `KAESO_DOMAIN` | ja | Die Domain, die Caddy bereitstellt und für die es ein Zertifikat holt. Die öffentliche URL ist `https://` und die Domain. |
| `POSTGRES_PASSWORD` | ja | Das Passwort des PostgreSQL-Benutzers `kaeso`. |
| `CLICKHOUSE_PASSWORD` | ja | Das Passwort des ClickHouse-Benutzers `kaeso`. |
| `KAESO_PUBLIC_URL` | nein | Die öffentliche URL, wenn sie nicht `https://` und `KAESO_DOMAIN` ist, etwa hinter einem eigenen Proxy. |
| `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_TLS`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | `KAESO_SMTP_HOST`, außer E-Mails werden nur protokolliert | Wie das Gateway E-Mails sendet. Siehe [E-Mail](email.md). |
| `KAESO_MAIL_TRANSPORT` | nein | `log`, um E-Mails nur zu protokollieren statt sie zu senden, zum Ausprobieren von Kaeso. |

Die Datenbankpasswörter wirken, wenn die Datenbanken zum ersten Mal in ihren Volumes angelegt werden. Sie später in `.env` zu ändern, ändert nicht die Passwörter, die die Datenbanken schon haben.

## Was beim ersten Start geschieht

```bash
docker compose up -d
docker compose logs gateway | grep setup_url
```

1. PostgreSQL, ClickHouse und Valkey starten; Compose wartet, bis PostgreSQL und Valkey healthy melden.
2. Das Gateway prüft seine Einstellungen und hält mit einer Meldung an, die die Einstellung nennt, wenn eine fehlt oder falsch ist. Geheime Werte gibt es nie aus.
3. Es migriert PostgreSQL auf seine Version, bevor es irgendetwas bereitstellt, und ClickHouse im Hintergrund, mit Wiederholungen, bis ClickHouse antwortet; bis dahin meldet `/readyz` `degraded`. Migrationen laufen unter einem Lock, sodass nur eine Gateway-Instanz migriert, während die anderen warten.
4. Solange die Installation keine Benutzer hat, protokolliert es einen Einrichtungslink. Der Link funktioniert einmal, 24 Stunden lang. Jeder Neustart und `docker compose exec gateway kaeso setup-link` geben einen neuen aus und machen die alten ungültig.
5. Sobald Gateway und Konsole healthy sind, startet Caddy und holt ein Zertifikat für `KAESO_DOMAIN`.

Die Einrichtungsseite legt den ersten Benutzer an, der Instanz-Admin und Inhaber der ersten Organisation wird, mit einem Standardprojekt, in dem er Admin ist. Danach kommen Personen per Einladung hinzu oder durch Registrierung, wo die Instanz sie erlaubt; siehe [Instanzverwaltung](../console/instance-administration.md).

## Auf dem eigenen Rechner ausprobieren

`KAESO_DOMAIN=localhost` setzen und `https://localhost` öffnen. Caddy stellt die Seite mit einem Zertifikat seiner eigenen lokalen Zertifizierungsstelle bereit, das der Browser einmal bestätigen lässt. `KAESO_MAIL_TRANSPORT=log` zu `.env` hinzufügen, wenn kein Mailserver zur Hand ist.

## Lokale Änderungen

Um eine Einstellung eines Dienstes zu ändern oder hinzuzufügen, sie in `compose.override.yaml` neben `compose.yaml` eintragen. Docker Compose führt beide zusammen, und ein Upgrade, das `compose.yaml` ersetzt, behält die eigenen Änderungen.

```yaml
services:
  gateway:
    environment:
      KAESO_LOG_LEVEL: info,kaeso_server=debug
      KAESO_INSTANCE_ACCOUNT_CREATION: everyone
```

Jede Einstellung steht unter [Konfiguration](configuration.md). Änderungen mit `docker compose up -d` übernehmen; Compose erstellt nur die Dienste neu, deren Einstellungen sich geändert haben.

## Die Kommandozeile

Das Gateway-Image ist zugleich seine Kommandozeile. Seine Befehle im laufenden Container ausführen, wo sie die Einstellungen des Gateways nutzen:

| Befehl | Was er tut |
| --- | --- |
| `kaeso setup-link` | Gibt einen neuen Einrichtungslink aus, solange die Installation keine Benutzer hat; ältere Links funktionieren dann nicht mehr |
| `kaeso master-key generate` | Gibt einen neuen Master-Schlüssel als `id:base64` aus |
| `kaeso master-key rewrap` | Verschlüsselt gespeicherte Secrets mit dem ersten Master-Schlüssel neu und meldet, welche Schlüssel-IDs noch genutzt werden; siehe [den Master-Schlüssel rotieren](backups.md#den-master-schlüssel-rotieren) |
| `kaeso user reset-2fa <email>` | Entfernt die Zwei-Faktor-Authentifizierung und die Wiederherstellungscodes eines Benutzers, beendet seine Sitzungen und benachrichtigt ihn per E-Mail |
| `kaeso migrate` | Führt die Migrationen von PostgreSQL und ClickHouse aus und beendet sich dann |
| `kaeso serve` | Startet das Gateway; der Standardbefehl des Images |

Zum Beispiel:

```bash
docker compose exec gateway kaeso user reset-2fa owner@example.com
```
