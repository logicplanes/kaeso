# Fixtures: `openai_transcriptions`

Wire bodies for `POST /v1/audio/transcriptions`, `POST /v1/audio/translations` and the providers that speak a form of OpenAI's transcription API.

## Sources

- OpenAI OpenAPI spec at commit `926a7b8133226630bac025350e84b0ca5c31d45a` (2026-11-07): `CreateTranscriptionRequest`, `CreateTranslationRequest`, `CreateTranscriptionResponseJson`, `CreateTranscriptionResponseVerboseJson`, `CreateTranscriptionResponseDiarizedJson`, `CreateTranslationResponseJson`, `TranscriptTextDeltaEvent`, `TranscriptTextSegmentEvent`, `TranscriptTextDoneEvent`, and the `/audio/transcriptions` examples.
- openai-python 3.41.0, whose multipart field names (`timestamp_granularities[]`, `include[]`, `known_speaker_names[]`) the uploads follow.
- No OpenAI fixture is recorded: no OpenAI key was available. The audio is a 1-second clip made for the tests.

The layout and the provenance marks are in [Fixtures and recordings](https://docs.kaeso.app/development/kpp/fixtures). Each provider folder's `README.md` lists its files.

## Files

| file | what it covers | provenance |
|---|---|---|
| `requests/transcribe.multipart` | the reference's default example | doc: openai-openapi@926a7b8 `/audio/transcriptions` example "Default" |
| `requests/transcribe-logprobs.multipart` | `include[]=logprobs` | doc: example "Logprobs" |
| `requests/transcribe-every-field.multipart` | `language`, `prompt`, `verbose_json`, `temperature`, both timestamp granularities | schema: `CreateTranscriptionRequest` |
| `requests/transcribe-srt.multipart`, `transcribe-vtt.multipart`, `transcribe-text.multipart` | subtitle and text forms, an explicit `stream=false` | schema: `AudioResponseFormat` |
| `requests/transcribe-diarize.multipart` | `diarized_json`, `chunking_strategy`, known speakers | doc: example "Diarization" |
| `requests/stream-transcribe.multipart` | `stream=true` | doc: example "Streaming" |
| `requests/translate.multipart`, `translate-verbose.multipart` | translations, plain and verbose | doc: `/audio/translations` example; schema: `CreateTranslationRequest` |
| `requests/*.meta.json` | each upload's content type with its boundary | as above |
| `responses/json.json`, `json-logprobs.json` | `json` with token usage, and with `logprobs` | doc: examples "Default" and "Logprobs" (logprobs shortened) |
| `responses/verbose-words.json`, `verbose-segments.json` | `verbose_json` with words, and with segments | doc: examples "Word timestamps" and "Segment timestamps" |
| `responses/diarized.json` | `diarized_json` | doc: example "Diarization" |
| `responses/translation.json` | a translation | doc: `/audio/translations` example |
| `responses/text.txt`, `subtitles.srt`, `subtitles.vtt` | the text forms of the segment example | schema: the `text/plain` reply; text from example "Segment timestamps" |
| `streams/stream-transcribe.sse` | deltas with logprobs and the end with token usage | doc: example "Streaming" (shortened) |
| `streams/stream-diarize.sse` | a segment, a delta with `segment_id` and the end | schema: `TranscriptTextSegmentEvent`, `TranscriptTextDeltaEvent`, `TranscriptTextDoneEvent` |
| `errors/file-too-large-413.json` | an upload over the limit | schema: `ErrorResponse`; the message is worded after the guide's 25 MB limit |
