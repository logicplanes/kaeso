# Fixtures: `openai_speech`

Wire bodies for `POST /v1/audio/speech` and the providers that speak a form of OpenAI's speech API.

## Sources

- OpenAI OpenAPI spec at commit `926a7b8133226630bac025350e84b0ca5c31d45a` (2026-11-07): `CreateSpeechRequest`, `CreateSpeechResponseStreamEvent`, `SpeechAudioDeltaEvent`, `SpeechAudioDoneEvent`, `ErrorResponse`, and the `/audio/speech` examples.
- No OpenAI fixture is recorded: no OpenAI key was available. The audio is a 1-second MP3 made for the tests.

The layout and the provenance marks are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). Each provider folder's `README.md` lists its files.

## Files

| file | what it covers | provenance |
|---|---|---|
| `requests/speech.json` | the reference's default example | doc: openai-openapi@926a7b8 `/audio/speech` example "Default" |
| `requests/stream-speech.json` | `stream_format: sse` | doc: openai-openapi@926a7b8 `/audio/speech` example "SSE Stream Format" |
| `requests/speech-every-field.json` | `instructions`, `response_format: wav`, `speed` and `stream_format: audio` | schema: openai-openapi@926a7b8 `CreateSpeechRequest` |
| `requests/speech-custom-voice.json` | a custom voice `{"id": …}`, `pcm` and an integer `speed` | schema: openai-openapi@926a7b8 `CreateSpeechRequest.voice` |
| `requests/speech-tts-1.json` | `tts-1-hd` with `opus` | schema: openai-openapi@926a7b8 `CreateSpeechRequest` |
| `responses/speech.mp3` | the audio reply, with `speech.mp3.meta.json` naming its content type | schema: the `audio/mpeg` reply of `/audio/speech`; test audio |
| `streams/stream-speech.sse` | two `speech.audio.delta` events and `speech.audio.done` with usage | schema: `SpeechAudioDeltaEvent`, `SpeechAudioDoneEvent`; the usage numbers are made up |
| `errors/invalid-voice-400.json` | an invalid voice | schema: `ErrorResponse`; the message is worded after the reference |

## Round trip

A request without `stream_format: sse` to a model that streams events goes out with `stream_format: sse`, since OpenAI reports usage only in its events; the round-trip test removes that field before comparing.
