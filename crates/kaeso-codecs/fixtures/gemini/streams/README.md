# `fixtures/gemini/streams`

| file | what it covers | provenance |
|---|---|---|
| `recorded-stream-text.sse` | text in chunks, then an empty text part with the thought signature and `finishReason` | RECORDED.md |
| `recorded-stream-thinking.sse` | `includeThoughts` with no thought parts streamed | RECORDED.md |
| `recorded-stream-thinking-high.sse` | a thought part, then text, `gemini-3.5-flash-lite` | RECORDED.md |
| `recorded-stream-speech.sse` | 16 chunks of 16-bit PCM speech, then an empty text part with `finishReason`, `gemini-3.8-flash-lite-tts` | RECORDED.md |
| `recorded-stream-tools.sse` | two function calls in two chunks, the first with the thought signature | RECORDED.md |
