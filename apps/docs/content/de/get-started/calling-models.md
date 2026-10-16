---
title: Modelle aufrufen
description: Modelle über Kaeso mit den SDKs von OpenAI und Anthropic, mit curl und per Streaming aufrufen.
order: 2
---

Apps rufen Kaeso mit den offiziellen SDKs auf, die sie schon nutzen. Nur die Basis-URL und der Schlüssel ändern sich, und jedes SDK kann jedes Modell im Katalog des Projekts aufrufen: ein OpenAI-SDK ein Claude-Modell und ein Anthropic-SDK ein OpenAI-Modell. Kaeso übersetzt Anfrage und Antwort.

| SDK | Basis-URL | Schlüssel |
| --- | --- | --- |
| OpenAI | `https://kaeso.example.com/v1` | `api_key`, gesendet als `Authorization: Bearer` |
| Anthropic | `https://kaeso.example.com` | `api_key`, gesendet als `x-api-key` |

Die Beispiele nutzen einen Schlüssel, der in einem Projekt unter **API-Schlüssel** erstellt wurde, wie im [Schnellstart](quick-start.md#einen-api-schlüssel-erstellen), und das Modell `claude-haiku-4-5`. Ein Modell verwenden, das im Katalog des eigenen Projekts steht.

## OpenAI-SDK

Die Responses API, in Python:

```python
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key="sk-kso-…")

response = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.")
print(response.output_text)
```

Derselbe Client bedient auch Chat Completions:

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

Streams funktionieren in jedem Format, mit den Events, die das SDK erwartet. Mit dem Anthropic-Client von oben:

```python
with client.messages.stream(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
) as stream:
    for text in stream.text_stream:
        print(text, end="")
```

Solange ein Stream still ist, etwa während ein Modell nachdenkt, sendet Kaeso Keepalives, die die SDKs ignorieren, sodass Proxys auf dem Weg die Verbindung offen halten. Siehe [lange Streams](../self-hosting/domains-and-tls.md#lange-streams).

## curl

Die OpenAI-Formate nehmen den Schlüssel als Bearer-Token:

```bash
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "messages": [{"role": "user", "content": "Hello"}]}'
```

Das Anthropic-Format nimmt ihn in `x-api-key`:

```bash
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "max_tokens": 200, "messages": [{"role": "user", "content": "Hello"}]}'
```

Kaeso akzeptiert beide Header an jedem Endpunkt der Modell-API. Die [Modell-API](../api/model-api.md) listet jeden Endpunkt auf.

## Eine Anfrage wiederfinden

Im Projekt **Anfrageprotokolle** öffnen. Dort steht jede Anfrage mit Status, Modell, Tokens, Kosten und Zeiten. Jede Antwort von Kaeso trägt einen Header `kaeso-request-id` mit derselben ID, sodass sich eine Anfrage aus den Protokollen der eigenen App heraus finden lässt.

Schlägt eine Anfrage fehl, nennt der Fehler den Grund, im Format, das das SDK erwartet. [Fehler](../api/errors.md) listet sie auf.
