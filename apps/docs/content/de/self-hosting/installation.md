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
| `master-keys` | Die Hauptschlüssel, dem Gateway als Docker-Secret `master_keys` übergeben. |
| `compose.override.yaml` | Optional: eigene Änderungen an den Diensten, die erhalten bleiben, wenn `compose.yaml` ersetzt wird. |

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
cat > .env <<EOF
KAESO_DOMAIN=kaeso.example.com
POSTGRES_PASSWORD=$(openssl rand -hex 24)
POSTGRES_SUPERUSER_PASSWORD=$(openssl rand -hex 24)
CLICKHOUSE_PASSWORD=$(openssl rand -hex 24)
VALKEY_PASSWORD=$(openssl rand -hex 24)
EOF
docker run --rm ghcr.io/logicplanes/kaeso-gateway:latest master-key generate > master-keys
chmod 644 master-keys
```

Jedes Release bringt außerdem [`env.example`](https://github.com/logicplanes/kaeso/releases/latest/download/env.example) mit, jede Variable von `.env` mit einem Beispielwert, und [`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml), jede Einstellung des Gateways; diese Links laden wie das `curl` oben die Dateien des neuesten stabilen Releases. Um ein bestimmtes Release zu installieren, stattdessen dessen Dateien von seinem Tag herunterladen, etwa `https://github.com/logicplanes/kaeso/releases/download/v{{version}}/compose.yaml`.

Das Gateway läuft in seinem Container als Benutzer ohne Sonderrechte und liest `master-keys` von dort, daher ist die Datei selbst lesbar; der Ordner hält sie vor anderen Benutzern des Servers verschlossen. `.env` und `master-keys` getrennt von den Datenbank-Backups sichern; siehe [Backups und Hauptschlüssel](backups.md).

## Die Datei .env

`compose.yaml` liest diese Variablen aus `.env`:

| Variable | Erforderlich | Was sie festlegt |
| --- | --- | --- |
| `KAESO_DOMAIN` | ja | Die Domain, die Caddy bereitstellt und für die es ein Zertifikat holt. Die öffentliche URL ist `https://` und die Domain. |
| `POSTGRES_PASSWORD` | ja | Das Passwort des PostgreSQL-Benutzers `kaeso`, mit dem sich das Gateway anmeldet. Ihm gehört die Datenbank `kaeso`, und er ist kein Superuser. |
| `POSTGRES_SUPERUSER_PASSWORD` | ja | Das Passwort des PostgreSQL-Superusers `postgres`, für die eigene Administration. Kaeso verwendet es nie, und `docker compose exec postgres psql -U postgres` meldet sich ohne es an. |
| `CLICKHOUSE_PASSWORD` | ja | Das Passwort des ClickHouse-Benutzers `kaeso`. |
| `VALKEY_PASSWORD` | ja | Das Passwort, das Valkey von jeder Verbindung verlangt. |
| `KAESO_PUBLIC_URL` | nein | Die öffentliche URL, wenn sie nicht `https://` und `KAESO_DOMAIN` ist, etwa hinter einem eigenen Proxy. |
| `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_TLS`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | `KAESO_SMTP_HOST`, außer E-Mails werden nur protokolliert | Wie das Gateway E-Mails sendet. Siehe [E-Mail](email.md). |
| `KAESO_MAIL_TRANSPORT` | nein | `log`, um E-Mails nur zu protokollieren statt sie zu senden, zum Ausprobieren von Kaeso. |

Die Passwörter stehen in Verbindungs-URLs, daher nur Buchstaben und Ziffern verwenden, wie `openssl rand -hex` sie schreibt. Die Passwörter von PostgreSQL und ClickHouse wirken, wenn die Datenbanken zum ersten Mal in ihren Volumes angelegt werden; sie später in `.env` zu ändern, ändert nicht die Passwörter, die die Datenbanken schon haben. Valkey liest sein Passwort bei jedem Start aus `.env`.

