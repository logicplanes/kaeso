---
title: Models and aliases
description: Choose the models a project may call, adjust their values, and give them names of your own.
order: 5
---

A project's catalogue, on its **Models** page, lists the models its keys may call. Kaeso refuses a request for a model outside it. Changing models, their values and aliases needs the right **Manage models**.

## Add models

Choose **Add models**, then pick a provider from the searchable list next to the search field and search its models. Providers your organization has connected come first. The dialog remembers the provider until you reload the console or sign out. If the organization has no [provider key](provider-keys.md) for it, the dialog says that requests to its models fail until someone adds one.

The dialog lists models newest first, each with its context window and its input and output prices. When more than 50 models match, it shows the first 50 and says how many match, so search to narrow them down. Select one or more and add them. **Include deprecated** lists deprecated models too, and **Review values** shows a model's values before you add it.

Each model brings its prices, limits and capabilities from [KPMR](../concepts/kpmr.md), the Kaeso Provider & Model Registry. Two shortcuts cover the rest:

- **Add every current OpenAI model**, and the same button for each other provider, adds each of the provider's text and embedding models that isn't deprecated. It skips models already in the catalogue, and doesn't add models that appear later. The dialog says how many models it adds, and warns before you add 50 or more.
- **Add a custom model** adds a model by its id. If KPMR doesn't list it, enter its inputs and outputs, limits, prices and the API format Kaeso sends it. An id can't be `.` or `..` or have either between slashes, because a URL would drop that part.

For one of your organization's [custom providers](custom-providers.md#add-its-models), the dialog lists the model ids the provider's own listing names instead. It fills in the limits and capabilities it finds, and you enter the prices.

The install keeps KPMR's data current by itself, about every hour by default, and instance admins see its state under [System](instance-administration.md#check-the-system). A model that KPMR stops listing stays in the catalogue with its last known values, marked **No longer listed**. A model that KPMR marks deprecated is marked **Deprecated**.

## Find a model

The **Catalogue** tab groups models by provider. Narrow it to one provider, to a capability such as **Tool calling** or **Image input**, or with **Only overridden** to models with values changed in this project. On a phone, these filters sit under **Filters**. The sort menu orders each provider's models by **Model id**, **Input price** or **Newest release**.

## Availability

A model in the catalogue isn't always one the provider serves to the project's provider key. The provider may have retired it, or the key may have no access to it. The list marks such models **Unavailable**, and each model's sheet shows its **Availability**:

| Availability | Meaning |
| --- | --- |
| Available | The provider served it at the last check or request |
| Unavailable | The provider said it doesn't serve it to this key |
| Not checked | Nothing has said so yet, or a provider key of the organization changed since |

Choose **Check availability** to ask each model's provider, with the project's provider key. Kaeso marks a model unavailable only when the provider answers "not found" and its model listing confirms the model is missing. When Kaeso can't check a model, the model keeps its availability, and the report says why. For example, the project has no key for that provider, or the provider refused the key, even when only its model listing refused it.

Requests update availability too. When the provider answers a request with "not found" and its listing confirms it, the model becomes unavailable, and the app gets an [error that says so](../api/errors.md). When a request for an unavailable model succeeds, the model becomes available again. Each answer counts only for the provider key that got it.

An unavailable model stays in the catalogue, and Kaeso answers requests for it with that error without calling the provider. Requests never go to another model. The model API's list of models (`GET /v1/models`) leaves it out.

To fix it, give the provider key access to the model, or remove the model from the catalogue. Every five minutes, Kaeso lets one request through to the provider to ask again. Once the key has access, the model becomes available again by itself, or at once when you choose **Check availability**. The audit log records every check.

## Change a model's values

A model's sheet shows the **Values in use**, each with where it comes from: KPMR or an override in this project. **Edit overrides** changes them for this project only, and **Reset to KPMR** returns a value to the source. If someone else changed the model after its sheet loaded, saving says so and loads the current values, so you can make your change again.

