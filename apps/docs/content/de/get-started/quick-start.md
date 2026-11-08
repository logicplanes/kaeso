---
title: Schnellstart
description: Installieren Sie Kaeso mit Docker Compose, verbinden Sie einen Anbieter, und senden Sie die erste Anfrage.
order: 1
---

Installieren Sie Kaeso auf einem Server, verbinden Sie einen Modellanbieter wie OpenAI oder Anthropic, und senden Sie Ihre erste Anfrage darüber.

## Bevor Sie beginnen

- Ein Server mit 2 vCPUs, 4 GB Arbeitsspeicher und 20 GB Festplatte sowie Docker Engine mit Docker Compose 2.24 oder neuer ([Anforderungen](../self-hosting/requirements.md)). Unter Linux bietet das Installationsskript an, Docker zu installieren, wenn es fehlt.
- Eine Domain wie `kaeso.example.com`, deren DNS-Eintrag auf den Server zeigt. Die Ports 80 und 443 müssen zum Internet offen sein, damit Caddy ein Zertifikat erhält.
- Ein API-Schlüssel eines [Modellanbieters, den Kaeso aufruft](../concepts/supported-providers.md), etwa OpenAI oder Anthropic.

> [!TIP]
> Um Kaeso auf Ihrem eigenen Rechner auszuprobieren, verwenden Sie `localhost` als Domain und öffnen `https://localhost`. Caddy signiert das Zertifikat dann selbst, und Ihr Browser fragt einmal, ob er es akzeptieren soll. [Auf dem eigenen Rechner ausprobieren](../self-hosting/installation.md#auf-dem-eigenen-rechner-ausprobieren) zeigt, wie curl und die SDKs ihm vertrauen.

## Kaeso installieren

Führen Sie das Installationsskript auf dem Server aus. Es legt einen Ordner `kaeso` an, den nur Sie lesen können, und lädt die `compose.yaml` des neuesten Releases hinein. Dann schreibt es `.env` mit Ihrer Domain und zufälligen Passwörtern, erzeugt den Hauptschlüssel und startet alle Dienste:

```bash
curl -fsSL https://kaeso.app/install.sh | sh -s -- --domain kaeso.example.com
```

Vor der Installation startet das Skript Docker, falls es gestoppt ist, und prüft, ob Ihre Domain einen DNS-Eintrag hat. Der erste Start dauert ein bis zwei Minuten, während die Images geladen werden. Danach prüft das Skript, ob Caddy das Zertifikat der Domain erhalten hat, und erklärt sonst die Antwort der Zertifizierungsstelle. Zum Schluss gibt es den Einrichtungslink aus und öffnet ihn im Browser, wenn Sie es auf Ihrem eigenen Rechner ausführen; `--no-open` lässt ihn geschlossen. Bis Sie [Ihren SMTP-Server eintragen](../self-hosting/email.md), schreibt das Gateway seine E-Mails nur in sein Log, Anmeldelinks und Einladungen kommen also nicht an. [Von Hand installieren](../self-hosting/installation.md#von-hand-installieren) zeigt jeden Schritt des Skripts.

Führen Sie das Skript im Ordner der Installation erneut aus, um Kaeso wieder zu starten; es behält die Einstellungen und den Hauptschlüssel. Das Skript schaltet auch den [Updater](../self-hosting/upgrades.md#der-updater) ein, sodass **Auf … aktualisieren** unter **Instanz** › **Updates** neue Releases installiert. Auf einem Rechner läuft ein Kaeso, deshalb lehnt das Skript eine zweite Installation in einem anderen Ordner ab.

> [!IMPORTANT]
> Bewahren Sie eine Kopie von `kaeso/master-keys` sicher auf, getrennt von Ihren Datenbank-Backups. Ohne sie kann Kaeso die gespeicherten Anbieter-Schlüssel nicht entschlüsseln, und Sie müssen sie neu eingeben.

## Den ersten Benutzer anlegen

Der Einrichtungslink, den das Skript ausgegeben hat, legt den ersten Benutzer an. Er funktioniert einmal, innerhalb von 24 Stunden. Einen neuen Link, der den alten ungültig macht, geben Sie im Ordner `kaeso` aus:

```bash
docker compose exec gateway kaeso setup-link
```

Öffnen Sie den Link, und geben Sie Ihren Namen, Ihre E-Mail-Adresse, ein Passwort mit 12 bis 128 Zeichen und einen Namen für Ihre Organisation ein. Wählen Sie dann **Kaeso einrichten**. Damit sind Sie Instanz-Admin und Inhaber der ersten Organisation, und Kaeso öffnet ihre **Übersicht**. Die Organisation hat noch keine Projekte.

## Einen Anbieter-Schlüssel hinzufügen

Anbieter-Schlüssel gehören zur Organisation, und jedes Projekt nutzt den Standardschlüssel des Anbieters.

1. Öffnen Sie in der Seitenleiste **Anbieter**, und wählen Sie **Anbieter-Schlüssel hinzufügen**.
2. Wählen Sie den Anbieter, und fügen Sie seinen Schlüssel ein.
3. Benennen Sie ihn, etwa `Production`, und wählen Sie **Schlüssel hinzufügen**.

Der erste Schlüssel eines Anbieters wird dessen Standard. Kaeso zeigt den Schlüssel nie wieder an; siehe [Anbieter-Schlüssel](../console/provider-keys.md).

## Ein Projekt erstellen

Modelle, API-Schlüssel und Anfrageprotokolle gehören zu einem Projekt.

1. Wählen Sie in der **Übersicht** der Organisation **Projekt erstellen**.
2. Benennen Sie es nach seinem Zweck, etwa `Support bot`, und wählen Sie **Projekt erstellen**.

Kaeso öffnet das neue Projekt, und Sie sind sein Admin; siehe [Organisationen und Projekte](../console/organizations-and-projects.md#projekt-erstellen).

## Ein Modell hinzufügen

Eine Anfrage kann nur ein Modell aus dem Katalog des Projekts nennen.

1. Wählen Sie im Projekt **Modelle**, dann **Modelle hinzufügen**.
2. Wählen Sie den Anbieter, und suchen Sie nach einem Modell, etwa `claude-haiku-4-5` von Anthropic oder `gpt-5-mini` von OpenAI.
3. Fügen Sie es hinzu.

Preise, Limits und Fähigkeiten des Modells kommen von [KPMR](../concepts/kpmr.md), der Kaeso Provider & Model Registry; siehe [Modelle und Aliasse](../console/models-and-aliases.md).

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

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)

response = client.responses.create(
    model="claude-haiku-4-5",
    input="Write a haiku about gateways.",
)
print(response.output_text)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});

const response = await client.responses.create({
  model: "claude-haiku-4-5",
  input: "Write a haiku about gateways.",
});
console.log(response.output_text);
```

Haben Sie ein anderes Modell hinzugefügt, verwenden Sie dessen ID. Die Antwort enthält das Haiku des Modells. Öffnen Sie im Projekt **Anfrageprotokolle**: Dort steht die Anfrage mit Status, Token, Kosten und Zeiten.

## Nächste Schritte

- [Modelle aufrufen](calling-models.md): Nutzen Sie auch das Anthropic oder Google Gen AI SDK, streamen Sie Antworten, und finden Sie eine Anfrage in den Protokollen.
- [Coding-Tools](coding-tools.md): Richten Sie Claude Code oder Codex auf Ihre Installation aus.
- [Die Konsole verwenden](../console/index.md): Laden Sie Personen ein, und legen Sie Limits und Budgets fest.
- [Selbst hosten](../self-hosting/index.md): Planen Sie Anmeldung, Backups, Upgrades und Monitoring.
