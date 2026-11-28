---
title: Sampling across formats
nav: Sampling
description: The sampling settings each API format takes, and the values Kaeso leaves out because they ask for nothing.
order: 3
---

Sampling settings decide how a model picks its words: temperature, top P and top K, the penalties, a seed, stop sequences and the output limit. Each API format takes a different set of them. This page lists what a target refuses, and the values Kaeso leaves out instead because they ask for nothing.

## Settings a target refuses

Sampling settings a target can't honour are `Unsupported`, never dropped or rescaled, except `top_k` ([top K](#top-k)) and a penalty of 0 ([neutral values](#neutral-values)):

| Target | Refused |
| --- | --- |
| Anthropic | `seed`, frequency and presence penalties, and a `temperature` outside 0 to 1, Anthropic's range |
| OpenAI Chat Completions | More than four stop sequences |
| OpenAI Responses | `seed`, the penalties and stop sequences, which Responses doesn't have, and an output limit below 16 tokens, the `CreateResponse` minimum; the gateway serves all but the penalties ([OpenAI Responses in the gateway](#openai-responses-in-the-gateway)) |
| Gemini API | More than five stop sequences (`GenerationConfig.stopSequences`) |
| Amazon Bedrock Converse | `seed` and the penalties, which go in a model family's own fields |

Numbers keep their spelling within a dialect: `temperature: 1` comes back as `1` and `1.0` as `1.0` while the value is unchanged. Across dialects the target's own spelling is used, so `temperature: 1` from Chat reaches Anthropic as `1.0`.

KPP has no `n`: a Chat request with `n` above 1 is refused when it is decoded, whatever the target. Chat's `logprobs`, `logit_bias` and `top_logprobs` are extras, which every other dialect refuses ([extras](cross-format.md#extras)) unless they hold their [neutral value](#neutral-values).

## OpenAI Responses in the gateway

The Responses codec refuses stop sequences, `seed` and an output limit below 16 tokens. When the gateway sends a request from another format to a model served through Responses, such as Grok, it serves them instead:

- **Stop sequences.** The request goes without them, and Kaeso ends the reply itself before the first stop sequence in its text. Open blocks stop, and the reply stops as the app's format says: `finish_reason: "stop"`, or `stop_reason: "stop_sequence"` with `stop_sequence` set. Text that could begin a stop sequence waits for the next delta, so a sequence split across deltas still ends the reply. Reasoning and tool arguments never end it. `kaeso-adapted` names the app's field, such as `stop`.
- **Output limits below 16 tokens.** Kaeso sends 16 and ends the reply at the app's limit, with the format's length reason. For a provider whose limit leaves out reasoning, such as xAI, reasoning doesn't count. `kaeso-adapted` names the app's field, such as `max_tokens`. A Responses app's own limit below 16 is still refused.
- **`seed`.** Left out and named in `kaeso-left-out`: OpenAI treats it as best effort, and Responses has no field for it.

Kaeso reads such a reply as a stream, also when the app asked for none, so the provider stops when Kaeso ends the reply. Kaeso estimates the usage from what it read, as for a [ChatGPT plan's output limit](openai-responses.md#chatgpt-plan-usage).

## Top K

`top_k` is a hint. Anthropic and Gemini get it, and so do Chat Completions providers that document `top_k`, such as Together AI, Fireworks AI and OpenRouter (`with_top_k()`, [provider profiles](openai-chat-providers.md#provider-profiles)). OpenAI's Chat Completions and Responses, the other Chat Completions providers and Amazon Bedrock Converse have no top-k sampling, and some Anthropic-format providers, such as DeepSeek and MiniMax, ignore it. For those, Kaeso leaves it out and names it in `kaeso-left-out` as the app wrote it: `generationConfig.topK` for a Gemini app, `top_k` for an Anthropic app.

Gemini CLI sends `topK` with every request and has no switch for it, so refusing it would refuse Gemini CLI on every such model. Google itself ignores `topK` on its newest models (`GenerationConfig.topK`).

## Neutral values

Chat apps such as NextChat send OpenAI's sampling fields on every request, at the value the API uses when they are absent. Such a value asks for nothing. Where the target or its provider's profile would refuse the field, Kaeso leaves it out instead and names it in `kaeso-left-out` as the app wrote it. Any other value is refused as before. The values are `cross::NEUTRAL_VALUES` and the penalties.

| Field | Value | Left out for |
| --- | --- | --- |
| `frequency_penalty`, `presence_penalty`, a Gemini app's `generationConfig.frequencyPenalty` and `.presencePenalty` | `0` | Anthropic, Responses, Amazon Bedrock Converse, and Chat providers that refuse them |
| `logprobs` | `false` | every other format, and Chat providers that refuse it |
| `top_logprobs` | `0` | the same |
| `logit_bias`, `metadata` | `{}` | the same |
| `store` | `false` | the same; a Responses target gets Kaeso's own `store: false`, so it isn't named there |
| `service_tier`, from Chat and from Anthropic | `"auto"` | the same, and Anthropic providers that refuse it |
| `verbosity` | `"medium"` | the same |
| `modalities` | `["text"]` | Chat providers that refuse it; other formats write text anyway |
| `thread`, from Anthropic | `null` | every format, Anthropic Messages too |

Each value is the field's `default` in OpenAI's OpenAPI document and Anthropic's API reference; an empty `logit_bias` or `metadata` means the same as their default, `null`. `thread: null` is the exception: no provider documents it, and it asks for no message thread, as leaving `thread` out does. `service_tier: "default"` isn't neutral: it asks for standard processing where a project's setting could choose another tier. `web_search_options: {}` isn't either: it makes a search model search. A field sent as `null` is form, which other formats never get; a Chat provider that refuses the field doesn't get the `null` either.

Fixtures: `crates/kaeso-codecs/fixtures/openai_chat/requests/nextchat-defaults.json` (NextChat's request), `crates/kaeso-codecs/fixtures/openai_chat/requests/neutral-defaults.json`.
