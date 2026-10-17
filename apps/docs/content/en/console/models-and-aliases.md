---
title: Models and aliases
description: Choose the models a project may call, adjust their values and give them aliases.
order: 4
---

A project's **catalogue**, on its **Models** page, lists the models its keys may call. Kaeso refuses a request for a model outside it. Changing models, overrides and aliases needs the right **Manage models**.

## Add models

**Add models** searches the models of one provider. Select one or more and add them; **Include deprecated** lists deprecated models too, and **Review values** shows a model's values before you add it. Each model brings its prices, limits and capabilities from [models.dev](https://models.dev), an open database of models.

- **Add every current OpenAI model**, or Anthropic model, adds each of the provider's text and embedding models that isn't deprecated, and skips those already in the catalogue. Models that appear later are not added on their own.
- **Add a custom model** adds a model by its id. If models.dev doesn't list it, enter its inputs and outputs, limits, input and output prices and the API format Kaeso sends it.

The install refreshes the models.dev data every 24 hours by default; instance admins see its state under [**Model sources**](instance-administration.md#model-sources). A model that models.dev stops listing stays in the catalogue with its last known values, marked **No longer listed**. One that models.dev marks deprecated is marked **Deprecated**.

## Availability

A model in the catalogue is not always one the provider serves to the project's provider key: the provider may have retired it, or the key may have no access to it. The list marks such models **Unavailable**, and each model's sheet shows its **Availability**:

| Availability | Meaning |
| --- | --- |
| Available | The provider served it at the last check or request |
| Unavailable | The provider said it doesn't serve it to this key |
| Not checked | Nothing has said so yet, or a provider key of the organization changed since |

**Check availability** asks the provider of each model, with the project's provider key, and reports what it found. A model counts as unavailable only when the provider answers not found and its model listing confirms that the model is missing. A model that can't be checked keeps its availability, and the report says why: the project has no key for the provider, the provider refused the key or didn't answer, the provider's listing doesn't answer, or the key changed during the check.

Requests teach Kaeso too. When the provider answers a request with not found and its listing confirms it, the model becomes unavailable and the app gets an [error that says so](../api/errors.md); when a request for an unavailable model succeeds, it becomes available again. An answer counts only for the key that gave it.

An unavailable model stays in the catalogue, and requests for it still go to its provider; Kaeso never sends them to another model. The model API's model list (`GET /v1/models`) leaves it out. Give the provider key access to the model, or remove the model from the catalogue. Checks are recorded in the audit log as `catalogue_model.check`.

## A model's values

A model's sheet shows the **Values in use**, each with where it comes from: models.dev, Kaeso's own additions, or an override set in this project. **Edit overrides** changes them for this project only, and **Reset to models.dev** returns one to the source. If someone else changed the model after its sheet loaded, saving says so and loads the current values, so you can make your change again.

| Value | What it is |
| --- | --- |
| Display name | The name shown in the console |
| Inputs, outputs | Text, images, audio, video, embeddings |
| Context, output limit | The model's token limits |
| Input, output, cache read and cache write prices | US dollars per million tokens; usage and budgets use the prices in force when a request is made |
| Sends as | The API format Kaeso calls the model with: Anthropic Messages, OpenAI Chat Completions, OpenAI Responses or OpenAI Embeddings |
| Tool calling, reasoning, structured output | What the model supports |
| Accepts temperature | Whether the model takes `temperature` |
| Accepts a forced tool choice | Whether the model takes `tool_choice` `required`, `any` or a named tool; Kaeso refuses such requests for a model that doesn't, before calling the provider |
| Adaptive thinking, effort levels | How the model's reasoning is turned on, and the efforts it takes |
| Timeout | Seconds a request to this model may take; empty uses the install's request timeout |
| Released | The model's release date |

**Remove from catalogue** takes a model out. Requests for it are refused from then on, and aliases and model lists that name it lose it.

## Aliases

An **alias** is a name of the project's own for a model in its catalogue, such as `fast` for `claude-haiku-4-5`. Apps send the alias as the model, and you change the model behind it without touching the apps.

On the **Aliases** tab, **Create alias** asks for a name and a **Target**. A name has 1 to 200 characters, without spaces or `/`. An alias wins over a model with the same name, and the list marks it **Shadows a model**.

## How a request names a model

Kaeso looks up the model a request names in this order:

1. An alias of the project: `fast`.
2. A provider and model: `anthropic/claude-haiku-4-5`.
3. A model id: `claude-haiku-4-5`.

If two providers have a model with the same id, use the `provider/model` form.
