---
title: Installation
description: Installieren Sie ein bestimmtes Release, probieren Sie Kaeso auf Ihrem Rechner aus, ändern Sie die Dienste und führen Sie Befehle aus.
order: 3
---

Der [Schnellstart](../get-started/quick-start.md) installiert das neueste Release auf einem Server. Diese Seite zeigt, was Sie anders machen können, und die Befehle, die Sie auf einer Installation ausführen.

## Ein bestimmtes Release installieren

Der Schnellstart lädt die `compose.yaml` des neuesten stabilen Releases. Für ein anderes Release laden Sie die Datei vom Tag dieses Releases:

```bash
curl -fsSLO https://github.com/logicplanes/kaeso/releases/download/v{{version}}/compose.yaml
```

Jedem Release liegen außerdem zwei Beispieldateien bei: [`env.example`](https://github.com/logicplanes/kaeso/releases/latest/download/env.example) mit den üblichen Variablen der `.env` und [`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml) mit jeder Einstellung des Gateways.

## Auf dem eigenen Rechner ausprobieren

Setzen Sie `KAESO_DOMAIN=localhost` in der `.env`, starten Sie Kaeso und öffnen Sie `https://localhost`. Caddy signiert das Zertifikat mit seiner eigenen lokalen Zertifizierungsstelle, die Ihr Browser einmal bestätigen lässt. Ohne Mailserver setzen Sie außerdem `KAESO_MAIL_TRANSPORT=log` ([E-Mail](email.md#kaeso-ohne-mailserver-ausprobieren)).

curl und die SDKs vertrauen Caddys lokaler Zertifizierungsstelle nicht. Kopieren Sie ihr Stammzertifikat aus dem Container:

```bash
docker compose cp caddy:/data/caddy/pki/authorities/local/root.crt caddy-root.crt
```

Übergeben Sie das Zertifikat dann dem Client, der Kaeso aufruft:

```bash tab="curl"
curl --cacert caddy-root.crt https://localhost/v1/models -H "Authorization: Bearer $KAESO_API_KEY"
```

```bash tab="Python"
SSL_CERT_FILE=caddy-root.crt python app.py
```

```bash tab="TypeScript"
NODE_EXTRA_CA_CERTS=caddy-root.crt node app.mjs
```

## Dienste ändern

Schreiben Sie Ihre eigenen Änderungen an den Diensten in die `compose.override.yaml` neben der `compose.yaml`. Compose führt beide Dateien zusammen, und ein Upgrade ersetzt nur die `compose.yaml`, sodass Ihre Änderungen bleiben. So protokolliert das Gateway zum Beispiel ausführlicher:

```yaml title="compose.override.yaml"
services:
  gateway:
    environment:
      KAESO_LOG_LEVEL: info,kaeso_server=debug
```

Übernehmen Sie die Änderung mit `docker compose up -d --wait`. Compose erstellt nur die Dienste neu, deren Einstellungen sich geändert haben. [Konfiguration](configuration.md) listet jede Einstellung des Gateways.

## Kaeso-Befehle ausführen

Das Gateway-Image ist zugleich die Kommandozeile von Kaeso. Führen Sie einen Befehl im laufenden Gateway-Container aus, wo er die Einstellungen des Gateways verwendet:

```bash
docker compose exec gateway kaeso user reset-2fa owner@example.com
```

| Befehl | Bewirkt |
| --- | --- |
| `kaeso setup-link` | Gibt den Einrichtungslink aus, solange die Installation keine Benutzer hat. Der Link funktioniert einmal, 24 Stunden lang, und ein neuer macht die älteren ungültig |
| `kaeso user sign-in-link <email>` | Gibt einen einmaligen Anmeldelink für einen aktiven Benutzer aus, 15 Minuten gültig ([Wieder hineinkommen](sign-in.md#wieder-hineinkommen)) |
| `kaeso user reset-2fa <email>` | Entfernt die Zwei-Faktor-Authentifizierung und die Wiederherstellungscodes eines Benutzers, beendet seine Sitzungen und benachrichtigt ihn per E-Mail. Das Audit-Log der Instanz hält es fest |
| `kaeso sso verify-domain <domain>` | Markiert eine E-Mail-Domain der SSO ohne ihren TXT-Eintrag als bestätigt, für eine Domain, für die das öffentliche DNS nicht antwortet. `--org <org>` nennt die Organisation, wenn mehrere die Domain führen. Das Audit-Log der Organisation verzeichnet es |
| `kaeso master-key generate` | Gibt einen neuen Hauptschlüssel als `id:base64` aus |
| `kaeso master-key rewrap` | Verschlüsselt gespeicherte Secrets mit dem ersten Hauptschlüssel neu und meldet, welche Schlüssel-IDs noch verwendet werden ([Hauptschlüssel rotieren](backups.md#hauptschlüssel-rotieren)) |
| `kaeso migrate` | Führt die Migrationen von PostgreSQL und ClickHouse aus und beendet sich |
| `kaeso serve` | Startet das Gateway. Das ist der Standardbefehl des Images |

## Nächste Schritte

- [Konfiguration](configuration.md): Schlagen Sie jede Einstellung nach und wohin sie gehört.
- [Domains und TLS](domains-and-tls.md): Beziehen Sie ein Zertifikat oder betreiben Sie Kaeso hinter Ihrem eigenen Proxy.
- [Backups und Hauptschlüssel](backups.md): Sichern Sie die Datenbanken und bewahren Sie die Hauptschlüssel sicher auf.
