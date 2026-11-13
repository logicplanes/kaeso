---
title: Gemini API
description: How every field of the Gemini API maps to KPP and back, for generation, token counts, embeddings and model listings.
order: 9
---

Look up any field of the Gemini API here: where it goes in KPP and the source of its mapping. Check it before you change how the codec reads or writes that field.

- **Codecs:** `kaeso_codecs::gemini::Gemini` (`crates/kaeso-codecs/src/gemini/`) and `kaeso_codecs::gemini_embeddings::GeminiEmbeddings` (`crates/kaeso-codecs/src/gemini_embeddings/`)
- **Endpoints:** `POST {models}/{model}:generateContent`, `:streamGenerateContent?alt=sse`, `:countTokens`, `:embedContent` and `:batchEmbedContents`, and the [models](#models) endpoints
- **Fixtures:** `crates/kaeso-codecs/fixtures/gemini/` and `crates/kaeso-codecs/fixtures/gemini_embeddings/`
- **Image requests:** apps of OpenAI's Images API reach Gemini image models through this codec, as `generateContent` calls ([Gemini image models](openai-images.md#gemini-image-models))

**Sources.** The mappings follow these sources:

- The Gemini API discovery document, `https://generativelanguage.googleapis.com/$discovery/rest?version=v1beta`, revision `20261106`. Schema names below refer to it.
- The [Gemini API documentation](https://aistudio.google.com/docs), read on 2026-11-08: text generation, thinking, thought signatures, function calling, structured output, token counting, embeddings, file input methods, audio understanding, video understanding, models and troubleshooting.
- The official Python SDK `google-genai` 2.32.0, which the [SDK conformance suite](../testing.md#sdk-conformance) runs against the encoders.

## How to read the tables

- **KPP**: the typed field the wire value maps to.
- **fields**: kept raw in `Extras::fields`, bound to `gemini` (or `gemini_embeddings`). Other dialects refuse them with `Unsupported` ([extras](cross-format.md#extras)). Nested objects use dotted key paths, such as `generationConfig.imageConfig`.
- **form**: a codec-private presentation marker in `Extras::form`, ignored by other dialects. `verbatim` means the generic `wire::Ext` mechanism (`null`, `false` and `[]` spellings).
- **raw**: kept whole in a `RawItem` or `RawUsage`.
- **Own** data has extras or an origin of `gemini`; **foreign** data comes from another dialect.

## Paths

A Gemini request names its model and its method in the path, so the codec builds the path from the request and decodes a request with both.

| Piece | Rule |
|---|---|
| Models path | `GeminiProfile::models_path` on `EgressCtx::gemini_profile`. `GeminiProfile::GOOGLE_AI_STUDIO` is `/v1beta/models`; `with_models_path` sets another per request, such as `/v1/projects/{project}/locations/{location}/publishers/google/models`. |
| Model | One percent-encoded path segment, `:` included. A leading `models/` is the same model's resource name and is removed; `tunedModels/<id>` addresses a tuned model next to the models path. `.` and `..` are `InvalidRequest`, as for [model retrieval](codec-design.md#model-listings). |
| Method | `GeminiAction`: `generateContent` and `streamGenerateContent` (`RequestKind::Generate`, streaming decided by the method), `countTokens` (`CountTokens`), `embedContent` and `batchEmbedContents` (`Embed`, the `gemini_embeddings` dialect). `streamGenerateContent` adds `?alt=sse`. |
| Decoding | `dispatch::decode_gemini_request(model, action, body, headers)`, or `decode_checked_gemini_request` for a body `limits::Checked` counted. `gemini::split_model_action` splits a segment such as `gemini-3.5-flash:streamGenerateContent`. `dispatch::decode_request` without a path decodes a generation body with an empty model. The gateway serves Gemini apps at `/v1beta/models/{model}:{method}` and `/v1/models/{model}:{method}` with both (`crates/kaeso-server/src/model_api/gemini.rs`). |
| Role on every content | `GeminiProfile::role_on_every_content`: a content an app sent without a `role` gets `user`. Off for Google AI Studio, on for Gemini Enterprise Agent Platform. |

## Requests

Schema `GenerateContentRequest`.

| Wire field | KPP | Notes |
|---|---|---|
| `contents[]` (`Content`) | `messages[]` | One message per content, consecutive same-role contents kept as written. Unknown keys → message fields. |
| `contents[].role` `user` / `model` | `Role::User` / `Role::Assistant` | Absent → `Role::User`, form `role: "absent"`. Any other role → a user message holding the whole content as one unknown block, refused across dialects as `` `<role>` message `` ([requests](cross-format.md#requests)). Gemini answered a content with the role `assistant` (`requests/role-assistant.json`). |
| `systemInstruction` | `system` | Its parts as blocks; `role` → form `systemInstruction.role`; unknown keys → fields `systemInstruction.*`. The system instruction holds text only, so a foreign non-text system block is `Unsupported`. |
| `tools[]` with only `functionDeclarations` | `tools[]` as `ToolDef::Function` | Each declaration's Tool index → form `group`, so a same-dialect encode keeps the grouping. Foreign tools go into one Tool. |
| `tools[]` with any other key (`googleSearch`, `urlContext`, `codeExecution`, `googleMaps`, `fileSearch`, `computerUse`, `mcpServers`, `googleSearchRetrieval`), or with empty `functionDeclarations` | `ToolDef::Unknown` (raw) | Refused across dialects. |
| `functionDeclarations[].name`, `.description` | `name`, `description` | |
| `functionDeclarations[].parametersJsonSchema` | `parameters` | Full JSON Schema. Foreign tools always get `parametersJsonSchema`, so nothing is lost to the OpenAPI subset. |
| `functionDeclarations[].parameters` (OpenAPI `Schema`) | `parameters` | form `schema: "parameters"`, so Gemini gets it back in `parameters`. Other dialects get it as a JSON Schema as written; a provider that needs standard type names refuses uppercase ones such as `OBJECT`. With `parametersJsonSchema` beside it → field `parameters`. |
| `functionDeclarations[].response`, `.responseJsonSchema`, `.behavior` | tool fields | |
| `toolConfig.functionCallingConfig` `mode: AUTO` / `NONE` / `ANY` | `tool_choice` `Auto` / `None` / `Required` | |
| `mode: ANY` with one `allowedFunctionNames` entry | `ToolChoice::Tool(name)` | |
| `mode: VALIDATED`, `ANY` with several names, any other combination | `ToolChoice::Unknown` (raw) | Refused across dialects. |
| `toolConfig.retrievalConfig`, `.includeServerSideToolInvocations` | fields `toolConfig.*` | |
| `generationConfig.temperature`, `.topP`, `.topK`, `.maxOutputTokens`, `.stopSequences`, `.seed`, `.presencePenalty`, `.frequencyPenalty` | `sampling.*` | Numbers keep their spelling within the dialect (form `integers`). |
| `generationConfig.candidateCount` | form `candidate_count` | Above 1 → `Unsupported` when decoded, as Chat's `n` ([sampling](cross-format.md#sampling)). |
| `responseMimeType: application/json` with `responseJsonSchema` | `output = JsonSchema` | |
| `responseMimeType: application/json` with `responseSchema` | `output = JsonSchema` | form `schema: "responseSchema"`; other dialects get the OpenAPI schema as written. |
| `responseMimeType: application/json` alone | `output = JsonObject` | |
| Any other `responseMimeType`, and a schema without `application/json` | fields `generationConfig.*` | |
| `generationConfig.thinkingConfig` | `reasoning` | See [thinking](#thinking). |
| `generationConfig.imageConfig`, `.mediaResolution`, `.responseLogprobs`, `.logprobs`, `.audioTranscriptionConfig`, `.translationConfig`, `.enableAffectiveDialog`, `.enableEnhancedCivicAnswers`, `.responseFormat` | fields `generationConfig.*` | |
| `generationConfig.responseModalities` | `output_modalities` | `TEXT`, `IMAGE`, `AUDIO` ↔ `Text`, `Image`, `Audio`; other values as written; `[]`: form `modalities_empty`. From other dialects: `["AUDIO"]` when they ask for audio, because a Chat app's `["text", "audio"]` means audio with its transcript and Gemini's speech models answer an exact `["AUDIO"]` only. |
| `generationConfig.speechConfig.voiceConfig.prebuiltVoiceConfig.voiceName`, or `.voiceConfig.voice` | `audio_output.voice` | form `voice` names the spelling; `languageCode`, `multiSpeakerVoiceConfig` and other keys: extras of `audio_output`, refused across dialects. From other dialects: a prebuilt voice the speech guide lists (Zephyr, Puck, Charon, Kore and 26 more), else `Unsupported`. |
| `generationConfig.responseFormat` with only `audio.mimeType` `AUDIO_WAV`, `AUDIO_MP3` or `AUDIO_L16` (at 24 kHz) | `audio_output.format` `Wav`, `Mp3`, `Pcm16` | Any other `responseFormat`, such as `AUDIO_MULAW` or an image format: field `generationConfig.responseFormat`. From other dialects: `flac`, `opus` and `aac` are `Unsupported`; nothing is transcoded. |
| `safetySettings`, `cachedContent`, `labels`, `serviceTier`, `store`, `continuationToken` | fields | |
| `model` (in a body) | form `model` | The path decides the model. |

### Parts

Schema `Part`. A part carries exactly one of `text`, `inlineData`, `fileData`, `functionCall` and `functionResponse`; any other part is kept whole.

| Wire part | KPP | Notes |
|---|---|---|
| `text` | `Block::Text` | `thought: false` → form `thought`. |
| `text` with `thought: true` | `Block::Reasoning` | Its text, and its `thoughtSignature` as opaque data. An empty thought part → form `thought_part`. |
| `thoughtSignature` on any other part | `Block::Reasoning` before the part's block | See [thought signatures](#thought-signatures). |
| `inlineData` `image/*` | `Block::Image`, base64 | `displayName` → fields `inlineData.*`. |
| `inlineData` `audio/*` / `video/*` | `Block::Audio` / `Block::Video`, base64 | The MIME type as sent (`audio/mp3`, `audio/mpeg`, `video/mp4`, …). |
| `inlineData` of another type | `Block::Document`, base64 | |
| `fileData` with a Files API URI (`files/…` or `https://generativelanguage.googleapis.com/v1beta/files/…`) | image or document, `Source::FileId` | Usable only at Gemini ([files](cross-format.md#files)). `mimeType` → form `mime_type`. |
| `fileData` with another URI | image, audio, video or document, `Source::Url` | By the `image/*`, `audio/*` or `video/*` of `mimeType`. A YouTube URL without one is a video (`requests/video-youtube.json`). |
| `functionCall` | `Block::ToolCall` | See [function calls](#function-calls). |
| `functionResponse` | `Block::ToolResult` | See [function calls](#function-calls). |
| `executableCode`, `codeExecutionResult`, `toolCall`, `toolResponse`, unknown parts, parts with two data fields | `Block::Unknown` (raw) | The part keeps its own `thoughtSignature`. Refused across dialects. |
| `videoMetadata`, `mediaResolution`, `partMetadata`, `mediaProcessing` and other part keys | block fields | Clipping and frame rate change what the model sees, so other dialects refuse them; no other format has them. |

### Function calls

| Wire field | KPP | Notes |
|---|---|---|
| `functionCall.id` | `ToolCall.id` | Absent → `kaeso_fc_<seed>_<n>`, form `no_id`. The seed is the reply's `responseId`, or the content and part index in a request. Ids with this prefix are left out again for Gemini. |
| `functionCall.name`, `.args` | `name`, `arguments` (raw) | `args` absent → `{}`, form `no_args`. |
| `functionResponse.id` | `ToolResult.call_id` | Absent → the earliest open call of the same name, form `no_id`. |
| `functionResponse.name` | — | Derived from the call; a different name → form `name`. |
| `functionResponse.response` `{"output": "<text>"}` | one text block | The documented output convention (`FunctionResponse.response`). |
| `functionResponse.response` `{"error": "<text>"}` | one text block, `is_error` | The documented error convention. |
| `functionResponse.response`, any other object | one text block with the compact JSON | form `response: "json"`: Gemini gets the object back; other dialects read the JSON as text. |
| `functionResponse.parts[]` | image and document blocks after the text | Audio, video and unknown parts stay raw: function responses take images and documents only. |
| `functionResponse.willContinue`, `.scheduling` | fields `functionResponse.*` | |

### Requests from other dialects

- **System messages** after the first turn join the next user turn, as for [Anthropic](cross-format.md#system-messages); leading ones join `systemInstruction`.
- **Tool results** of consecutive user messages, as Chat Completions sends them, go into one content, so parallel calls are answered as `FC1, FC2, FR1, FR2`; the thought signatures guide answers interleaved calls and results with a 400.
- **Function responses** take their name from the call with their id. A result whose call isn't in the request is `InvalidRequest`. Text becomes `{"output": …}` and an error `{"error": …}`, its text blocks joined; images and documents go into `functionResponse.parts`, which Gemini 3 models take.
- **Ids** of other dialects are sent as `functionCall.id` and `functionResponse.id`.
- **Arguments** that aren't an object become one, as for Anthropic ([tool call arguments](cross-format.md#tool-call-arguments)).
- **Image URLs** get `fileData.mimeType` from the URL's extension (`.png`, `.jpg`, `.jpeg`, `.webp`, `.gif`, `.heic`, `.heif`; documents also `.pdf`, `.txt`), as the file input guide asks; an image URL without one is `Unsupported`. Text documents are sent as base64 `inlineData`.
- **Audio and video URLs** get it the same way, from the audio and video guides' supported formats (`.wav`, `.mp3`, `.aiff`, `.aac`, `.ogg`, `.flac`, `.m4a`, `.opus`, `.webm`; `.mp4`, `.mpeg`, `.mpg`, `.mov`, `.avi`, `.flv`, `.webm`, `.wmv`, `.3gp`). A YouTube URL goes without one; another URL without a known extension is `Unsupported`. Base64 audio and video go as `inlineData` with their media type. Audio and video in a tool result are `Unsupported`.
- **Refused** with `Unsupported`: refusal blocks, which Gemini has no part for (as for Anthropic), a document's title, context or citations, more than five stop sequences (`GenerationConfig.stopSequences`), `parallel_tool_calls: false`, an output schema with a description or `strict: false`, and an effort or thinking mode Gemini has no setting for.
- **Strict tools.** A function tool with `strict: true` and an automatic `tool_choice` sends `mode: VALIDATED`, which checks calls against their schemas.
- **Hosted web search** is [left out](cross-format.md#tools-left-out).
- **Cache hints** are dropped: Gemini caches implicitly.
- **Audio replies** from other dialects reach Gemini apps as `inlineData` with their media type; a transcript has no place there and is left out.
- **`user`** is dropped: Gemini has no end-user field.

### Requests from google-genai

`google-genai` 2.32.0 spells some keys in snake case, and the API takes them because proto JSON accepts every field's original name: `mime_type` and `file_uri` in `inlineData` and `fileData`, `include_thoughts`, `thinking_budget` and `thinking_level` in `thinkingConfig`, and `parameters_json_schema` and `response_json_schema` in `functionDeclarations`. `gemini::sdk` reads them under their camel-case names and sets the form marker `sdk_names`. A request that goes to Gemini gets them back as the app spelled them.

The SDK also sends `topK` as a float, such as `20.0`. KPP reads a whole float as a whole number, with form `top_k_float`, and writes it back as a float for Gemini; a fraction is `InvalidRequest`. The fixture `requests/sdk-tools-snake-case.json` comes from `google-genai` 2.32.0.

### Thinking

| Wire field | KPP | Notes |
|---|---|---|
| `thinkingLevel` `MINIMAL` / `LOW` / `MEDIUM` / `HIGH` | `reasoning.effort` | Other spellings → form `level`; unknown values → `ReasoningEffort::Other`. |
| `thinkingBudget: 0` | `reasoning.mode = Disabled` | |
| `thinkingBudget: -1` | `reasoning.mode = Adaptive` | Dynamic thinking. |
| `thinkingBudget` above 0 | `mode = Enabled`, `budget_tokens` | Other negative values → reasoning field `thinkingBudget`. |
| `includeThoughts: true` | `reasoning.summary = Auto` | `false` → form `include_thoughts`. |

From other dialects: the efforts `minimal`, `low`, `medium` and `high` become `thinkingLevel`, and `xhigh` and `max` become `HIGH`. A budget becomes `thinkingBudget`, which Gemini 3 maps to the nearest level. The effort `none` and disabled thinking become `thinkingBudget: 0`, and adaptive or enabled thinking without either becomes `-1`. `includeThoughts: true` is sent when the app asked for a reasoning summary, or for Anthropic thinking that isn't `omitted`. [Reasoning](reasoning.md#effort-and-budget) has the rules.

### Thought signatures

Gemini signs its thinking and puts the signature on a part: on a function call, on text, on an image, or on an empty text part at the end of a stream ([thought signatures](https://aistudio.google.com/docs/thought-signatures)).

- **Decoding.** A `thoughtSignature` becomes `Opaque { kind: GeminiThoughtSignature }`. On a `thought: true` part it stays in that part's reasoning block; on any other known part it becomes a reasoning block of its own, with the origin `gemini`, no text and only the signature, right before the part's block.
- **Encoding.** Such a signature-only block puts its signature on the next part Kaeso writes, or on an empty text part when none follows. Signatures go only to Gemini; reasoning of other dialects is dropped ([reasoning](reasoning.md)).
- **Across apps.** Anthropic and Responses apps get the block in a [reasoning blob](reasoning.md#reasoning-blobs). Chat Completions apps get nothing, and the gateway keeps the signature by tool call through `continuity::reasoning_for_tool_calls` and `ReasoningCollector`, as it keeps Anthropic signatures ([reasoning kept by the gateway](reasoning.md#reasoning-kept-by-the-gateway)).
- **Dummy signature.** A foreign function call that is the first call of a model content in the current turn, after the last user content that holds more than function responses, gets the documented `"skip_thought_signature_validator"` when no signature comes before it. `EgressCtx::adapted` names it as `thought signature of tool call <id>`. Gemini's own calls are sent as received.

## Replies

Schema `GenerateContentResponse`.

| Wire field | KPP | Notes |
|---|---|---|
| `responseId` | `id` | Absent → form `no_response_id`. |
| `modelVersion` | `model` | Absent → form `no_model_version`. |
| `candidates[0].content.parts` | `blocks` | As in requests. Image parts become `Block::Image`. |
| More than one candidate | `Unsupported` | |
| `candidates[0].finishReason` | `stop` | See [finish reasons](#finish-reasons). |
| `candidates[0]` other keys (`index`, `finishMessage`, `safetyRatings`, `citationMetadata`, `groundingMetadata`, `urlContextMetadata`, `avgLogprobs`, `logprobsResult`, `tokenCount`) | fields `candidate.*` | `citationMetadata` indexes the candidate's whole text, which KPP citations, bound to one block, can't express. |
| `candidates[0].content.role` | — | Another value or none → form `no_role`. |
| No `candidates` | no blocks, `stop = ContentFilter` | form `no_candidates`; `promptFeedback` stays a field. |
| `usageMetadata` | `usage` | See [usage](#usage). |
| `promptFeedback`, `modelStatus` and other keys | fields | |

Replies to Gemini apps from other dialects get `role: "model"`, `index: 0`, and the reply's id and model. Reasoning of other dialects is left out, as for Chat Completions apps ([replies](cross-format.md#replies)): a thought part can't carry a [reasoning blob](reasoning.md#reasoning-blobs), because the `google-genai` SDK reads `thoughtSignature` as base64.

### Finish reasons

| `finishReason` | KPP | Back to Gemini |
|---|---|---|
| `STOP` | `EndTurn`, or `ToolUse` when the reply calls functions | `STOP`, also for `StopSequence` |
| `MAX_TOKENS` | `MaxTokens` | `MAX_TOKENS` |
| `SAFETY`, `BLOCKLIST`, `PROHIBITED_CONTENT`, `SPII`, `RECITATION`, `IMAGE_SAFETY`, `IMAGE_PROHIBITED_CONTENT`, `IMAGE_RECITATION` | `ContentFilter`, the value in form `finish` | `SAFETY` for `ContentFilter` and `Refusal` |
| Any other value, such as `MALFORMED_FUNCTION_CALL` | `Other(value)` | the value when the discovery document lists it, else `OTHER` |

### Usage

`promptTokenCount` includes the cached prompt tokens but not the tool-use ones, so `input_tokens = promptTokenCount + toolUsePromptTokenCount − cachedContentTokenCount`, `cache_read_tokens = cachedContentTokenCount`, `output_tokens = candidatesTokenCount + thoughtsTokenCount` and `reasoning_tokens = thoughtsTokenCount` ([usage](cross-format.md#usage)). `promptTokensDetails` counts the cached tokens too, so `input_image_tokens` and `input_audio_tokens` are its `IMAGE` and `AUDIO` counts minus those of `cacheTokensDetails`, and `cache_read_image_tokens` is the `IMAGE` count of `cacheTokensDetails`. `Usage::raw` keeps the whole `usageMetadata`. Usage of other dialects is written as `promptTokenCount`, `candidatesTokenCount`, `totalTokenCount`, and `cachedContentTokenCount` and `thoughtsTokenCount` when they aren't 0.

## Token counts

Schemas `CountTokensRequest` and `CountTokensResponse`.

| Wire field | KPP | Notes |
|---|---|---|
| `contents` | `messages` | |
| `generateContentRequest` | the whole request | form `count: "wrapped"`; its `model` → form `model`. Sending both is `InvalidRequest`. |
| `totalTokens` | `input_tokens` | |
| `cachedContentTokenCount`, `promptTokensDetails`, `cacheTokensDetails` | fields | |

A count from another dialect with a system prompt or tools goes as `generateContentRequest` with `model: "models/<id>"` and no `generationConfig`; otherwise as `contents`.

## Streams

`streamGenerateContent?alt=sse` sends every chunk as a whole `GenerateContentResponse` in a `data:` line. Function calls arrive whole: the documentation has no streamed function call arguments.

- **Audio.** A part that holds only `inlineData` audio, as speech models stream 16-bit PCM pieces (`streams/recorded-stream-speech.sse`), opens an audio block, and the next such parts of the same media type continue it as `Audio` deltas; other dialects get the pieces in order.
- **Blocks.** Text parts continue the open text block and thought parts the open reasoning block. A function call is a whole tool call block: start, one argument delta, stop. A signature on a thought part is an `Opaque` delta in its block; on another part it is a signature-only reasoning block before it. Other parts are unknown blocks.
- **Usage and the end.** Every chunk carries `usageMetadata`; the one with `finishReason` becomes a `ResponseDelta` after the last block stops. A stream that ends without a `finishReason` ends with an error.
- **Same-dialect replay.** The first event of each chunk carries the form marker `chunk`: the chunk without its parts, its finish reason and its final usage. The encoder puts them back, so every chunk comes out as received.
- **Other dialects** get one chunk per text delta, one per function call, and a last chunk with an empty text part, the `finishReason` and `usageMetadata`.
- **Errors.** A `data:` line with an `error` object is an error event; the encoder writes errors the same way.
- **Keepalives.** A blank line ([keepalives](cross-format.md#keepalives)). The `google-genai` SDK reads every line that doesn't start with `data: ` as JSON, so a comment would break it.
- Comments, named events and frames that aren't JSON objects pass through as `Extra`.

## Errors

Google's error body `{"error": {"code", "message", "status", "details"}}` ([troubleshooting](https://aistudio.google.com/docs/troubleshooting)).

| `status` | KPP | Notes |
|---|---|---|
| `INVALID_ARGUMENT` | `InvalidRequest` | `Authentication` when `details[].reason` is `API_KEY_INVALID` (`errors/recorded-error-invalid-key.json`), `ContextTooLong` when the message says the input exceeds the maximum number of tokens. |
| `FAILED_PRECONDITION`, `OUT_OF_RANGE` | `InvalidRequest` | Such as image generation not available in the key's country. |
| `UNAUTHENTICATED` | `Authentication` | |
| `PERMISSION_DENIED` | `PermissionDenied` | |
| `NOT_FOUND` | `NotFound` | |
| `RESOURCE_EXHAUSTED` | `RateLimited` | `RetryInfo.retryDelay`, such as `"23s"`, → `retry_after`. |
| `UNAVAILABLE` | `Overloaded` | |
| `DEADLINE_EXCEEDED` | `Timeout` | |
| `INTERNAL` and the other codes | `ProviderError` | |
| No `status` | by HTTP status | |

`BadRequest.fieldViolations[0].field` → `param`. A Gemini error reaches Gemini apps as received. Errors of other dialects are written with the kind's code and status, a 429 or 503 with `RetryInfo` and `retry-after`: `RateLimited` and `Billing` 429 `RESOURCE_EXHAUSTED`, `Overloaded` 503 `UNAVAILABLE`, `Authentication` 401 `UNAUTHENTICATED`.

## Embeddings

The `gemini_embeddings` codec. Schemas `EmbedContentRequest`, `BatchEmbedContentsRequest`, `EmbedContentResponse` and `BatchEmbedContentsResponse`.

| Wire field | KPP | Notes |
|---|---|---|
| `content` with one text part | one `EmbedInput::Text` | Any other content → field `content` (or `requests[i].content`) and an empty input, refused across dialects. |
| `content.role` `user` | form `user_role:<i>` | `google-genai` sends it on every content (`requests/sdk-batch-user-role.json`). Gemini gets it back; other dialects get the input without it. Any other role keeps the content raw, refused across dialects. |
| `requests[]` | one input each | |
| `outputDimensionality`, `embedContentConfig.outputDimensionality` | `dimensions` | When the requests of a batch differ → fields `requests[i].outputDimensionality`. `embedContentConfig` → form `config_dimensions`. |
| `taskType`, `title`, other `embedContentConfig` keys | fields | |
| `model` in the body | — | `models/<path model>` is derived; another spelling → form `models`. |
| `embedding.values`, `embeddings[].values` | `data[].vector` (float) | `shape` → embedding fields. |
| `usageMetadata.promptTokenCount` | `usage.input_tokens` | `gemini-embedding-2` and `gemini-embedding-001` report it; `Usage::raw` keeps it. |

An OpenAI Embeddings request goes as `batchEmbedContents` with `model: "models/<id>"` and `outputDimensionality` on each request; token-id inputs are `Unsupported`. A reply goes back in the shape of the app's method, `embedding` for `embedContent` and `embeddings` for `batchEmbedContents`; base64 vectors of another dialect become floats. Embeddings never stream.

## Models

`GET {models}` and `GET {models}/{model}` (schemas `ListModelsResponse` and `Model`).

| Wire field | KPP | Notes |
|---|---|---|
| `name` `models/<id>` | `id` | Without the prefix → form `bare_name`. |
| `displayName` | `display_name` | |
| `inputTokenLimit`, `outputTokenLimit` | `context_window`, `max_output_tokens` | |
| `version`, `baseModelId`, `description`, `supportedGenerationMethods`, `thinking`, `temperature`, `maxTemperature`, `topP`, `topK` and other keys | fields | |
| `nextPageToken` | `has_more`, field `nextPageToken` | |
| Query `pageSize` | `limit` | |
| Query `pageToken` | `after_id` | A Gemini app treats the token as opaque, so Kaeso's own listings use the last id of the page before as the token. |

`dispatch::encode_models_request_with` and `encode_model_request_with` use the models path of `EgressCtx::gemini_profile`. A listing request sends `limit` as `pageSize` and `after_id` as `pageToken`; `before_id`, a lifecycle filter and other query parameters are `Unsupported`.

`dispatch::next_models_query(dialect, query, page)` gives the query for the page after a decoded one, or `None` after the last: for Gemini the `nextPageToken` the page gave, for the other dialects the page's last id. The gateway pages through Google's listing with it when a model check needs the whole list.

## Providers that use this format

| Provider | Profile | Key |
|---|---|---|
| Google AI Studio (`google`) | `GeminiProfile::GOOGLE_AI_STUDIO`, which the gateway hands to the encoder with `EgressCtx::with_gemini_profile` for every request, model check and listing | `x-goog-api-key` (`AuthStyle::XGoogApiKey`), never in the URL |
| Gemini Enterprise Agent Platform (`gemini-agent-platform`) | `GeminiProfile::AGENT_PLATFORM`: `/v1/publishers/google/models` for an express mode key, and for a service account key `with_models_path("/v1/projects/{project}/locations/{location}/publishers/google/models")` per request, from the key's `project_id` and the credential's location ([REST reference](https://docs.cloud.google.com/gemini-enterprise-agent-platform/reference/rest)). `role_on_every_content` is on, because the [token counting reference](https://docs.cloud.google.com/gemini-enterprise-agent-platform/reference/models/count-tokens) asks for a role even in single-turn requests, while the discovery document calls it optional | An express mode key as `x-goog-api-key`; a service account key's access token as `Authorization: Bearer` |

## Round trips

Every fixture decodes and encodes to an equal JSON value: requests with their path, replies, token counts, embeddings, model listings and errors. Every stream fixture decodes to the same events under every chunking and replays chunk for chunk. Recordings have no exceptions.

## Next steps

- [Crossing formats](cross-format.md): Look up what happens to data that moves from one API format to another, and why.
- [Reasoning across formats](reasoning.md): See how thinking levels, budgets and thought signatures cross formats.
- [Fixtures and recordings](fixtures.md): Record Gemini fixtures with `cargo xtask record gemini`.
