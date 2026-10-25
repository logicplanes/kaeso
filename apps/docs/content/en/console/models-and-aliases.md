---
title: Models and aliases
description: Choose the models a project may call, adjust their values, and give them names of your own.
order: 5
---

A project's catalogue, on its **Models** page, lists the models its keys may call. Kaeso refuses a request for a model outside it. Changing models, their values and aliases needs the right **Manage models**.

## Add models

Choose **Add models**, then pick a provider from the searchable list next to the search field and search its models. Providers your organization has connected come first. The dialog remembers the provider until you reload the console or sign out. If the organization has no [provider key](provider-keys.md) for it, the dialog says that requests to its models fail until someone adds one.

The dialog lists models newest first, each with its context window and its input and output prices. When more than 50 models match, it shows the first 50 and says how many match, so search to narrow them down. Select one or more and add them. **Include deprecated** lists deprecated models too, and **Review values** shows a model's values before you add it.

Each model brings its prices, limits and capabilities from [models.dev](https://models.dev), an open database of models. Two shortcuts cover the rest:

- **Add every current OpenAI model**, and the same button for each other provider, adds each of the provider's text and embedding models that isn't deprecated. It skips models already in the catalogue, and doesn't add models that appear later. The dialog says how many models it adds, and warns before you add 50 or more.
- **Add a custom model** adds a model by its id. If models.dev doesn't list it, enter its inputs and outputs, limits, prices and the API format Kaeso sends it. An id can't be `.` or `..` or have either between slashes, because a URL would drop that part.

For one of your organization's [custom providers](custom-providers.md#add-its-models), the dialog lists the model ids the provider's own listing names instead. It fills in the limits and capabilities it finds, and you enter the prices.

The install refreshes the models.dev data every 24 hours by default, and instance admins see its state under [Model sources](instance-administration.md#sync-model-data). A model that models.dev stops listing stays in the catalogue with its last known values, marked **No longer listed**. A model that models.dev marks deprecated is marked **Deprecated**.

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

A model's sheet shows the **Values in use**, each with where it comes from: models.dev, Kaeso's own additions, or an override in this project. **Edit overrides** changes them for this project only, and **Reset to models.dev** returns a value to the source. If someone else changed the model after its sheet loaded, saving says so and loads the current values, so you can make your change again.

| Value | What it is |
| --- | --- |
| Display name | The name the console shows |
| Inputs, outputs | Inputs: text, images, PDF, audio and video. Outputs: text, images, audio and embeddings |
| Context, output limit | The model's token limits |
| Input, output, cache read and cache write prices | US dollars per million tokens. Usage and budgets use the prices in force when a request is made. The cache write price is the five-minute one, as models.dev lists it. Writes to Anthropic's one-hour cache cost twice the input price, [Anthropic's rule](https://platform.claude.com/docs/en/about-claude/pricing), or the cache write price when the model has no input price |
| Sends as | The API format Kaeso calls the model with: Anthropic Messages, OpenAI Chat Completions, OpenAI Responses or OpenAI Embeddings |
| Tool calling, reasoning, structured output | What the model supports |
| Temperature | Whether the model takes `temperature` |
| Forced tool choice | Whether the model takes `tool_choice` set to `required`, `any` or a named tool. Kaeso refuses such requests for a model that doesn't, before it calls the provider |
| Adaptive thinking, efforts | How the model's reasoning is turned on, and the efforts it takes |
| Thinking binding | Whether an Anthropic model checks that replayed thinking comes from the same conversation. For such a model, Kaeso asks Anthropic to drop thinking that apps in other formats send back changed, instead of failing the request |
| Timeout | How many seconds a request to this model may take. Empty uses the install's request timeout |
| Released | The model's release date |

## Remove a model

Choose **Remove from catalogue** in a model's sheet. Kaeso refuses requests for the model from then on, and aliases and model lists that name it lose it.

## Create an alias

An alias is the project's own name for a model in its catalogue, such as `fast` for `claude-haiku-4-5`. Apps send the alias as the model, and you can change the model behind it without touching the apps.

On the **Aliases** tab, choose **Create alias** and enter a name and a **Target**. A name has 1 to 200 characters, without spaces or `/`. An alias wins over a model with the same name, and the list marks it **Shadows a model**.

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
