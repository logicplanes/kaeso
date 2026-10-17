---
title: Schnellstart
description: Kaeso mit Docker Compose installieren, einen Anbieter verbinden und die erste Anfrage senden.
order: 1
---

Eine Compose-Datei betreibt die ganze Installation: das Gateway, die Konsole, PostgreSQL, ClickHouse, Valkey und Caddy, das ein TLS-Zertifikat für die eigene Domain besorgt. Diese Seite führt von einem leeren Server bis zur ersten Anfrage.

## Was benötigt wird

- Ein Server mit Docker Engine und Docker Compose 2.24 oder neuer, 2 vCPUs, 4 GB Arbeitsspeicher und 20 GB Festplatte ([Anforderungen](../self-hosting/requirements.md)).
- Eine Domain wie `kaeso.example.com`, deren DNS-Eintrag auf den Server zeigt, und die Ports 80 und 443 offen zum Internet, damit Caddy ein Zertifikat erhält.
- Ein API-Schlüssel von OpenAI oder Anthropic.

> [!TIP]
> Zum Ausprobieren auf dem eigenen Rechner `localhost` als Domain verwenden und `https://localhost` öffnen. Caddy signiert das Zertifikat dann mit seiner eigenen lokalen Zertifizierungsstelle. Der Browser fragt einmal nach, ob er es akzeptieren soll; [Auf dem eigenen Rechner ausprobieren](../self-hosting/installation.md#auf-dem-eigenen-rechner-ausprobieren) zeigt, wie curl und die SDKs ihm vertrauen.

## Die Compose-Datei herunterladen

Jedes Release veröffentlicht eine `compose.yaml`, die seine Gateway- und Konsolen-Images festlegt. Die Datei in einen neuen Ordner legen, den nur der eigene Benutzer lesen kann:

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

## Die Einstellungen schreiben

`.env` mit der Domain und vier zufälligen Passwörtern anlegen:

```bash
cat > .env <<EOF
KAESO_DOMAIN=kaeso.example.com
POSTGRES_PASSWORD=$(openssl rand -hex 24)
POSTGRES_SUPERUSER_PASSWORD=$(openssl rand -hex 24)
CLICKHOUSE_PASSWORD=$(openssl rand -hex 24)
VALKEY_PASSWORD=$(openssl rand -hex 24)
EOF
```

Das Gateway verschickt Anmeldelinks, Einladungen und Warnungen per E-Mail und startet nicht ohne einen Weg, sie zu senden. Den SMTP-Server ergänzen; Kaeso erreicht ihn über Port 587 mit STARTTLS, sofern [nichts anderes eingestellt](../self-hosting/email.md) ist:

```bash
cat >> .env <<'EOF'
KAESO_SMTP_HOST=smtp.example.com
KAESO_SMTP_USER=kaeso
KAESO_SMTP_PASSWORD=…
KAESO_SMTP_FROM="Kaeso <kaeso@example.com>"
EOF
```

Ohne Mailserver lässt sich Kaeso ausprobieren, indem das Gateway seine E-Mails nur protokolliert. Anmeldung mit Passwort und Einrichtungslink funktionieren weiterhin; Anmeldelinks und Einladungen werden nicht zugestellt.

```bash
echo KAESO_MAIL_TRANSPORT=log >> .env
```

## Den Hauptschlüssel erzeugen

Der Hauptschlüssel verschlüsselt die Anbieter-Schlüssel, die Kaeso speichert. Einen in die Datei `master-keys` erzeugen, die Compose dem Gateway als Docker-Secret übergibt:

```bash
docker run --rm ghcr.io/logicplanes/kaeso-gateway:latest master-key generate > master-keys
chmod 644 master-keys
```

Das Gateway läuft in seinem Container als Benutzer ohne Sonderrechte, deshalb muss die Datei lesbar sein; der Ordner hält andere Benutzer des Servers fern.

> [!IMPORTANT]
> Eine Kopie von `master-keys` sicher aufbewahren, getrennt von den Datenbank-Backups. Ohne sie lassen sich die gespeicherten Anbieter-Schlüssel nicht entschlüsseln und müssen neu eingegeben werden.

## Kaeso starten

```bash
docker compose up -d --wait
```

Der erste Start dauert ein bis zwei Minuten, während die Images geladen werden und das Gateway die Datenbanken vorbereitet. Der Befehl kehrt zurück, sobald jeder Dienst healthy meldet, Caddy eingeschlossen, oder schlägt fehl und nennt den Dienst, der es nicht tut. Caddy ist healthy, sobald es Anfragen bedient, und beantragt das Zertifikat für eine neue Domain im Hintergrund; bis es eines hat, können HTTPS-Anfragen noch fehlschlagen. `docker compose logs caddy` zeigt, wann es das Zertifikat erhält oder warum nicht.

## Den ersten Benutzer anlegen

Der Einrichtungslink legt den ersten Benutzer an. Ihn im Terminal ausgeben:

```bash
docker compose exec gateway kaeso setup-link
```

Der Link funktioniert einmal, innerhalb von 24 Stunden. Ein erneuter Aufruf des Befehls gibt einen neuen Link aus und macht den alten ungültig.

Den Link öffnen, Name, E-Mail-Adresse, ein Passwort mit 12 bis 128 Zeichen und einen Namen für die Organisation eingeben und **Kaeso einrichten** wählen. Damit ist man Instanz-Admin und Inhaber der ersten Organisation, die ein Standardprojekt mitbringt.

## Einen Anbieter-Schlüssel hinzufügen

Anbieter-Schlüssel gehören zur Organisation. In der Seitenleiste **Anbieter** öffnen und **Schlüssel hinzufügen** wählen. OpenAI oder Anthropic wählen, den Schlüssel des Anbieters einfügen, ihn benennen, etwa `Production`, und **Schlüssel hinzufügen** wählen.

Der erste Schlüssel eines Anbieters wird dessen Standard, den jedes Projekt nutzt. Kaeso zeigt den Schlüssel nie wieder an. Siehe [Anbieter-Schlüssel](../console/provider-keys.md).

## Ein Modell hinzufügen

Eine Anfrage kann nur Modelle aus dem Katalog des Projekts nennen. Das Standardprojekt öffnen, **Modelle** wählen, dann **Modelle hinzufügen**. Den Anbieter wählen, nach einem Modell suchen, etwa `claude-haiku-4-5` von Anthropic oder `gpt-5-mini` von OpenAI, und es hinzufügen. Preise, Limits und Fähigkeiten kommen von [models.dev](https://models.dev); siehe [Modelle und Aliasse](../console/models-and-aliases.md).

## Einen API-Schlüssel erstellen

Im Projekt **API-Schlüssel** wählen, dann **Schlüssel erstellen**. Den Schlüssel nach der App benennen, die ihn nutzen wird, etwa `support-bot`, die Laufzeit von 90 Tagen und den Zugriff **Inferenz** beibehalten und ihn erstellen.

Den Schlüssel sofort kopieren. Er wird nur einmal angezeigt, und Kaeso speichert nur einen Hash davon: Ein verlorener Schlüssel lässt sich nicht wiederherstellen, nur ersetzen.

## Die erste Anfrage senden

Den Schlüssel in die Shell übernehmen und das Modell mit curl aufrufen:

```bash
export KAESO_API_KEY=sk-kso-…
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "messages": [{"role": "user", "content": "Write a haiku about gateways."}]}'
```

Apps behalten ihr offizielles SDK und ändern nur Basis-URL und Schlüssel. Das OpenAI-SDK für Python (`pip install openai`) ruft dasselbe Claude-Modell auf; Kaeso übersetzt Anfrage und Antwort.

```python
import os
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

response = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.")
print(response.output_text)
```

Das Modell verwenden, das hinzugefügt wurde. Dann im Projekt **Anfrageprotokolle** öffnen: Beide Anfragen stehen dort mit Status, Tokens, Kosten und Zeiten.

## Weiter

| Ziel | Seite |
| --- | --- |
| Modelle aus TypeScript oder mit dem Anthropic-SDK aufrufen und Antworten streamen | [Modelle aufrufen](calling-models.md) |
| Claude Code oder Codex auf die eigene Installation ausrichten | [Coding-Tools](coding-tools.md) |
| Personen einladen, Limits und Budgets festlegen | [Die Konsole nutzen](../console/index.md) |
| Über den eigenen Identitätsanbieter anmelden | [Anmeldung und Single Sign-on](../self-hosting/sign-in.md) |
| Backups, Upgrades und Monitoring planen | [Selbst hosten](../self-hosting/index.md) |
