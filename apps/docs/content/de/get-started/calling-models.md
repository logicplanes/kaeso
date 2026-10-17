---
title: Modelle aufrufen
description: Modelle über Kaeso mit den SDKs von OpenAI und Anthropic, mit curl und per Streaming aufrufen.
order: 2
---

Apps rufen Kaeso mit den offiziellen SDKs auf, die sie ohnehin nutzen; nur Basis-URL und Schlüssel ändern sich. Jedes SDK kann jedes Chat-Modell im Katalog des Projekts aufrufen, ein OpenAI-SDK also auch Claude und ein Anthropic-SDK ein OpenAI-Modell. Kaeso übersetzt Anfrage und Antwort.

| SDK | Basis-URL | Der Schlüssel geht als |
| --- | --- | --- |
| OpenAI | `https://kaeso.example.com/v1` | `Authorization: Bearer` |
| Anthropic | `https://kaeso.example.com` | `x-api-key` |

Die Beispiele lesen einen API-Schlüssel aus `KAESO_API_KEY`, erstellt wie im [Schnellstart](quick-start.md#einen-api-schlüssel-erstellen), und rufen `claude-haiku-4-5` auf. Ein Modell aus dem Katalog des eigenen Projekts verwenden.

## OpenAI-SDK

Die Responses API in Python:

```python
import os
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

response = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.")
print(response.output_text)
```

Derselbe Client ruft Chat Completions auf:

```python
completion = client.chat.completions.create(
    model="claude-haiku-4-5",
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(completion.choices[0].message.content)
```

In TypeScript:

```ts
import OpenAI from "openai";

const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1", apiKey: process.env.KAESO_API_KEY });

const response = await client.responses.create({ model: "claude-haiku-4-5", input: "Write a haiku about gateways." });
console.log(response.output_text);
```

## Anthropic-SDK

In Python:

```python
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

In TypeScript:

```ts
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic({ baseURL: "https://kaeso.example.com", apiKey: process.env.KAESO_API_KEY });

const message = await client.messages.create({
  model: "claude-haiku-4-5",
  max_tokens: 200,
  messages: [{ role: "user", content: "Write a haiku about gateways." }],
});
console.log(message.content);
```

## Streaming

Jedes Format streamt, mit den Events, die sein SDK erwartet. Mit dem Anthropic-Client von oben:

```python
with client.messages.stream(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
) as stream:
    for text in stream.text_stream:
        print(text, end="")
```

Solange ein Stream still ist, etwa während das Modell nachdenkt, sendet Kaeso Keepalives, die die SDKs ignorieren, damit Proxys auf dem Weg die Verbindung offen halten ([lange Streams](../self-hosting/domains-and-tls.md#lange-streams)).

## curl

Die OpenAI-Formate:

```bash
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "messages": [{"role": "user", "content": "Hello"}]}'
```

Das Anthropic-Format:

```bash
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "max_tokens": 200, "messages": [{"role": "user", "content": "Hello"}]}'
```

Jeder Endpunkt nimmt den Schlüssel in beiden Headern an. Die [Modell-API](../api/model-api.md) listet die Endpunkte auf.

## Aus dem Browser

Kaeso beantwortet Anfragen einer Webseite nur, wenn ihr Origin in `KAESO_CORS_ALLOWED_ORIGINS` steht, und jeder, der die Seite nutzt, kann den Schlüssel darin lesen. [Aufrufe aus dem Browser](../api/model-api.md#aufrufe-aus-dem-browser) behandelt beides.

## Eine Anfrage wiederfinden

Im Projekt **Anfrageprotokolle** öffnen: Dort steht jede Anfrage mit Status, Modell, Tokens, Kosten und Zeiten. Jede Antwort trägt einen Header `kaeso-request-id` mit der ID, die das Protokoll zeigt, sodass sich eine Anfrage aus den Logs der eigenen App in Kaeso wiederfinden lässt.

Eine fehlgeschlagene Anfrage erhält einen Fehler im Format, das ihr SDK erwartet, mit dem Grund. [Fehler](../api/errors.md) listet sie auf.
