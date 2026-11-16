---
title: Guardrails
description: Have services of your own check requests before a model gets them, and the replies before apps do.
order: 4
---

A guardrail is an HTTP service of your organization that Kaeso asks before a request goes to a model and, if you choose, before the model's reply goes back. It can let the request through, block it or rewrite its text. [Built-in rules](#built-in-rules) do the most common checks inside Kaeso, without a service of yours. Kaeso knows which key, member and project sent each request, and the request log keeps every verdict.

You need `guardrails.view` to see **Organization settings** › **Guardrails** and `guardrails.manage` to change it.

## Add a guardrail

1. Open **Organization settings** › **Guardrails** and choose **Add guardrail**.
2. Enter a name and the address Kaeso posts to, such as `https://guardrail.example.com/check`. Apps see the name when the guardrail blocks them.
3. Choose what it checks: **Requests**, **Replies** or both. For replies, choose whether streamed replies go out unchecked (**Stream unchecked**) or wait for the check (**Hold and check**).
4. Set the timeout, 2000 milliseconds unless you change it, at most 10000.
5. Choose what happens when the guardrail can't answer: **Refuse the request** or **Let it through**.
6. Choose **Add guardrail** and copy the signing secret. It is shown once.

An address on a private network works only when the install allows its network in `guardrails.allowed_networks` ([configuration](../self-hosting/configuration.md#guardrails)). Plain `http` works only on those networks, and Kaeso never follows a redirect.

The row menu has **Edit**, **Test**, **New secret** and **Delete**. **New secret** replaces the signing secret at once. **Test** sends a sample prompt, or a prompt and a reply, the way a request would and shows the verdict; nothing of the sample is stored.

## Choose the projects

A guardrail checks nothing until a project uses it. Open the project's **Settings** and, under **Guardrails**, choose **Choose guardrails**. Tick the guardrails and put them in order: a request passes them from top to bottom, and each one sees the text the one before it left.

Only people with `guardrails.manage` in the organization choose them. A project's own admins see the list but can't take a guardrail off: it is the organization's policy.

## Built-in rules

Built-in rules find secrets and personal data in a request without a service of your own. Choose **Add guardrail**, then **Built-in rules**, name them and choose what each detector does: **Mask** replaces each value with a placeholder such as `[EMAIL_1]`, **Block** refuses the request with 400 and names what it found, never the value, and **Count only** lets it through and counts the values.

| Detector | Finds |
| --- | --- |
| Secrets and API keys | Keys of the providers Kaeso supports and Kaeso's own `sk-kso-…` keys, AWS access key ids, GitHub and GitLab tokens as `[API_KEY_1]`; private key PEM blocks as `[PRIVATE_KEY_1]`; JSON Web Tokens as `[TOKEN_1]` |
| Email addresses | Addresses such as `ada@example.com`, as `[EMAIL_1]` |
| Phone numbers | Numbers in E.164, such as `+4930123456`, as `[PHONE_1]` |
| IBANs | IBANs whose mod-97 check holds, as `[IBAN_1]` |
| Payment card numbers | 13 to 19 digits, also with spaces or hyphens, whose Luhn check holds, as `[CARD_1]` |
| IP addresses | IPv4 and IPv6 addresses, as `[IP_1]` |

The same value gets the same placeholder throughout a request, so the model can still tell two people apart. The rules read the text of the system prompt and the messages, tool results and the arguments of tool calls; media and tool definitions stay as they are.

Add up to 20 custom patterns: a name of lowercase letters, digits and `_`, and a regular expression of at most 500 characters in the syntax of Rust's `regex` crate, which matches in time linear in the text. A match of `order_id` becomes `[ORDER_ID_1]`.

- **Mask in the request log** stores placeholders in the [request log](request-logs.md), also for values the rules only count. The request then goes out unchanged while the log keeps no value.
- **Restore in the reply** puts the original values back where the model's reply names their placeholders, before the reply reaches the app. In a stream, Kaeso holds back at most the length of one placeholder, so one split across two pieces is still found. Streams get their text restored; whole replies also their tool call arguments.

**Try rules** checks a sample text before you save the rules; Kaeso stores nothing of it. Names, postal addresses and other free-form personal data need a language model: use [a service of your own](#add-a-guardrail) for them. The inspector shows how many values each kind found, never the values, and `kaeso_masked_values_total{detector, action}` counts them.

## What Kaeso checks

Guardrails check chat requests: OpenAI Chat Completions and Responses, Anthropic Messages, Gemini and the Playground. Embeddings, reranking, decisions, images, audio, video, realtime sessions and token counts pass unchecked.

The request phase runs once per request, after Kaeso has read it and knows the caller may use the model, and before the [response cache](response-cache.md) and the first step of an [alias route](../concepts/alias-routes.md). A blocked request is never answered from the cache, and every step of a route gets the checked text.

A caller over one of its request rates is refused before Kaeso asks the guardrail. A blocked request reaches no provider and costs nothing, but counts as a request in the log and against the request rates.

The reply phase runs on a whole reply before Kaeso writes it for the app, including a reply from the response cache. A streamed reply is checked only when the guardrail holds streams: Kaeso then reads the provider's whole stream, checks it and streams it to the app, so the app waits longer for the first token.

## The request

Kaeso posts JSON to the guardrail's address:

```json
{
  "id": "req_01KA2Z8M3C0Y6V1W9T4H5J7Q2E.request",
  "phase": "request",
  "request_id": "req_01KA2Z8M3C0Y6V1W9T4H5J7Q2E",
  "organization": "org_01K9ZB2M7VQ6CG3N8R4T5W1X2Y",
  "project": "proj_01K9ZB3D1F8H2J4K6M8N0P2Q4R",
  "key": "key_01K9ZB4E2G9J3K5M7P9R1T3V5X",
  "member": "usr_01K9ZB1A6C8E0G2J4L6N8Q0S2U",
  "service_account": null,
  "model": "claude-haiku-4-5",
  "format": "anthropic_messages",
  "source": "api",
  "messages": [
    { "role": "system", "content": [{ "type": "text", "text": "Answer in one sentence." }] },
    {
      "role": "user",
      "content": [
        { "type": "text", "text": "What is in this picture?" },
        { "type": "image", "media_type": "image/png", "size": 48213 }
      ]
    }
  ]
}
```

- `phase` is `request` or `reply`. A reply check adds `reply`, one `assistant` message with the reply's parts.
- `model` is the name the app sent, an alias included. `format` is the app's format, and `source` is `api`, `playground` or `test`; a test names no project.
- A part is `text`, `refusal`, `tool_call` (with `id`, `name` and `arguments` as JSON text), `tool_result` (with `call_id`, `is_error` and its own `content`), `json`, `reasoning` or `other`. Images, audio, video and documents are `image`, `audio`, `video` and `document` with their `media_type` and `size` in bytes, never their bytes.

## The answer

The guardrail answers with a JSON object and a 2xx status:

```json
{ "action": "allow" }
```

```json
{ "action": "block", "message": "Customer data can't go to a model.", "category": "pii" }
```

```json
{
  "action": "replace",
  "category": "pii",
  "messages": [
    { "role": "system", "content": [{ "type": "text", "text": "Answer in one sentence." }] },
    {
      "role": "user",
      "content": [
        { "type": "text", "text": "What is in this picture?" },
        { "type": "image", "media_type": "image/png", "size": 48213 }
      ]
    }
  ]
}
```

- `category` is optional on every action and is kept in the log, at most 64 characters.
- `message` is optional; Kaeso adds it to the app's error, at most 300 characters.
- `replace` sends back the messages Kaeso sent, with the same roles and parts in the same order. Kaeso takes their text, tool call arguments and tool results, and keeps media, ids and names as they were. For a reply, `messages` holds the one `assistant` message. A held stream can't be rewritten.

Kaeso ignores fields it doesn't know. An answer that isn't one of these, a status other than 2xx, a `replace` of another shape or no answer within the timeout counts as a failure: the guardrail's **When it fails** setting decides.

## Check the signature

Kaeso signs each request like a [Standard Webhooks](https://www.standardwebhooks.com) delivery. The headers `webhook-id`, `webhook-timestamp` and `webhook-signature` carry the delivery's id, the time in Unix seconds and `v1,` followed by the base64 HMAC-SHA256 of `id.timestamp.body`. The key is the signing secret without its `whsec_` prefix, decoded from base64. Any Standard Webhooks library checks it; refuse requests whose timestamp is more than a few minutes old.

## What apps see

A blocked request gets 400 in the app's own format, with the error kind for content filters. The message names the guardrail and adds its reason, as in "The guardrail `PII check` blocked this request: Customer data can't go to a model." A blocked reply says "blocked the model's reply" instead. Kaeso doesn't fall back to another step of a route, since every step would get the same request.

A guardrail that fails while it refuses requests on failure gets the app a 503, or 500 in the OpenAI formats: "The guardrail `PII check` could not check this request, so Kaeso refused it; try again shortly."

Every checked reply carries `kaeso-guardrails`, a structured-field list with one item per guardrail: its name, `phase`, `action`, `category`, `failure` and `dur` in milliseconds.

```http
kaeso-guardrails: "PII check";phase=request;action=replace;category="pii";dur=18
```

The [request log](request-logs.md) shows the same in the inspector under **Guardrails**, and the **Guardrail** filter finds requests a guardrail allowed, blocked or rewrote. `kaeso_guardrail_runs_total{phase, action}` and `kaeso_guardrail_duration_seconds{phase}` count them in the [metrics](../self-hosting/monitoring.md).

## Next steps

- [Request logs](request-logs.md): Find the requests a guardrail blocked or rewrote.
- [Request flow](../concepts/request-flow.md): See where guardrails run among the gateway's checks.
- [Errors](../api/errors.md): Read the errors apps get when a guardrail refuses a request.
