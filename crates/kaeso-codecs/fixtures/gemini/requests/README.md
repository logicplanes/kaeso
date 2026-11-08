# `fixtures/gemini/requests`

A `<name>.meta.json` names the model or the method of a request when the round-trip tests and `cargo xtask record` need another than `gemini-3.5-flash` and `generateContent`.

| file | what it covers | provenance |
|---|---|---|
| `text-simple.json` | one user turn | schema: GenerateContentRequest (discovery v1beta revision 20261106) |
| `system-instruction.json` | `systemInstruction`, temperature, `topP`, `topK`, `maxOutputTokens`, `stopSequences`, `seed`, `candidateCount: 1` | schema: GenerateContentRequest, GenerationConfig (discovery v1beta revision 20261106) |
| `multi-turn.json` | user, model and user contents; `thinkingLevel: MINIMAL` | schema: Content, ThinkingConfig (discovery v1beta revision 20261106) |
| `role-assistant.json` | a content with the role `assistant`, which Gemini answered | schema: Content (discovery v1beta revision 20261106); recorded |
| `thinking-include-thoughts.json` | `includeThoughts` with `thinkingLevel: LOW` | doc: thinking, thought summaries |
| `thinking-budget.json` | `thinkingBudget: 1024` with `includeThoughts` | doc: thinking, thinking budgets |
| `tools-single.json` | one function declaration with OpenAPI `parameters`, `mode: AUTO` | doc: function calling |
| `tools-parallel.json` | `parametersJsonSchema`, parallel calls with thoughts | doc: function calling, parallel function calling |
| `tools-forced.json` | `mode: ANY` with one `allowedFunctionNames` entry | schema: FunctionCallingConfig (discovery v1beta revision 20261106) |
| `tool-result-signature.json` | the model content of `responses/recorded-tools-parallel.json` sent back with its thought signature, and two function responses | doc: thought signatures, parallel function calling |
| `tool-result-dummy-signature.json` | a function call from another model with the documented dummy signature and an id in Anthropic's pattern | doc: thought signatures, FAQ |
| `tool-result-media.json` | a function response with an image part and a `$ref` to it | doc: function calling, multimodal function responses |
| `history-without-ids.json` | function calls and responses without ids, matched by name; a response that is a JSON object | schema: FunctionCall, FunctionResponse (discovery v1beta revision 20261106) |
| `structured-json-schema.json` | `responseMimeType: application/json` with `responseJsonSchema` | doc: structured output |
| `structured-response-schema.json` | `responseSchema` (OpenAPI subset) with `propertyOrdering` | doc: structured output |
| `image-input.json` | an 8×8 PNG as `inlineData`, `mediaResolution` | doc: image understanding |
| `image-output.json` | `responseModalities: [TEXT, IMAGE]` for `gemini-3.1-flash-image` | doc: image generation |
| `max-tokens.json` | `maxOutputTokens: 16` | schema: GenerationConfig (discovery v1beta revision 20261106) |
| `safety-settings.json` | `safetySettings`, `serviceTier` | schema: SafetySetting (discovery v1beta revision 20261106) |
| `stream-text.json`, `stream-thinking.json`, `stream-thinking-high.json`, `stream-tools.json` | `streamGenerateContent` with text, thoughts and parallel calls | doc: text generation, streaming |
| `error-invalid-argument.json` | an unknown `thinkingLevel` | schema: ThinkingConfig (discovery v1beta revision 20261106) |
| `error-invalid-key.json` | sent with a made-up key | schema: GenerateContentRequest (discovery v1beta revision 20261106) |
| `error-unknown-model.json` | a model that doesn't exist | schema: GenerateContentRequest (discovery v1beta revision 20261106) |
| `sdk-tools-snake-case.json` | a function declaration and `thinkingLevel` as google-genai 2.32.0 writes them: `parameters_json_schema` and `thinking_level` in snake case | sdk: google-genai 2.32.0, `models.generate_content` with a `FunctionDeclaration(parameters_json_schema=…)` and `ThinkingConfig(thinking_level=LOW)` |
| `unknown-parts.json` | audio and video parts, `executableCode`, `codeExecutionResult`, `partMetadata`, `videoMetadata`, a Files API URI, `googleSearch` and `codeExecution` tools, `cachedContent`, `labels`; MADE-UP part `xFuturePart` and field `xFutureField` | schema: Part, Tool (discovery v1beta revision 20261106); invented on purpose (unknown-* rule) |
| `count_tokens/contents.json` | `countTokens` with `contents` | schema: CountTokensRequest (discovery v1beta revision 20261106) |
| `count_tokens/generate-content-request.json` | `countTokens` with `generateContentRequest`, a system instruction and tools | schema: CountTokensRequest (discovery v1beta revision 20261106) |
