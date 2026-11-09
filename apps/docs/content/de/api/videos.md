---
title: Videos-API
description: Videos über Kaeso mit der Videos API von OpenAI machen, bei jedem Anbieter, der Videos macht.
order: 2
---

Apps machen Videos über Kaeso mit der Videos API von OpenAI und den offiziellen SDKs von OpenAI. Kaeso startet den Job beim Anbieter des Modells in dessen eigener Form, verfolgt ihn unter einer eigenen ID und antwortet in der Form von OpenAI.

```bash tab="curl"
curl https://kaeso.example.com/v1/videos \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "grok-imagine-video-1.5", "prompt": "A lighthouse at dusk, waves rolling in", "seconds": "8", "size": "1280x720"}'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

video = client.videos.create_and_poll(
    model="grok-imagine-video-1.5",
    prompt="A lighthouse at dusk, waves rolling in",
    seconds="8",
    size="1280x720",
)
if video.status == "completed":
    client.videos.download_content(video.id).write_to_file("lighthouse.mp4")
```

```ts tab="TypeScript"
import { writeFile } from "node:fs/promises";
import { setTimeout } from "node:timers/promises";
import OpenAI from "openai";

const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1", apiKey: process.env.KAESO_API_KEY });

let video = await client.videos.create({
  model: "grok-imagine-video-1.5",
  prompt: "A lighthouse at dusk, waves rolling in",
  seconds: "8",
  size: "1280x720",
});
while (video.status === "queued" || video.status === "in_progress") {
  await setTimeout(5000);
  video = await client.videos.retrieve(video.id);
}
const file = await client.videos.downloadContent(video.id);
await writeFile("lighthouse.mp4", Buffer.from(await file.arrayBuffer()));
```

