---
title: ElevenLabs API
description: Call ElevenLabs speech, sound, music and transcription through Kaeso with the official ElevenLabs SDKs.
order: 2
---

Apps built with the official `elevenlabs` SDKs call ElevenLabs through Kaeso: set Kaeso as the base URL and a Kaeso key as the API key. Kaeso serves ElevenLabs' generation and listing endpoints with keys, limits, logs and costs, and sends each request to the model's provider, so an ElevenLabs app can also reach other providers' speech and transcription models.

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

The SDK sends the key in `xi-api-key`; Kaeso takes a Kaeso key there as in `Authorization: Bearer`. Add ElevenLabs as a provider with an ElevenLabs key in the console, or use the [Audio API](audio.md) with the OpenAI SDKs.

## Endpoints

| Endpoint | SDK call | Kaeso |
| --- | --- | --- |
| `POST /v1/text-to-speech/{voice_id}` and `/stream`, `/with-timestamps`, `/stream/with-timestamps` | `text_to_speech.convert`, `.stream`, `.convert_with_timestamps`, `.stream_with_timestamps` | speech, the audio sent as it is made; timestamps as ElevenLabs' JSON or its lines of JSON |
| `POST /v1/text-to-dialogue` and its streaming and timestamp forms | `text_to_dialogue.convert` and the rest | a dialogue, each line in its voice |
| `POST /v1/sound-generation` | `text_to_sound_effects.convert` | a sound effect, billed by the minute of its length |
| `POST /v1/music` and `/v1/music/stream` | `music.compose`, `music.stream` | music; ElevenLabs refuses it on its free tier |
| `POST /v1/voice-changer/{voice_id}` and `/stream` | `speech_to_speech.convert`, `.stream` | speech in another voice, from an upload |
| `POST /v1/speech-to-text` | `speech_to_text.convert` | a transcript with timed words and speakers |
| `GET /v1/models` | `models.list` | the project's ElevenLabs models, from Kaeso's catalogue |
| `GET /v1/voices`, `GET /v2/voices`, `GET /v1/voices/{voice_id}` | `voices.get_all`, `voices.search`, `voices.get` | the voices of the project's ElevenLabs account, asked of ElevenLabs with the project's ElevenLabs key; each listing counts against the key's request rates |

`output_format`, `enable_logging` and `optimize_streaming_latency` pass to ElevenLabs as written. The request body of an upload is bounded by [`body_limits.audio`](../self-hosting/configuration.md#request-and-response-bodies). A voice id is letters and digits, as ElevenLabs lists them; any other is refused with 400 before anything is sent.

## Other providers

An ElevenLabs app's request is a Kaeso request: an alias or a model of another provider serves it when it can. Text to speech reaches OpenAI, Google and every provider of the [Audio API](audio.md#providers) with the voice id as the voice; speech to text reaches every transcription provider, timed words included where the provider times them. What only ElevenLabs takes, such as `voice_settings` beyond `speed`, pronunciation dictionaries or `keyterms`, is refused for other providers with the field in `param`.

## What Kaeso refuses

Account, voice cloning and design, dubbing, studio projects, history, pronunciation dictionaries, workspaces, agents, single-use tokens and the other management endpoints stay at ElevenLabs. Kaeso answers them with 404 in ElevenLabs' error shape, `{"detail": {"type": "not_found", "message": "Kaeso does not serve `/v1/dubbing`; manage voices, dubbing and your account at ElevenLabs."}}`. `POST /v1/music/detailed` and `/v1/music/plan` are refused too.

The realtime WebSocket APIs (text to speech input streaming, multi-context, the dialogue WebSocket, realtime speech to text and agents) are not served. A WebSocket session holds one upstream connection for its whole life, takes audio and text in both directions and bills by session, while Kaeso's model pipeline translates one request and its reply; serving them needs a session layer of its own, which [an issue](https://github.com/logicplanes/kaeso/issues/803) tracks.

## Costs and logs

ElevenLabs bills speech per character, at KPMR's price per million characters of each model; Kaeso logs the characters ElevenLabs reports in its `character-cost` header. The voice changer's cost stays unknown: Kaeso doesn't decode the audio to measure its length, and KPMR has no price for the speech-to-speech models, so only the header's characters are logged when ElevenLabs sends it. Transcripts are billed by the hour of audio, sound effects and music by the minute of their length. The [request log](../console/request-logs.md) names the endpoint, such as `text-to-speech` or `speech-to-text`, and never stores audio.
