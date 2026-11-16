# Fixtures: Amazon Bedrock (`amazon-bedrock`) on `openai_chat`

Amazon Bedrock's OpenAI-compatible Chat Completions on its runtime endpoint, `POST https://bedrock-runtime.{region}.amazonaws.com/openai/v1/chat/completions`, with a Bedrock API key as `Authorization: Bearer`. Each key calls one region, so Kaeso's base URL is the runtime host of the key's region and `/openai/v1` takes the place of `/v1`. Model ids name their maker and often a version, such as `openai.gpt-oss-120b-1:0`, and inference profiles prefix a geography, such as `us.openai.gpt-6-sol`. A model whose catalogue entry names Anthropic Messages goes to `/anthropic/v1/messages` instead (`anthropic_messages/providers/amazon-bedrock`).

No key was available. AWS publishes no reply of this route, so there are requests and errors only: the requests are the examples of AWS's Chat Completions guide, and the errors were answered live in `us-east-1` with made-up keys (request ids are only in the headers, which aren't kept).

## Sources (read 2026-11-14)

| Source | Address |
|---|---|
| Chat Completions guide (the runtime and mantle base URLs, the examples, guardrails, no `GET /models` on runtime) | https://docs.aws.amazon.com/bedrock/latest/userguide/inference-chat-completions.html |
| Endpoints (what bedrock-runtime and bedrock-mantle each serve) | https://docs.aws.amazon.com/bedrock/latest/userguide/endpoints.html |
| API keys (long-term and short-term keys, `Authorization: Bearer`) | https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys.html |
| Which models take Chat Completions | https://docs.aws.amazon.com/bedrock/latest/userguide/models-api-compatibility.html |

## Files

| File | What it covers | Provenance |
|---|---|---|
| `requests/greeting-doc.json` | the guide's runtime request to `openai.gpt-oss-120b-1:0` | doc |
| `requests/system-stream-doc.json` | a `system` message and `stream: true`, from the guide's mantle examples; the body is the same on runtime | doc |
| `requests/guardrail-config-doc.json` | Bedrock's own `amazon-bedrock-guardrailConfig` body field, from the guide's guardrail example | doc |
| `requests/invalid-api-key.json`, `errors/recorded-invalid-api-key.json` | 401 `permission_denied_error` / `access_denied`, "Invalid API Key format: Must start with pre-defined prefix", for a key in neither of Bedrock's formats | live, with a made-up key |
| `requests/long-term-key-format.json`, `errors/recorded-long-term-key-format.json` | 401, "Invalid API Key format: Delimiter ':' not found", for an `ABSK` key whose decoded text has no `:` | live, with a made-up key |
| `requests/short-term-key-format.json`, `errors/recorded-short-term-key-format.json` | 401, "Missing required parameters in the API Key", for a `bedrock-api-key-` key that isn't a signed request | live, with a made-up key |
| `requests/refused-api-key.json`, `errors/recorded-refused-api-key.json` | 401, "Authentication failed: Please make sure your API Key is valid.", for a well-formed `ABSK` key Bedrock doesn't know | live, with a made-up key |
| `requests/missing-api-key.json`, `errors/recorded-missing-api-key.json` | 401, "Authorization header is missing" | live |
| `errors/recorded-native-invalid-api-key.json` | 403 `{"Message": …}` with `x-amzn-ErrorType: AccessDeniedException`, the shape bedrock-runtime answers its own operations with (recorded on `POST /model/{modelId}/converse`), without OpenAI's `error` object | live, with a made-up key |
| `errors/built-unrecognized-client.json` | 403 `{"Message": …}`, "The security token included in the request is invalid.", what AWS answers a SigV4 request whose access key it doesn't know (`UnrecognizedClientException`) | built from the native shape above and AWS's common errors (https://docs.aws.amazon.com/bedrock/latest/APIReference/CommonErrors.html) |

## Where Amazon Bedrock differs from OpenAI Chat Completions

| Difference | How KPP carries it | Source |
|---|---|---|
| the API sits at `/openai/v1` on a host per region, `bedrock-runtime.{region}.amazonaws.com` | the key's region picks the host and `/openai/v1` is the API root (`EgressCtx::with_api_root`) | Chat Completions guide |
| bedrock-runtime has no `GET /openai/v1/models`; it answers 404 `<UnknownOperationException/>` | a model check is inconclusive and sends nothing | Chat Completions guide; live |
| a refused key is a 401 `permission_denied_error` with `code: "access_denied"`, and the native operations answer 403 `{"Message": …}` | `KppError` of kind `Authentication` from the 401; the message of a native body is read from `Message`, and the gateway reads the three refusals above as a refused key | live |
| `amazon-bedrock-guardrailConfig` in the body and the `X-Amzn-Bedrock-Guardrail…` headers apply a guardrail | the body field is a request `Extra`, sent as written and refused when the request goes to another dialect | Chat Completions guide |
| closed-weight GPT models take only an inference profile id, such as `us.openai.gpt-6-sol`, on bedrock-runtime | KPMR names Chat Completions only for the profile ids; the foundation ids are refused before sending | Responses guide (https://docs.aws.amazon.com/bedrock/latest/userguide/inference-responses-api.html) |
