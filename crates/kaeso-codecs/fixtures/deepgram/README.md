# Fixtures: `deepgram`

Deepgram's pre-recorded speech-to-text API, `POST /v1/listen`, and its Aura text-to-speech API, `POST /v1/speak`. Every option is a query parameter, so each request fixture has a `.meta.json` with its `kind` (`transcription` or `speech`), its `query` and, for `/v1/listen`, the body's `content_type`. A speech reply's `.meta.json` holds its content type and the response headers Kaeso reads.

No successful reply is recorded: no Deepgram key was available. The errors and the model listing below were recorded live on 2026-11-14 without a key or with a made-up one, and the request ids in them are zeroed. The OpenAPI document is `deepgram/deepgram-api-specs` at commit `0ccc6f4680b3fe5b8f9002be6a78cd50e5b32116`.

| file | what it covers | provenance |
|---|---|---|
| `requests/listen-url.json` | `{"url"}` as JSON, `model=whisper` | doc: https://developers.deepgram.com/docs/deepgram-whisper-cloud, "Transcribe a Remote File" (read 2026-11-14) |
| `requests/listen-upload.wav` | the audio as the body (`audio/wav`), `detect_language=true` | doc: https://developers.deepgram.com/docs/language-detection, the cURL example (read 2026-11-14); the 44 bytes are a made-up empty WAV |
| `requests/speak.json` | `{"text"}`, the voice as the model, `encoding=linear16&container=wav` | doc: https://developers.deepgram.com/docs/tts-encoding, "Linear16 encoding" (read 2026-11-14) |
| `responses/listen-nova3-doc.json` | a Nova-3 transcript: metadata, `punctuated_word`, paragraphs | doc: https://developers.deepgram.com/docs/pre-recorded-audio, the reply example (read 2026-11-14), verbatim |
| `responses/listen-utterances-schema.json` | utterances with speakers, words with `speaker` and `speaker_confidence`, `detected_language` and `language_confidence` | built from the schema: the OpenAPI document's `ListenV1Response` and the examples of https://developers.deepgram.com/docs/utterances and https://developers.deepgram.com/docs/language-detection; the words, times and confidences are made up |
| `responses/speak.mp3` | the audio itself with `dg-char-count` | doc: https://developers.deepgram.com/docs/text-to-speech, "Example response headers" (read 2026-11-14); the ten audio bytes are made up |
| `errors/recorded-invalid-auth-401.json` | `INVALID_AUTH` | live: `POST /v1/listen?model=nova-3` with `Authorization: Token` and a made-up key; `/v1/speak`, the EU host, `Bearer` and a bare key answer the same |
| `errors/recorded-invalid-query-400.json` | `err_code: "Bad Request"`, a bad query refused before the key is read | live: `POST /v1/listen?model=nova-3&utt_split=abc` without a key |
| `errors/recorded-unsupported-audio-format-400.json` | `UNSUPPORTED_AUDIO_FORMAT` | live: `POST /v1/speak?model=aura-2-thalia-en&encoding=mp3&sample_rate=8000` on `api.eu.deepgram.com` with a made-up key (the errors page names this code `INVALID_QUERY_PARAMETER`) |
| `errors/recorded-model-uuid-404.txt` | a plain-text error | live: `GET /v1/models/aura-2-thalia-en` without a key (the path takes a model's `uuid`) |
| `errors/corrupt-audio-doc-400.json` | a file Deepgram can't read | doc: https://developers.deepgram.com/docs/errors, "Unknown request body format" (read 2026-11-14) |
| `errors/invalid-json-doc-400.json` | the `category`, `message`, `details` shape | doc: as above, "Invalid JSON submitted" |
| `errors/payment-required-doc-402.json` | `ASR_PAYMENT_REQUIRED` | doc: as above, "Insufficient credits" |
| `errors/model-access-doc-403.json` | `INSUFFICIENT_PERMISSIONS` | doc: as above, "Insufficient permissions" (403) |
| `errors/text-too-long-doc-413.json` | `err_code: "Payload Too Large"` | doc: as above, "Input text exceeded character limit", with the limit of 2000 filled in |
| `errors/too-many-requests-doc-429.json` | `TOO_MANY_REQUESTS` | doc: as above, "Rate limit exceeded" (speech to text) |
| `models/recorded-list.json` | `stt`, `tts` and `languages` | live: `GET /v1/models` without a key (it is public), trimmed to seven of 636 entries and four languages |

## Where Deepgram differs from OpenAI's audio APIs

| | OpenAI | Deepgram |
|---|---|---|
| Auth | `Authorization: Bearer` | `Authorization: Token` |
| Options | form fields or JSON | the query string |
| Transcription upload | `multipart/form-data` with `file` | the audio as the body with its media type, or `{"url"}` |
| Language | detected when left out | English when left out; `detect_language=true` detects it |
| Transcript forms | `json`, `text`, `srt`, `vtt`, `verbose_json`, `diarized_json` | JSON only; segments are `utterances=true`, speakers `diarize_model` |
| Speech voice | `voice` beside `model` | one model per voice, `aura-2-{voice}-{language}` |
| Speech format | `response_format` | `encoding`, `container`, `sample_rate`, `bit_rate` |
| Billed characters | none | `dg-char-count` |
| Errors | `{"error": {…}}` | `{"err_code", "err_msg"}`, `{"category", "message", "details"}` or plain text |
