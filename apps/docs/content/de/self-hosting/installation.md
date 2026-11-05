---
title: Installation
description: Installieren Sie Kaeso oder ein Release von Hand, testen Sie es lokal, ändern Sie Dienste, führen Sie Befehle aus und entfernen Sie es.
order: 3
---

Der [Schnellstart](../get-started/quick-start.md) installiert das neueste Release mit einem Befehl auf einem Server. Diese Seite zeigt jeden Schritt dieses Befehls, was Sie anders machen können, und die Befehle, die Sie auf einer Installation ausführen.

## Von Hand installieren

Das [Installationsskript](../get-started/quick-start.md#kaeso-installieren) erledigt diese Schritte für Sie. Gehen Sie sie selbst durch, um Ordner, Passwörter und E-Mail-Einstellungen unterwegs zu wählen.

### Die Compose-Datei herunterladen

Jedes Release veröffentlicht eine `compose.yaml`, die seine Images auf Version und Digest festlegt. Laden Sie sie in einen neuen Ordner, den nur Sie lesen können:

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO \
  https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

### Die Einstellungen schreiben

Legen Sie `.env` mit Ihrer Domain und vier zufälligen Passwörtern an. `umask 077` macht sie, und jede Datei, die Sie in dieser Shell danach anlegen, nur für Sie lesbar:

```bash
umask 077
cat > .env <<EOF
KAESO_DOMAIN=kaeso.example.com
POSTGRES_PASSWORD=$(openssl rand -hex 24)
POSTGRES_SUPERUSER_PASSWORD=$(openssl rand -hex 24)
CLICKHOUSE_PASSWORD=$(openssl rand -hex 24)
VALKEY_PASSWORD=$(openssl rand -hex 24)
EOF
```

Das Gateway verschickt Anmeldelinks, Einladungen und Warnungen per E-Mail und startet nicht ohne einen Weg, sie zu senden. Ergänzen Sie Ihren SMTP-Server. Kaeso erreicht ihn über Port 587 mit STARTTLS, sofern [Sie nichts anderes einstellen](../self-hosting/email.md):

```bash
cat >> .env <<'EOF'
KAESO_SMTP_HOST=smtp.example.com
KAESO_SMTP_USER=kaeso
KAESO_SMTP_PASSWORD=…
KAESO_SMTP_FROM="Kaeso <kaeso@example.com>"
EOF
```

Um Kaeso ohne Mailserver auszuprobieren, lassen Sie das Gateway seine E-Mails nur protokollieren. Anmeldung mit Passwort und Einrichtungslink funktionieren weiterhin, Anmeldelinks und Einladungen werden aber nicht zugestellt:

```bash
echo KAESO_MAIL_TRANSPORT=log >> .env
```

### Den Hauptschlüssel erzeugen

Der Hauptschlüssel verschlüsselt die Anbieter-Schlüssel, die Kaeso speichert. Erzeugen Sie einen in die Datei `master-keys`, die Compose dem Gateway als Docker-Secret übergibt:

```bash
docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} \
  master-key generate > master-keys
sudo chown 65532:65532 master-keys && sudo chmod 0400 master-keys
```

Das Gateway läuft in seinem Container als Benutzer 65532, deshalb gehört die Datei diesem Benutzer, und niemand sonst kann sie lesen. Docker Desktop und Colima lesen den Ordner als Sie, führen Sie dort also nur `chmod 0400 master-keys` aus. Mit rootless Docker übergeben Sie die Datei dem Benutzer, auf den 65532 auf dem Host abgebildet wird.

> [!IMPORTANT]
> Bewahren Sie eine Kopie von `master-keys` sicher auf, getrennt von Ihren Datenbank-Backups. Ohne sie kann Kaeso die gespeicherten Anbieter-Schlüssel nicht entschlüsseln, und Sie müssen sie neu eingeben.

### Kaeso starten

Starten Sie alle Dienste, und warten Sie, bis jeder gesund ist:

```bash
docker compose up -d --wait
```

Der erste Start dauert ein bis zwei Minuten, während die Images geladen werden und das Gateway die Datenbanken vorbereitet. Der Befehl kehrt zurück, sobald jeder Dienst gesund ist, oder nennt den Dienst, der es nicht ist. Caddy beantragt das Zertifikat im Hintergrund, deshalb können HTTPS-Anfragen fehlschlagen, bis es eines hat. `docker compose logs caddy` zeigt, wann es das Zertifikat erhält oder warum nicht.

## Ein bestimmtes Release installieren

Das Installationsskript lädt die `compose.yaml` des neuesten stabilen Releases. Für ein anderes Release geben Sie dem Skript dessen Version, etwa `--version {{version}}`, oder laden die Datei vom Tag dieses Releases:

```bash
curl -fsSLO \
  https://github.com/logicplanes/kaeso/releases/download/v{{version}}/compose.yaml
```

Jedem Release liegen außerdem zwei Beispieldateien bei: [`env.example`](https://github.com/logicplanes/kaeso/releases/latest/download/env.example) mit den üblichen Variablen der `.env` und [`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml) mit jeder Einstellung des Gateways.

## Auf dem eigenen Rechner ausprobieren

Setzen Sie `KAESO_DOMAIN=localhost` in der `.env`, starten Sie Kaeso und öffnen Sie `https://localhost`. Caddy signiert das Zertifikat mit seiner eigenen lokalen Zertifizierungsstelle, die Ihr Browser einmal bestätigen lässt. Sind die Ports 80 und 443 auf Ihrem Rechner belegt, legt das Installationsskript Caddy in einer `compose.override.yaml` auf das erste freie Paar, etwa 9080 und 9443, und setzt `KAESO_PUBLIC_URL` auf `https://localhost:9443`. Ohne Mailserver setzen Sie außerdem `KAESO_MAIL_TRANSPORT=log` ([E-Mail](email.md#kaeso-ohne-mailserver-ausprobieren)).

curl und die SDKs vertrauen Caddys lokaler Zertifizierungsstelle nicht. Kopieren Sie ihr Stammzertifikat aus dem Container:

```bash
docker compose cp \
  caddy:/data/caddy/pki/authorities/local/root.crt caddy-root.crt
```

Übergeben Sie das Zertifikat dann dem Client, der Kaeso aufruft:

```bash tab="curl"
curl --cacert caddy-root.crt https://localhost/v1/models \
  -H "Authorization: Bearer $KAESO_API_KEY"
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

## Eine öffentliche Demo betreiben

Mit einer Demo können sich Menschen eine echte Konsole ansehen, ohne Kaeso zu installieren. Füllen Sie eine Installation mit einer Organisation, Projekten und Nutzung, und nennen Sie dann einen ihrer Benutzer als Demo-Benutzer:

```bash
echo "KAESO_AUTH_DEMO_USER=owner@example.com" >> .env
docker compose up -d --wait
```

Die Anmeldeseite bietet dann **Demo erkunden** an, womit jeder Besucher als dieser Benutzer angemeldet wird. Die Installation lehnt jede Änderung mit dem Hinweis ab, dass sie eine schreibgeschützte Demo ist, und die Modell-API und MCP sind aus, sodass nichts über sie gesendet werden kann. Entfernen Sie die Einstellung, um die Installation wieder normal zu nutzen.

## Festlegen, wie Anfrageprotokolle gespeichert werden

Anfrageprotokolle liegen in ClickHouse. Das mitgelieferte ClickHouse in der `compose.yaml` ist die empfohlene Einrichtung: Es braucht nichts von Ihnen und hält die Protokolle auf demselben Server. Nutzung, Limits, Budgets und Budget-Warnungen hängen nicht davon ab, denn Kaeso speichert die Nutzung in PostgreSQL. ClickHouse braucht von allen Diensten den meisten Arbeitsspeicher ([Anforderungen](requirements.md#arbeitsspeicher-und-images)). Eine Installation kann deshalb auch ein ClickHouse nutzen, das Sie bereits betreiben, oder gar keine Anfrageprotokolle speichern.

| Anfrageprotokolle | `KAESO_CLICKHOUSE_URL` in `.env` | Mitgelieferter Dienst `clickhouse` |
| --- | --- | --- |
| Im mitgelieferten ClickHouse, die Voreinstellung | Nicht gesetzt | Läuft |
| In Ihrem eigenen ClickHouse | Die URL Ihres ClickHouse | Aus |
| Aus | Gesetzt und leer | Aus |

Das Gateway liest die Einstellung beim Start. Um zur Voreinstellung zurückzukehren, entfernen Sie beide Änderungen unten und führen `docker compose up -d --wait` aus.

### Anfrageprotokolle ausschalten

1. Fügen Sie `KAESO_CLICKHOUSE_URL=` mit leerem Wert in die `.env` ein. Lassen Sie `CLICKHOUSE_PASSWORD` dort stehen: Compose prüft es auch, wenn das mitgelieferte ClickHouse aus ist.
2. Verhindern Sie, dass Compose das mitgelieferte ClickHouse startet, indem Sie es in ein Profil legen, das Sie nie einschalten:

   ```yaml title="compose.override.yaml"
   services:
     clickhouse:
       profiles: [logs]
   ```

3. Starten Sie das Gateway mit den neuen Einstellungen und entfernen Sie dann den ClickHouse-Container, der noch läuft. Sein Volume `kaeso_clickhouse` behält die bisherigen Protokolle, bis Sie es entfernen.

   ```bash
   docker compose up -d --wait
   docker compose rm -sf clickhouse
   ```

Die Konsole zeigt dann keine **Anfrageprotokolle**, und die Seite **Protokollierung** der Organisation sagt, dass Anfrageprotokolle aus sind. Die Platform API antwortet auf den Protokoll-Endpunkten mit `404`, und `/readyz` meldet `clickhouse` als `off`. Anfragen, die bei ausgeschalteten Protokollen bedient werden, werden nie protokolliert, auch nicht, nachdem Sie die Protokolle wieder einschalten.

### Ihr eigenes ClickHouse nutzen

Das Gateway braucht nur die HTTP-Schnittstelle von ClickHouse, unverschlüsselt oder mit TLS, und einen Benutzer, der eine Datenbank und ihre Tabellen anlegen darf. Schreiben Sie die URL mit diesem Benutzer und Passwort in die `.env`. Kodieren Sie darin jedes Zeichen außer Buchstaben und Ziffern als Prozentkodierung:

```bash
KAESO_CLICKHOUSE_URL=https://kaeso:secret@clickhouse.example.com:8443
```

Schalten Sie dann das mitgelieferte ClickHouse mit derselben `compose.override.yaml` und denselben Befehlen wie [oben](#anfrageprotokolle-ausschalten) ab. Das Gateway legt die Datenbank `KAESO_CLICKHOUSE_DATABASE` an, standardmäßig `kaeso`, und führt dort seine Migrationen aus. Sichern Sie dieses ClickHouse so, wie Sie den Rest davon sichern.

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
| `kaeso instance admin-sso <org\|none>` | Wählt die Organisation, deren Single Sign-on die Instanz verwalten darf, über eine ID wie `org_…`, oder mit `none` keine, wenn kein Instanz-Admin es in der Konsole ändern kann ([Instanz-Admins](sign-in.md#instanz-admins)). Die Organisation braucht eine SSO-Verbindung. Das Audit-Log der Instanz hält es fest |
| `kaeso master-key generate` | Gibt einen neuen Hauptschlüssel als `id:base64` aus |
| `kaeso master-key rewrap` | Verschlüsselt gespeicherte Secrets mit dem ersten Hauptschlüssel neu und meldet, welche Schlüssel-IDs noch verwendet werden ([Hauptschlüssel rotieren](backups.md#hauptschlüssel-rotieren)) |
| `kaeso migrate` | Führt die Migrationen von PostgreSQL aus, bei eingeschalteten Anfrageprotokollen auch die von ClickHouse, und beendet sich |
| `kaeso serve` | Startet das Gateway. Das ist der Standardbefehl des Images |

## Kaeso entfernen

Kaeso läuft einmal pro Rechner. Entfernen Sie deshalb eine Installation, bevor Sie Kaeso an anderer Stelle neu installieren. Halten Sie Kaeso im Ordner der Installation an und löschen Sie seine Container, Netzwerke und Daten:

```bash
docker compose down -v
```

`-v` löscht die Datenbanken mit allen Benutzern, Schlüsseln, Einstellungen und Anfrageprotokollen sowie die Zertifikate von Caddy. Ohne `-v` bleiben die Daten für eine Installation im selben Ordner in Docker. Löschen Sie danach den Ordner, nachdem Sie `master-keys` kopiert haben, falls Sie die Daten behalten.

## Nächste Schritte

- [Konfiguration](configuration.md): Schlagen Sie jede Einstellung nach und wohin sie gehört.
- [Domains und TLS](domains-and-tls.md): Beziehen Sie ein Zertifikat oder betreiben Sie Kaeso hinter Ihrem eigenen Proxy.
- [Backups und Hauptschlüssel](backups.md): Sichern Sie die Datenbanken und bewahren Sie die Hauptschlüssel sicher auf.
