---
title: DashScope
description: Wie die Bilder, Sprachausgaben, Transkriptionen und Videojobs von KPP auf die eigene DashScope-API von QwenCloud abgebildet werden.
order: 8
---

Schlagen Sie hier nach, wie eine Anfrage die Modelle von QwenCloud auf DashScope erreicht: welchen Body ein Modell nimmt, welches Feld wohin gehört, wie eine Antwort zurückkommt und woher jede Abbildung stammt.

- **Codec:** `kaeso_codecs::dashscope::DashScope` (`crates/kaeso-codecs/src/dashscope/`)
- **Pfade:** ab der Wurzel des Hosts von QwenCloud, mit `Authorization: Bearer`:
  - Video: `POST /api/v1/services/aigc/video-generation/video-synthesis`, `POST /api/v1/services/aigc/image2video/video-synthesis` (erstes und letztes Bild bei Wan 2.2 und 2.1), beide mit `X-DashScope-Async: enable`, und `GET /api/v1/tasks/{task_id}`;
  - Bilder, Qwen3-TTS und Qwen-Audio-ASR: `POST /api/v1/services/aigc/multimodal-generation/generation`, in derselben Anfrage beantwortet;
  - Qwen-Audio-TTS: `POST /api/v1/services/audio/tts/SpeechSynthesizer` mit `X-DashScope-SSE: enable`.
- **Fixtures:** `crates/kaeso-codecs/fixtures/dashscope/`, aus den Referenzen von QwenCloud gebaut, keine aufgezeichnet