Fügen Sie das Videomodell zuerst dem Katalog des Projekts hinzu ([Modelle und Aliase](../console/models-and-aliases.md)). Jeder [Modellname](model-api.md#modellnamen) und jeder Alias funktioniert, wie bei jedem anderen Endpunkt.

## Endpunkte

| Methode und Pfad | Was er tut |
| --- | --- |
| `POST /v1/videos` | Startet ein Video aus einem Prompt, als JSON oder als Upload (`multipart/form-data`) mit einem Bild `input_reference` als erstem Bild (`client.videos.create`) |
| `GET /v1/videos/{video_id}` | Der Job des Videos, nachdem Kaeso den Anbieter gefragt hat, wie er steht (`client.videos.retrieve`, `client.videos.poll`) |
| `GET /v1/videos/{video_id}/content` | Die fertige Datei, mit `variant` `video` (Standard), `thumbnail` oder `spritesheet` (`client.videos.download_content`) |
| `POST /v1/videos/{video_id}/remix` | Startet ein neues Video aus einem fertigen und einem neuen Prompt (`client.videos.remix`) |
| `GET /v1/videos` | Die Videos des Projekts, mit `after`, `limit` (bis 100, standardmäßig 20) und `order` (standardmäßig `desc`) (`client.videos.list`) |
| `DELETE /v1/videos/{video_id}` | Löscht das Video (`client.videos.delete`) |
| `POST /v1/videos/edits`, `/v1/videos/extensions`, `/v1/videos/characters` | Antwortet mit 404: Diese Endpunkte gehörten zu Sora. Remixen Sie stattdessen ein Video oder erstellen Sie ein neues. |

Fehler haben das Fehlerformat von OpenAI. Der größte Anfragekörper, Uploads eingeschlossen, ist `body_limits.model_api`, standardmäßig 32 MiB ([Konfiguration](../self-hosting/configuration.md#anfrage--und-antwortkörper)).

Kaeso liest die Felder von OpenAI: `model`, `prompt`, `seconds`, `size` und `input_reference`. Eine `size` erreicht einen Anbieter, der Auflösung und Seitenverhältnis nimmt, nur wenn er beides listet, so wie `1280x720` `720p` bei `16:9` ist. Jedes andere Feld erreicht nur Together AI, das die Form von OpenAI nimmt; jeder andere Anbieter antwortet mit 400 und dem Feld in `param`.

## Wie ein Job läuft

Ein Video zu erstellen antwortet sofort mit seinem Job, `queued` oder `in_progress`, unter einer ID wie `video_06a1…`. Die ID gehört Kaeso und zeigt nie die des Anbieters.

Jeder Abruf fragt den Anbieter, wie der Job steht, solange er läuft, und behält, was er sagt. Kann der Anbieter nicht antworten, gibt der Abruf den Job so zurück, wie Kaeso ihn zuletzt gesehen hat; fragen Sie dann später noch einmal. Nach einem beendeten Job wird nie wieder gefragt. `poll` und `create_and_poll` der SDKs rufen den Job ab, bis er endet.

Eine Liste zeigt jedes Video so, wie Kaeso es zuletzt gesehen hat, ohne die Anbieter zu fragen. Ein Video gehört dem Projekt, dessen Schlüssel es gestartet hat; der Schlüssel eines anderen Projekts bekommt 404, dieselbe Antwort wie für eine ID, die es nicht gibt. Wird der Anbieterschlüssel ersetzt, der ein Video gestartet hat, erreicht Kaeso den Job nicht mehr und antwortet mit 409.

## Ein Video herunterladen

Kaeso streamt die Datei von dort, wo der Anbieter sie aufbewahrt, ohne sie zu speichern, und reicht `Range` durch. Veo und Gemini Omni senden die Datei in ihrer Antwort; Kaeso liest sie für jeden Download neu, begrenzt durch `body_limits.provider_response` (standardmäßig 128 MiB).

Anbieter bewahren Dateien nur begrenzt auf. Wo ein Anbieter kurzlebige Links ausgibt, wie MiniMax und ElevenLabs, holt Kaeso für jeden Download einen frischen Link. Nach `expires_at` des Jobs antwortet der Download mit 404. Nur Z.ai bietet ein `thumbnail`, und kein Anbieter bietet ein `spritesheet`; die Frage danach antwortet mit 404 und `param` `variant`.

Ein Video zu löschen löscht es bei den Anbietern, die das können, Google AI Studio und der H3-API von MiniMax; überall sonst vergisst Kaeso es.

## Ein Video remixen

Ein Remix startet ein neues Video aus einem fertigen und einem neuen Prompt, beim selben Anbieter mit demselben Modell:

```python
remix = client.videos.remix(video.id, prompt="The same lighthouse in a snowstorm")
```

SpaceX remixt über seine API für Bearbeitungen, und Gemini Omni setzt die frühere Interaktion fort. Jeder andere Anbieter antwortet mit 400 und `param` `video_id`.

## Anbieter

Die Dokumentation jedes eingebauten Anbieters wurde am 2026-11-09 auf Videoerzeugung gelesen. Kaeso lehnt ein Feld, das der Anbieter des Modells nicht nimmt, vor dem Senden ab, mit dem Feld in `param`.

| Anbieter | Modelle | Bilder | Remix | Quelle |
| --- | --- | --- | --- | --- |
| SpaceX | `grok-imagine-video-1.5`, `grok-imagine-video-1.5-lite`, `grok-imagine-video` | Erstes Bild, hochgeladen oder verlinkt | Ja | [Videos](https://docs.x.ai/developers/rest-api-reference/inference/videos) |
| Google AI Studio | `gemini-omni-1.2-flash`, `gemini-omni-1.1-flash` | Erstes Bild, hochgeladen oder verlinkt | Ja | [Gemini Omni](https://aistudio.google.com/docs/omni) |
| Gemini Enterprise Agent Platform | `veo-3.1-generate-001`, `veo-3.1-fast-generate-001`, `veo-3.1-lite-generate-001`, mit einem Dienstkontoschlüssel | Erstes Bild, hochgeladen | Nein | [Veo 3.1](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/veo/3-1-generate) |
| Gemini Enterprise Agent Platform | `gemini-omni-1.2-flash`, `gemini-omni-1.1-flash-preview`, `gemini-omni-flash-preview`, mit einem Dienstkontoschlüssel | Erstes Bild, hochgeladen oder als Link `gs://` | Ja | [Videos aus Text erzeugen](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/video/generate-videos-from-text) |
| MiniMax | `MiniMax-Hailuo-2.3`, `MiniMax-Hailuo-2.3-Fast`, `MiniMax-Hailuo-02`, `MiniMax-H3` und seine Varianten | Erstes Bild, hochgeladen oder verlinkt | Nein | [Videoerzeugung](https://platform.minimax.io/docs/api-reference/video-generation-t2v) |
| Z.ai | `cogvideox-3` | Erstes Bild, hochgeladen oder verlinkt | Nein | [Video erzeugen](https://docs.z.ai/api-reference/video/generate-video) |
| OpenRouter | Seine Videomodelle, etwa `google/veo-3.1` und `alibaba/wan-3.0` | Erstes Bild, hochgeladen oder verlinkt | Nein | [Videoerzeugung](https://openrouter.ai/docs/api/api-reference/video-generation/submit-a-video-generation-request) |
| Together AI | Seine Videomodelle, etwa `ByteDance/Seedance-2.0` und `Wan-AI/wan2.7-t2v` | Erstes Bild, nur verlinkt | Nein | [Video erstellen](https://docs.together.ai/reference/create-videos) |
| ElevenLabs | `veo-3.1-generate-001`, `veo-3.1-fast-generate-001`, `bytedance-seedance-v2` und seine Varianten | Erstes Bild, hochgeladen | Nein | [OpenAPI-Dokument](https://api.elevenlabs.io/openapi.json) |
| OpenAI | Nein: OpenAI hat Sora am 24. September 2026 abgeschaltet, deshalb antwortet ein Modell `sora-*` mit 400 | — | — | [Abkündigungen](https://platform.openai.com/docs/deprecations) |
| Vercel | Nein: Vercel dokumentiert Video nur über sein AI SDK, nicht über HTTP | — | — | [Videoerzeugung](https://vercel.com/docs/ai-gateway/modalities/video-generation) |
| QwenCloud | Text zu Video und Bild zu Video mit Wan, etwa `wan3.0-video`, `wan2.7-t2v` und `wan2.6-i2v-flash` | Erstes Bild, hochgeladen oder verlinkt, bei Bild-zu-Video-Modellen und Wan 3.0 | Nein | [Videoerzeugung](https://docs.qwencloud.com/api-reference/video-generation/wan-text-to-video/create-task) |
| Anthropic, DeepSeek, Groq, Mistral, Cohere, Ollama Cloud, Moonshot, Cerebras | Vom Anbieter nicht angeboten | — | — | Die API-Referenz jedes Anbieters |

Ein Projekt führt ein Modell für Video, wenn KPMR oder sein eigener Eintrag sagt, dass das Modell Video macht. Veo auf Gemini Enterprise Agent Platform braucht einen Dienstkontoschlüssel: Der Express-Modus listet keine Veo-Modelle, deshalb lehnt Kaeso einen Express-Schlüssel für sie ab. Auch Gemini Omni auf der Agent Platform braucht einen Dienstkontoschlüssel, weil der Express-Modus keine Omni-Modelle listet, und geht an den globalen Standort der Plattform, gleich welchen Standort der Schlüssel nennt, weil Omni nur dort angeboten wird.

Das [Mapping von OpenAI Videos](../development/kpp/openai-videos.md) und die Anbieterseiten daneben listen jedes Feld und wie jeder Anbieter es nimmt.

## Kosten und Logs

Ein Video zu starten kostet nichts. Kaeso berechnet es einmal, wenn die erste Anfrage es fertig sieht, zu den Preisen des Modells aus KPMR: pro Sekunde oder pro Video, nach Auflösung, Ton und Länge, wo die Preise sich unterscheiden. Gemini Omni wird nach seinen Token berechnet. Ein gescheitertes Video kostet nichts, und ein Modell ohne Preise in KPMR wird ohne Kosten geloggt. Kaeso behält `outputs.cost` von Together AI beim Job, rechnet es aber nicht ab: Die Referenz von Together gibt es als ganze Zahl ohne Einheit an.

Ausgabenbudgets prüfen den teuersten Fall eines Videos, bevor es startet, und seine Kosten zählen, sobald es endet. Die Anfrage-Logs zeigen den Start als `videos` oder `videos/remix` und das fertige Video als `videos/completed`, mit seinen Clips, Sekunden und der Auflösung. Video- und Bild-Bytes behalten sie nie ([Anfrage-Logs](../console/request-logs.md)).

## Nächste Schritte

- [Modell-API](model-api.md): Jeden Endpunkt, Header und Modellnamen nachschlagen.
- [Anfrage-Logs](../console/request-logs.md): Ein Video finden und sehen, was es gekostet hat.
- [Unterstützte Anbieter](../concepts/supported-providers.md): Sehen, welche Anbieter Kaeso aufruft und wie Sie ihre Schlüssel hinzufügen.
