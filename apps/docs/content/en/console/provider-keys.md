---
title: Provider keys
description: Connect your organization's keys for model providers, and choose which projects use each one.
order: 3
---

Provider keys are your organization's keys for the model providers Kaeso calls. Kaeso stores them encrypted under the install's master key and never shows them again. Apps never see them: they call Kaeso with [API keys](api-keys.md) instead.

## Add a key

**Providers** lists the providers your organization has keys for. Until it has one, the page explains provider keys and offers **Add provider key**.

1. On **Providers**, choose **Add key**.
2. **Which provider is the key for?** Search for the provider and choose it. Providers that already have keys come first, with their key counts.
3. Paste the key and give it a name, such as `Production`. The dialog links the provider's page for creating keys, and the table below lists them too.
4. If the provider already has a default key, choose whether the new key becomes the default.

| Provider | Create a key at | The key starts with |
| --- | --- | --- |
| OpenAI | [platform.openai.com/api-keys](https://platform.openai.com/api-keys) | `sk-` |
| Anthropic | [platform.claude.com/settings/keys](https://platform.claude.com/settings/keys) | `sk-ant-` |
| DeepSeek | [platform.deepseek.com/api_keys](https://platform.deepseek.com/api_keys) | `sk-` |

Each action on provider keys needs its own right:

| Action | Right |
| --- | --- |
| Add a key | **Add provider keys** |
| Replace, rename or assign a key, or make it the default | **Change provider keys** |
| Delete a key | **Delete provider keys** |

Every action except deleting also needs a recent sign-in.

## Choose which key a project uses

Each provider has one default key, which every project without an assigned key uses. The first key you add for a provider becomes its default. To move the default, choose **Make default** in another key's menu.

To give some projects a different key, choose **Assign to projects** in that key's menu and pick the projects. They then use that key for its provider instead of the default. A project uses one key per provider.

## Replace, rename or delete a key

The list shows each key's name, its last four characters, the projects that use it, who added it and when it was last used. A key's menu has these actions:

| Action | What it does |
| --- | --- |
| Replace | Takes a new secret for the same key, for example after you rotated it at the provider |
| Rename | Changes the name the console shows |
| Delete | Removes the key. Its projects use the default again |

When you delete the default, the provider's most recently added other key becomes the default. If there is none, requests to that provider fail. The dialog tells you which applies.

> [!NOTE]
> A project's key decides which models the provider serves it. Adding, replacing, assigning or deleting a key, or making it the default, resets the [availability](models-and-aliases.md#availability) of that provider's models to **Not checked** in every project.

## Next steps

- [Models and aliases](models-and-aliases.md): Add the models a project may call.
- [API keys](api-keys.md): Create the keys your apps call Kaeso with.
