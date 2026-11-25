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
| Test, replace, rename or assign a key, or make it the default | **Change provider keys** |
| Delete a key | **Delete provider keys** |

Every action except deleting also needs a recent sign-in.

## Add a Google Cloud service account key

For Gemini Enterprise Agent Platform, a provider key is either an express mode API key or a Google Cloud service account's JSON key. An express mode key reaches only the Gemini models that express mode lists. A service account key reaches every model the platform serves to its project.

1. In the Google Cloud console, under **IAM & Admin → Service accounts**, create a service account in the project that should pay for the requests. Give it the role **Agent Platform User** (`roles/aiplatform.user`), the least role that calls models ([access control](https://docs.cloud.google.com/gemini-enterprise-agent-platform/machine-learning/general/access-control)).
2. Open the service account, then **Keys → Add key → Create new key → JSON**. Google downloads the key file. Organizations created since May 2024 block key creation by default with the policy `iam.disableServiceAccountKeyCreation`; an organization policy administrator can allow it for the project ([create and delete service account keys](https://docs.cloud.google.com/iam/docs/keys-create-delete)).
3. In Kaeso, on **Providers**, choose **Add provider key**, then **Gemini Enterprise Agent Platform**, then **Service account key**.
4. Paste the key file's contents or choose the file, choose a **Location** and give the key a name. `global` serves the most models; choose a region or `us` or `eu` when the requests must stay there.

Kaeso checks that the file is a service account key whose `token_uri` is `https://oauth2.googleapis.com/token`, and refuses it otherwise. It stores the whole file encrypted like any other key and never shows it again; the list shows the service account's email address, its project, the location and the last four characters of the key's id. For each request, Kaeso signs a token request with the key, trades it at Google for an access token that lasts an hour, and keeps the token in memory until five minutes before it expires. When Google refuses the key, apps get a refused provider key.

To call another location, choose **Edit** in the key's menu. To use a new key file for the same service account, choose **Replace**. Delete the file you downloaded once Kaeso has the key.

## Cloudflare AI Gateway keys

For Cloudflare AI Gateway, a provider key is a Cloudflare API token, and it names the account it calls and, optionally, the gateway its requests go through.

1. In the Cloudflare dashboard, under **My Profile → API Tokens** or the account's **Manage account → Account API Tokens**, create a token with the permission **Account → Workers AI → Read** for the account. Every model request needs it, also for third-party models ([REST API](https://developers.cloudflare.com/ai-gateway/usage/rest-api/#authentication)); a token with AI Gateway permissions only is refused.
2. Copy the account ID from the account's home page or its dashboard address ([find account and zone IDs](https://developers.cloudflare.com/fundamentals/account/find-account-and-zone-ids/)).
3. In Kaeso, on **Providers**, choose **Add provider key**, then **Cloudflare AI Gateway**, and paste the token and the account ID.
4. Optionally, give a **Gateway**: the ID of a gateway of the account. Requests then go through it, with its logs, caching, rate limits and guardrails, and can call its aliases (`custom/…`) and dynamic routes (`dynamic/…`). Without one, third-party models go through the account's `default` gateway, which Cloudflare creates on the first such request, and Workers AI models are called directly, without a gateway.

Third-party models are paid with the account's Unified Billing credits, or with the provider keys stored on the gateway under the alias `default` ([bring your own keys](https://developers.cloudflare.com/ai-gateway/configuration/bring-your-own-keys/)). Without either, Cloudflare answers with an empty balance, which apps get as a provider key refused for billing reasons. Workers AI models are billed through Workers AI, also when the account has no credits.

The list shows the account ID and the gateway beside the key. To move the key to another account or gateway, choose **Edit** in its menu.

## Amazon Bedrock keys

For Amazon Bedrock, a provider key names the region its requests run in, and signs in one of three ways. On **Providers**, choose **Add provider key**, then **Amazon Bedrock**, and answer **How does Kaeso sign in to Amazon Bedrock?**:

- **Bedrock API key.** In the [Amazon Bedrock console](https://console.aws.amazon.com/bedrock/), choose **API keys**, then generate a key under **Long-term API keys** or **Short-term API keys** ([Bedrock API keys](https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys.html)). A long-term key starts `ABSK` and lasts until the expiry you set. A short-term key starts `bedrock-api-key-`, lasts at most 12 hours and works only in the region that made it, so replace it in Kaeso before it expires. Paste the key and choose its **Region**: the region whose models the key calls, for a short-term key the one that made it.
- **Access key.** An IAM user's access key, made in the AWS console under **Security credentials** ([manage access keys](https://docs.aws.amazon.com/IAM/latest/UserGuide/id_credentials_access-keys.html)). Give the **Access key ID** and the **Secret access key**, and for temporary credentials the **Session token**, which stop working when they expire. Kaeso signs each request with them using [Signature Version 4](https://docs.aws.amazon.com/IAM/latest/UserGuide/reference_sigv4.html); the list shows the access key ID.
- **The gateway's AWS identity.** Shown only to instance admins, and only when the install allows it with `aws.ambient_credentials` ([configuration](../self-hosting/configuration.md#provider-calls)). Kaeso signs each request with the AWS identity it runs as: its `AWS_*` environment variables, a web identity token (IAM roles for service accounts on EKS), the ECS or EKS container role, or the EC2 instance role, the order AWS's SDKs use. Give a **Role to assume** to sign in through STS `AssumeRole` instead, with an **External ID** when the role's trust policy asks for one. Kaeso renews the temporary credentials five minutes before they expire. The list shows the role's name, or `gateway` without one.

The key's identity needs `bedrock:InvokeModel` on the models it calls, and `bedrock:InvokeModelWithResponseStream` for streamed replies ([endpoints](https://docs.aws.amazon.com/bedrock/latest/userguide/endpoints.html)). Which inference profiles a key can call, such as `us.anthropic.claude-sonnet-5` or `global.openai.gpt-6-sol`, depends on its region: a `us.` profile works from regions in the United States, an `eu.` profile from regions in Europe ([cross-Region inference](https://docs.aws.amazon.com/bedrock/latest/userguide/cross-region-inference.html)).

The list shows the region beside the key. To move the key to another region, choose **Edit** in its menu.

## Azure keys

For Azure, a provider key belongs to one Azure OpenAI or Foundry resource and names that resource's endpoint. It signs in with the resource's key or with a Microsoft Entra ID service principal. On **Providers**, choose **Add provider key**, then **Azure**, and answer **How does Kaeso sign in to Azure?**:

- **Resource key.** In the [Azure portal](https://portal.azure.com/), open the resource and its **Resource Management → Keys and Endpoint** page ([find the key and endpoint](https://learn.microsoft.com/en-us/azure/ai-services/authentication)). Copy **KEY 1** or **KEY 2**, and the **Endpoint**, such as `https://example.openai.azure.com`. An endpoint ending in `.services.ai.azure.com` or `.cognitiveservices.azure.com` works too, and a pasted `/openai/v1` is left out. Paste the key and the endpoint. Kaeso sends the key in `api-key`.
- **Microsoft Entra ID.** For a resource whose key access is turned off. In the [Microsoft Entra admin center](https://entra.microsoft.com/), register an app under **App registrations** and add a client secret to it; on the resource, give the app the **Cognitive Services OpenAI User** role. Give the **Directory (tenant) ID** (or one of the tenant's domains), the **Application (client) ID**, the client secret's value and the resource's endpoint. Kaeso exchanges the secret for access tokens with the [client credentials grant](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-client-creds-grant-flow), for the scope `https://cognitiveservices.azure.com/.default` unless you choose `https://ai.azure.com/.default`, and sends each token as a bearer. It renews a token five minutes before it expires. When Entra ID refuses the secret, for example because it expired, the key shows **Signed out**, the people who can change provider keys get an email, and **Replace** takes a new secret.

Kaeso calls the resource's OpenAI v1 API, `{endpoint}/openai/v1`. A request names a deployment of the resource: Kaeso sends the model's id, or its **Provider model ID** when the deployment has another name ([change a model's values](models-and-aliases.md#change-a-models-values)).

The list shows the endpoint's host beside the key, and for a service principal its tenant and the client ID's last four characters. To move the key to another resource, choose **Edit** in its menu and give the other resource's endpoint; a new key of the same resource keeps the endpoint.

## Sign in with a ChatGPT account

For OpenAI, a provider key can also be a ChatGPT account. Its requests then use the account's ChatGPT plan instead of an OpenAI API account, within the plan's limits.

> [!WARNING]
> Use your own account only; sharing it breaks OpenAI's terms.

1. On **Providers**, choose **Add provider key**, then **OpenAI**.
2. Choose **ChatGPT account**. If OpenAI already has a default key, choose whether the account becomes the default.
3. Give the key a name and choose **Continue with ChatGPT**. ChatGPT opens in a new tab: sign in and approve Kaeso.
4. The tab then goes to an address that starts with `http://127.0.0.1` and can't load. That's expected. Copy that address from the address bar.
5. Paste it into **Address of the page** and choose **Add account**.

Finish within ten minutes, as the person who began. Kaeso checks the address, signs the account in with OpenAI and keeps its tokens encrypted like any other key. It renews them by itself before they expire, ahead of time in the background when the account was used in the last week, so requests don't wait for OpenAI.

A ChatGPT account serves requests under OpenAI's rules for plan usage:

- Every request goes to `https://api.openai.com/v1/responses` as a stream that OpenAI doesn't store, whatever base URL the install sets for OpenAI. Kaeso joins the stream into one reply for apps that ask for one.
- Kaeso leaves out the fields a plan refuses: `temperature`, `top_p`, the output and tool call limits, `metadata`, `truncation`, user ids and the other [preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations).
- Kaeso still keeps a reply to the app's output limit: it ends the reply itself once the estimated output reaches the limit, as the model would at `max_tokens`, and stops the plan's work on it.
- Images, speech, transcriptions, embeddings and token counts need an API key; Kaeso refuses them for the account before anything is sent, and the error says so.
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

Finish within ten minutes, as the person who began. Kaeso keeps the account's tokens encrypted like any other key and renews them by itself before they expire: ahead of time in the background when the account was used in the last week, otherwise in the last two minutes.

A Grok account serves requests like an xAI API key, at `https://api.x.ai/v1` with the account's token, whatever base URL the install sets for SpaceX. If xAI refuses the account because its plan has no API access, the key shows **No API access** and its requests fail.

## Choose which key a project uses

Each provider has one default key, which every project without an assigned key uses. The first key you add for a provider becomes its default. To move the default, choose **Make default** in another key's menu.

To give some projects a different key, choose **Assign to projects** in that key's menu and pick the projects. They then use that key for its provider instead of the default.

A project can use several keys of one provider, such as accounts whose rate limits add up or Azure resources in several regions: assign each of them to it. Each caller, by its API key, service account or member, then draws one of the project's keys and keeps it, so the provider's prompt cache keeps working; a key with a higher **Weight in** the project is drawn by more callers, in proportion to the weights, from 1 to 100. When a key is refused, signed out, out of credit or rate limited, the request moves to the project's next key before an alias falls back to another model, and the key waits: until the provider's `retry-after` for a rate limit, five minutes otherwise. Every gateway of an install sees the same draw and the same waiting keys. Video jobs, which Kaeso asks about later, use the project's first assigned key.

## Replace, rename or delete a key

The list shows each key's name, its last four characters, the projects that use it, who added it and when it was last used. An account shows its email address and its product, such as **ChatGPT** or **Grok**, instead of the last four characters; a Grok account is also marked **Experimental**. A key's menu has these actions:

| Action | What it does |
| --- | --- |
| Test key | Asks the provider whether it takes the key, by reading its model list with it: it says the provider takes the key, and how many models it lists for it, or shows the provider's refusal. Nothing is billed. A provider that lists no models where the key calls it, such as Amazon Bedrock, can't tell; only a model request confirms the key |
| Replace | Takes a new secret for the same key, for example after you rotated it at the provider; for a service account key, a new JSON key file |
| Sign in again | For an account: signs the same account in again, for example after it shows **Signed out** or **No API access**, and ends its previous session at OpenAI or xAI |
| Manage usage | For a ChatGPT account: opens ChatGPT's usage settings, where its owner reviews and limits what Kaeso uses |
| Rename | Changes the name the console shows. For a service account key it's **Edit**, which also changes the location; for a Cloudflare AI Gateway key, **Edit** also changes the account and the gateway, and for an Amazon Bedrock key the region |
| Delete | Removes the key. Its projects without another key of the provider use the default again. For an account, Kaeso also ends its session at OpenAI or xAI |

An account shows **Signed out** when the provider has ended its session, for example after its owner disconnected Kaeso in ChatGPT. A Grok account shows **No API access** when xAI refuses its plan. In both cases, its requests fail until someone with **Change provider keys** chooses **Sign in again**.

When a provider refuses a key three times in a row, or ends an account's session, Kaeso emails everyone with **Change provider keys**. The email names the provider, the key and its last four characters or the account, the projects that use it, what the provider answered, when the refusals happened and when the key was last used, so the reader can decide whether to replace it, top up the account or sign in again ([choose your emails](account.md#choose-your-emails)).

When you delete the default, the provider's most recently added other key becomes the default. If there is none, requests to that provider fail. The dialog tells you which applies.

> [!NOTE]
> A project's key decides which models the provider serves it. Adding, replacing, assigning or deleting a key, or making it the default, resets the [availability](models-and-aliases.md#availability) of that provider's models to **Not checked** in every project.

## Next steps

- [Models and aliases](models-and-aliases.md): Add the models a project may call.
- [API keys](api-keys.md): Create the keys your apps call Kaeso with.
