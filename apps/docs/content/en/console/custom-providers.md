---
title: Custom providers
description: Add a server of your own, such as a self-hosted vLLM or an internal proxy, as a provider of your organization.
order: 4
---

A custom provider is a server your organization adds itself, such as a self-hosted vLLM, Ollama, LM Studio, TGI or SGLang, or an internal proxy. Kaeso calls it in an API format it already translates. Apps reach its models like those of any [supported provider](../concepts/supported-providers.md), from any format and with the same rules.

## Add a custom provider

1. On **Providers**, open **Custom providers** and choose **Add custom provider**.
2. Give it a **Name**, such as `Research vLLM`. The **Id** follows the name until you change it. Model names start with it, such as `research-vllm/llama-3.3-70b`, and it never changes.
3. Enter the **Base URL**, such as `https://vllm.example.com`.
4. Choose the **API format**, and whether it **Also serves embeddings** or **Counts tokens** ([pick the format](#pick-the-format)).
5. Choose how it takes its **Key**: **Bearer token**, **x-api-key header** or **No key** for a server on your own network.

Then add a key for it under **Provider keys**, as for any provider, unless it takes none ([provider keys](provider-keys.md)). Last, add its models to a project ([add its models](#add-its-models)).

An id is lowercase letters and digits with single hyphens between them, at most 40 characters. It can't be the id of a built-in provider, or of a provider listed on [models.dev](https://models.dev), because Kaeso may add that provider later. Each organization has its own ids, so two organizations can both have a `research-vllm`.

Kaeso appends the format's path to the base URL, such as `/v1/chat/completions`, as it does for every provider. A path in the base URL stays: `https://proxy.example.com/llm` gets requests at `https://proxy.example.com/llm/v1/chat/completions`. A server whose API doesn't live under `/v1` can't be added yet.

## Pick the format

Choose the format your server answers in:

| Your server | API format |
| --- | --- |
| vLLM, Ollama, LM Studio, TGI, SGLang and most OpenAI-compatible servers | OpenAI Chat Completions |
| A server or proxy with OpenAI's `POST /v1/responses` | OpenAI Responses |
| A server or proxy with Anthropic's `POST /v1/messages` | Anthropic Messages |

- **Also serves embeddings:** turn it on when the server answers `POST /v1/embeddings` in OpenAI's format. Its embeddings models then serve `/v1/embeddings`.
- **Counts tokens:** only Responses and Messages have an endpoint for token counts, `/v1/responses/input_tokens` and `/v1/messages/count_tokens`. Turn it on only when your server answers it. Otherwise Kaeso answers count requests for its models with 400: "The model `…` cannot count tokens."

Kaeso speaks the plain format to a custom provider. It knows how each built-in provider differs from its format, such as fields Mistral refuses, but it can't know that of your server. A field your server doesn't take comes back as your server's own error. A field the format can't carry is refused as for any provider ([what Kaeso refuses](../concepts/format-translation.md#what-kaeso-refuses)).

## What Kaeso can't know

models.dev knows nothing about your server's models, so Kaeso doesn't either. When you add a model, you give everything it needs:

- **Prices.** Input and output prices, and cache prices if the server reports cached tokens. Kaeso charges every request at these prices and holds budgets to them ([limits and budgets](limits-and-budgets.md)). Set `0` for a server you don't pay per token.
- **Limits.** The context window and the output limit. Kaeso uses them for budgets and to fill in the output limit that Messages requires.
- **Capabilities.** Inputs, outputs, tool calling, reasoning, structured output, temperature and effort levels. Kaeso refuses a request that needs a capability the model doesn't have before it calls your server.

These values stay as you set them. Nothing updates them later.

## Add its models

On a project's **Models** page, choose **Add models**. Custom providers are in the provider list next to the built-in ones. Choose yours, and the dialog asks the server's model listing, `GET /v1/models`, with the project's key. It lists the ids the server names that the catalogue doesn't have yet.

Choose an id, or **Type a model id**, then enter the model's values. When the listing fails, the dialog says why and you type the id. Adding models needs the right **Manage models** in the project ([models and aliases](models-and-aliases.md)).

Apps then call the model by its name, with the provider's id in front:

```bash tab="curl"
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "model": "research-vllm/llama-3.3-70b",
    "messages": [{"role": "user", "content": "Write a haiku about gateways."}]
  }'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)
completion = client.chat.completions.create(
    model="research-vllm/llama-3.3-70b",
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(completion.choices[0].message.content)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});
const completion = await client.chat.completions.create({
  model: "research-vllm/llama-3.3-70b",
  messages: [{ role: "user", content: "Write a haiku about gateways." }],
});
console.log(completion.choices[0].message.content);
```

The reply's `kaeso-provider` header, the request logs and usage name the provider by its id. The install's metrics count all custom providers as `custom`.

## Addresses Kaeso reaches

Kaeso checks the address of every connection to a custom provider after DNS resolution, not only when you add it. It refuses private, loopback, link-local and cloud metadata addresses, such as `10.0.0.5`, `127.0.0.1` and `169.254.169.254`, unless the install allows the network. Plain `http` reaches only the networks the install allows. Kaeso follows no redirects from a custom provider and calls it without a proxy.

Adding a provider at such an address fails with "`base_url` points to an address this install does not allow." When an address stops being allowed later, its requests fail with 502: "The custom provider `…` is at an address this install does not allow; ask its operator." Operators allow networks with `KAESO_CUSTOM_PROVIDERS_ALLOWED_NETWORKS` ([configuration](../self-hosting/configuration.md#custom-providers)).

> [!WARNING]
> Whoever can manage custom providers decides where Kaeso sends the provider's keys and your apps' prompts. Give the right only to people you'd trust with those.

## Change or delete a custom provider

A provider's menu has **Edit** and **Delete**. Editing changes everything but the id. A new base URL, key style or format resets the [availability](models-and-aliases.md#availability) of its models to **Not checked**. Before a provider can take **No key**, delete its keys.

Deleting a provider also deletes its provider keys and its models in every project's catalogue, with their aliases. Requests for those models fail from then on. The dialog says how many keys and models in how many projects go, and the audit log keeps what was removed.

## When Kaeso adds a provider with the same id

A later release of Kaeso may add a built-in provider whose id your organization already uses. Inside your organization, your custom provider keeps the id: its keys and requests still go to your server, never to the new provider. The list marks it **Replaces** with the built-in provider's name.

To use the built-in provider instead, delete your custom provider, then add keys and models for the built-in one.

## Rights

| Action | Right |
| --- | --- |
| See custom providers and their base URLs | **View custom providers** |
| Add, edit and delete custom providers | **Manage custom providers**, with a recent sign-in |

Owners and admins hold both, and developers and auditors can view. Everyone in the organization sees a custom provider's name in provider lists, but not its base URL. If the install turns custom providers off, the organization keeps them, but requests to their models fail with "Custom providers are turned off on this install, so `…` cannot be called."

## Next steps

- [Provider keys](provider-keys.md): Add the key your custom provider takes.
- [Models and aliases](models-and-aliases.md): Add its models to a project with their prices and limits.
- [Configuration](../self-hosting/configuration.md#custom-providers): Allow the networks custom providers may reach.
