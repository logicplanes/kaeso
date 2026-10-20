---
title: Schnellstart
description: Installieren Sie Kaeso mit Docker Compose, verbinden Sie einen Anbieter, und senden Sie die erste Anfrage.
order: 1
---

Installieren Sie Kaeso auf einem Server, verbinden Sie einen Modellanbieter wie OpenAI oder Anthropic, und senden Sie Ihre erste Anfrage darüber.

## Bevor Sie beginnen

- Ein Server mit Docker Engine und Docker Compose 2.24 oder neuer, 2 vCPUs, 4 GB Arbeitsspeicher und 20 GB Festplatte ([Anforderungen](../self-hosting/requirements.md)).
- Eine Domain wie `kaeso.example.com`, deren DNS-Eintrag auf den Server zeigt. Die Ports 80 und 443 müssen zum Internet offen sein, damit Caddy ein Zertifikat erhält.
- Ein API-Schlüssel eines [Modellanbieters, den Kaeso aufruft](../concepts/supported-providers.md), etwa OpenAI oder Anthropic.

> [!TIP]
> Um Kaeso auf Ihrem eigenen Rechner auszuprobieren, verwenden Sie `localhost` als Domain und öffnen `https://localhost`. Caddy signiert das Zertifikat dann selbst, und Ihr Browser fragt einmal, ob er es akzeptieren soll. [Auf dem eigenen Rechner ausprobieren](../self-hosting/installation.md#auf-dem-eigenen-rechner-ausprobieren) zeigt, wie curl und die SDKs ihm vertrauen.

## Die Compose-Datei herunterladen

Jedes Release veröffentlicht eine `compose.yaml`, die seine Images auf Version und Digest festlegt. Laden Sie sie in einen neuen Ordner, den nur Sie lesen können:

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

## Die Einstellungen schreiben

Legen Sie `.env` mit Ihrer Domain und vier zufälligen Passwörtern an:

```bash
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

## Den Hauptschlüssel erzeugen

Der Hauptschlüssel verschlüsselt die Anbieter-Schlüssel, die Kaeso speichert. Erzeugen Sie einen in die Datei `master-keys`, die Compose dem Gateway als Docker-Secret übergibt:

```bash
docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate > master-keys
chmod 644 master-keys
```

Das Gateway läuft in seinem Container als Benutzer ohne Sonderrechte, deshalb muss die Datei lesbar sein. Die Rechte des Ordners halten andere Benutzer des Servers fern.

> [!IMPORTANT]
> Bewahren Sie eine Kopie von `master-keys` sicher auf, getrennt von Ihren Datenbank-Backups. Ohne sie kann Kaeso die gespeicherten Anbieter-Schlüssel nicht entschlüsseln, und Sie müssen sie neu eingeben.

## Kaeso starten

Starten Sie alle Dienste, und warten Sie, bis jeder gesund ist:

```bash
docker compose up -d --wait
```

Der erste Start dauert ein bis zwei Minuten, während die Images geladen werden und das Gateway die Datenbanken vorbereitet. Der Befehl kehrt zurück, sobald jeder Dienst gesund ist, oder nennt den Dienst, der es nicht ist. Caddy beantragt das Zertifikat im Hintergrund, deshalb können HTTPS-Anfragen fehlschlagen, bis es eines hat. `docker compose logs caddy` zeigt, wann es das Zertifikat erhält oder warum nicht.

## Den ersten Benutzer anlegen

Der Einrichtungslink legt den ersten Benutzer an. Geben Sie ihn im Terminal aus:

```bash
docker compose exec gateway kaeso setup-link
```

Der Link funktioniert einmal, innerhalb von 24 Stunden. Rufen Sie den Befehl erneut auf, gibt er einen neuen Link aus und macht den alten ungültig.

Öffnen Sie den Link, und geben Sie Ihren Namen, Ihre E-Mail-Adresse, ein Passwort mit 12 bis 128 Zeichen und einen Namen für Ihre Organisation ein. Wählen Sie dann **Kaeso einrichten**. Damit sind Sie Instanz-Admin und Inhaber der ersten Organisation, die ein Standardprojekt mitbringt.

## Einen Anbieter-Schlüssel hinzufügen

Anbieter-Schlüssel gehören zur Organisation, und jedes Projekt nutzt den Standardschlüssel des Anbieters.

1. Öffnen Sie in der Seitenleiste **Anbieter**, und wählen Sie **Anbieter-Schlüssel hinzufügen**.
2. Wählen Sie den Anbieter, und fügen Sie seinen Schlüssel ein.
3. Benennen Sie ihn, etwa `Production`, und wählen Sie **Schlüssel hinzufügen**.

Der erste Schlüssel eines Anbieters wird dessen Standard. Kaeso zeigt den Schlüssel nie wieder an; siehe [Anbieter-Schlüssel](../console/provider-keys.md).

## Ein Modell hinzufügen

Eine Anfrage kann nur ein Modell aus dem Katalog des Projekts nennen.

1. Öffnen Sie das Standardprojekt, und wählen Sie **Modelle**, dann **Modelle hinzufügen**.
2. Wählen Sie den Anbieter, und suchen Sie nach einem Modell, etwa `claude-haiku-4-5` von Anthropic oder `gpt-5-mini` von OpenAI.
3. Fügen Sie es hinzu.

Preise, Limits und Fähigkeiten des Modells kommen von [models.dev](https://models.dev); siehe [Modelle und Aliasse](../console/models-and-aliases.md).

## Einen API-Schlüssel erstellen

1. Wählen Sie im Projekt **API-Schlüssel**, dann **Schlüssel erstellen**.
2. Benennen Sie den Schlüssel nach der App, die ihn nutzen wird, etwa `support-bot`.
3. Behalten Sie die Laufzeit von 90 Tagen und den Zugriff **Inferenz** bei, und erstellen Sie den Schlüssel.

Kopieren Sie den Schlüssel sofort. Kaeso zeigt ihn nur einmal an und speichert nur einen Hash davon: Einen verlorenen Schlüssel können Sie nicht wiederherstellen, nur ersetzen. Übernehmen Sie ihn in Ihre Shell:

```bash
export KAESO_API_KEY=sk-kso-…
```

## Die erste Anfrage senden

Rufen Sie das Modell mit curl auf, oder mit dem OpenAI SDK für Python (`pip install openai`) oder TypeScript (`npm install openai`). Das OpenAI SDK ruft das Claude-Modell auf, das Sie hinzugefügt haben: Kaeso übersetzt Anfrage und Antwort.

```bash tab="curl"
curl https://kaeso.example.com/v1/responses \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "input": "Write a haiku about gateways."}'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

response = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.")
print(response.output_text)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1", apiKey: process.env.KAESO_API_KEY });

const response = await client.responses.create({ model: "claude-haiku-4-5", input: "Write a haiku about gateways." });
console.log(response.output_text);
```

Haben Sie ein anderes Modell hinzugefügt, verwenden Sie dessen ID. Die Antwort enthält das Haiku des Modells. Öffnen Sie im Projekt **Anfrageprotokolle**: Dort steht die Anfrage mit Status, Token, Kosten und Zeiten.

## Nächste Schritte

- [Modelle aufrufen](calling-models.md): Nutzen Sie auch das Anthropic SDK, streamen Sie Antworten, und finden Sie eine Anfrage in den Protokollen.
- [Coding-Tools](coding-tools.md): Richten Sie Claude Code oder Codex auf Ihre Installation aus.
- [Die Konsole verwenden](../console/index.md): Laden Sie Personen ein, und legen Sie Limits und Budgets fest.
- [Selbst hosten](../self-hosting/index.md): Planen Sie Anmeldung, Backups, Upgrades und Monitoring.
