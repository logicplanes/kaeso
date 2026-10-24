---
title: Provider keys
description: Connect your organization's keys for model providers, and choose which projects use each one.
order: 3
---

Provider keys are your organization's keys for the [model providers Kaeso calls](../concepts/supported-providers.md). Kaeso stores them encrypted under the install's master key and never shows them again. Apps never see them: they call Kaeso with [API keys](api-keys.md) instead.

## Add a key

**Providers** lists the providers your organization has keys for under **Provider keys**. Until it has one, the page explains provider keys and offers **Add provider key**. Once there are more than eight keys, a search above the list finds them by provider, key or project name. Next to it, **Custom providers** lists the servers your organization added itself ([custom providers](custom-providers.md)).

1. On **Providers**, choose **Add provider key**.
2. **Which provider is the key for?** Search for the provider and choose it. Providers that already have keys come first, with their key counts.
3. Paste the key and give it a name, such as `Production`. The dialog links the provider's page for creating keys, and [supported providers](../concepts/supported-providers.md) lists them too.
4. If the provider already has a default key, choose whether the new key becomes the default.

Each action on provider keys needs its own right:

| Action | Right |
| --- | --- |
| Add a key | **Add provider keys** |
| Replace, rename or assign a key, or make it the default | **Change provider keys** |
| Delete a key | **Delete provider keys** |

Every action except deleting also needs a recent sign-in.

## Sign in with a ChatGPT account

For OpenAI, a provider key can also be a ChatGPT account. Its requests then use the account's ChatGPT plan instead of an OpenAI API account, within the plan's limits.

> [!WARNING]
> Use your own account only; sharing it breaks OpenAI's terms.

1. On **Providers**, choose **Add provider key**, then **OpenAI**.
2. Choose **ChatGPT account**. If OpenAI already has a default key, choose whether the account becomes the default.
3. Give the key a name and choose **Continue with ChatGPT**. ChatGPT opens in a new tab: sign in and approve Kaeso.
4. The tab then goes to an address that starts with `http://127.0.0.1` and can't load. That's expected. Copy that address from the address bar.
5. Paste it into **Address of the page** and choose **Add account**.

Finish within ten minutes, as the person who began. Kaeso checks the address, signs the account in with OpenAI and keeps its tokens encrypted like any other key. It renews them by itself before they expire.

A ChatGPT account serves requests under OpenAI's rules for plan usage:

- Every request goes to `https://api.openai.com/v1/responses` as a stream that OpenAI doesn't store, whatever base URL the install sets for OpenAI. Kaeso joins the stream into one reply for apps that ask for one.
- Kaeso leaves out the fields a plan refuses: `temperature`, `top_p`, the output and tool call limits, `metadata`, `truncation`, user ids and the other [preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations).
- Embeddings and token counts need an API key; Kaeso refuses them for the account.
- When the plan's usage limit is reached, apps get a `429` that links to [ChatGPT's usage settings](https://chatgpt.com/settings/usage).
- **Check availability** offers only the models the account lists for its plan.

## Sign in with a Grok account

For SpaceX, a provider key can also be a Grok account. Its requests then use the account's Grok plan instead of an xAI API account, if the plan includes API access.

> [!WARNING]
> Grok accounts are experimental: xAI may change how its sign-in works. Use your own account only; sharing it breaks SpaceX's terms.

1. On **Providers**, choose **Add provider key**, then **SpaceX**.
2. Choose **Grok account**, marked **Experimental**. If SpaceX already has a default key, choose whether the account becomes the default.
3. Give the key a name and choose **Get a sign-in code**. The dialog shows a code, a link to xAI's sign-in page and a QR code of that link.
4. Open the page on any device, sign in to Grok and enter the code.
5. Kaeso adds the key by itself once you approve. If you decline or the code expires, choose **Get a new code**.

Finish within ten minutes, as the person who began. Kaeso keeps the account's tokens encrypted like any other key and renews them by itself in the last two minutes before they expire.

A Grok account serves requests like an xAI API key, at `https://api.x.ai/v1` with the account's token, whatever base URL the install sets for SpaceX. If xAI refuses the account because its plan has no API access, the key shows **No API access** and its requests fail.

## Choose which key a project uses

Each provider has one default key, which every project without an assigned key uses. The first key you add for a provider becomes its default. To move the default, choose **Make default** in another key's menu.

To give some projects a different key, choose **Assign to projects** in that key's menu and pick the projects. They then use that key for its provider instead of the default. A project uses one key per provider.

## Replace, rename or delete a key

The list shows each key's name, its last four characters, the projects that use it, who added it and when it was last used. An account shows its email address and its product, such as **ChatGPT** or **Grok**, instead of the last four characters; a Grok account is also marked **Experimental**. A key's menu has these actions:

| Action | What it does |
| --- | --- |
| Replace | Takes a new secret for the same key, for example after you rotated it at the provider |
| Sign in again | For an account: signs the same account in again, for example after it shows **Signed out** or **No API access**, and ends its previous session at OpenAI or xAI |
| Manage usage | For a ChatGPT account: opens ChatGPT's usage settings, where its owner reviews and limits what Kaeso uses |
| Rename | Changes the name the console shows |
| Delete | Removes the key. Its projects use the default again. For an account, Kaeso also ends its session at OpenAI or xAI |

An account shows **Signed out** when the provider has ended its session, for example after its owner disconnected Kaeso in ChatGPT. A Grok account shows **No API access** when xAI refuses its plan. In both cases, its requests fail until someone with **Change provider keys** chooses **Sign in again**.

When you delete the default, the provider's most recently added other key becomes the default. If there is none, requests to that provider fail. The dialog tells you which applies.

> [!NOTE]
> A project's key decides which models the provider serves it. Adding, replacing, assigning or deleting a key, or making it the default, resets the [availability](models-and-aliases.md#availability) of that provider's models to **Not checked** in every project.

## Next steps

- [Models and aliases](models-and-aliases.md): Add the models a project may call.
- [API keys](api-keys.md): Create the keys your apps call Kaeso with.
