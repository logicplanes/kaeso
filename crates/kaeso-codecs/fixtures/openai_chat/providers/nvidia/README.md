# openai_chat / providers / nvidia (NVIDIA NIM)

NVIDIA NIM, provider id `nvidia`, base URL `https://integrate.api.nvidia.com`, `POST /v1/chat/completions` with `Authorization: Bearer`. API reference: https://docs.api.nvidia.com/nim/reference/llm-apis, read on 2026-10-19.

Nothing here was recorded live: Kaeso has no NVIDIA NIM test key. Every file is an official example taken without changes:

- `doc:` the `x-nvai-meta.examples[]` strings (`requestJson`, `responseJson`) or the `responses.<status>.content.application/json.example` of the OpenAPI document that the model's page on docs.api.nvidia.com embeds, read on 2026-10-19. These examples are written by NVIDIA, not captured, so ids such as `id-123` and the `system_fingerprint` are placeholders. The `stream: true` in the request examples is as published. The `responses.<status>` examples are JSON objects in the document, not strings, so they are written out with 2-space indentation; keys, values and their order are NVIDIA's.
- `doc (NIM for LLMs):` a code block of the self-hosted NIM for LLMs 1.15 documentation, read on 2026-10-19.
- `sdk:` a literal from the unit tests of NVIDIA's `langchain-nvidia-ai-endpoints` at commit `455a534f5e13f2ca2dbac5f1c9bd2f8ccd712d59` (2026-10-16), as JSON: Python `None` is `null`, and the stream frames are joined with `\n\n` as the test joins them, plus the blank line that ends the last event.

| file | what it covers | provenance |
|---|---|---|
| `requests/doc-llama-3-3-70b-seed.json` | `seed`, `top_p`, `max_tokens` on a model id with a slash | doc: https://docs.api.nvidia.com/nim/reference/meta-llama-3_3-70b-instruct-infer (example 1) |
| `requests/doc-nemotron-super-49b-system-stop-null.json` | `system` message and `stop: null` | doc: https://docs.api.nvidia.com/nim/reference/nvidia-llama-3_3-nemotron-super-49b-v1-infer (example 1) |
| `requests/doc-deepseek-v4-flash-reasoning-effort-none.json` | `reasoning_effort: none` | doc: https://docs.api.nvidia.com/nim/reference/deepseek-ai-deepseek-v4-flash-infer (example 1) |
| `requests/doc-deepseek-v4-flash-chat-template-kwargs.json` | reasoning toggle `chat_template_kwargs: {thinking, reasoning_effort}` (Extras) | doc: https://docs.api.nvidia.com/nim/reference/deepseek-ai-deepseek-v4-flash-infer (example 2) |
| `requests/doc-nemotron-3-super-reasoning-budget.json` | `chat_template_kwargs: {enable_thinking}` and top-level `reasoning_budget` (Extras) | doc: https://docs.api.nvidia.com/nim/reference/nvidia-nemotron-3-super-120b-a12b-infer (example 2) |
| `requests/doc-nemotron-3-super-low-effort.json` | `chat_template_kwargs: {enable_thinking, low_effort}` (Extras) | doc: https://docs.api.nvidia.com/nim/reference/nvidia-nemotron-3-super-120b-a12b-infer (example 3) |
| `requests/doc-kimi-k3-image-reasoning-effort.json` | text and `image_url` parts with `reasoning_effort: low` | doc: https://docs.api.nvidia.com/nim/reference/moonshotai-kimi-k3-infer (example 1) |
| `responses/doc-llama-3-3-70b.json` | plain text reply; usage without details | doc: https://docs.api.nvidia.com/nim/reference/meta-llama-3_3-70b-instruct-infer (example 1) |
| `responses/doc-kimi-k3-image.json` | plain text reply without `system_fingerprint` | doc: https://docs.api.nvidia.com/nim/reference/moonshotai-kimi-k3-infer (example 1) |
| `responses/sdk-parallel-tool-calls.json` | two `tool_calls` with vLLM ids (`chatcmpl-tool-…`), choice-level `logprobs: null` and `stop_reason: null`, usage keys in vLLM order | sdk: https://github.com/langchain-ai/langchain-nvidia/blob/455a534f5e13f2ca2dbac5f1c9bd2f8ccd712d59/libs/ai-endpoints/tests/unit_tests/test_parallel_tool_calls.py (`test_invoke_parallel_tool_calls`) |
| `streams/sdk-parallel-tool-calls-incremental.sse` | two tool calls streamed argument by argument, `system_fingerprint: null`, `logprobs: null`, ended by `data: [DONE]` | sdk: https://github.com/langchain-ai/langchain-nvidia/blob/455a534f5e13f2ca2dbac5f1c9bd2f8ccd712d59/libs/ai-endpoints/tests/unit_tests/test_parallel_tool_calls.py (`test_stream_parallel_tool_calls_A`) |
| `streams/sdk-parallel-tool-calls-one-chunk.sse` | both tool calls whole in one chunk together with `finish_reason` and `stop_reason`, `delta.role: null` | sdk: https://github.com/langchain-ai/langchain-nvidia/blob/455a534f5e13f2ca2dbac5f1c9bd2f8ccd712d59/libs/ai-endpoints/tests/unit_tests/test_parallel_tool_calls.py (`test_stream_parallel_tool_calls_B`) |
| `errors/doc-nvcf-unprocessable-entity.json` | NVCF problem details (`type`, `title`, `status`, `detail`, `instance`, `requestId`), HTTP 422; `detail: "string"` is a placeholder in the original | doc: https://docs.api.nvidia.com/nim/reference/meta-llama-3_3-70b-instruct-infer (`responses.422` example) |
| `errors/doc-nvcf-internal-server-error.json` | NVCF problem details, HTTP 500 | doc: https://docs.api.nvidia.com/nim/reference/meta-llama-3_3-70b-instruct-infer (`responses.500` example) |
| `errors/doc-nim-missing-messages-400.json` | vLLM error at the top level (`object: "error"`, `message`, `type`, `param`, numeric `code`), HTTP 400; the `...` in `message` is in the original | doc (NIM for LLMs): https://docs.nvidia.com/nim/large-language-models/1.15.0/getting-started.html |
| `models/list-doc-nim.json` | model list with vLLM's `root`, `parent` and `permission` | doc (NIM for LLMs): https://docs.nvidia.com/nim/large-language-models/1.15.0/getting-started.html |

The three error bodies have no `error` object, so the Chat codec answers apps with an OpenAI error that quotes them; `openai_chat::tests::error_bodies_without_an_error_object_reach_apps_as_openai_errors_that_quote_them` checks that instead of the round trip.

No official example exists for a plain-text stream, a stream with usage, a reply with `reasoning_content`, or a 401, 404 or 429 body from integrate.api.nvidia.com, so there is no fixture for them.
