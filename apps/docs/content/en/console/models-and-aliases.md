---
title: Models and aliases
description: Choose the models a project may call, adjust their values, and give them names of your own.
order: 4
---

A project's catalogue, on its **Models** page, lists the models its keys may call. Kaeso refuses a request for a model outside it. Changing models, their values and aliases needs the right **Manage models**.

## Add models

Choose **Add models** and search one provider's models. Select one or more and add them. **Include deprecated** lists deprecated models too, and **Review values** shows a model's values before you add it.

Each model brings its prices, limits and capabilities from [models.dev](https://models.dev), an open database of models. Two shortcuts cover the rest:

- **Add every current OpenAI model**, or Anthropic model, adds each of the provider's text and embedding models that isn't deprecated. It skips models already in the catalogue, and doesn't add models that appear later.
- **Add a custom model** adds a model by its id. If models.dev doesn't list it, enter its inputs and outputs, limits, prices and the API format Kaeso sends it.

The install refreshes the models.dev data every 24 hours by default, and instance admins see its state under [Model sources](instance-administration.md#sync-model-data). A model that models.dev stops listing stays in the catalogue with its last known values, marked **No longer listed**. A model that models.dev marks deprecated is marked **Deprecated**.

## Availability

A model in the catalogue isn't always one the provider serves to the project's provider key. The provider may have retired it, or the key may have no access to it. The list marks such models **Unavailable**, and each model's sheet shows its **Availability**:

| Availability | Meaning |
| --- | --- |
| Available | The provider served it at the last check or request |
| Unavailable | The provider said it doesn't serve it to this key |
| Not checked | Nothing has said so yet, or a provider key of the organization changed since |

Choose **Check availability** to ask each model's provider, with the project's provider key. Kaeso marks a model unavailable only when the provider answers "not found" and its model listing confirms the model is missing. A model that can't be checked keeps its availability, and the report says why, for example that the project has no key for that provider.

Requests update availability too. When the provider answers a request with "not found" and its listing confirms it, the model becomes unavailable, and the app gets an [error that says so](../api/errors.md). When a request for an unavailable model succeeds, the model becomes available again. Each answer counts only for the provider key that got it.

An unavailable model stays in the catalogue, and Kaeso still sends requests for it to its provider, never to another model. The model API's list of models (`GET /v1/models`) leaves it out. To fix it, give the provider key access to the model, or remove the model from the catalogue. The audit log records every check.

## Change a model's values

A model's sheet shows the **Values in use**, each with where it comes from: models.dev, Kaeso's own additions, or an override in this project. **Edit overrides** changes them for this project only, and **Reset to models.dev** returns a value to the source. If someone else changed the model after its sheet loaded, saving says so and loads the current values, so you can make your change again.

| Value | What it is |
| --- | --- |
| Display name | The name the console shows |
| Inputs, outputs | Text, images, audio, video, embeddings |
| Context, output limit | The model's token limits |
| Input, output, cache read and cache write prices | US dollars per million tokens. Usage and budgets use the prices in force when a request is made |
| Sends as | The API format Kaeso calls the model with: Anthropic Messages, OpenAI Chat Completions, OpenAI Responses or OpenAI Embeddings |
| Tool calling, reasoning, structured output | What the model supports |
| Accepts temperature | Whether the model takes `temperature` |
| Accepts a forced tool choice | Whether the model takes `tool_choice` set to `required`, `any` or a named tool. Kaeso refuses such requests for a model that doesn't, before it calls the provider |
| Adaptive thinking, effort levels | How the model's reasoning is turned on, and the efforts it takes |
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

A model id can contain `/`, such as OpenRouter's `anthropic/claude-sonnet-4.5` or Groq's `openai/gpt-oss-20b`. Because a provider and a model come before a model id, a name that starts with another provider's id goes to that provider: in a project with OpenRouter's `openai/gpt-4o` and OpenAI's `gpt-4o`, `openai/gpt-4o` calls OpenAI. Put the provider in front to be sure: `openrouter/openai/gpt-4o` always calls OpenRouter's model, and `groq/openai/gpt-oss-20b` always calls Groq's.

## Next steps

- [Calling models](../get-started/calling-models.md): Call the models in your catalogue from an app.
- [Limits and budgets](limits-and-budgets.md): Cap the requests, tokens and spending of a project or a key.
- [Playground](playground.md): Compare models side by side before you choose one.
