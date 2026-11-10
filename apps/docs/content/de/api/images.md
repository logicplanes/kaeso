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

Anbieter, die keine Bilder streamen, bekommen die Anfrage ganz. Kaeso schreibt ihre Antwort als ein Ereignis `image_generation.completed` pro Bild, so wie OpenAI mit `partial_images` 0. Teilbilder von diesen Anbietern zu verlangen beantwortet Kaeso mit 400 und `param` `partial_images`.

## Anbieter

Die Dokumentation jedes eingebauten Anbieters wurde am 2026-11-08 auf Bildgenerierung gelesen. Die Anbieter bedienen die Modelle unten in ihrer eigenen Form. Einen Parameter, den der Anbieter des Modells nicht nimmt, lehnt Kaeso ab, bevor etwas gesendet wird, mit dem Parameter in `param`.

| Anbieter | Generierung | Bearbeitung | Modelle | Quelle |
| --- | --- | --- | --- | --- |
| OpenAI | Ja, gestreamt | Uploads oder JSON, mit Maske | `gpt-image-2.6-sunburst`, `gpt-image-2.6-flare`, `gpt-image-2.5-sunburst`, `gpt-image-2.5-flare`, `gpt-image-2`, `gpt-image-1.5`, `gpt-image-1-mini`, `chatgpt-image-latest` | [Image generation](https://developers.openai.com/api/docs/guides/image-generation) |
| SpaceX | Ja | Bis zu 5 Bilder, keine Maske | `grok-imagine-image-2.0`, `grok-imagine-image` | [Images](https://docs.x.ai/developers/rest-api-reference/inference/images) |
| Google AI Studio | Ja | Bilder im Prompt, keine Maske | `gemini-nano-banana-2.1`, `gemini-3.1-flash-image`, `gemini-3.1-flash-lite-image`, `gemini-3-pro-image`, `gemini-2.5-flash-image` | [Image generation](https://aistudio.google.com/docs/image-generation) |
| Gemini Enterprise Agent Platform | Ja | Bilder im Prompt, keine Maske | Dieselben Bildmodelle von Gemini | [Image generation](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/capabilities/image-generation) |
| OpenRouter | Ja, für die Modelle von OpenAI gestreamt | Als Referenzbilder | Sein Bildkatalog, etwa `openai/gpt-image-2` und `google/gemini-3.1-flash-image` | [Image generation](https://openrouter.ai/docs/guides/overview/multimodal/image-generation) |
| Vercel | Ja | Uploads oder JSON, mit Maske, wo das Modell eine nimmt | Seine Bildmodelle, etwa `openai/gpt-image-2` und `bfl/flux-2-pro` | [Image generation](https://vercel.com/docs/ai-gateway/capabilities/image-generation/openai) |
| Together AI | Ja | Ein Bild per Link | Seine Bildmodelle, etwa `black-forest-labs/FLUX.2-pro` | [Create image](https://docs.together.ai/reference/post-images-generations) |
| QwenCloud | Ja, außer `wan2.6-image` | Bis zu 10 Bilder bei Qwen-Image 2.1, 3 bei den übrigen Qwen-Image-Modellen, 9 bei Wan 2.7, 1 bis 4 bei Wan 2.6, keine bei Z-Image und den Text-zu-Bild-Modellen von Wan; keine Maske | OpenAI-kompatibler Modus: `qwen-image-3.0-pro`, `qwen-image-3.0`, `qwen-image-2.1-pro`; DashScope: `qwen-image-2.1-turbo`, `qwen-image-2.0-pro`, `qwen-image-2.0`, `wan2.7-image-pro`, `wan2.7-image`, `wan2.6-image`, `wan2.6-t2i`, `wan2.5-t2i-preview`, `wan2.2-t2i-plus`, `wan2.2-t2i-flash`, `wan2.1-t2i-plus`, `wan2.1-t2i-turbo`, `z-image-turbo` | [OpenAI-compatible text-to-image](https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image-openai), [Qwen-Image](https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image), [Wan 2.7 Image](https://docs.qwencloud.com/api-reference/image-generation/wan27-image-gen-edit/synchronous), [Wan 2.6 Image](https://docs.qwencloud.com/api-reference/image-generation/wan26-image-gen-edit/synchronous), [Wan text-to-image](https://docs.qwencloud.com/api-reference/image-generation/wan-text-to-image-v2/synchronous), [Z-Image](https://docs.qwencloud.com/api-reference/image-generation/z-image) |
| Z.ai | Ja, ein Bild pro Anfrage | Nein | `glm-image` | [Generate image](https://docs.z.ai/api-reference/image/generate-image) |
| MiniMax | Ja | Nein | `image-01` | [Text to image](https://platform.minimax.io/docs/api-reference/image-generation-t2i) |
| Mistral | Nein: Bilder nur als Werkzeug seiner Agenten | Nein | — | [Image generation tool](https://docs.mistral.ai/studio/agents/agent-tools/image_generation) |
| ElevenLabs | Ja, ein Bild pro Anfrage; Kaeso wartet auf die Generierung (`/v1/flows/image`) | Bis zu 5, 10 oder 14 Bilder als Upload oder Asset, je nach Modell; eine Maske bei GPT Image | `gpt-image-2.5-sunburst`, `gpt-image-2.5-flare`, `gpt-image-2`, `gpt-image-1.5`, `gpt-image-1`, `gemini-3.1-flash-image`, `gemini-3.1-flash-lite-image`, `gemini-3-pro-image`, `gemini-2.5-flash-image`, `bytedance-seedream-5-pro`, `bytedance-seedream-5-lite` | [Leitfaden zu Bild und Video](https://elevenlabs.io/docs/eleven-api/guides/cookbooks/image-and-video) |
| Anthropic, DeepSeek, Groq, Cohere, Ollama Cloud, Moonshot, Cerebras | Vom Anbieter nicht angeboten | — | — | Die API-Referenz jedes Anbieters |
| Fireworks AI, Nebius Token Factory, SambaNova, Scaleway, Hugging Face, NVIDIA, Baseten, SiliconFlow | Nicht über Kaeso | — | — | [Unterstützte Anbieter](../concepts/supported-providers.md) |

Google hat Imagen eingestellt, deshalb ruft Kaeso nur die Bildmodelle von Gemini auf. Eine Größe erreicht sie nur, wenn die Größentabellen von Google sie nennen, etwa `1024x1024` (`1:1` bei `1K`). Senden Sie für andere `aspect_ratio` und `resolution`.

SpaceX nimmt `aspect_ratio` und `resolution` statt `size`. Z.ai antwortet nur mit Links, die nach 30 Tagen ablaufen, deshalb beantwortet Kaeso dort `response_format: "b64_json"` mit 400. Auch QwenCloud antwortet mit Links, die 24 Stunden gelten. Für `b64_json` lädt Kaeso sie über https nur von den Ergebnis-Hosts von QwenCloud (`dashscope-*.aliyuncs.com`), ohne den Anbieterschlüssel und ohne Weiterleitungen, innerhalb des Limits für Anbieterantworten und des Zeitlimits der Anfrage. Gemini und OpenRouter antworten nur mit Base64, deshalb beantwortet Kaeso dort `response_format: "url"` mit 400.

Der OpenAI-kompatible Modus von QwenCloud bedient nur Qwen-Image 3.0 und 2.1 Pro, deshalb gehen seine übrigen Bildmodelle an seine eigene DashScope-API, die bei Wan `resolution` (`1K`, `2K`, `4K`) nimmt. Seine Wan-Text-zu-Bild-Modelle vor 2.6 erzeugen Bilder nur als Tasks: Kaeso startet den Task und wartet innerhalb des Timeouts der Anfrage auf seine Bilder, sodass Apps sie in derselben Anfrage erhalten. Qwen-Image 3.0 wird in der Stufe abgerechnet, die QwenCloud meldet, `1K` oder `2K`, und Z-Image zum doppelten Preis pro Bild, wenn die App `prompt_extend: true` sendet, so wie QwenCloud es abrechnet. Keines seiner Modelle setzt ein Wasserzeichen, solange die App nicht `watermark: true` sendet.

Die Images API von OpenAI nimmt nur einen API-Schlüssel. In einem Projekt, dessen OpenAI-Schlüssel ein [ChatGPT-Konto](../console/provider-keys.md#mit-einem-chatgpt-konto-anmelden) ist, antwortet Kaeso mit 400, bevor etwas gesendet wird: „A ChatGPT account can't make images. Use an API key for OpenAI in this project instead.“.

Die [Abbildung von OpenAI Images](../development/kpp/openai-images.md) nennt jeden Parameter und wie jeder Anbieter ihn nimmt; [DashScope](../development/kpp/dashscope.md) nennt die eigene API von QwenCloud.

## Kosten und Logs

Kaeso berechnet Bild-Token zu den Bildpreisen des Modells aus KPMR, wenn der Anbieter sie meldet, wie OpenAI und Google es tun. Den Rest berechnet es zu den Token-Preisen. Anbieter, die pro Bild oder pro Megapixel abrechnen, bekommen ihre Preise pro Einheit aus KPMR.

Nennt die Anfrage die Größe, Qualität oder Auflösung nicht, von der ein Preis abhängt, gilt der niedrigste gelistete Preis. Das ist der Standard des Anbieters. Einen Preis, den KPMR als Schätzung markiert, zeigt die Modellansicht als Schätzung. Ausgabenbudgets reservieren vor dem Senden den teuersten Fall jeder Anfrage.

Die Anfrage-Logs zeigen den Endpunkt, etwa `images/edits`, die gesendeten und erzeugten Bilder und die Größe und Qualität. Solange Organisation und Projekt keine [Medien aufbewahren](../console/request-logs.md#medien-in-anfragen-und-antworten), wird ein Upload als seine Felder geloggt, jedes Bild als Medientyp und Größe, ebenso jedes Bild, das ein Modell erzeugt hat.

## Das Bildwerkzeug von OpenAI in Responses

Eine Responses-App kann einem Modell von OpenAI das Werkzeug `image_generation` geben. Kaeso reicht das Werkzeug, seine Einträge `image_generation_call` und ihre gestreamten Ereignisse unverändert zwischen App und OpenAI weiter. Andere Anbieter können das Werkzeug von OpenAI nicht ausführen, deshalb beantwortet Kaeso eine Anfrage damit an einen anderen Anbieter mit 400.

## Nächste Schritte

- [Playground](../console/playground.md): Ein Bildmodell in der Konsole ausprobieren, bevor Sie Code schreiben.
- [Modell-API](model-api.md): Jeden Endpunkt, Header und Modellnamen nachschlagen.
- [Anfrage-Logs](../console/request-logs.md): Eine Bildanfrage finden und sehen, was sie gekostet hat.
- [Unterstützte Anbieter](../concepts/supported-providers.md): Sehen, welche Anbieter Kaeso aufruft und wie Sie ihre Schlüssel hinzufügen.
