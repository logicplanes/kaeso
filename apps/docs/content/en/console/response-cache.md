---
title: Response cache
description: Answer repeated identical requests from a stored reply, at no provider cost, and choose how long replies are kept.
order: 9
---

The response cache answers a request the project has already sent, byte for byte, with the reply the provider gave the first time. The app gets it in milliseconds, and the provider charges nothing. It suits evaluation suites that rerun the same cases, support bots with recurring questions, and retries after a timeout.

The cache is off until you turn it on for a project. Once it's on, send the same request twice and the second answer carries `kaeso-cache: hit`:

```bash tab="curl"
curl -i https://kaeso.example.com/v1/messages \
  -H "x-api-key: $KAESO_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "max_tokens": 256,
    "messages": [{"role": "user", "content": "What are your opening hours?"}]
  }'
```

```python tab="Python"
import os
from anthropic import Anthropic

client = Anthropic(
    base_url="https://kaeso.example.com",
    api_key=os.environ["KAESO_API_KEY"],
)

for _ in range(2):
    raw = client.messages.with_raw_response.create(
        model="claude-haiku-4-5",
        max_tokens=256,
        messages=[{"role": "user", "content": "What are your opening hours?"}],
    )
    print(raw.headers.get("kaeso-cache"))
```

```ts tab="TypeScript"
import Anthropic from "@anthropic-ai/sdk";

const client = new Anthropic({
  baseURL: "https://kaeso.example.com",
  apiKey: process.env.KAESO_API_KEY,
});

for (let i = 0; i < 2; i++) {
  const { response } = await client.messages
    .create({
      model: "claude-haiku-4-5",
      max_tokens: 256,
      messages: [{ role: "user", content: "What are your opening hours?" }],
    })
    .withResponse();
  console.log(response.headers.get("kaeso-cache"));
}
```

## Turn the cache on

1. Open the project's **Settings**.
2. Under **Response cache**, turn on **Cache replies**.
3. In **Keep replies for**, choose how long a reply stays in the cache: from 5 minutes to 7 days. The default is 1 hour.

Changing the project's settings needs the **Edit project** right. A new length applies to replies stored from then on.

## What counts as the same request

Kaeso compares the request it would send to the provider after translating it, so the same request from an OpenAI-format app and an Anthropic-format app can share an entry. Any difference misses: the prompt, the model, `max_tokens`, `temperature`, the tools, or a header such as `anthropic-beta`.

- **Shared within the project.** Every key and member of the project shares its entries. No other project, and no other organization, ever reads them.
- **One reply per request.** A hit returns the reply the model gave the first time, even when `temperature` would make a new call answer differently.
- **Cached.** Successful non-streamed requests to `/v1/chat/completions`, `/v1/responses`, `/v1/messages` and `/v1/embeddings`.
- **Not cached.** Streamed requests, token counts, errors, and replies larger than the install allows (1 MiB unless the operator changed it).

## Control a single request

Apps decide per request with the standard `Cache-Control` request header:

| `Cache-Control` | What Kaeso does |
| --- | --- |
| None | Answers from the cache when it can, and stores new replies |
| `no-cache` | Asks the provider again and stores the new reply in place of the old one |
| `no-store` | Skips the cache: no lookup, nothing stored |

Every answer of a project with the cache on carries `kaeso-cache`: `hit`, `miss` or `bypass` (not cacheable, or skipped with `no-store`). A hit also carries `age`, the seconds since the reply was stored.

## Costs, limits and logs

A hit costs nothing: its cost is 0 in usage, in the request log and against spend budgets. Its tokens still count as the stored reply reports them, so usage shows what the app received. Request and token limits count hits like other requests ([limits and budgets](limits-and-budgets.md)).

The request log shows each request's result under **Response cache** ([request logs](request-logs.md)), and the Platform API's log list filters by it with `response_cache`. A hit has no time to first byte and no provider request id.

## Clear the cache

Under **Response cache**, select **Clear cache**. Stored replies stop being served at once, on every gateway instance. Turning **Cache replies** off clears the cache too.

## Allow or forbid it for the organization

An organization admin decides whether projects may use the cache at all. Open **Organization settings**, then **Logging**, and turn **Allow the response cache** off or on. Off stops the cache in every project at once and clears their entries. Turning it back on leaves each project's own switch as it was. This needs the **Edit organization** right.

Operators can turn the cache off for the whole install, or tune it, with [the response cache settings](../self-hosting/configuration.md#response-cache).

> [!NOTE]
> Kaeso keeps cached replies in Valkey, encrypted with the install's master key. The keys of the entries are derived from the requests with a keyed hash, so Valkey never holds a prompt in readable form.

## Next steps

- [Request logs](request-logs.md): See which requests the cache answered.
- [Limits and budgets](limits-and-budgets.md): Cap what a project may spend.
- [Model API](../api/model-api.md#response-headers): Read every header Kaeso adds to its answers.
