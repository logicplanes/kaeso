---
title: Bilder-API
description: Bilder über Kaeso mit der Images API von OpenAI erzeugen und bearbeiten, bei jedem Anbieter, der Bilder macht.
order: 2
---

Apps erzeugen und bearbeiten Bilder über Kaeso mit der Images API von OpenAI und den offiziellen SDKs von OpenAI. Kaeso sendet jede Anfrage in der eigenen Form des Anbieters an das Modell, und die Antwort kommt in der Form von OpenAI zurück.

```bash tab="curl"
curl https://kaeso.example.com/v1/images/generations \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "gpt-image-2", "prompt": "A lighthouse at dusk, watercolor", "size": "1024x1024"}'
```

```python tab="Python"
import base64
import os
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

image = client.images.generate(
    model="gpt-image-2",
    prompt="A lighthouse at dusk, watercolor",
    size="1024x1024",
)
with open("lighthouse.png", "wb") as f:
    f.write(base64.b64decode(image.data[0].b64_json))
```

```ts tab="TypeScript"
import { writeFile } from "node:fs/promises";
import OpenAI from "openai";

const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1", apiKey: process.env.KAESO_API_KEY });

const image = await client.images.generate({
  model: "gpt-image-2",
  prompt: "A lighthouse at dusk, watercolor",
  size: "1024x1024",
});
await writeFile("lighthouse.png", Buffer.from(image.data[0].b64_json!, "base64"));
```

