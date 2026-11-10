---
title: Audio-API
description: Sprache erzeugen und Audio transkribieren oder übersetzen, über Kaeso mit der Audio-API von OpenAI, bei jedem Anbieter, der sie anbietet.
order: 3
---

Apps erzeugen Sprache und transkribieren Audio über Kaeso mit der Audio-API von OpenAI und den offiziellen SDKs von OpenAI. Kaeso sendet jede Anfrage in der eigenen Form des Anbieters an das Modell, und die Antwort kommt in der Form von OpenAI zurück: das Audio selbst, Ereignisse `speech.audio.*` oder ein Transkript in der Form, die die App verlangt hat.

```bash tab="curl"
curl https://kaeso.example.com/v1/audio/speech \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "gpt-4o-mini-tts", "input": "Your order shipped today.", "voice": "coral"}' \
  --output speech.mp3
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

with client.audio.speech.with_streaming_response.create(
    model="gpt-4o-mini-tts",
    voice="coral",
    input="Your order shipped today.",
) as response:
    response.stream_to_file("speech.mp3")

with open("meeting.mp3", "rb") as audio:
    transcript = client.audio.transcriptions.create(model="whisper-1", file=audio, response_format="srt")
print(transcript)
```

```ts tab="TypeScript"
import fs from "node:fs";
import OpenAI from "openai";

const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1", apiKey: process.env.KAESO_API_KEY });

const speech = await client.audio.speech.create({ model: "gpt-4o-mini-tts", voice: "coral", input: "Your order shipped today." });
fs.writeFileSync("speech.mp3", Buffer.from(await speech.arrayBuffer()));

const transcript = await client.audio.transcriptions.create({ model: "gpt-4o-transcribe", file: fs.createReadStream("meeting.mp3") });
console.log(transcript.text);
```

## Endpunkte

| Endpunkt | Body | Antwort |
| --- | --- | --- |
| `POST /v1/audio/speech` | JSON: `model`, `input`, `voice`, `instructions`, `response_format`, `speed`, `stream_format` | das Audio im verlangten Format, gesendet, während es entsteht; mit `stream_format: "sse"` Ereignisse `speech.audio.delta` und `speech.audio.done` mit der Nutzung |
| `POST /v1/audio/transcriptions` | ein Upload: `file`, `model`, `language`, `prompt`, `response_format`, `temperature`, `timestamp_granularities[]`, `stream` und für OpenAI `include[]`, `chunking_strategy` und die bekannten Sprecher | `json`, `text`, `srt`, `verbose_json`, `vtt` oder `diarized_json`; mit `stream: true` Ereignisse `transcript.text.*` |
| `POST /v1/audio/translations` | ein Upload: `file`, `model`, `prompt`, `response_format`, `temperature` | das englische Transkript, `json`, `text`, `srt`, `verbose_json` oder `vtt` |

