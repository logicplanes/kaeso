---
title: ElevenLabs API
description: Call ElevenLabs speech, sound, music, images and transcription through Kaeso with the official ElevenLabs SDKs.
order: 5
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
| `POST /v1/music` and `/v1/music/stream` | `music.compose`, `music.stream` | music; ElevenLabs refuses it on plans without music |
| `POST /v1/voice-changer/{voice_id}` and `/stream` | `speech_to_speech.convert`, `.stream` | speech in another voice, from an upload |
| `POST /v1/speech-to-text` | `speech_to_text.convert` | a transcript with timed words and speakers |
| `GET /v1/models` | `models.list` | the project's ElevenLabs models, from Kaeso's catalogue |
| `GET /v1/voices`, `GET /v2/voices`, `GET /v1/voices/{voice_id}` | `voices.get_all`, `voices.search`, `voices.get` | the voices of the project's ElevenLabs account, asked of ElevenLabs with the project's ElevenLabs key; each listing counts against the key's request rates |
| `POST /v1/flows/image` | `flows.image.create` | an image generation with one of the project's ElevenLabs image models, answered at once with an id of Kaeso's own ([image generations](#image-generations)) |
| `GET /v1/flows/image/{generation_id}`, `GET /v1/flows/image` | `flows.image.get`, `flows.image.list` | the generation after asking ElevenLabs how it stands, and the project's generations, newest first |

`output_format`, `enable_logging` and `optimize_streaming_latency` pass to ElevenLabs as written. The request body of an upload is bounded by [`body_limits.audio`](../self-hosting/configuration.md#request-and-response-bodies). A voice id is letters and digits, as ElevenLabs lists them; any other is refused with 400 before anything is sent.

## Image generations

An ElevenLabs app makes images with the image models ElevenLabs serves, such as `gpt-image-1.5`, `gemini-3-pro-image` or `bytedance-seedream-5-pro`, through `flows.image`. Kaeso starts the generation at ElevenLabs with the project's ElevenLabs key, keeps it under an id of its own, such as `imggen_01k7a3f8m2xq4v9c5r6t8w0yz1`, and answers `{"id": …, "status": "pending"}` at once. The app then asks `GET /v1/flows/image/{generation_id}` until the status is `completed`, with the image's `content_url`, or `failed`, with its `failure_reason`; ElevenLabs asks callers to wait two seconds between calls.

```bash
curl https://kaeso.example.com/v1/flows/image \
  -H "xi-api-key: $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model_id": "gpt-image-1.5", "prompt": "A lighthouse at dusk", "aspect_ratio": "3:2"}'
```

- **Ids.** A generation belongs to the project whose key started it, and every key of the project that may use its model sees it. A key of another project gets 404, as for an id that doesn't exist.
- **Status.** Each `GET` of a generation that hasn't failed asks ElevenLabs how it stands and counts against the key's request rates. A completed generation gets a fresh `content_url` each time, since ElevenLabs keeps a link valid for about an hour.
- **Listing.** `GET /v1/flows/image` lists the project's generations newest first, as Kaeso last saw them, without asking ElevenLabs, so a link in the list may have run out. It takes `page_size` (1 to 100, 30 by default), `cursor` (the `next_cursor` of the page before), `status` and `model_id`, the model name the app sent.
- **Models.** Kaeso serves `flows.image` with the project's ElevenLabs image models, by name or through an alias: other providers answer with the image itself, which leaves no generation to ask about later. A model of another provider is refused with 400 and `param: model_id` before anything is sent; call it through the [Images API](images.md).
- **What is refused.** `webhook` is refused with 400, because the webhook would carry ElevenLabs' own id for the generation, which Kaeso doesn't give out. A field the model doesn't take, such as `aspect_ratio` `16:9` on `gpt-image-1.5`, is refused before sending, with the field in `param`.
- **Key.** A generation stays with the ElevenLabs key that started it. Once the project uses another ElevenLabs key, asking about the generation gets 409.
- **Costs and logs.** The [request log](../console/request-logs.md) shows the start as `flows/image`, which costs nothing, and the finished image as `flows/image/completed`, which carries the cost, once however many requests see it end. ElevenLabs bills images in credits, which KPMR doesn't list, so their cost stays unknown. An uploaded image is logged as its media type and size unless the organization and the project [keep media](../console/request-logs.md#media-in-requests-and-replies).

## Realtime sessions

Kaeso serves ElevenLabs' realtime WebSockets: text to speech input streaming (`/v1/text-to-speech/{voice_id}/stream-input`) and its multi-context form (`/multi-stream-input`), the text to dialogue sockets (`/v1/text-to-dialogue/stream-input` and `/multi-stream-input`), and realtime speech to text (`/v1/speech-to-text/realtime`). The SDKs' realtime clients work against Kaeso as they are, such as text to speech from text that arrives in pieces:

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

- **Key.** Send the Kaeso key in `xi-api-key` or `Authorization: Bearer`, or as `xi_api_key` in the first message of a speech socket, as ElevenLabs takes it. Kaeso takes it out of the message and opens ElevenLabs' socket with the project's ElevenLabs key. Single-use tokens (`token`, `single_use_token`) are ElevenLabs' own, so Kaeso refuses them with 401.
- **Model.** Name the model in `model_id`; a socket without one is refused with 400, because Kaeso prices and limits by model. Only the project's ElevenLabs models serve these sockets: no other provider takes ElevenLabs' messages, so an alias or another provider's model is refused.
- **Limits.** A session counts as one request against the key's request rates when it opens. While it runs, Kaeso checks the hard spend budgets every 30 seconds and closes the session when one runs out. A session ends after it is idle for `upstream.stream_idle_timeout`, after one hour, or when Kaeso restarts. A message may be up to 1 MiB.
- **Messages.** Messages pass between the app and ElevenLabs as they are; Kaeso reads them only to count what they carry and to take out keys. ElevenLabs' own messages and close codes reach the app unchanged.
- **Errors.** What Kaeso refuses before the socket opens comes back over HTTP in ElevenLabs' error shape. Later, Kaeso sends the socket's own error message, `{"message_type": "auth_error", "error": …}` on speech to text and `{"message", "error", "code", "param"}` on the speech sockets, then closes with 1008 for what the app can fix and 1011 otherwise.
- **Logs and costs.** The [request log](../console/request-logs.md) shows a session as one request, such as `text-to-speech/stream-input`, with the characters it sent to speak or the length of the audio it sent to transcribe, never the audio. Speech is billed per character, and `scribe_v2_realtime` at KPMR's price per hour of audio, measured from the session's `audio_format` (`pcm_*` or `ulaw_8000`).

## Other providers

An ElevenLabs app's request is a Kaeso request: an alias or a model of another provider serves it when it can. Text to speech reaches OpenAI, Google and every provider of the [Audio API](audio.md#providers) with the voice id as the voice; speech to text reaches every transcription provider, timed words included where the provider times them; sound effects also reach QwenCloud's Qwen-Audio-TTS-Next, without `duration_seconds`, which it has no field for ([QwenCloud mapping](../development/kpp/dashscope.md#audio-generation)). What only ElevenLabs takes, such as `voice_settings` beyond `speed`, pronunciation dictionaries or `keyterms`, is refused for other providers with the field in `param`.

## What Kaeso refuses

Account, voice cloning and design, dubbing, studio projects, history, pronunciation dictionaries, workspaces, agents, single-use tokens and the other management endpoints stay at ElevenLabs. Kaeso answers them with 404 in ElevenLabs' error shape, `{"detail": {"type": "not_found", "message": "Kaeso does not serve `/v1/dubbing`; manage voices, dubbing and your account at ElevenLabs."}}`. `POST /v1/music/detailed` and `/v1/music/plan` are refused too.

Agents (`/v1/convai/…`) are not served: an agent runs on ElevenLabs' own platform with its own language model, tools and knowledge base, which Kaeso doesn't route. Call them at ElevenLabs.

## Costs and logs

ElevenLabs bills speech per character, at KPMR's price per million characters of each model; Kaeso logs the characters ElevenLabs reports in its `character-cost` header. The voice changer is billed per minute of the audio it hears, at KPMR's speech-to-speech prices; ElevenLabs reports no length, so Kaeso reads it from the uploaded file's header without decoding the audio, and the cost is marked as measured ([audio](audio.md#costs-and-logs)); a file that states no length leaves it unknown. Transcripts are billed by the hour of audio, sound effects and music by the minute of their length. The [request log](../console/request-logs.md) names the endpoint, such as `text-to-speech` or `speech-to-text`, and never stores audio.
