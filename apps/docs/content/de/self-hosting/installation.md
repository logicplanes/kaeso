---
title: Installation
description: Die Dateien einer Installation, ihr erster Start, lokale Änderungen und die Kommandozeile.
order: 2
---

Der [Schnellstart](../get-started/quick-start.md) ist die Kurzfassung dieser Seite.

## Die Dateien

Eine Installation ist ein Ordner, auf den nur der eigene Benutzer zugreifen kann, mit diesen Dateien:

| Datei | Inhalt |
| --- | --- |
| `compose.yaml` | Die Dienste, aus dem Kaeso-Release. Für ein Upgrade durch die Datei des neuen Releases ersetzen. |
| `.env` | Die eigenen Einstellungen: Domain, Datenbankpasswörter, E-Mail. Docker Compose liest sie aus demselben Ordner. |
| `master-keys` | Die Hauptschlüssel, die das Gateway als Docker-Secret `master_keys` erhält. |
| `compose.override.yaml` | Optional: eigene Änderungen an den Diensten, die erhalten bleiben, wenn `compose.yaml` ersetzt wird. |

Ordner anlegen, `compose.yaml` herunterladen, `.env` mit zufälligen Passwörtern schreiben und einen Hauptschlüssel erzeugen:

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

Das Gateway läuft in seinem Container als Benutzer ohne Rechte, daher muss `master-keys` lesbar sein; die Rechte des Ordners halten andere Benutzer des Servers fern. Kopien von `.env` und `master-keys` getrennt von den Datenbank-Backups aufbewahren; siehe [Backups und Hauptschlüssel](backups.md).

