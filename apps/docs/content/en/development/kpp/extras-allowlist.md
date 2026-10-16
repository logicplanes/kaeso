---
title: Extras allowlist
description: The extras that are dropped instead of refused when a request changes format, and why each is safe.
order: 3
---

Extras captured from dialect A are written back only to dialect A. When a request carrying extras goes to another dialect, the codec's `check()` returns `Unsupported` naming the field, unless the field is listed here: then it is dropped silently ([extras](cross-format.md#extras)).

An entry is allowed only when the field is known to have no effect on the model's output. Every entry needs a one-line reason and the fixture that shows a real client sending it.

Entries are scoped (`cross::EXTRAS_ALLOWLIST`): a key is only allowlisted where it sits. The scopes are `request`, `message`, `tool`, `output`, `reasoning`, and the block kinds `text`, `image`, `document`, `tool_call`, `tool_result` and `refusal`. Headers are request-scoped keys `header:<name>`.

Fixture paths on this page are relative to `crates/kaeso-codecs/fixtures/`.

## Entries

| Origin dialect | Scope | Key | Reason | Fixture / client |
| --- | --- | --- | --- | --- |
| `anthropic_messages` | request | `header:anthropic-beta` | Beta headers only switch on provider features. A feature a request actually uses appears as a body field or block, which is checked on its own. | Claude Code sends `claude-code-20250219,interleaved-thinking-…` and further betas on every request; `anthropic_messages/requests/claude-code-mid-conversation-system.headers.json`, `anthropic_messages/requests/thinking-interleaved-tools.headers.json` |
| `anthropic_messages` | request | `header:anthropic-version` | API versioning of the Anthropic wire format; meaningless elsewhere. A value equal to the pinned version is not stored at all ([the Anthropic version](cross-format.md#the-anthropic-version)). | Anthropic SDKs |
| `openai_chat`, `openai_responses`, `openai_embeddings` | request | `header:openai-beta` | As `anthropic-beta`. | Codex sends `OpenAI-Beta: responses=experimental` |
| `anthropic_messages` | request | `cache_control` | Top-level automatic prompt caching. Cache hints are hints ([cache hints](cross-format.md#cache-hints)). | `anthropic_messages/requests/system-string-multi-turn.json`, `explicit-nulls.json` |
| `anthropic_messages` | reasoning | `display` (`thinking.display`) | Only controls whether thinking text is returned (`summarized` or `omitted`). OpenAI Responses gets `reasoning.summary: "auto"` instead unless it is `omitted` ([readable reasoning and summaries](cross-format.md#readable-reasoning-and-summaries)). | `anthropic_messages/requests/thinking-display-omitted.json`, `thinking-adaptive-effort.json` |
| `openai_chat` | image | `image_url.detail` | Image resolution hint (`low`, `high`, `auto`): it changes image token cost, not what is asked; targets without it choose their own resolution. | `openai_chat/requests/image-base64-detail.json` |
| `openai_chat` | document | `file.filename` | The file name OpenAI requires next to inline `file_data`. Metadata, no instruction. | `openai_chat/requests/file-pdf-data.json` |
| `openai_chat` | request | `prompt_cache_key` | Routes requests to a prompt cache. A cache hint ([cache hints](cross-format.md#cache-hints)). | `openai_chat/requests/all-params-sdk.json` |
| `openai_responses` | request | `prompt_cache_key` | As above. | `openai_responses/requests/sampling-and-ids.json` |
| `openai_responses` | image | `detail` (on `input_image`; `auto` is already a form marker) | Image resolution hint, as Chat's `image_url.detail`. `input_file.detail` (PDF rendering) is **not** allowlisted. | `openai_responses/requests/image-base64-file-id.json` |

## Conditional entries

These fields cross only with the values below (`cross::is_droppable_value`, and the per-message rule in `cross::check_foreign`); any other value stays `Unsupported`.

| Origin dialect | Scope | Key | Condition and reason | Fixture / client |
| --- | --- | --- | --- | --- |
| `anthropic_messages` | request | `context_management` | Holds only `edits`, and every edit is `clear_thinking_*`. Thinking-block clearing only decides which earlier Anthropic `thinking` blocks the model sees, and those never reach another dialect ([reasoning](cross-format.md#reasoning)). | Claude Code 2.1.296 sends `{"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]}` on every request; `anthropic_messages/requests/claude-code-mid-conversation-system.json` |
| `anthropic_messages` | message (`role: "system"`) | `output_config` | `{}`, or only `effort` equal to the request's `output_config.effort`. Repeating the request's effort changes nothing. | Claude Code 2.1.296 repeats the request's effort on its mid-conversation system message; same fixture |
