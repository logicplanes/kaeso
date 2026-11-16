# Fixtures: `bedrock_converse`

Amazon Bedrock's Converse API: `POST /model/{modelId}/converse` and `/model/{modelId}/converse-stream`. Kaeso only sends Converse, so there are no app requests here: the codec tests build their requests in code and compare the body Kaeso sends.

## Sources

- Converse: https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_Converse.html, read 2026-11-16.
- ConverseStream: https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ConverseStream.html, read 2026-11-16.
- Amazon Nova tool use (the `top_song` tool, its call and its `tooluse_…` id): https://docs.aws.amazon.com/nova/latest/userguide/tool-use-invocation.html, read 2026-11-16.
- The event stream encoding: https://smithy.io/2.0/aws/amazon-eventstream.html, read 2026-11-16.
- No fixture is recorded: they are built in the documented shapes.

A stream is stored as `streams/<name>.events.json`, a list of `{"event": "<:event-type>", "data": {…}}` or `{"exception": "<:exception-type>", "data": {…}}`. The tests and the mock provider encode each entry as one event stream message, since Bedrock sends binary frames, not text. Each stream is the same reply as the response of the same name. An error's `.meta.json` holds its status and the `x-amzn-errortype` header Bedrock names the error in.

## Files

| file | what it covers | provenance |
|---|---|---|
| `responses/built-nova-text.json` | a text reply | built from the Converse reference |
| `responses/built-nova-tool-use.json` | Nova's chain-of-thought text, then a tool call, `stopReason: tool_use` | built from the Nova tool use guide |
| `streams/built-nova-text.events.json` | the text reply as a stream | built from the ConverseStream reference |
| `streams/built-nova-tool-use.events.json` | the tool call reply as a stream, the arguments in two deltas | built from the ConverseStream reference |
| `streams/built-throttling-exception.events.json` | a `throttlingException` after the first delta | built from the ConverseStream reference |
| `errors/built-validation-too-long.json` | 400 `ValidationException`, a prompt beyond the model's context | built from the Converse reference |
| `errors/built-throttling.json` | 429 `ThrottlingException` | built from the Converse reference |
| `errors/built-access-denied.json` | 403 `AccessDeniedException` | built from the Converse reference |