Compose liest aus `.env` nur diese Variablen. Jede andere [Einstellung](configuration.md) gehört in `compose.override.yaml`, wie [lokale Änderungen](#lokale-änderungen) zeigt.

## Was beim ersten Start geschieht

```bash
docker compose up -d --wait
docker compose exec gateway kaeso setup-link
```

Mit `--wait` kehrt `docker compose up` zurück, sobald jeder Dienst läuft und jeder Dienst mit Healthcheck, also alle außer Caddy, healthy meldet, und schlägt fehl, wenn einer das nicht tut. Ohne `--wait` kehrt Compose zurück, sobald es die Container gestartet hat: Mit dem mitgelieferten Caddy, der auf ein healthy Gateway und eine healthy Konsole wartet, ist das spät genug, hinter [einem eigenen Reverse Proxy](domains-and-tls.md#ein-eigener-reverse-proxy) aber sofort, bevor das Gateway für `kaeso setup-link` bereit ist. Danach gibt `kaeso setup-link` den Einrichtungslink aus.

1. PostgreSQL, ClickHouse und Valkey starten; Compose wartet, bis PostgreSQL und Valkey healthy melden.
2. Das Gateway prüft seine Einstellungen und hält mit einer Meldung an, die die Einstellung nennt, wenn eine fehlt oder falsch ist. Geheime Werte gibt es nie aus.
3. Es migriert PostgreSQL auf seine Version, bevor es irgendetwas bereitstellt, und ClickHouse im Hintergrund, mit Wiederholungen, bis ClickHouse antwortet; bis dahin meldet `/readyz` `degraded`. Migrationen laufen unter einem Lock, sodass nur eine Gateway-Instanz migriert, während die anderen warten.
4. Solange die Installation keine Benutzer hat, protokolliert es, dass sie noch nicht eingerichtet ist und wie man den Einrichtungslink erhält, nie den Link selbst: Wer das Log liest, könnte damit die Installation übernehmen. `docker compose exec gateway kaeso setup-link` gibt den Link im Terminal aus; er funktioniert einmal, 24 Stunden lang, und jeder neue macht die älteren ungültig.
5. Sobald Gateway und Konsole healthy sind, startet Caddy und holt ein Zertifikat für `KAESO_DOMAIN`.

Die Einrichtungsseite legt den ersten Benutzer an, der Instanz-Admin und Inhaber der ersten Organisation wird, mit einem Standardprojekt, in dem er Admin ist. Danach kommen Personen per Einladung hinzu oder durch Registrierung, wo die Instanz sie erlaubt; siehe [Instanzverwaltung](../console/instance-administration.md).

## Auf dem eigenen Rechner ausprobieren

`KAESO_DOMAIN=localhost` setzen und `https://localhost` öffnen. Caddy stellt die Seite mit einem Zertifikat seiner eigenen lokalen Zertifizierungsstelle bereit, das der Browser einmal bestätigen lässt. `KAESO_MAIL_TRANSPORT=log` zu `.env` hinzufügen, wenn kein Mailserver zur Hand ist.

curl und die SDKs vertrauen Caddys lokaler Zertifizierungsstelle nicht. Ihr Zertifikat aus dem Container kopieren und ihnen übergeben:

```bash
docker compose cp caddy:/data/caddy/pki/authorities/local/root.crt caddy-root.crt
curl --cacert caddy-root.crt https://localhost/v1/models -H "Authorization: Bearer $KAESO_API_KEY"
```

Die Python-SDKs lesen es aus `SSL_CERT_FILE=caddy-root.crt`, die TypeScript-SDKs aus `NODE_EXTRA_CA_CERTS=caddy-root.crt`.

## Lokale Änderungen

Um eine Einstellung eines Dienstes zu ändern oder hinzuzufügen, sie in `compose.override.yaml` neben `compose.yaml` eintragen. Docker Compose führt beide zusammen, und ein Upgrade, das `compose.yaml` ersetzt, behält die eigenen Änderungen.

```yaml
services:
  gateway:
    environment:
      KAESO_LOG_LEVEL: info,kaeso_server=debug
      KAESO_INSTANCE_ACCOUNT_CREATION: everyone
```

Jede Einstellung steht unter [Konfiguration](configuration.md). Änderungen mit `docker compose up -d --wait` übernehmen; Compose erstellt nur die Dienste neu, deren Einstellungen sich geändert haben.

## Die Kommandozeile

Das Gateway-Image ist zugleich seine Kommandozeile. Seine Befehle im laufenden Container ausführen, wo sie die Einstellungen des Gateways nutzen:

| Befehl | Was er tut |
| --- | --- |
| `kaeso setup-link` | Gibt einen neuen Einrichtungslink aus, solange die Installation keine Benutzer hat; ältere Links funktionieren dann nicht mehr |
| `kaeso master-key generate` | Gibt einen neuen Hauptschlüssel als `id:base64` aus |
| `kaeso master-key rewrap` | Verschlüsselt gespeicherte Secrets mit dem ersten Hauptschlüssel neu und meldet, welche Schlüssel-IDs noch genutzt werden; siehe [den Hauptschlüssel rotieren](backups.md#den-hauptschlüssel-rotieren) |
| `kaeso user reset-2fa <email>` | Entfernt die Zwei-Faktor-Authentifizierung und die Wiederherstellungscodes eines Benutzers, beendet seine Sitzungen und benachrichtigt ihn per E-Mail |
| `kaeso user sign-in-link <email>` | Gibt einen einmaligen Anmeldelink für einen Benutzer aus, 15 Minuten gültig, auch wenn Anmeldelinks per E-Mail ausgeschaltet sind; für den Fall, dass kein anderer Weg hineinführt, etwa bevor Single Sign-on eingerichtet ist; siehe [Wieder hineinkommen](sign-in.md#wieder-hineinkommen) |
| `kaeso migrate` | Führt die Migrationen von PostgreSQL und ClickHouse aus und beendet sich dann |
| `kaeso serve` | Startet das Gateway; der Standardbefehl des Images |

Zum Beispiel:

```bash
docker compose exec gateway kaeso user reset-2fa owner@example.com
```
