---
title: Provider keys
description: Connect the organization's OpenAI and Anthropic keys and choose which projects use them.
order: 3
---

Provider keys are the organization's keys for OpenAI and Anthropic, the two providers Kaeso calls. Kaeso stores them encrypted under the install's master key and never shows them again; apps never see them and call Kaeso with [API keys](api-keys.md) instead.

## Add a key

On **Providers**, choose **Add key**, or **Connect** next to a provider that has none.

1. **Which provider is the key for?**
2. Paste the key and give it a name, such as `Production`. An OpenAI key, from platform.openai.com, starts with `sk-`; an Anthropic key, from console.anthropic.com, with `sk-ant-`.
3. If the provider already has a default key, choose whether this one becomes the default.

| Action | Right |
| --- | --- |
| Add a key | **Add provider keys** |
| Replace, rename, assign or make a key the default | **Change provider keys** |
| Delete a key | **Delete provider keys** |

Every action but deleting also needs a recent sign-in.

## Defaults and assignments

Each provider has one **default** key, which every project without an assigned key uses. The first key added for a provider becomes its default; **Make default** in another key's menu moves it.

**Assign to projects**, in a key's menu, makes the chosen projects use that key for its provider instead of the default. A project uses one key per provider.

## Change or remove a key

The list shows each key's name, its last four characters, the projects that use it, who added it and when it was last used. A key's menu has these actions:

| Action | What it does |
| --- | --- |
| Replace | Takes a new secret for the same key, for example after you rotated it at the provider |
| Rename | Changes the name shown in the console |
| Delete | Removes the key, and its projects use the default again. If it was the default, the provider's most recently added other key becomes the default; if there is none, requests to that provider fail. The dialog says which applies |

Which key a project uses decides which models the provider serves it. Adding, replacing, assigning, making default or deleting a key therefore resets the [availability](models-and-aliases.md#availability) of that provider's models to **Not checked** in every project of the organization; **Check availability** on a project's **Models** page asks the provider again.
