---
title: Schnellstart
description: Kaeso mit Docker Compose starten, sich anmelden, einen Anbieter verbinden und die erste Anfrage senden.
order: 1
---

Diese Seite führt von einem leeren Server zu einer App, die über Kaeso ein Modell aufruft. Eine einzige Compose-Datei startet die ganze Installation: das Gateway, die Konsole, PostgreSQL, ClickHouse, Valkey und Caddy, das sich selbst ein TLS-Zertifikat für die Domain holt.

## Was benötigt wird

- Ein Server mit Docker Engine und Docker Compose 2.24 oder neuer, mindestens 2 vCPUs, 4 GB Arbeitsspeicher und 20 GB Festplatte. Details unter [Anforderungen](../self-hosting/requirements.md).
- Eine Domain wie `kaeso.example.com`, deren DNS-Eintrag auf den Server zeigt.
- Die Ports 80 und 443, aus dem Internet erreichbar, damit Caddy das Zertifikat erhält.
- Ein Schlüssel von OpenAI oder Anthropic.

> [!TIP]
> Um Kaeso stattdessen auf dem eigenen Rechner auszuprobieren, unten `localhost` als Domain verwenden und `https://localhost` öffnen. Caddy nutzt dann ein Zertifikat seiner eigenen lokalen Zertifizierungsstelle, das der Browser einmal bestätigen lässt; [Auf dem eigenen Rechner ausprobieren](../self-hosting/installation.md#auf-dem-eigenen-rechner-ausprobieren) zeigt, wie curl und die SDKs ihm vertrauen.

## Die Compose-Datei herunterladen

Jedes Kaeso-Release enthält eine `compose.yaml`, die die Gateway- und Konsolen-Images dieses Releases festlegt. Die Datei in einen Ordner legen, den nur der eigene Benutzer lesen kann:

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

## Die Einstellungen schreiben

Neben `compose.yaml` eine `.env` mit der Domain und vier zufälligen Passwörtern anlegen:

```bash
cat > .env <<EOF
KAESO_DOMAIN=kaeso.example.com
POSTGRES_PASSWORD=$(openssl rand -hex 24)
POSTGRES_SUPERUSER_PASSWORD=$(openssl rand -hex 24)
CLICKHOUSE_PASSWORD=$(openssl rand -hex 24)
VALKEY_PASSWORD=$(openssl rand -hex 24)
EOF
```

Das Gateway sendet Anmeldelinks, Einladungen und Warnungen per E-Mail und startet nicht, solange es sie nicht versenden kann. Den SMTP-Server in `.env` eintragen:

```bash
KAESO_SMTP_HOST=smtp.example.com
KAESO_SMTP_USER=kaeso
KAESO_SMTP_PASSWORD=…
KAESO_SMTP_FROM=Kaeso <kaeso@example.com>
```

Um Kaeso ohne Mailserver auszuprobieren, stattdessen `KAESO_MAIL_TRANSPORT=log` eintragen. Das Gateway protokolliert dann nur, dass eine E-Mail gesendet worden wäre; die Anmeldung mit Passwort funktioniert weiterhin. Alle Optionen stehen unter [E-Mail](../self-hosting/email.md).

## Den Hauptschlüssel erzeugen

Der Hauptschlüssel verschlüsselt die Anbieter-Schlüssel, die später gespeichert werden. Einen in die Datei `master-keys` erzeugen, die die Compose-Datei dem Gateway als Docker-Secret übergibt:

```bash
docker run --rm ghcr.io/logicplanes/kaeso-gateway:latest master-key generate > master-keys
chmod 644 master-keys
```

Das Gateway läuft in seinem Container als Benutzer ohne Sonderrechte und muss die Datei lesen können; der Ordner hält sie vor anderen Benutzern des Servers verschlossen.

> [!IMPORTANT]
> Eine Kopie von `master-keys` sicher aufbewahren, getrennt von den Datenbank-Backups. Ohne sie lassen sich die gespeicherten Anbieter-Schlüssel nicht entschlüsseln und müssen neu eingegeben werden.

## Kaeso starten

```bash
docker compose up -d
```

Der erste Start dauert ein bis zwei Minuten: Die Images werden heruntergeladen, die Datenbanken starten, das Gateway bereitet sie vor, und Caddy beantragt das Zertifikat. `docker compose ps` zeigt jeden Dienst als healthy, sobald er bereit ist.

## Mit dem Einrichtungslink anmelden

Solange die Installation keine Benutzer hat, protokolliert das Gateway einen Einrichtungslink:

```bash
docker compose logs gateway | grep setup_url
```

Den Link im Browser öffnen. Er funktioniert einmal, 24 Stunden lang. Name, E-Mail-Adresse, ein Passwort mit 12 bis 128 Zeichen und den Namen der Organisation eingeben, dann **Kaeso einrichten** wählen.

Wer die Einrichtung abschließt, ist damit der erste Instanz-Admin der Installation und Inhaber ihrer ersten Organisation, die mit einem Standardprojekt beginnt.

Ist der Link abgelaufen oder verloren gegangen, einen neuen ausgeben; ältere Links funktionieren dann nicht mehr:

```bash
docker compose exec gateway kaeso setup-link
```

## Einen Anbieter-Schlüssel hinzufügen

Anbieter-Schlüssel gehören zur Organisation. In der Seitenleiste **Anbieter** öffnen und **Schlüssel hinzufügen** wählen. OpenAI oder Anthropic wählen, den Schlüssel des Anbieters einfügen, ihm einen Namen wie `Production` geben und **Schlüssel hinzufügen** wählen.

Der erste Schlüssel eines Anbieters wird sein Standard, den jedes Projekt nutzt. Kaeso zeigt das Secret nie wieder an. Mehr unter [Anbieter-Schlüssel](../console/provider-keys.md).

## Ein Modell hinzufügen

Anfragen dürfen nur Modelle aus dem Katalog des Projekts nennen. Das Standardprojekt öffnen, **Modelle** wählen, dann **Modelle hinzufügen**. Den Anbieter wählen, nach einem Modell suchen, etwa `claude-haiku-4-5` unter Anthropic oder `gpt-5-mini` unter OpenAI, es auswählen und hinzufügen. Preise, Limits und Fähigkeiten kommen von [models.dev](https://models.dev). Mehr unter [Modelle und Aliasse](../console/models-and-aliases.md).

## Einen API-Schlüssel erstellen

Im Projekt **API-Schlüssel** wählen, dann **Schlüssel erstellen**. Den Schlüssel nach der App benennen, die ihn nutzen wird, etwa `support-bot`, die Laufzeit von 90 Tagen und den Zugriff **Inferenz** beibehalten, der die Modell-API aufruft, und ihn erstellen.

> [!IMPORTANT]
> Der Schlüssel wird einmal angezeigt. Kaeso speichert nur einen Hash davon, sodass sich ein verlorener Schlüssel nicht wiederherstellen lässt; stattdessen einen neuen erstellen.

## Die erste Anfrage senden

Apps nutzen die offiziellen SDKs; nur die Basis-URL und der Schlüssel ändern sich. Jedes SDK kann jedes Modell im Katalog aufrufen: Kaeso übersetzt Anfrage und Antwort.

| SDK | Basis-URL |
| --- | --- |
| OpenAI | `https://kaeso.example.com/v1` |
| Anthropic | `https://kaeso.example.com` |

Mit dem OpenAI-SDK für Python (`pip install openai`):

```python
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key="sk-kso-…")

response = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.")
print(response.output_text)
```

Mit dem Anthropic-SDK für Python (`pip install anthropic`):

```python
from anthropic import Anthropic

client = Anthropic(base_url="https://kaeso.example.com", api_key="sk-kso-…")

message = client.messages.create(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(message.content[0].text)
```

Beide Beispiele rufen `claude-haiku-4-5` auf, das OpenAI-SDK spricht also mit einem Claude-Modell; jedes Modell, das dem Projekt hinzugefügt wurde, ist möglich. Im Projekt **Anfrageprotokolle** öffnen: Dort steht jede Anfrage mit Status, Modell, Tokens, Kosten und Zeiten.

## Weiter

| Ziel | Lesen |
| --- | --- |
| Modelle aus TypeScript oder mit curl aufrufen, Antworten streamen und eine Anfrage wiederfinden | [Modelle aufrufen](calling-models.md) |
| Claude Code oder Codex auf die eigene Installation ausrichten | [Coding-Tools](coding-tools.md) |
| Personen einladen, Limits und Budgets festlegen | [Die Konsole nutzen](../console/index.md) |
| Backups, Upgrades und Monitoring planen | [Selbst hosten](../self-hosting/index.md) |
