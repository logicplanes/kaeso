# `openai_speech/providers/siliconflow`

SiliconFlow's speech endpoint, `POST https://api.siliconflow.com/v1/audio/speech`, which answers with the audio itself. Nothing here is recorded.

| file | what it covers | provenance |
|---|---|---|
| `requests/speech.json` | the reference's example for `FunAudioLLM/CosyVoice2-0.5B`, with a voice named `<model>:<voice>` and an instruction before `<\|endofprompt\|>` | doc: https://docs.siliconflow.com/en/api-reference/audio/create-speech, read 2026-11-10 |

The reference also documents `sample_rate`, `speed`, `gain`, `stream` (on by default) and `references` with reference audio instead of a voice.