**Quellen.** Die Referenzen von QwenCloud zu [Text zu Video bis Wan 2.6](https://docs.qwencloud.com/api-reference/video-generation/wan-text-to-video/create-task), [Bild zu Video aus dem ersten Bild](https://docs.qwencloud.com/api-reference/video-generation/wan-image-to-video-first-frame/create-task), [Text zu Video mit Wan 2.7](https://docs.qwencloud.com/api-reference/video-generation/wan27-text-to-video/create-task) und [seinem Abfrageergebnis](https://docs.qwencloud.com/api-reference/video-generation/wan27-text-to-video/query-result), [Bild zu Video mit Wan 2.7](https://docs.qwencloud.com/api-reference/video-generation/wan27-image-to-video/create-task), [Wan 3.0](https://docs.qwencloud.com/api-reference/video-generation/wan30-video/create-task), [asynchronen Aufgaben](https://docs.qwencloud.com/api-reference/more/manage-asynchronous-tasks), [Qwen-Image](https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image) und [seiner Bearbeitung](https://docs.qwencloud.com/api-reference/image-generation/qwen-image-editing), [Wan 2.7 Image](https://docs.qwencloud.com/api-reference/image-generation/wan27-image-gen-edit/synchronous), [Wan 2.6 Image](https://docs.qwencloud.com/api-reference/image-generation/wan26-image-gen-edit/synchronous), [Z-Image](https://docs.qwencloud.com/api-reference/image-generation/z-image), [Qwen-Audio-TTS](https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-audio-tts/http-api) und [seinen Stimmen](https://docs.qwencloud.com/developer-guides/speech/voice-list/qwen-audio-tts), [Qwen3-TTS](https://docs.qwencloud.com/api-reference/speech-synthesis/qwen-tts) und [seinen Stimmen](https://docs.qwencloud.com/developer-guides/speech/voice-list/qwen-tts) sowie [Qwen-Audio-ASR](https://docs.qwencloud.com/api-reference/speech-recognition/qwen-audio-asr/http-api), gelesen am 2026-11-09.

## Entscheidungen für Videos

- **Das Modell wählt den Body.** `dashscope::family` liest den Modellnamen: `wan3.0-video*`, `wan2.7-t2v*`, `wan2.7-i2v*`, `*-kf2v-*` (erstes und letztes Bild), `*-t2v*` und `*-i2v*` (Wan 2.6 und älter). Modelle, die Video bearbeiten oder referenzieren, etwa `wan2.7-r2v`, `wan2.7-videoedit`, `wan2.1-vace-plus` und `wan2.2-animate-*`, werden mit `param` `model` abgelehnt.
- **Größen nur, wo QwenCloud sie listet.** Wan 2.6 und älter nehmen eine Pixelgröße `size` als `width*height`. Text zu Video mit Wan 2.7 nimmt eine Stufe und ein Verhältnis, deshalb geht eine `size` nur durch, wenn sie eine der zehn Größen seiner Referenz ist, so wie `1104x832` `720P` bei `4:3` ist. Bild zu Video nimmt nur eine Stufe, weil das Bild das Seitenverhältnis bestimmt, deshalb wird eine `size` dort abgelehnt.
- **Kein Remix, kein Löschen.** DashScope bricht nur Aufgaben ab, die noch nicht begonnen haben, und das ist kein Löschen; deshalb vergisst Kaeso ein gelöschtes Video.
- **Zeiten sind Pekinger Zeit.** `submit_time` und `end_time` sind UTC+8 ohne Offset; `dashscope::unix_of_beijing` liest sie. Der Link des Videos gilt 24 Stunden, deshalb läuft ein fertiger Job 24 Stunden nach `end_time` ab.

## Videoanfrage

| KPP | Wan 2.6 und älter | Wan 2.7 | Wan 3.0 |
| --- | --- | --- | --- |
| `model`, `prompt` | `model`, `input.prompt` | ebenso | ebenso |
| `negative_prompt` | `input.negative_prompt` | `input.negative_prompt` | abgelehnt |
| Bild `FirstFrame` | `input.img_url`; erstes und letztes Bild bei Wan 2.2 und 2.1: `input.first_frame_url` | `input.media[]` `{type: "first_frame", url}` | ebenso |
| Bild `LastFrame` | `input.last_frame_url`, nur Weblinks | `input.media[]` `{type: "last_frame", url}` | ebenso |
| Bilder `Reference` | abgelehnt | abgelehnt | `input.media[]` `{type: "reference_image", url}`, nie zusammen mit Bildern für Anfang oder Ende |
| `size` | `parameters.size`, `1280*720`, auch aus einer Auflösung und einem Seitenverhältnis (Text zu Video) | `parameters.resolution` und `ratio`, aus den gelisteten Größen (Text zu Video) | abgelehnt |
| `resolution` | `parameters.resolution` (Bild zu Video), in Großbuchstaben | `parameters.resolution` | `parameters.resolution` |
| `aspect_ratio` | abgelehnt | `parameters.ratio` (Text zu Video) | `parameters.ratio` |
| `seconds`, `seed` | `parameters.duration`, `parameters.seed` | ebenso | ebenso |
| `audio` | `parameters.audio`, nur `wan2.6-i2v-flash` | abgelehnt | `parameters.audio` |
| andere Felder von `input` und `parameters`, etwa `audio_url`, `prompt_extend`, `watermark` | Extras | Extras | Extras |

Bilder gehen als Links oder Data-URLs, außer erstem und letztem Bild bei Wan 2.2 und 2.1, die QwenCloud nur als öffentliche Links nimmt. Medien anderer Typen, etwa `driving_audio` oder `first_clip`, werden beim Dekodieren einer Anfrage abgelehnt.

## Videoantwort

| DashScope | KPP |
| --- | --- |
| `output.task_id` | `VideoJob.id` |
| `output.task_status` `PENDING`, `RUNNING`, `SUCCEEDED` | `queued`, `in_progress`, `completed` |
| `output.task_status` `FAILED`, `CANCELED`, `UNKNOWN` | `failed`, mit `output.code` und `output.message`, oder dem Status als Code |
| `output.video_url` | der Link des Videos, `video/mp4` |
| `output.submit_time`, `output.end_time` | `created_at`, `completed_at` |
| `usage.output_video_duration`, `usage.SR`, `usage.ratio` | `seconds`, `resolution`, `aspect_ratio` |
| `request_id`, andere Felder von `output` und `usage` | Extras |

## Bilder

`dashscope::images::image_family` liest den Modellnamen: `qwen-image-*` (Qwen-Image 2.x und 3.0), `wan2.7-image*`, `wan2.6-image*`, `z-image*`, `wan2.6-t2i*` und die älteren Text-zu-Bild-Modelle `wan*-t2i*`. Andere Namen werden mit `param` `model` abgelehnt. Bilder und Prompt sind eine Nachricht des Nutzers, die Bilder zuerst; die älteren Text-zu-Bild-Modelle nehmen stattdessen `input.prompt`.

| KPP | Qwen-Image | Wan 2.7 Image | Wan 2.6 Image | Z-Image |
| --- | --- | --- | --- | --- |
| `prompt` | Teil `{"text"}` | ebenso | ebenso | ebenso |
| `images` | Teile `{"image"}`, Links oder Data-URLs; 1 bis 10 bei 2.1, sonst 1 bis 3 | 0 bis 9 | 1 bis 4, erforderlich | abgelehnt |
| `count` | `parameters.n`, höchstens 6 | `parameters.n` | `parameters.n`, höchstens 4; `1`, wenn eine App es weglässt, weil das Modell sonst 4 macht | nicht gesendet; über 1 abgelehnt |
| `size` | `parameters.size` als `width*height`; `auto` lässt es weg | ebenso | ebenso | ebenso |
| `resolution` | abgelehnt | `parameters.size` `1K`, `2K` oder `4K` | `1K` oder `2K` | abgelehnt |
| `negative_prompt` | `parameters.negative_prompt`, bei 2.1 abgelehnt | abgelehnt | `parameters.negative_prompt` | abgelehnt |
| `seed` | `parameters.seed` | ebenso | ebenso | ebenso |
| `user` | weggelassen, in `kaeso-left-out` genannt | ebenso | ebenso | ebenso |
| `mask`, `quality`, `output_format`, `output_compression`, `background`, `aspect_ratio`, Datei-IDs | abgelehnt | abgelehnt | abgelehnt | abgelehnt |

- **Nur Links.** DashScope antwortet mit Links, die 24 Stunden gelten, deshalb lehnt der Codec `b64_json` ab; das Gateway nimmt es aus der Anfrage und lädt die Bilder für eine App herunter, die Bytes verlangt ([Bilder-API](../../api/images.md#anbieter)).
- **Erweiterungen gehen mit.** `prompt_extend`, `prompt_extend_mode`, `enable_thinking` und `watermark` einer App im Format von OpenAI gehen nach `parameters`, so wie der OpenAI-kompatible Modus von QwenCloud sie dokumentiert. Jedes andere fremde Feld wird abgelehnt.
- **Standardmäßig kein Wasserzeichen.** Jedes dieser Modelle lässt das Wasserzeichen weg, solange `watermark` nicht `true` ist; Kaeso sendet deshalb nichts.
- **Ganze Antworten.** Die Bilder kommen in einer Antwort; eine gestreamte Anfrage bekommt sie als fertige Events.
- **Text-zu-Bild-Tasks.** Die Text-zu-Bild-Modelle bis Wan 2.5 (`wan2.5-t2i-preview`, `wan2.2-t2i-*`, `wan2.1-t2i-*`) antworten nur als Tasks: Die Anfrage geht an `/api/v1/services/aigc/text2image/image-synthesis` mit `X-DashScope-Async: enable`, der Prompt in `input.prompt` und der negative Prompt in `input.negative_prompt`. `dispatch::pending_task` nennt den Task, solange er läuft, und das Gateway fragt `GET /api/v1/tasks/{task_id}`, bis er endet. `wan2.6-t2i` antwortet in derselben Anfrage. Beide nehmen nur einen Prompt, `n` bis 4 (`1`, wenn eine App es weglässt) und eine `size` in Pixeln.

| Antwort von DashScope | KPP |
| --- | --- |
| `output.choices[].message.content[].image` | die Bilder, in ihrer Reihenfolge |
| ein Teil `text` neben den Bildern (Z-Image) | `revised_prompt` jedes Bildes |
| `usage.output_width` und `output_height`, `width` und `height` oder `size` | `settings.size` |
| `usage.input_tokens`, `output_tokens` | `usage`, das ganze `usage` bleibt erhalten |
| `request_id`, `output.finished`, `output.rewrite_status` und die übrigen Felder der Choices | Extras |
| `output.results[].url` eines Tasks | die Bilder, in ihrer Reihenfolge; ein Ergebnis mit `code` und `message` ist allein gescheitert und ergibt kein Bild |
| `output.results[].actual_prompt` | `revised_prompt` jedes Bildes |
| `output.task_status` `FAILED` mit `output.code` und `output.message` | ein Fehler nach seinem Code |

## Sprachausgabe

`dashscope::speech::speech_family` liest den Modellnamen: `qwen-audio-*-tts-next` (Qwen-Audio-TTS-Next, Audio-Generierung), `qwen-audio-*-tts-*` (Qwen-Audio-TTS 3.0 und 3.1) und `qwen3-tts-flash*`. Echtzeitmodelle nehmen nur WebSocket-Verbindungen und werden abgelehnt.

| KPP | Qwen-Audio-TTS | Qwen3-TTS Flash |
| --- | --- | --- |
| `input`, `voice` | `input.text`, `input.voice` | ebenso |
| `format` | `input.format`: `mp3` (Standard), `wav`, `opus`, `pcm16` als `pcm` mit 24 kHz | `wav` (Standard) oder `pcm16`; andere abgelehnt |
| `sample_rate` | `input.sample_rate` | nur 24.000 |
| `speed` | `input.rate`, 0,5 bis 2,0 | abgelehnt |
| `instructions` | `input.instruction` | abgelehnt |
| `language` | `input.language_hints` `[language]` | `input.language_type`, `Chinese` für `zh` und neun weitere |
| `seed` | `input.seed` | abgelehnt |
| `bitrate` | `input.bit_rate`, nur mit `opus` | abgelehnt |
| `timestamps`, `duration`, Dialog, Quellaudio | abgelehnt | abgelehnt |

- **Gestreamt, wann immer das Format es erlaubt.** Qwen-Audio-TTS bekommt immer `X-DashScope-SSE: enable` und sendet Base64-Stücke im gewünschten Format. Qwen3-TTS streamt nur PCM mit 24 kHz, deshalb wird `pcm16` gestreamt und `wav` als Link zur Datei ganz beantwortet.
- **Stimmen aus den Listen.** Eine benannte Stimme muss eine Systemstimme des Modells sein oder eine seiner Basisstimmen namens `<model>-<suffix>`. Eine ausdrückliche Stimm-ID, etwa einer geklonten Stimme, geht unverändert.
- **Keine Umwandlung.** Ein Format, das das Modell nicht erzeugt, wird vor dem Senden abgelehnt.

| Antwort von DashScope | KPP |
| --- | --- |
| `output.audio.data` eines Events | ein Stück des Audios, mit dem gewünschten Format gekennzeichnet |
| das Event, dessen `output.finish_reason` `stop` ist | das Ende, mit `usage` und `usage.characters` als abgerechneten Zeichen |
| `output.audio.url` einer ganzen Antwort | das Audio als Link |
| ein Event `error` oder eine Zeile `:HTTP_STATUS/` außerhalb von 2xx | der Fehler |


### Audio-Generierung

Qwen-Audio-TTS-Next ([Referenz zur Audio-Generierung](https://docs.qwencloud.com/api-reference/audio-generation), gelesen am 2026-11-09) erzeugt Sprache, Soundeffekte und Umgebungsgeräusche aus einer beschriebenen Szene, auf dem Pfad von Qwen-Audio-TTS, und antwortet als Ganzes mit einem Link. Es nimmt keine Stimmen: Der Prompt beschreibt sie.

| KPP | Qwen-Audio-TTS-Next |
| --- | --- |
| `SoundEffect` oder `Speech` ohne Stimme, mit `input` | `input.text_prompt`; eine native Anfrage mit `text_prompt` wird als `SoundEffect` gelesen |
| `format` | `input.format`: `wav` (Standard), `mp3`, `pcm16` als `pcm`; andere werden abgelehnt |
| `sample_rate` | `input.sample_rate`: 8.000, 16.000, 24.000, 44.100 oder 48.000 |
| `speed`, `seed` | `input.rate` (0,5 bis 2,0), `input.seed` |
| `bitrate` | `input.bit_rate` mit `input.enable_cbr: true`, nur bei `mp3` |
| `voice`, `duration`, `timestamps`, `instructions`, `language` | abgelehnt: Das Modell hat kein Feld dafür |
| `Music`, `Dialogue`, `VoiceChange` | abgelehnt; QwenCloud bietet kein Musikmodell |
| `output.audio.url`, `output.audio.duration` | das Audio als Link, den Kaeso herunterlädt; die Länge bleibt in den Extras der Antwort |

Referenzclips (`input.references`) sowie `channels`, `volume`, `quality` und `enable_aigc_tag` sind eigene Felder von DashScope: Sie gehen zwischen DashScope-Apps und QwenCloud hin und her und werden aus jedem anderen Format abgelehnt. Die Antwort enthält keine Tokenzahlen, nach denen QwenCloud sie bepreist, deshalb bleiben ihre Kosten unbekannt.

## Transkription

Qwen-Audio-ASR Flash (`qwen-audio-3.0-asr-flash`, `qwen-audio-3.1-asr-flash`) antwortet in derselben Anfrage. Die Modelle für Dateitranskription beantworten nur asynchrone Aufgaben, die Streaming-, Echtzeit- und Message-Modelle nur WebSocket-Verbindungen; sie werden deshalb abgelehnt.

| KPP | Qwen-Audio-ASR |
| --- | --- |
| das Audio | `input.messages[].content[]` `{"type": "input_audio", "input_audio": {"data"}}`, ein Link oder eine Data-URL |
| sein Medientyp oder Dateiname | `parameters.format`: `wav`, `mp3` oder `opus`; anderes Audio abgelehnt |
| `prompt` | eine Nachricht des Nutzers mit einem Teil `input_text` vor dem Audio |
| `language` | `parameters.language_hints` `[language]` |
| `format` | `json` und `text`; die Formen mit Zeiten werden abgelehnt |
| `temperature`, `timestamp_granularities`, `diarize`, Sprecher, Übersetzungen | abgelehnt |

`output.text` der Antwort ist das Transkript und `usage.duration` die Sekunden, die QwenCloud abrechnet; der Satz und seine Wörter bleiben in den Extras.

## Fehler

Fehler sind `{"request_id", "code", "message"}`: `InvalidApiKey` ist ein Authentifizierungsfehler, `Throttling…` ein Ratenlimit und `DataInspection…` ein Inhaltsfilter; andere Codes folgen dem HTTP-Status.
