---
title: Modelle aufrufen
description: Jedes Modell Ihres Projekts mit dem OpenAI, Anthropic oder Google Gen AI SDK oder mit curl aufrufen.
order: 2
---

Richten Sie das offizielle OpenAI oder Anthropic SDK oder Googles Gen AI SDK auf Ihre Installation, und geben Sie ihm einen API-Schlüssel von Kaeso. Jedes SDK kann jedes Chat-Modell Ihres Projekts aufrufen, gleich welcher [Anbieter](../concepts/providers.md) es bedient, das OpenAI SDK also auch Claude.

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

Die Beispiele lesen Ihren Schlüssel aus `KAESO_API_KEY` und rufen `claude-haiku-4-5` auf. Erstellen Sie einen Schlüssel wie im [Schnellstart](quick-start.md), und verwenden Sie ein Modell aus dem Katalog Ihres Projekts.

Das OpenAI SDK erhält `https://kaeso.example.com/v1` als Basis-URL und sendet den Schlüssel als `Authorization: Bearer`. Derselbe Client ruft auch Chat Completions auf. Die [Modell-API](../api/model-api.md) zeigt diese Anfrage und listet jeden Endpunkt.

## Anthropic SDK verwenden

Setzen Sie die Basis-URL auf `https://kaeso.example.com`, ohne `/v1`. Das SDK sendet den Schlüssel als `x-api-key`, `Authorization: Bearer` funktioniert ebenso:

```bash tab="curl"
curl https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "max_tokens": 200,
    "messages": [
      {"role": "user", "content": "Write a haiku about gateways."}
    ]
  }'
```

```python tab="Python"
import os
from anthropic import Anthropic

client = Anthropic(
    base_url="https://kaeso.example.com",
    api_key=os.environ["KAESO_API_KEY"],
)

message = client.messages.create(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(message.content[0].text)
```

```ts tab="TypeScript"
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic({
  baseURL: "https://kaeso.example.com",
  apiKey: process.env.KAESO_API_KEY,
});

const message = await client.messages.create({
  model: "claude-haiku-4-5",
  max_tokens: 200,
  messages: [{ role: "user", content: "Write a haiku about gateways." }],
});
for (const block of message.content) {
  if (block.type === "text") console.log(block.text);
}
```

Jeder Endpunkt nimmt den Schlüssel in `Authorization: Bearer`, `x-api-key`, `x-goog-api-key` oder `xi-api-key` an. Ein Tool, das nur `Authorization: Bearer` sendet, funktioniert hier also auch.

## Google Gen AI SDK verwenden

Setzen Sie die Basis-URL auf `https://kaeso.example.com`, ohne Version, und die API-Version auf `v1beta`: `google-genai` nimmt ab Version 3 sonst `v1`. Das SDK sendet den Schlüssel als `x-goog-api-key`:

```bash tab="curl"
curl https://kaeso.example.com/v1beta/models/claude-haiku-4-5:generateContent \
  -H "x-goog-api-key: $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "contents": [
      {"role": "user", "parts": [{"text": "Write a haiku about gateways."}]}
    ]
  }'
```

```python tab="Python"
import os
from google import genai

client = genai.Client(
    api_key=os.environ["KAESO_API_KEY"],
    http_options={"base_url": "https://kaeso.example.com", "api_version": "v1beta"},
)

response = client.models.generate_content(
    model="claude-haiku-4-5",
    contents="Write a haiku about gateways.",
)
print(response.text)
```

```ts tab="TypeScript"
import { GoogleGenAI } from "@google/genai";

const ai = new GoogleGenAI({
  apiKey: process.env.KAESO_API_KEY,
  httpOptions: { baseUrl: "https://kaeso.example.com", apiVersion: "v1beta" },
});

const response = await ai.models.generateContent({
  model: "claude-haiku-4-5",
  contents: "Write a haiku about gateways.",
});
console.log(response.text);
```

Kaeso bedient die Gemini API nur unter `/v1beta`; ein Client mit `v1` erhält 404 mit einer Meldung, die dorthin verweist. Zum Streamen rufen Sie `generate_content_stream` auf. Das SDK fordert mit `alt=sse` Server-Sent Events an, die einzige Art von Stream, die Kaeso an Gemini-Apps sendet:

```bash tab="curl"
curl -N "https://kaeso.example.com/v1beta/models/claude-haiku-4-5:streamGenerateContent?alt=sse" \
  -H "x-goog-api-key: $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "contents": [
      {"role": "user", "parts": [{"text": "Write a haiku about gateways."}]}
    ]
  }'
```

```python tab="Python"
import os
from google import genai

client = genai.Client(
    api_key=os.environ["KAESO_API_KEY"],
    http_options={"base_url": "https://kaeso.example.com", "api_version": "v1beta"},
)

stream = client.models.generate_content_stream(
    model="claude-haiku-4-5",
    contents="Write a haiku about gateways.",
)
for chunk in stream:
    print(chunk.text or "", end="")
```

```ts tab="TypeScript"
import { GoogleGenAI } from "@google/genai";

const ai = new GoogleGenAI({
  apiKey: process.env.KAESO_API_KEY,
  httpOptions: { baseUrl: "https://kaeso.example.com", apiVersion: "v1beta" },
});

const stream = await ai.models.generateContentStream({
  model: "claude-haiku-4-5",
  contents: "Write a haiku about gateways.",
});
for await (const chunk of stream) {
  process.stdout.write(chunk.text ?? "");
}
```

## Antworten streamen

Fordern Sie einen Stream so an, wie Ihr SDK es vorsieht. Kaeso sendet die Ereignisse, die dieses SDK erwartet, gleich welcher Anbieter das Modell bedient:

```bash tab="curl"
curl -N https://kaeso.example.com/v1/responses \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "input": "Write a haiku about gateways.",
    "stream": true
  }'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)

stream = client.responses.create(
    model="claude-haiku-4-5",
    input="Write a haiku about gateways.",
    stream=True,
)
for event in stream:
    if event.type == "response.output_text.delta":
        print(event.delta, end="")
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});

const stream = await client.responses.create({
  model: "claude-haiku-4-5",
  input: "Write a haiku about gateways.",
  stream: true,
});
for await (const event of stream) {
  if (event.type === "response.output_text.delta") {
    process.stdout.write(event.delta);
  }
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
