# `openai_transcriptions/providers/siliconflow`

SiliconFlow's transcriptions, `POST https://api.siliconflow.com/v1/audio/transcriptions`, with `FunAudioLLM/SenseVoiceSmall` and `TeleAI/TeleSpeechASR`. The form takes only `file` and `model`. Nothing here is recorded.

| file | what it covers | provenance |
|---|---|---|
| `responses/json.json` | the reply, the text alone | doc: `AudioResponse` in https://docs.siliconflow.com/en/api-reference/audio/create-audio-transcriptions, read 2026-11-10 |
