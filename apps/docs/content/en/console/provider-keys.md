---
title: Provider keys
description: Connect the organization's OpenAI and Anthropic keys and choose which projects use them.
order: 3
---

Provider keys are the keys Kaeso uses to call OpenAI and Anthropic. They belong to the organization, live encrypted under the install's master key, and are never shown again after you add them. Apps never see them; they use Kaeso [API keys](api-keys.md).

Kaeso calls two providers: OpenAI and Anthropic.

## Add a key

Open **Providers** and choose **Add key**, or **Connect** next to a provider without one.

1. Choose the provider.
2. Paste the key: an OpenAI key from platform.openai.com starts with `sk-`, an Anthropic key from console.anthropic.com with `sk-ant-`. Name it, such as `Production`.
3. If the provider already has a default key, choose whether this one becomes the default.

Adding a key needs the right **Add provider keys**. Replacing, renaming, assigning and making a key the default need **Change provider keys**, and deleting one **Delete provider keys**. Adding and changing keys ask for a recent sign-in.

## Defaults and assignments

Each provider has one **default** key, which every project without an assigned key uses. The first key added for a provider becomes its default; **Make default** in another key's menu moves it.

To give some projects a different key, choose **Assign to projects** in the key's menu. Those projects use it for that provider instead of the default. A project uses one key per provider.

## Change or remove a key

| Action | What it does |
| --- | --- |
| Replace | Paste a new secret for the same key, for example after rotating it at the provider |
| Rename | Changes the name shown in the console |
| Delete | Removes the key; its projects use the default again. Deleting the default makes the most recently added other key of the provider the default; without one, requests to that provider fail. The dialog says which applies |

The table shows each key's last four characters, its projects, who added it and when it was last used.
