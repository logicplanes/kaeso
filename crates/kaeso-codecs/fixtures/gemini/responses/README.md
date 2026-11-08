# `fixtures/gemini/responses`

| file | what it covers | provenance |
|---|---|---|
| `recorded-*.json` | the live reply to the request fixture of the same name | RECORDED.md |
| `image-output.json` | text and an image part with a thought signature, image tokens in `candidatesTokensDetails` | schema: GenerateContentResponse, Part (discovery v1beta revision 20261106); the signature is made up |
| `prompt-blocked.json` | `promptFeedback.blockReason` with no candidates | schema: PromptFeedback (discovery v1beta revision 20261106) |
| `unknown-parts.json` | `executableCode` and `codeExecutionResult` parts, `MALFORMED_FUNCTION_CALL`, `finishMessage`, `safetyRatings`, `citationMetadata`, cached and tool-use token counts, `modelStatus`; MADE-UP keys `xFutureKey` and `xFutureField` | schema: Candidate, UsageMetadata (discovery v1beta revision 20261106); invented on purpose (unknown-* rule) |
| `count_tokens/recorded-*.json` | `countTokens` replies | RECORDED.md |
