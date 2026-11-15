# anthropic_messages / providers / amazon-bedrock (Amazon Bedrock)

Amazon Bedrock's Anthropic Messages route on its runtime endpoint, `POST https://bedrock-runtime.{region}.amazonaws.com/anthropic/v1/messages`, with the Bedrock API key as `x-api-key` and `anthropic-version: 2023-06-01`, the model in the body as with Anthropic (an inference profile id such as `us.anthropic.claude-sonnet-5`, or a foundation model id). A Claude model sends in this format when its catalogue entry names Messages, which KPMR sets from AWS's API compatibility table; every other model goes to `/openai/v1/chat/completions` (`openai_chat/providers/amazon-bedrock`).

No key was available. AWS publishes no reply of this route, and its stream format isn't documented beyond the Anthropic SDK working against it, so there are requests and errors only: the request is the example of AWS's Messages guide, and the errors were answered live in `us-east-1` with made-up keys (request ids are only in the headers, which aren't kept).

| file | what it covers | provenance |
|---|---|---|
| `requests/explain-doc.json` | the guide's `curl` request with an API key | doc (https://docs.aws.amazon.com/bedrock/latest/userguide/inference-messages-api.html, 2026-11-14) |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 403 `permission_error`, "Invalid API Key format: Must start with pre-defined prefix" | live, with a made-up key |
| `requests/long-term-key-format.json`, `errors/recorded-long-term-key-format.json` | 403 `permission_error`, "Invalid API Key format: Delimiter ':' not found" | live, with a made-up `ABSK` key |
| `requests/short-term-key-format.json`, `errors/recorded-short-term-key-format.json` | 403 `permission_error`, "Missing required parameters in the API Key" | live, with a made-up `bedrock-api-key-` key |
| `requests/refused-api-key.json`, `errors/recorded-refused-api-key.json` | 403 `permission_error`, "Authentication failed: Please make sure your API Key is valid." | live, with a well-formed made-up `ABSK` key |
| `requests/missing-api-key.json`, `errors/recorded-missing-api-key.json` | 403 `permission_error`, "Authorization header is missing" | live |

A refused key is a 403 `permission_error` here, not Anthropic's 401 `authentication_error`; the codec reads it as `PermissionDenied`, and the gateway reads its message as a refused key (the provider's `refused_key_messages`). The features Anthropic lists as unavailable on bedrock-runtime (https://platform.claude.com/docs/en/build-with-claude/claude-on-amazon-bedrock-legacy) that would otherwise travel unnoticed, `mcp_servers` and `container`, are refused before sending; URL sources, the Files API and server tools are left to Bedrock's own 400. `POST /anthropic/v1/messages/count_tokens` is served only on bedrock-mantle (https://docs.aws.amazon.com/bedrock/latest/userguide/count-tokens.html), so token counts are refused.
