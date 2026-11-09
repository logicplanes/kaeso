---
title: ElevenLabs-API
description: Sprache, Klänge, Musik und Transkription von ElevenLabs über Kaeso mit den offiziellen SDKs von ElevenLabs aufrufen.
order: 5
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
| `GET /v1/voices`, `GET /v2/voices`, `GET /v1/voices/{voice_id}` | `voices.get_all`, `voices.search`, `voices.get` | die Stimmen des ElevenLabs-Kontos des Projekts, bei ElevenLabs mit dem ElevenLabs-Schlüssel des Projekts abgefragt; jede Liste zählt gegen die Anfrageraten des Schlüssels |

`output_format`, `enable_logging` und `optimize_streaming_latency` gehen wie geschrieben an ElevenLabs. Den Anfrage-Body eines Uploads begrenzt [`body_limits.audio`](../self-hosting/configuration.md#anfrage--und-antwortkörper). Eine Stimmen-ID besteht aus Buchstaben und Ziffern, wie ElevenLabs sie listet; jede andere wird mit 400 abgelehnt, bevor etwas gesendet wird.

## Echtzeit-Sitzungen

Kaeso bedient die Echtzeit-WebSockets von ElevenLabs: Text to Speech mit gestreamter Eingabe (`/v1/text-to-speech/{voice_id}/stream-input`) und seine Multi-Context-Form (`/multi-stream-input`), die Text-to-Dialogue-Sockets (`/v1/text-to-dialogue/stream-input` und `/multi-stream-input`) und Speech to Text in Echtzeit (`/v1/speech-to-text/realtime`). Die Echtzeit-Clients der SDKs funktionieren mit Kaeso unverändert, etwa Text to Speech aus Text, der in Teilen ankommt:

```python
import os
from elevenlabs import ElevenLabs

client = ElevenLabs(base_url="https://kaeso.example.com", api_key=os.environ["KAESO_API_KEY"])


def sentences():
    yield "Your order shipped today. "
    yield "It arrives on Friday."


with open("speech.mp3", "wb") as f:
    for chunk in client.text_to_speech.convert_realtime(
        voice_id="21m00Tcm4TlvDq8ikWAM",
        text=sentences(),
        model_id="eleven_flash_v2_5",
    ):
        f.write(chunk)
```

- **Schlüssel.** Senden Sie den Kaeso-Schlüssel in `xi-api-key` oder `Authorization: Bearer` oder als `xi_api_key` in der ersten Nachricht eines Sprach-Sockets, so wie ElevenLabs ihn nimmt. Kaeso nimmt ihn aus der Nachricht und öffnet den Socket bei ElevenLabs mit dem ElevenLabs-Schlüssel des Projekts. Einmal-Tokens (`token`, `single_use_token`) gehören ElevenLabs, deshalb lehnt Kaeso sie mit 401 ab.
- **Modell.** Nennen Sie das Modell in `model_id`; ein Socket ohne Modell wird mit 400 abgelehnt, weil Kaeso nach Modell bepreist und begrenzt. Nur die ElevenLabs-Modelle des Projekts bedienen diese Sockets: Kein anderer Anbieter nimmt die Nachrichten von ElevenLabs, deshalb werden ein Alias oder das Modell eines anderen Anbieters abgelehnt.
- **Limits.** Eine Sitzung zählt beim Öffnen als eine Anfrage gegen die Anfrageraten des Schlüssels. Solange sie läuft, prüft Kaeso alle 30 Sekunden die harten Ausgabenbudgets und schließt die Sitzung, wenn eines aufgebraucht ist. Eine Sitzung endet, wenn sie `upstream.stream_idle_timeout` lang untätig ist, nach einer Stunde oder wenn Kaeso neu startet. Eine Nachricht darf bis zu 1 MiB groß sein.
- **Nachrichten.** Nachrichten gehen unverändert zwischen App und ElevenLabs; Kaeso liest sie nur, um zu zählen, was sie tragen, und um Schlüssel zu entfernen. Die eigenen Nachrichten und Close-Codes von ElevenLabs erreichen die App unverändert.
- **Fehler.** Was Kaeso vor dem Öffnen des Sockets ablehnt, kommt über HTTP im Fehlerformat von ElevenLabs zurück. Danach sendet Kaeso die eigene Fehlernachricht des Sockets, `{"message_type": "auth_error", "error": …}` bei Speech to Text und `{"message", "error", "code", "param"}` bei den Sprach-Sockets, und schließt mit 1008 bei allem, was die App beheben kann, sonst mit 1011.
- **Logs und Kosten.** Das [Anfrage-Log](../console/request-logs.md) zeigt eine Sitzung als eine Anfrage, etwa `text-to-speech/stream-input`, mit den Zeichen, die sie zum Sprechen gesendet hat, oder der Länge des Audios, das sie zum Transkribieren gesendet hat, nie mit dem Audio. Sprache wird pro Zeichen abgerechnet und `scribe_v2_realtime` zum KPMR-Preis pro Stunde Audio, gemessen am `audio_format` der Sitzung (`pcm_*` oder `ulaw_8000`).

## Andere Anbieter

Die Anfrage einer ElevenLabs-App ist eine Kaeso-Anfrage: Ein Alias oder ein Modell eines anderen Anbieters bedient sie, wenn es kann. Text to Speech erreicht OpenAI, Google und jeden Anbieter der [Audio-API](audio.md#anbieter), mit der Stimmen-ID als Stimme; Speech to Text erreicht jeden Anbieter für Transkription, zeitlich markierte Wörter eingeschlossen, wo der Anbieter Zeiten liefert. Was nur ElevenLabs nimmt, etwa `voice_settings` über `speed` hinaus, Aussprachewörterbücher oder `keyterms`, wird für andere Anbieter mit dem Feld in `param` abgelehnt.

## Was Kaeso ablehnt

Konto, Klonen und Gestalten von Stimmen, Synchronisation, Studio-Projekte, Verlauf, Aussprachewörterbücher, Workspaces, Agenten, Einmal-Token und die übrigen Verwaltungsendpunkte bleiben bei ElevenLabs. Kaeso beantwortet sie mit 404 in der Fehlerform von ElevenLabs, `{"detail": {"type": "not_found", "message": "Kaeso does not serve `/v1/dubbing`; manage voices, dubbing and your account at ElevenLabs."}}`. `POST /v1/music/detailed` und `/v1/music/plan` werden ebenfalls abgelehnt.

Agenten (`/v1/convai/…`) werden nicht bedient: Ein Agent läuft auf der eigenen Plattform von ElevenLabs mit eigenem Sprachmodell, eigenen Tools und eigener Wissensbasis, die Kaeso nicht routet. Rufen Sie sie bei ElevenLabs auf.

## Kosten und Logs

ElevenLabs rechnet Sprache pro Zeichen ab, zum Preis von KPMR pro Million Zeichen jedes Modells; Kaeso loggt die Zeichen, die ElevenLabs in seinem Header `character-cost` meldet. Die Kosten des Voice Changers bleiben unbekannt: KPMR bepreist die Speech-to-Speech-Modelle pro Minute Eingabe-Audio, aber Kaeso dekodiert das Audio nicht, um seine Länge zu messen; geloggt werden nur die Zeichen des Headers, wenn ElevenLabs ihn sendet. Transkripte werden nach Stunden Audio abgerechnet, Soundeffekte und Musik nach Minuten ihrer Länge. Das [Anfrage-Log](../console/request-logs.md) nennt den Endpunkt, etwa `text-to-speech` oder `speech-to-text`, und speichert nie Audio.