| Value | What it is |
| --- | --- |
| Display name | The name the console shows |
| Inputs, outputs | Inputs: text, images, PDF, audio and video. Outputs: text, images, audio and embeddings |
| Context, output limit | The model's token limits |
| Input, output, cache read, cache write and one-hour cache write prices | US dollars per million tokens. Usage and budgets use the prices in force when a request is made. The cache write price is the five-minute one. Writes to Anthropic's one-hour cache cost the one-hour cache write price, which KPMR sets where the provider documents one, such as twice the input price for Anthropic's models. Without it, or when you override the input price, they cost twice the input price |
| Long-context prices | The prices KPMR lists for long prompts, such as **Above 272K: $20.00 / $75.00** for input and output. When a request's prompt, cached tokens included, is longer than that many tokens, all of the request's tokens cost these prices, output included, in usage and budgets alike. A price a tier leaves out is the model's own. Overriding the input, output, cache read or cache write price turns KPMR's long-context prices off, so your price holds for every prompt |
| Audio input and audio output prices | The prices KPMR lists for audio tokens, such as a transcribe model's audio input. |
| Image input, image output and image cache read prices; prices per unit | The prices KPMR lists for image tokens, and per image, megapixel, character, second, minute, hour or video, such as **$0.04 per image of output (low, 1K)** or **$0.143 per video of output (720p, 5 s long)**. Image tokens a provider reports cost the image prices, and images cost the prices per unit, in usage and budgets alike; a price the provider publishes only as an estimate says so. Overriding a token price without them turns KPMR's off. A custom image model, whose outputs are images and not text, needs its inputs, outputs and a price per unit or an output price |
| Sends as | The API format Kaeso calls the model with: Anthropic Messages, OpenAI Chat Completions, OpenAI Responses, OpenAI Embeddings, Gemini API, Gemini Embeddings or ElevenLabs. For Gemini Enterprise Agent Platform, a new custom model starts with the format of its family ([supported providers](../concepts/supported-providers.md#notes)) |
| Tool calling, reasoning, structured output | What the model supports |
| Temperature | Whether the model takes `temperature` |
| Forced tool choice | Whether the model takes `tool_choice` set to `required`, `any` or a named tool. Kaeso refuses such requests for a model that doesn't, before it calls the provider |
| Adaptive thinking, efforts | How the model's reasoning is turned on, and the efforts it takes |
| Thinking binding | Whether an Anthropic model checks that replayed thinking comes from the same conversation. For such a model, Kaeso asks Anthropic to drop thinking that apps in other formats send back changed, instead of failing the request |
| Timeout | How many seconds a request to this model may take. Empty uses the install's request timeout |
| Released | The model's release date |

## Remove a model

Choose **Remove from catalogue** in a model's sheet. Kaeso refuses requests for the model from then on, and model lists that name it lose it. Aliases that route only to it are deleted; routes that also use other models skip it.

## Create an alias

An alias is the project's own name for a model, such as `fast`. Apps send it as the model, and its route picks the model that answers, so you change models without touching the apps.

On the **Aliases** tab, choose **Create alias**. The route editor fills the page: name the alias, then pick a shape under **Start from** (one model, a fallback chain, a route by a header, long prompts to another model, or an A/B test), or choose the first step's model and add fallbacks, conditions and splits with **+** ([alias routes](../concepts/alias-routes.md)). A name has 1 to 200 characters, without spaces or `/`. An alias wins over a model with the same name, and the list marks it **Shadows a model**.

Conditions and splits branch downward, their branches side by side; a model's fallbacks hang under it on a dashed line, in the order they are tried, so the route stays as narrow as its widest row of steps. A step's menu, from its **⋯** or with Shift F10, puts a step before it, moves a model to be tried earlier or later, swaps a condition's **Then** and **Else**, or removes it. ⌘Z undoes, ⌘⇧Z redoes and ⌘S saves.

Requests take the route once you choose **Save**. **Audit log** is the [audit log](organizations-and-projects.md#read-the-audit-log) of this alias alone: who created, changed or deleted it, and when, filtered by actor and days. Choose an entry to see the alias before and after; **Restore this version** puts that route back in the editor, and it takes effect once you save it.

### Test a route

**Test** has two modes. **Real request** sends a message through the route as you, saved or not, with the headers you add. The canvas colors each step the request tried by its answer: green for a 2xx status, amber for a 4xx and red for a 5xx, with the status and, for a step that failed, its time. The panel lists the steps it took, then the reply. The request counts against your limits, is charged to the project and appears in the [request log](request-logs.md) with the source **Playground**; **Open in request log** shows it. **Preview** describes a request instead (its API format, input tokens, what it uses, its API key and its headers) and lights up the steps it would take, each model with its try; nothing is sent. **Draw again** draws anew the splits a request draws for itself.

A split by caller keeps every caller on one branch, so all your real requests take the same branch whatever its weight; choose an API key in **Preview** to see where that key's requests go. A split by header does the same for each value of its header, and draws anew for a request without it.

## How a request names a model

Kaeso looks up the model a request names in this order:

1. An alias of the project: `fast`.
2. A provider and a model: `anthropic/claude-haiku-4-5`.
3. A model id: `claude-haiku-4-5`.

If two providers have a model with the same id, use the `provider/model` form. Kaeso splits it at the first `/`, so a model id that contains `/` keeps it: `togetherai/Qwen/Qwen3.5-9B` names Together AI's `Qwen/Qwen3.5-9B`.

A model id can contain `/`, such as OpenRouter's `anthropic/claude-sonnet-4.5` or Groq's `openai/gpt-oss-20b`. Kaeso looks for a provider and a model before a model id, so a name that starts with another provider's id goes to that provider. In a project with OpenRouter's `openai/gpt-4o` and OpenAI's `gpt-4o`, `openai/gpt-4o` calls OpenAI. Put the provider in front to be sure: `openrouter/openai/gpt-4o` always calls OpenRouter's model, and `groq/openai/gpt-oss-20b` always calls Groq's.

## Next steps

- [Calling models](../get-started/calling-models.md): Call the models in your catalogue from an app.
- [Limits and budgets](limits-and-budgets.md): Cap the requests, tokens and spending of a project or a key.
- [Playground](playground.md): Compare models side by side before you choose one.