Nehmen Sie das Bildmodell zuerst in den Katalog des Projekts auf ([Modelle und Aliasse](../console/models-and-aliases.md)). Jeder [Modellname](model-api.md#modellnamen) und jeder Alias funktioniert, wie bei jedem anderen Endpunkt.

## Endpunkte

| Methode und Pfad | Was er tut |
| --- | --- |
| `POST /v1/images/generations` | Erzeugt Bilder aus einem Prompt (`client.images.generate`) |
| `POST /v1/images/edits` | Erzeugt Bilder aus gegebenen Bildern und einem Prompt, als Upload (`multipart/form-data`) oder als JSON mit `images` aus `image_url` oder `file_id` (`client.images.edit`) |
| `POST /v1/images/variations` | Antwortet für jedes Modell mit 400: OpenAI hat Variationen eingestellt, und kein anderer Anbieter bietet sie. Senden Sie das Bild stattdessen mit einem Prompt an `/v1/images/edits`. |

Fehler haben das Fehlerformat von OpenAI. Der größte Anfrage-Body, Uploads eingeschlossen, ist `body_limits.images`, standardmäßig 64 MiB ([Konfiguration](../self-hosting/configuration.md)).

## Ein Bild bearbeiten

Laden Sie die zu ändernden Bilder hoch und, wo der Anbieter eine nimmt, eine Maske, deren transparente Bereiche markieren, was sich ändern soll:

```python tab="Python"
image = client.images.edit(
    model="gpt-image-2",
    image=[open("room.png", "rb")],
    mask=open("mask.png", "rb"),
    prompt="Add a reading lamp next to the sofa",
)
```

```ts tab="TypeScript"
import { createReadStream } from "node:fs";

const image = await client.images.edit({
  model: "gpt-image-2",
  image: [createReadStream("room.png")],
  mask: createReadStream("mask.png"),
  prompt: "Add a reading lamp next to the sofa",
});
```

Kaeso sendet einen Upload an OpenAI als Upload und als JSON mit Data-URLs an Anbieter, die Bearbeitungen nur als JSON nehmen. Ein Bild wird nie umgewandelt oder skaliert.

## Teilbilder streamen

Mit `stream: true` sendet OpenAI Teilbilder, während es zeichnet, bis zu `partial_images` (0 bis 3), dann das fertige Bild:

```python
stream = client.images.generate(model="gpt-image-2", prompt="A red fox", stream=True, partial_images=2)
for event in stream:
    print(event.type)
```

Anbieter, die keine Bilder streamen, bekommen die Anfrage ganz, und Kaeso schreibt ihre Antwort als ein Ereignis `image_generation.completed` pro Bild, so wie OpenAI mit `partial_images` 0. Teilbilder von diesen Anbietern zu verlangen beantwortet Kaeso mit 400 und `param` `partial_images`.

## Anbieter

Die Dokumentation jedes eingebauten Anbieters wurde am 2026-11-08 auf Bildgenerierung gelesen. Die Anbieter bedienen die Modelle unten in ihrer eigenen Form; einen Parameter, den der Anbieter des Modells nicht nimmt, lehnt Kaeso ab, bevor etwas gesendet wird, mit dem Parameter in `param`.

| Anbieter | Generierung | Bearbeitung | Modelle | Quelle |
| --- | --- | --- | --- | --- |
| OpenAI | Ja, gestreamt | Uploads oder JSON, mit Maske | `gpt-image-2.6-sunburst`, `gpt-image-2.6-flare`, `gpt-image-2.5-sunburst`, `gpt-image-2.5-flare`, `gpt-image-2`, `gpt-image-1.5`, `gpt-image-1-mini`, `chatgpt-image-latest` | [Image generation](https://developers.openai.com/api/docs/guides/image-generation) |
| SpaceX | Ja | Bis zu 5 Bilder, keine Maske | `grok-imagine-image-2.0`, `grok-imagine-image` | [Images](https://docs.x.ai/developers/rest-api-reference/inference/images) |
| Google AI Studio | Ja | Bilder im Prompt, keine Maske | `gemini-nano-banana-2.1`, `gemini-3.1-flash-image`, `gemini-3.1-flash-lite-image`, `gemini-3-pro-image`, `gemini-2.5-flash-image` | [Image generation](https://aistudio.google.com/docs/image-generation) |
| Gemini Enterprise Agent Platform | Ja | Bilder im Prompt, keine Maske | Dieselben Bildmodelle von Gemini | [Image generation](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/capabilities/image-generation) |
| OpenRouter | Ja, für die Modelle von OpenAI gestreamt | Als Referenzbilder | Sein Bildkatalog, etwa `openai/gpt-image-2` und `google/gemini-3.1-flash-image` | [Image generation](https://openrouter.ai/docs/guides/overview/multimodal/image-generation) |
| Vercel | Ja | Uploads oder JSON, mit Maske, wo das Modell eine nimmt | Seine Bildmodelle, etwa `openai/gpt-image-2` und `bfl/flux-2-pro` | [Image generation](https://vercel.com/docs/ai-gateway/capabilities/image-generation/openai) |
| Together AI | Ja | Ein Bild per Link | Seine Bildmodelle, etwa `black-forest-labs/FLUX.2-pro` | [Create image](https://docs.together.ai/reference/post-images-generations) |
| QwenCloud | Ja | Bis zu 10 Bilder, keine Maske | `qwen-image-3.0-pro`, `qwen-image-3.0`, `qwen-image-2.1-pro` | [OpenAI-compatible text-to-image](https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image-openai) |
| Z.ai | Ja, ein Bild pro Anfrage | Nein | `glm-image` | [Generate image](https://docs.z.ai/api-reference/image/generate-image) |
| MiniMax | Ja | Nein | `image-01` | [Text to image](https://platform.minimax.io/docs/api-reference/image-generation-t2i) |
| Mistral | Nein: Bilder nur als Werkzeug seiner Agenten | Nein | — | [Image generation tool](https://docs.mistral.ai/studio/agents/agent-tools/image_generation) |
| Anthropic, DeepSeek, Groq, Cohere, Ollama Cloud, Moonshot, Cerebras | Vom Anbieter nicht angeboten | — | — | Die API-Referenz jedes Anbieters |

Google hat Imagen eingestellt, deshalb ruft Kaeso nur die Bildmodelle von Gemini auf. Eine Größe erreicht sie nur, wenn die Größentabellen von Google sie nennen, etwa `1024x1024` (`1:1` bei `1K`); senden Sie für andere `aspect_ratio` und `resolution`. SpaceX nimmt `aspect_ratio` und `resolution` statt `size`. Z.ai und QwenCloud antworten nur mit Links, die nach 30 Tagen bzw. 24 Stunden ablaufen, deshalb beantwortet Kaeso dort `response_format: "b64_json"` mit 400. Gemini und OpenRouter antworten nur mit Base64, deshalb beantwortet Kaeso dort `response_format: "url"` mit 400. Die [Abbildung von OpenAI Images](../development/kpp/openai-images.md) nennt jeden Parameter und wie jeder Anbieter ihn nimmt.

## Kosten und Logs

Kaeso berechnet Bild-Token zu den Bildpreisen des Modells aus KPMR, wenn der Anbieter sie meldet, wie OpenAI und Google es tun, und den Rest zu seinen Token-Preisen. Anbieter, die pro Bild oder pro Megapixel abrechnen, bekommen ihre Preise pro Einheit aus KPMR; nennt die Anfrage die Größe, Qualität oder Auflösung nicht, von der ein Preis abhängt, gilt der niedrigste gelistete Preis, denn das ist der Standard des Anbieters. Einen Preis, den KPMR als Schätzung markiert, zeigt die Modellansicht als Schätzung. Ausgabenbudgets reservieren vor dem Senden den teuersten Fall jeder Anfrage.

Die Anfrage-Logs zeigen den Endpunkt, etwa `images/edits`, die gesendeten und erzeugten Bilder und die Größe und Qualität. Bild-Bytes behalten sie nie: Ein Upload wird als seine Felder geloggt, jedes Bild als Medientyp und Größe ([Anfrage-Logs](../console/request-logs.md)).

## Das Bildwerkzeug von OpenAI in Responses

Eine Responses-App kann einem Modell von OpenAI das Werkzeug `image_generation` geben. Kaeso reicht das Werkzeug, seine Einträge `image_generation_call` und ihre gestreamten Ereignisse unverändert zwischen App und OpenAI weiter. Andere Anbieter können das Werkzeug von OpenAI nicht ausführen, deshalb beantwortet Kaeso eine Anfrage damit an einen anderen Anbieter mit 400.

## Nächste Schritte

- [Modell-API](model-api.md): Jeden Endpunkt, Header und Modellnamen nachschlagen.
- [Anfrage-Logs](../console/request-logs.md): Eine Bildanfrage finden und sehen, was sie gekostet hat.
- [Unterstützte Anbieter](../concepts/supported-providers.md): Sehen, welche Anbieter Kaeso aufruft und wie Sie ihre Schlüssel hinzufügen.
