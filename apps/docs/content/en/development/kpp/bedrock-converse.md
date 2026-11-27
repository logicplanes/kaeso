---
title: Amazon Bedrock Converse
description: How KPP's generation requests map to Amazon Bedrock's Converse API, the format Kaeso sends Nova, Llama and Bedrock's other chat models in.
order: 9
---

Look up how a generation request reaches Amazon Bedrock's Converse API here: which field goes where, how the binary stream is read, and the source of each mapping.

- **Codec:** `kaeso_codecs::bedrock_converse::BedrockConverse` (`crates/kaeso-codecs/src/bedrock_converse/`)
- **Paths:** `POST /model/{modelId}/converse` and `/model/{modelId}/converse-stream`, the model id or inference profile percent-encoded, on the runtime host of the key's region
- **Fixtures:** `crates/kaeso-codecs/fixtures/bedrock_converse/`, built from the references below, none recorded

**Sources.** [Converse](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_Converse.html), [ConverseStream](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ConverseStream.html), [ContentBlock](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ContentBlock.html), [Amazon Nova tool use](https://docs.aws.amazon.com/nova/latest/userguide/tool-use-invocation.html) and the [event stream encoding](https://smithy.io/2.0/aws/amazon-eventstream.html), read 2026-11-16.

## Decisions

- **Egress only.** No app sends Converse to Kaeso; the codec refuses to decode a request or write a reply in it.
- **Which models.** KPMR names `bedrock_converse` as the egress dialect of the Bedrock chat models that answer neither Chat Completions nor Messages, such as Amazon Nova, Meta Llama and Mistral Pixtral. Claude models that Messages serves stay on Messages.
- **Keys.** Converse takes a Bedrock API key as a bearer token, and AWS credentials signed with Signature Version 4.
- **What a family needs of its own isn't written yet.** Reasoning settings, JSON output formats and other fields that go in `additionalModelRequestFields` are `Unsupported`, never dropped. `top_k`, a hint, is left out and named in `kaeso-left-out` ([top K](sampling.md#top-k)).

## Request

| KPP | Converse |
| --- | --- |
| `system` text blocks | `system[]` `{text}` |
| `messages[]` | `messages[]` `{role, content[]}`, consecutive messages of one role joined, since Converse wants the roles to alternate |
| text, refusal | `{text}` |
| image, by its bytes or a `data:` URL | `{image: {format, source: {bytes}}}`, format `png`, `jpeg`, `gif` or `webp` |
| document | `{document: {format, name, source}}`: bytes, or `text` for a text document; `name` from the title, or `Document N` |
| tool call | `{toolUse: {toolUseId, name, input}}` |
| tool result | `{toolResult: {toolUseId, content[], status}}`, its content as `text`, `json`, `image` or `document`, `status: "error"` for an error |
| Converse's own reasoning | `{reasoningContent: {reasoningText: {text, signature}}}` or `{redactedContent}` |
| cache hint | a `{cachePoint: {type: "default", ttl}}` block after the hinted block or tool, `ttl` `5m` or `1h` |
| `max_output_tokens`, `temperature`, `top_p`, `stop` | `inferenceConfig.maxTokens`, `temperature`, `topP`, `stopSequences` |
| function tools | `toolConfig.tools[].toolSpec` `{name, description, inputSchema: {json}, strict}` |
| `tool_choice` auto, required, a tool | `toolConfig.toolChoice` `{auto}`, `{any}`, `{tool: {name}}` |

Tool names and call ids that Converse's patterns refuse (names `[a-zA-Z0-9_-]`, ids `[a-zA-Z0-9_.:-]`, at most 64 characters) are rewritten on the way out and restored in the reply.

Refused with `Unsupported`: images and documents by web link or file id, audio and video blocks, document citations, hosted tools, `tool_choice: none`, `parallel_tool_calls: false`, reasoning settings, structured output, `seed` and the penalties, except a penalty of `0`, which is left out ([neutral values](sampling.md#neutral-values)). Reasoning of another format isn't sent.

## Reply

| Converse | KPP |
| --- | --- |
| `output.message.content[]` `text` | text block |
| `toolUse` | tool call, its id and name as the request sent them |
| `reasoningContent` | reasoning block, the signature or redacted content as opaque data |
| other blocks | unknown blocks |
| `stopReason` `end_turn`, `tool_use`, `max_tokens`, `stop_sequence` | the same stop reason |
| `guardrail_intervened`, `content_filtered` | `content_filter` |
| `model_context_window_exceeded` | `max_tokens` |
| `usage.inputTokens`, `outputTokens`, `cacheReadInputTokens`, `cacheWriteInputTokens` | input, output, cache read and cache write tokens |
| `x-amzn-requestid` header | the reply's id |

## Streams

`converse-stream` answers in AWS's binary event stream encoding (`application/vnd.amazon.eventstream`), not server-sent events. `bedrock_converse::eventstream::Framer` reads its messages: a 12-byte prelude with the total length, the headers' length and their CRC32, the headers, the payload and the message's CRC32. A checksum that doesn't match ends the stream with an error.

| Event | KPP |
| --- | --- |
| `messageStart` | response start |
| `contentBlockStart` with `toolUse` | a tool call block starts |
| `contentBlockDelta` `text`, `toolUse.input`, `reasoningContent` | text, tool arguments, reasoning text or opaque deltas; a text or reasoning block starts with its first delta |
| `contentBlockStop` | the block stops |
| `messageStop` | the stop reason, held for the usage |
| `metadata` | the usage, then the response stops |
| an exception message, such as `throttlingException` | an error event in the app's format |

## Errors

Bedrock names the error in the `x-amzn-errortype` header, or in the body's `__type`, with the message in `message`:

| Error | KPP |
| --- | --- |
| `ValidationException` | `invalid_request`, or `context_too_long` when the message says the input is too long |
| `ThrottlingException`, `ServiceQuotaExceededException` | `rate_limited` |
| `AccessDeniedException` | `permission_denied`, a refused key |
| `UnrecognizedClientException`, `InvalidSignatureException`, `ExpiredTokenException` | `authentication` |
| `ModelNotReadyException`, `ServiceUnavailableException` | `overloaded` |
| `ModelTimeoutException` | `timeout` |
| `ResourceNotFoundException` | `not_found` |
| `ModelErrorException`, `InternalServerException` | `provider_error` |
