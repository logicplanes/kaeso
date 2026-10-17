---
title: Models and aliases
description: Choose the models a project may call, adjust their values and give them aliases.
order: 4
---

A project's **catalogue** lists the models its keys may call. A request for a model outside the catalogue is refused. The catalogue is on the project's **Models** page.

## Add models

Choose **Add models**, choose OpenAI or Anthropic, search, and select one or more models; **Include deprecated** lists deprecated ones too, and **Review values** shows a single model's values before you add it. Kaeso takes each model's prices, limits and capabilities from [models.dev](https://models.dev), an open database of models. **Add every current Anthropic model**, or OpenAI model, adds each of the provider's models that isn't deprecated and produces text or embeddings, and skips those already in the catalogue; models that appear later are not added by themselves.

**Add a custom model** adds a model by its id. If models.dev doesn't list it, enter its inputs and outputs, limits, prices and the API format it uses.

The install refreshes models.dev data every 24 hours by default. Instance admins see its state, and sync it now, under **Instance** › **Model sources**. A model that models.dev stops listing stays in the catalogue with its last known values, marked **No longer listed**.

## A model's values

A model's sheet shows the values in use and where each comes from: models.dev, Kaeso's own additions, or an override set in this project. **Edit overrides** changes them for this project only; **Reset to models.dev** returns one to the source. If someone else changed the model after its sheet loaded, saving or removing it says so and loads the current values, so you can make the change again.

| Value | What it does |
| --- | --- |
| Display name | The name shown in the console |
| Inputs, outputs | Text, images, audio, video, embeddings |
| Context, output limit | The model's token limits |
| Input, output, cache read and cache write prices | Per million tokens, in US dollars; usage and budgets are charged at the prices in force when the request is made |
| Sends as | The API format Kaeso uses to call the model: Anthropic Messages, OpenAI Chat Completions, OpenAI Responses or OpenAI Embeddings |
| Tool calling, reasoning, structured output | What the model supports |
| Accepts temperature | Whether the model takes `temperature` |
| Accepts a forced tool choice | Whether the model takes `tool_choice` `required`, `any` or a named tool; Kaeso refuses such requests for a model that doesn't, before calling the provider |
| Adaptive thinking, effort levels | How the model's reasoning is turned on, and the efforts it takes |
| Timeout | Seconds for a request to this model; empty uses the install's request timeout |
| Released | The model's release date |

**Remove from catalogue** takes a model out; requests for it are refused from then on, and aliases and model lists that name it lose it.

## Aliases

An **alias** is a name of the project's own that points at a model of its catalogue, such as `fast` for `claude-haiku-4-5`. Apps call the alias, and you change the model behind it without changing the apps.

On the **Aliases** tab, **Create alias** asks for a name and a target. A name has 1 to 200 characters, without spaces or `/`. An alias with the same name as a model wins over it; the console marks it as shadowing the model.

## How a request names a model

A request may name a model in three ways, and Kaeso looks in this order:

1. An alias of the project: `fast`.
2. The provider and the model: `anthropic/claude-haiku-4-5`.
3. The model's own id: `claude-haiku-4-5`.

If two providers have a model with the same id, use the `provider/model` form.

Changing models, overrides and aliases needs the right **Manage models**.
