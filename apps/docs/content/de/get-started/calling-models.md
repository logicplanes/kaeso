---
title: Modelle aufrufen
description: Jedes Modell Ihres Projekts mit dem OpenAI oder Anthropic SDK oder mit curl aufrufen.
order: 2
---

Richten Sie das offizielle OpenAI oder Anthropic SDK auf Ihre Installation, und geben Sie ihm einen API-Schlüssel von Kaeso. Jedes SDK kann jedes Chat-Modell Ihres Projekts aufrufen, das OpenAI SDK also auch Claude.

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

Die Beispiele lesen Ihren Schlüssel aus `KAESO_API_KEY` und rufen `claude-haiku-4-5` auf. Erstellen Sie einen Schlüssel wie im [Schnellstart](quick-start.md), und verwenden Sie ein Modell aus dem Katalog Ihres Projekts.

Das OpenAI SDK erhält `https://kaeso.example.com/v1` als Basis-URL und sendet den Schlüssel als `Authorization: Bearer`. Derselbe Client ruft auch Chat Completions auf. Die [Modell-API](../api/model-api.md) zeigt diese Anfrage und listet jeden Endpunkt.

## Anthropic SDK verwenden

Setzen Sie die Basis-URL auf `https://kaeso.example.com`, ohne `/v1`. Das SDK sendet den Schlüssel als `x-api-key`:

```bash tab="curl"
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "max_tokens": 200, "messages": [{"role": "user", "content": "Write a haiku about gateways."}]}'
```

```python tab="Python"
import os
from anthropic import Anthropic

client = Anthropic(base_url="https://kaeso.example.com", api_key=os.environ["KAESO_API_KEY"])

message = client.messages.create(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(message.content[0].text)
```

```ts tab="TypeScript"
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic({ baseURL: "https://kaeso.example.com", apiKey: process.env.KAESO_API_KEY });

const message = await client.messages.create({
  model: "claude-haiku-4-5",
  max_tokens: 200,
  messages: [{ role: "user", content: "Write a haiku about gateways." }],
});
for (const block of message.content) if (block.type === "text") console.log(block.text);
```

Jeder Endpunkt nimmt den Schlüssel in beiden Headern an. Ein Tool, das nur `Authorization: Bearer` sendet, funktioniert hier also auch.

## Antworten streamen

Fordern Sie einen Stream so an, wie Ihr SDK es vorsieht. Kaeso sendet die Ereignisse, die dieses SDK erwartet, gleich welcher Anbieter das Modell bedient:

```bash tab="curl"
curl -N https://kaeso.example.com/v1/responses \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "input": "Write a haiku about gateways.", "stream": true}'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

stream = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.", stream=True)
for event in stream:
    if event.type == "response.output_text.delta":
        print(event.delta, end="")
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1", apiKey: process.env.KAESO_API_KEY });

const stream = await client.responses.create({ model: "claude-haiku-4-5", input: "Write a haiku about gateways.", stream: true });
for await (const event of stream) {
  if (event.type === "response.output_text.delta") process.stdout.write(event.delta);
}
```

Solange ein Stream still ist, etwa während das Modell nachdenkt, sendet Kaeso Keepalives, die die SDKs ignorieren. So halten Proxys unterwegs die Verbindung offen.

## Aus dem Browser aufrufen

Kaeso beantwortet Anfragen einer Webseite nur, wenn ihr Origin in `KAESO_CORS_ALLOWED_ORIGINS` steht. Die [Modell-API](../api/model-api.md) zeigt, wie Sie die Einstellung setzen.

> [!WARNING]
> Jeder, der die Seite nutzt, kann den Schlüssel darin lesen. Geben Sie einer Browser-App einen eigenen Schlüssel, beschränkt auf die Modelle und Raten, die sie braucht.

## Eine Anfrage in den Protokollen finden

Jede Antwort trägt einen Header `kaeso-request-id`. Protokollieren Sie ihn in Ihrer App, damit Sie dieselbe Anfrage in Kaeso wiederfinden:

1. Öffnen Sie im Projekt **Anfrageprotokolle**.
2. Wählen Sie **Anfrage per ID öffnen**, und fügen Sie die ID ein.

Der Eintrag zeigt Status, Modell, Token, Kosten und Zeiten. Eine fehlgeschlagene Anfrage zeigt auch ihren Fehler, den Ihre App im Format ihres SDK erhalten hat.

## Nächste Schritte

- [Coding-Tools](coding-tools.md): Richten Sie Claude Code, Codex und andere Tools auf Ihre Installation aus.
- [Modell-API](../api/model-api.md): Schlagen Sie jeden Endpunkt, jeden Header und jeden Modellnamen nach.
- [Fehler](../api/errors.md): Erfahren Sie, was ein Fehler bedeutet und wie Ihre App damit umgeht.
- [Formatübersetzung](../concepts/format-translation.md): Sehen Sie, was erhalten bleibt, wenn ein SDK das Modell eines anderen Anbieters aufruft.
