# `openai_transcriptions/providers/alibaba`

QwenCloud serves `qwen3-asr-flash` through its OpenAI-compatible `chat/completions`, not a transcription endpoint; the `openai_chat` codec reads this reply as a transcript. Nothing here is recorded.

| file | what it covers | provenance |
|---|---|---|
| `responses/chat.json` | a chat completion with the transcript, an `audio_info` annotation and usage with `seconds` | doc: https://www.alibabacloud.com/help/en/model-studio/qwen-asr-api-reference (read 2026-11-08); the id is shortened |