Schlüssel, die Auflösung von Modellen mit Aliassen und Routen, Limits und Budgets, Anfrage-Logs und Fehler im Format von OpenAI funktionieren wie bei jeder anderen Modellanfrage. Den Body des Uploads begrenzt [`body_limits.audio`](../self-hosting/configuration.md#anfrage--und-antwortkörper), standardmäßig 26 MiB; OpenAI nimmt Dateien bis 25 MB.

## Sprache

- **Stimmen.** Eine Stimme geht nur dorthin, wo der Anbieter sie listet: die Stimmen von OpenAI an OpenAI, die 30 vorgefertigten Stimmen von Gemini an Modelle von Google, die Stimmen-IDs von SpaceX an SpaceX. Anbieter, deren Stimmen vom Modell abhängen (Groq, Together AI, Mistral, OpenRouter, MiniMax), nehmen den Namen, wie er ist. QwenCloud nimmt eine Stimme, die seine Listen für das Modell nennen, etwa `Cherry` für `qwen3-tts-flash`, oder eine Stimm-ID, etwa die einer geklonten Stimme. ElevenLabs nimmt eine Stimme über ihre ID, wie sein `GET /v1/voices` sie listet: Senden Sie die ID als `voice` oder als `{"id": "…"}`; ein Name wie `alloy` wird abgelehnt, denn Kaeso rät nicht, welche Stimme von ElevenLabs er meint.
- **Formate** werden nie umgewandelt. Ein Format, das der Anbieter nicht hat, wird abgelehnt, bevor etwas gesendet wird, mit `response_format` in `param`. Nennt die App kein Format, antwortet OpenAI mit mp3, deshalb verlangt Kaeso von jedem Anbieter ausdrücklich mp3; `qwen3-tts-flash` erzeugt nur wav und pcm, deshalb nennt eine App eines davon.
- **Streaming.** Das Audio geht an die App, wie der Anbieter es sendet. Wird `stream_format: "sse"` verlangt, schreibt Kaeso Ereignisse `speech.audio.delta` von jedem Anbieter; ein Anbieter, der sein Audio ganz sendet, ergibt ein Delta und das Ende. OpenAI meldet die Nutzung nur in seinen Ereignissen, deshalb verlangt Kaeso von OpenAI Ereignisse und macht daraus wieder Audio für eine App, die das Audio will; `tts-1` und `tts-1-hd` haben keine Ereignisse und werden pro Zeichen abgerechnet. Audio antwortet mit seinem Status erst, wenn die ersten Bytes des Anbieters da sind: Scheitert der Anbieter davor, bekommt die App einen Fehler im Format von OpenAI, und bricht er später ab, endet die Antwort mit einem Fehler, nie als sauber beendete, zu kurze Datei.
- **Einstellungen**, die ein Anbieter nicht hat, etwa `instructions` bei Gemini oder `speed` bei Together AI, werden mit dem Feld in `param` abgelehnt.
- **QwenCloud** streamt seine Sprachausgabe, während es sie erzeugt, und Kaeso verlangt immer das. Eine ganze Antwort von `qwen3-tts-flash` in wav ist ein Link, den Kaeso so herunterlädt wie [die Bilder von QwenCloud](images.md#anbieter). Seine Echtzeitmodelle nehmen nur WebSocket-Verbindungen und werden abgelehnt.

## Transkription und Übersetzung

- **Jede Form.** Hat der Anbieter die Form der App nicht, verlangt Kaeso zeitlich markierte Segmente und schreibt die Untertitel oder Segmente selbst: `srt` und `vtt` aus `verbose_json` bei Groq, aus den Segmenten von Mistral, aus den zeitlich markierten Wörtern von Gemini und SpaceX und aus den Wörtern von ElevenLabs. Eine Form, die keine Zeitangabe eines Anbieters ergeben kann, etwa `diarized_json` ohne Sprecher, wird abgelehnt.
- **Links.** Der Upload einer App geht als Upload. Einen Link, den eine App in `url` oder `file_url` sendet oder eine ElevenLabs-App in `source_url`, bekommen die Anbieter, die Links nehmen, als Link (`url` bei Groq und SpaceX, `file` bei Together AI, `file_url` bei Mistral, `source_url` bei OpenRouter und ElevenLabs); OpenAI nimmt keine.
- **Streaming.** OpenAI, Mistral, Z.ai und MiniMax streamen ihre Transkripte; Kaeso schreibt sie als `transcript.text.delta` und `transcript.text.done`. Andere Anbieter antworten ganz, und die App bekommt ein Delta und das Ende.
- **Übersetzungen** gehen nur an Anbieter mit einem Endpunkt für Übersetzungen: OpenAI, Groq und Together AI.
- **Die Modelle für Dateitranskription von QwenCloud** (`*-filetrans`) beantworten nur asynchrone Aufgaben, seine Streaming- und Echtzeitmodelle nur WebSocket-Verbindungen; sie werden deshalb abgelehnt.

## Anbieter

| Anbieter | Sprache | Transkription und Übersetzung | Quelle |
| --- | --- | --- | --- |
| OpenAI | `/v1/audio/speech`: `gpt-4o-mini-tts`, `tts-1`, `tts-1-hd` | `/v1/audio/transcriptions` und `/translations`: `gpt-transcribe`, `gpt-4o-transcribe`, `gpt-4o-mini-transcribe`, `gpt-4o-transcribe-diarize`, `whisper-1` | [text to speech](https://developers.openai.com/api/docs/guides/text-to-speech), [speech to text](https://developers.openai.com/api/docs/guides/speech-to-text) |
| Google AI Studio | `generateContent` mit `responseModalities: ["AUDIO"]`: `gemini-3.8-flash-tts`, `gemini-3.8-flash-lite-tts` | `generateContent` mit `audioTranscriptionConfig`: `gemini-3.5-transcribe` | [speech generation](https://aistudio.google.com/docs/speech-generation), [audio transcription](https://aistudio.google.com/docs/transcribe) |
| Gemini Enterprise Agent Platform | `generateContent` mit `AUDIO`: `gemini-3.8-flash-tts`, `gemini-3.8-flash-lite-tts`, `gemini-2.5-*-tts` | `generateContent` mit `audioTranscriptionConfig` | [Gemini-TTS](https://docs.cloud.google.com/text-to-speech/docs/gemini-tts) |
| Groq | Orpheus: `canopylabs/orpheus-v1-english`, `canopylabs/orpheus-arabic-saudi`, nur wav | `whisper-large-v3`, `whisper-large-v3-turbo`; Übersetzungen | [text to speech](https://console.groq.com/docs/text-to-speech/orpheus), [speech to text](https://console.groq.com/docs/speech-to-text) |
| Together AI | Orpheus, Cartesia Sonic, Kokoro; mp3 oder wav | Whisper, Parakeet, Nemotron ASR; Übersetzungen | [text to speech](https://docs.together.ai/docs/inference/text-to-speech/overview), [transcription](https://docs.together.ai/docs/inference/transcription/overview) |
| Mistral | Voxtral TTS (`voxtral-mini-tts-3-0`) | Voxtral Transcribe 3 | [audio](https://docs.mistral.ai/studio/audio/migrate-from-voxtral-2) |
| OpenRouter | `/api/v1/audio/speech`, mp3 oder pcm | `/api/v1/audio/transcriptions` | [TTS](https://openrouter.ai/docs/guides/overview/multimodal/tts), [STT](https://openrouter.ai/docs/guides/overview/multimodal/stt) |
| Vercel | `/v4/ai/speech-model`, mp3 | `/v4/ai/transcription-model` | [text to speech](https://vercel.com/docs/ai-gateway/modalities/text-to-speech), [speech to text](https://vercel.com/docs/ai-gateway/modalities/speech-to-text) |
| SpaceX | `/v1/tts` | `/v1/stt`: `grok-voice-transcribe-2.0` | [voice](https://docs.x.ai/developers/rest-api-reference/inference/voice) |
| MiniMax | `/v1/t2a_v2`: `speech-2.8-*`, `speech-2.6-*`, `speech-02-*` | `/v1/speech_to_text`: `asr-1.0` | [T2A](https://platform.minimax.io/docs/api-reference/speech-t2a-http), [speech to text](https://platform.minimax.io/docs/api-reference/speech-to-text) |
| ElevenLabs | Text to Speech: `eleven_v4`, `eleven_v3`, `eleven_multilingual_v2`, `eleven_flash_v2_5` und die übrigen | Speech to Text: `scribe_v2`, `scribe_v2_medical` | [ElevenLabs-API](elevenlabs.md) |
| Z.ai | nicht angeboten | `/api/paas/v4/audio/transcriptions`: `glm-asr-2512` | [GLM-ASR](https://docs.z.ai/guides/audio/glm-asr-2512) |
| QwenCloud | DashScope: `qwen-audio-3.1-tts-flash`, `qwen-audio-3.1-tts-plus`, `qwen-audio-3.0-tts-flash`, `qwen-audio-3.0-tts-plus` (mp3, wav, opus, pcm), `qwen3-tts-flash` (wav oder pcm) | `qwen3-asr-flash` über `chat/completions`; DashScope: `qwen-audio-3.1-asr-flash`, `qwen-audio-3.0-asr-flash`, `json` und `text` | [Qwen-ASR](https://www.alibabacloud.com/help/en/model-studio/qwen-asr-api-reference), [Qwen-Audio-TTS](https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-audio-tts/http-api), [Qwen3-TTS](https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-tts), [Qwen-Audio-ASR](https://docs.qwencloud.com/api-reference/speech-recognition/qwen-audio-asr/http-api) |
| Cohere | nicht angeboten | `/v2/audio/transcriptions`: `cohere-transcribe-03-2026`, `language` verlangt | [transcribe](https://docs.cohere.com/v2/docs/transcribe) |
| Anthropic, DeepSeek, Moonshot, Cerebras, Ollama Cloud | nicht angeboten | nicht angeboten | [Unterstützte Anbieter](../concepts/supported-providers.md) |

Eine Anfrage an einen Anbieter ohne die API wird abgelehnt, bevor etwas gesendet wird, zum Beispiel „Anthropic offers no speech API Kaeso can call, so `claude-haiku-4-5` can't speak.“.

## Kosten und Logs

- **Kosten.** Sprache wird pro Zeichen abgerechnet (`tts-1`, Groq, Together AI, Mistral, MiniMax, ElevenLabs, QwenCloud) oder nach Token zum Preis für Audioausgabe des Modells (`gpt-4o-mini-tts`, Gemini). Transkripte werden nach Minuten oder Stunden Audio abgerechnet (Whisper, Scribe, Voxtral, Groq mit seinem Minimum von 10 Sekunden) oder pro Sekunde (QwenCloud) oder nach Token zum Preis für Audioeingabe. Die Preise stammen aus den Preisen pro Einheit und den Audio-Token-Preisen von KPMR; MiniMax, ElevenLabs und QwenCloud melden die abgerechneten Zeichen, die Kaeso verwendet.
- **Die Länge des Audios**, nach der ein Transkript oder ein Voice Changer abgerechnet wird, kommt in dieser Reihenfolge: aus dem, was der Anbieter meldet (`usage.seconds` und `duration` von OpenAI, `prompt_audio_seconds` von Mistral, `audio_duration_secs` von ElevenLabs, `usage.duration` von QwenCloud, `duration` eines ausführlichen Transkripts); sonst aus der Länge, die die Datei in ihren Headern angibt und die Kaeso liest, ohne das Audio zu dekodieren (WAV, FLAC, MP3, Ogg Opus und Vorbis, MP4 und M4A, WebM und Matroska); sonst gibt es keine, und die Kosten sind **Unbekannt**. Formate wie `text`, `srt` und `vtt` enthalten keine Nutzung, und ElevenLabs meldet für seinen Voice Changer keine Länge, deshalb kommen ihre Kosten aus der Datei. Das Anfrage-Log kennzeichnet solche Kosten als von Kaeso gemessen (`cost_basis: measured`). Eine Datei wird nie abgelehnt, weil ihre Länge nicht lesbar ist; rohes PCM und im Browser aufgenommene Dateien ohne angegebene Länge lassen sie unbekannt.
- **Logs.** Das [Anfrage-Log](../console/request-logs.md) zeigt den Endpunkt, die Zeichen der Sprache oder die Länge des transkribierten Audios und das Format. Solange Organisation und Projekt keine [Medien aufbewahren](../console/request-logs.md#medien-in-anfragen-und-antworten), wird ein Upload als seine Felder geloggt und Audio als Medientyp und Größe. Bei eingeschalteter Inhaltsprotokollierung gehören der Text eines Transkripts und eine Sprachantwort zum geloggten Inhalt.

## Nächste Schritte

- [Playground](../console/playground.md): Ein Sprach- oder Transkriptionsmodell in der Konsole ausprobieren, bevor Sie Code schreiben.
- [Anfrage-Logs](../console/request-logs.md): Eine Audioanfrage finden und sehen, was sie gekostet hat.
- [Unterstützte Anbieter](../concepts/supported-providers.md): Sehen, welche Anbieter Kaeso aufruft und wie Sie ihre Schlüssel hinzufügen.
