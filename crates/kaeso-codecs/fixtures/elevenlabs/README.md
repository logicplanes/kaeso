# Fixtures: `elevenlabs`

ElevenLabs' API, as ElevenLabs-format apps send it and as ElevenLabs answers. Each request's `<name>.meta.json` names its path and query (and an upload's content type); each binary reply's `.meta.json` its content type and the `character-cost` header.

## Sources

- ElevenLabs' OpenAPI document, https://api.elevenlabs.io/openapi.json (read 2026-11-08), and its errors page https://elevenlabs.io/docs/eleven-api/resources/errors.
- **Recorded live** on 2026-11-08 at 17:58 UTC with a free-tier key, once per endpoint, about 40 credits in all (the sum of the `character-cost` headers): voice `hpp4J3VqNfWAUOO0d1Us` (Bella, premade), `eleven_flash_v2_5` for speech, the default model for sound effects, dialogue and the voice changer, `scribe_v2` for speech to text, and a 1-second test MP3.

## Files

| file | what it covers | provenance |
|---|---|---|
| `requests/speech*.json` | text to speech, plain, `/stream`, `/with-timestamps` and `/stream/with-timestamps` | recorded (the bodies sent) |
| `requests/speech-every-field.json` | voice settings, `language_code`, `seed`, `previous_text`, `apply_text_normalization`, and the query's `enable_logging` and `optimize_streaming_latency` | schema: `Body_text_to_speech_v1_text_to_speech__voice_id__post` |
| `requests/sound.json`, `dialogue.json` | sound effects with `duration_seconds`, two dialogue lines | recorded |
| `requests/music.json` | music with `music_length_ms` | recorded; ElevenLabs refused it for the free tier |
| `requests/transcribe.multipart` | speech to text with `scribe_v2` | recorded (the fields sent; the form is rebuilt with a fixed boundary) |
| `requests/transcribe-every-field.multipart` | language, diarization, speakers, character timestamps, `tag_audio_events`, `temperature` | schema: `Body_speech_to_text_v1_speech_to_text_post` |
| `requests/voice-change.multipart` | the voice changer's upload | recorded (rebuilt with a fixed boundary) |
| `responses/recorded-speech.mp3`, `recorded-speech-stream.pcm` | the audio of the plain and streamed calls | recorded |
| `responses/recorded-speech-with-timestamps.json` | `audio_base64` with `alignment` and `normalized_alignment` | recorded |
| `responses/recorded-transcribe.json` | `scribe_v2`'s transcript with words, spacing and `audio_duration_secs` | recorded |
| `responses/recorded-sound.mp3`, `recorded-dialogue.mp3`, `recorded-voice-change.mp3` | the audio of sound effects, dialogue and the voice changer | recorded |
| `streams/recorded-speech-stream-with-timestamps.ndjson` | the lines of `/stream/with-timestamps` | recorded |
| `errors/recorded-voice-not-found-404.json` | an unknown voice id | recorded |
| `errors/recorded-music-free-tier-402.json` | music refused on the free tier | recorded |
| `models/recorded-list.json` | the first two entries of `GET /v1/models` | recorded |
| `voices/recorded-search.json` | `GET /v2/voices?page_size=3`, cut to its two premade voices with the account-bound fields emptied | recorded |
| `realtime/text-to-speech-first.json` | the first message of `/stream-input` with a key in `xi_api_key` | spec-built: the official Python SDK's `convert_realtime` first message, with the key field the reference documents |
| `realtime/text-to-speech-text.json` | a text message of `/stream-input` | spec-built: the SDK's `convert_realtime` text message |
| `realtime/text-to-speech-contexts.json` | a text message of `/multi-stream-input` | spec-built: the SDK's `websocket_tts_client_message_multi` type |
| `realtime/text-to-dialogue-inputs.json` | two dialogue lines | spec-built: the text to dialogue WebSocket reference (`inputs`, `voice_id`, `new_turn`) |
| `realtime/speech-to-text-chunk.json` | an audio chunk of realtime speech to text | spec-built: the SDK's `RealtimeConnection.send` message, with 33 bytes of silence |
