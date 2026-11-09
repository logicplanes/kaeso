---
title: ElevenLabs-API
description: Sprache, Klänge, Musik und Transkription von ElevenLabs über Kaeso mit den offiziellen SDKs von ElevenLabs aufrufen.
order: 2
---

Apps, die mit den offiziellen SDKs `elevenlabs` gebaut sind, rufen ElevenLabs über Kaeso auf: Setzen Sie Kaeso als Basis-URL und einen Kaeso-Schlüssel als API-Schlüssel. Kaeso bedient die Endpunkte von ElevenLabs zum Erzeugen und Auflisten mit Schlüsseln, Limits, Logs und Kosten und sendet jede Anfrage an den Anbieter des Modells, sodass eine ElevenLabs-App auch die Sprach- und Transkriptionsmodelle anderer Anbieter erreicht.

```bash tab="curl"
curl "https://kaeso.example.com/v1/text-to-speech/21m00Tcm4TlvDq8ikWAM?output_format=mp3_44100_128" \
  -H "xi-api-key: $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"text": "Your order shipped today.", "model_id": "eleven_flash_v2_5"}' \
  --output speech.mp3
```

```python tab="Python"
import os
from elevenlabs import ElevenLabs

client = ElevenLabs(base_url="https://kaeso.example.com", api_key=os.environ["KAESO_API_KEY"])

audio = client.text_to_speech.convert(
    voice_id="21m00Tcm4TlvDq8ikWAM",
    text="Your order shipped today.",
    model_id="eleven_flash_v2_5",
    output_format="mp3_44100_128",
)
with open("speech.mp3", "wb") as f:
    for chunk in audio:
        f.write(chunk)
```

```ts tab="TypeScript"
import fs from "node:fs";
import { ElevenLabsClient } from "@elevenlabs/elevenlabs-js";

const client = new ElevenLabsClient({ baseUrl: "https://kaeso.example.com", apiKey: process.env.KAESO_API_KEY });

const audio = await client.textToSpeech.convert("21m00Tcm4TlvDq8ikWAM", {
  text: "Your order shipped today.",
  modelId: "eleven_flash_v2_5",
});
fs.writeFileSync("speech.mp3", Buffer.from(await new Response(audio).arrayBuffer()));
```

Das SDK sendet den Schlüssel in `xi-api-key`; Kaeso nimmt dort einen Kaeso-Schlüssel wie in `Authorization: Bearer`. Fügen Sie ElevenLabs in der Konsole als Anbieter mit einem Schlüssel von ElevenLabs hinzu, oder verwenden Sie die [Audio-API](audio.md) mit den SDKs von OpenAI.

## Endpunkte

| Endpunkt | SDK-Aufruf | Kaeso |
| --- | --- | --- |
| `POST /v1/text-to-speech/{voice_id}` und `/stream`, `/with-timestamps`, `/stream/with-timestamps` | `text_to_speech.convert`, `.stream`, `.convert_with_timestamps`, `.stream_with_timestamps` | Sprache, das Audio gesendet, während es entsteht; Zeitmarken als JSON von ElevenLabs oder als seine JSON-Zeilen |
| `POST /v1/text-to-dialogue` und seine Formen mit Streaming und Zeitmarken | `text_to_dialogue.convert` und die übrigen | ein Dialog, jede Zeile in ihrer Stimme |
| `POST /v1/sound-generation` | `text_to_sound_effects.convert` | ein Soundeffekt, nach Minuten seiner Länge abgerechnet |
| `POST /v1/music` und `/v1/music/stream` | `music.compose`, `music.stream` | Musik; im kostenlosen Tarif lehnt ElevenLabs sie ab |
| `POST /v1/voice-changer/{voice_id}` und `/stream` | `speech_to_speech.convert`, `.stream` | Sprache in einer anderen Stimme, aus einem Upload |
| `POST /v1/speech-to-text` | `speech_to_text.convert` | ein Transkript mit zeitlich markierten Wörtern und Sprechern |
| `GET /v1/models` | `models.list` | die ElevenLabs-Modelle des Projekts, aus dem Katalog von Kaeso |
| `GET /v1/voices`, `GET /v2/voices`, `GET /v1/voices/{voice_id}` | `voices.get_all`, `voices.search`, `voices.get` | die Stimmen des ElevenLabs-Kontos des Projekts, bei ElevenLabs mit dem ElevenLabs-Schlüssel des Projekts abgefragt |

`output_format`, `enable_logging` und `optimize_streaming_latency` gehen wie geschrieben an ElevenLabs. Den Anfrage-Body eines Uploads begrenzt [`body_limits.audio`](../self-hosting/configuration.md#anfrage--und-antwortkörper).

## Andere Anbieter

Die Anfrage einer ElevenLabs-App ist eine Kaeso-Anfrage: Ein Alias oder ein Modell eines anderen Anbieters bedient sie, wenn es kann. Text to Speech erreicht OpenAI, Google und jeden Anbieter der [Audio-API](audio.md#anbieter), mit der Stimmen-ID als Stimme; Speech to Text erreicht jeden Anbieter für Transkription, zeitlich markierte Wörter eingeschlossen, wo der Anbieter Zeiten liefert. Was nur ElevenLabs nimmt, etwa `voice_settings` über `speed` hinaus, Aussprachewörterbücher oder `keyterms`, wird für andere Anbieter mit dem Feld in `param` abgelehnt.

## Was Kaeso ablehnt

Konto, Klonen und Gestalten von Stimmen, Synchronisation, Studio-Projekte, Verlauf, Aussprachewörterbücher, Workspaces, Agenten, Einmal-Token und die übrigen Verwaltungsendpunkte bleiben bei ElevenLabs. Kaeso beantwortet sie mit 404 in der Fehlerform von ElevenLabs, `{"detail": {"type": "not_found", "message": "Kaeso does not serve `/v1/dubbing`; manage voices, dubbing and your account at ElevenLabs."}}`. `POST /v1/music/detailed` und `/v1/music/plan` werden ebenfalls abgelehnt.

Die Echtzeit-APIs über WebSocket (Text to Speech mit gestreamter Eingabe, Multi-Context, der Dialog-WebSocket, Speech to Text in Echtzeit und Agenten) werden nicht bedient. Eine WebSocket-Sitzung hält eine Verbindung zum Anbieter für ihre ganze Dauer, nimmt Audio und Text in beide Richtungen und rechnet pro Sitzung ab, während die Modell-Pipeline von Kaeso eine Anfrage und ihre Antwort übersetzt; sie zu bedienen braucht eine eigene Sitzungsschicht, die [ein Issue](https://github.com/logicplanes/kaeso/issues/803) verfolgt.

## Kosten und Logs

ElevenLabs rechnet Sprache pro Zeichen ab, zum Preis von KPMR pro Million Zeichen jedes Modells; Kaeso loggt die Zeichen, die ElevenLabs in seinem Header `character-cost` meldet. Die Kosten des Voice Changers bleiben unbekannt: Kaeso dekodiert das Audio nicht, um seine Länge zu messen, und KPMR hat keinen Preis für die Speech-to-Speech-Modelle; geloggt werden nur die Zeichen des Headers, wenn ElevenLabs ihn sendet. Transkripte werden nach Stunden Audio abgerechnet, Soundeffekte und Musik nach Minuten ihrer Länge. Das [Anfrage-Log](../console/request-logs.md) nennt den Endpunkt, etwa `text-to-speech` oder `speech-to-text`, und speichert nie Audio.