Das `curl` oben lädt das neueste stabile Release. Für ein bestimmtes Release stattdessen von dessen Tag laden, etwa `https://github.com/logicplanes/kaeso/releases/download/v{{version}}/compose.yaml`. Jedes Release enthält außerdem [`env.example`](https://github.com/logicplanes/kaeso/releases/latest/download/env.example) mit jeder Variable der `.env` und einem Beispielwert sowie [`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml) mit jeder Einstellung des Gateways.

## Die Datei .env

`compose.yaml` liest diese Variablen aus der `.env`:

| Variable | Pflicht | Was sie festlegt |
| --- | --- | --- |
| `KAESO_DOMAIN` | ja | Die Domain, die Caddy bedient und für die er ein Zertifikat holt. Die öffentliche URL ist `https://` und die Domain. |
| `POSTGRES_PASSWORD` | ja | Das Passwort des PostgreSQL-Benutzers `kaeso`, als der sich das Gateway anmeldet. Ihm gehört die Datenbank `kaeso`; ein Superuser ist er nicht. |
| `POSTGRES_SUPERUSER_PASSWORD` | ja | Das Passwort des PostgreSQL-Superusers `postgres`, für die eigene Administration. Kaeso nutzt es nie, und `docker compose exec postgres psql -U postgres` meldet sich ohne es an. |
| `CLICKHOUSE_PASSWORD` | ja | Das Passwort des ClickHouse-Benutzers `kaeso`. |
| `VALKEY_PASSWORD` | ja | Das Passwort, das Valkey von jeder Verbindung verlangt. |
| `KAESO_PUBLIC_URL` | nein | Die öffentliche URL, wenn sie nicht `https://` und `KAESO_DOMAIN` ist, etwa hinter einem eigenen Proxy. |
| `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_TLS`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | `KAESO_SMTP_HOST`, außer E-Mails werden nur protokolliert | Wie das Gateway E-Mails versendet. Siehe [E-Mail](email.md). |
| `KAESO_MAIL_TRANSPORT` | nein | `log`, um E-Mails nur zu protokollieren statt zu senden, zum Ausprobieren. |

In den Passwörtern nur Buchstaben und Ziffern verwenden, wie `openssl rand -hex` sie erzeugt, denn sie landen in Verbindungs-URLs. PostgreSQL und ClickHouse übernehmen ihre Passwörter, wenn ihre Volumes zum ersten Mal angelegt werden; eine spätere Änderung in der `.env` ändert die Passwörter der Datenbanken nicht. Valkey liest sein Passwort bei jedem Start aus der `.env`.

Compose reicht nur diese Variablen weiter. Jede andere [Einstellung](configuration.md) gehört in die `compose.override.yaml`, wie [Lokale Änderungen](#lokale-änderungen) zeigt.

## Was beim ersten Start geschieht

Die Installation starten, dann den Einrichtungslink ausgeben:

```bash
docker compose up -d --wait
docker compose exec gateway kaeso setup-link
```

Mit `--wait` kehrt Compose erst zurück, wenn jeder Dienst healthy meldet, und schlägt fehl, wenn einer das nicht schafft. Caddy gilt als healthy, sobald er Anfragen bedient; das Zertifikat für eine neue Domain kann dann noch unterwegs sein, siehe [Zertifikate](domains-and-tls.md#zertifikate). Ohne `--wait` kehrt Compose zurück, sobald die Container existieren; hinter einem [eigenen Reverse Proxy](domains-and-tls.md#ein-eigener-reverse-proxy), wo kein Caddy auf das Gateway wartet, ist das zu früh für `kaeso setup-link`.

1. PostgreSQL, ClickHouse und Valkey starten. Das Gateway wartet, bis PostgreSQL und Valkey healthy melden.
2. Das Gateway prüft seine Einstellungen. Fehlt eine oder ist sie ungültig, hält es mit einer Meldung an, die die Einstellung nennt, nie ihren Wert.
3. Es migriert PostgreSQL, bevor es irgendetwas bereitstellt, und ClickHouse im Hintergrund, mit neuen Versuchen, bis ClickHouse antwortet; bis dahin meldet `/readyz` `degraded`. Migrationen laufen unter einem Lock, sodass nur eine Gateway-Instanz migriert und die anderen warten.
4. Solange die Installation keine Benutzer hat, protokolliert das Gateway, dass sie noch nicht eingerichtet ist und wie man den Einrichtungslink erhält, aber nie den Link selbst: Wer das Log lesen kann, könnte sonst die Installation übernehmen. `kaeso setup-link` gibt den Link im Terminal aus. Er funktioniert einmal, 24 Stunden lang, und jeder neue Link macht die älteren ungültig.
5. Sobald Gateway und Konsole healthy sind, startet Caddy, meldet healthy, sobald er Anfragen bedient, und holt im Hintergrund ein Zertifikat für `KAESO_DOMAIN`.

Der Einrichtungslink legt den ersten Benutzer an. Er wird Instanz-Admin, Inhaber der ersten Organisation und Admin ihres Standardprojekts. Alle anderen kommen per Einladung dazu oder registrieren sich, wo die Instanz das erlaubt; siehe [Instanzverwaltung](../console/instance-administration.md). Eine Installation, die sich nur per Single Sign-on anmelden soll, wird eingerichtet, wie [Eine Installation ohne Passwörter](sign-in.md#eine-installation-ohne-passwörter) beschreibt.

## Auf dem eigenen Rechner ausprobieren

`KAESO_DOMAIN=localhost` setzen und `https://localhost` öffnen. Caddy liefert die Seite mit einem Zertifikat seiner eigenen lokalen Zertifizierungsstelle aus, das der Browser einmal bestätigen lässt. Ohne Mailserver zur Hand `KAESO_MAIL_TRANSPORT=log` in die `.env` aufnehmen.

curl und die SDKs vertrauen Caddys lokaler Zertifizierungsstelle nicht. Ihr Stammzertifikat aus dem Container kopieren und ihnen übergeben:

```bash
docker compose cp caddy:/data/caddy/pki/authorities/local/root.crt caddy-root.crt
curl --cacert caddy-root.crt https://localhost/v1/models -H "Authorization: Bearer $KAESO_API_KEY"
```

Die Python-SDKs lesen es aus `SSL_CERT_FILE=caddy-root.crt`, die TypeScript-SDKs aus `NODE_EXTRA_CA_CERTS=caddy-root.crt`.

## Lokale Änderungen

Eigene Änderungen an den Diensten gehören in die `compose.override.yaml` neben der `compose.yaml`. Docker Compose führt beide Dateien zusammen, und ein Upgrade, das `compose.yaml` ersetzt, behält die Änderungen:

```yaml
services:
  gateway:
    environment:
      KAESO_LOG_LEVEL: info,kaeso_server=debug
      KAESO_INSTANCE_ACCOUNT_CREATION: everyone
```

[Konfiguration](configuration.md) listet jede Einstellung. Änderungen mit `docker compose up -d --wait` übernehmen; Compose erstellt nur die Dienste neu, deren Einstellungen sich geändert haben.

## Die Kommandozeile

Das Gateway-Image ist zugleich die Kommandozeile von Kaeso. Ihre Befehle im laufenden Container ausführen, wo sie die Einstellungen des Gateways verwenden:

```bash
docker compose exec gateway kaeso user reset-2fa owner@example.com
```

| Befehl | Was er tut |
| --- | --- |
| `kaeso setup-link` | Gibt einen neuen Einrichtungslink aus, solange die Installation keine Benutzer hat; ältere Links werden ungültig |
| `kaeso master-key generate` | Gibt einen neuen Hauptschlüssel als `id:base64` aus |
| `kaeso master-key rewrap` | Verschlüsselt gespeicherte Geheimnisse mit dem ersten Hauptschlüssel neu und meldet, welche Schlüssel-IDs noch verwendet werden; siehe [Den Hauptschlüssel rotieren](backups.md#den-hauptschlüssel-rotieren) |
| `kaeso user reset-2fa <email>` | Entfernt die Zwei-Faktor-Authentifizierung und die Wiederherstellungscodes eines Benutzers, beendet seine Sitzungen, benachrichtigt ihn per E-Mail und verzeichnet das im Audit-Log der Instanz |
| `kaeso user sign-in-link <email>` | Gibt einen einmaligen Anmeldelink für einen aktiven Benutzer aus, 15 Minuten gültig, für den Fall, dass kein anderer Weg hinein funktioniert; siehe [Wieder hineinkommen](sign-in.md#wieder-hineinkommen) |
| `kaeso migrate` | Führt die Migrationen von PostgreSQL und ClickHouse aus und beendet sich |
| `kaeso serve` | Startet das Gateway; der Standardbefehl des Images |
