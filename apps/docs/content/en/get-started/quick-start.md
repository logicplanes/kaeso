---
title: Quick start
description: Install Kaeso with Docker Compose, connect a provider and send your first request.
order: 1
---

Install Kaeso on a server, connect a model provider such as OpenAI or Anthropic, and send your first request through it.

## Before you begin

- A server with 2 vCPUs, 4 GB of memory and 20 GB of disk, and Docker Engine with Docker Compose 2.24 or newer ([requirements](../self-hosting/requirements.md)). On Linux, the installer offers to install Docker when it's missing.
- A domain, such as `kaeso.example.com`, whose DNS record points at the server. Ports 80 and 443 must be open to the internet, so Caddy can get a certificate.
- An API key from a [model provider Kaeso calls](../concepts/supported-providers.md), such as OpenAI or Anthropic.

> [!TIP]
> To try Kaeso on your own computer, use `localhost` as the domain and open `https://localhost`. Caddy then signs the certificate itself, and your browser asks you to accept it once. [Trying it on your computer](../self-hosting/installation.md#trying-it-on-your-computer) shows how to make curl and the SDKs trust it.

## Install Kaeso

Run the installer on the server. It creates a folder `kaeso` that only you can read and downloads the newest release's `compose.yaml` into it. Then it writes `.env` with your domain and random passwords, creates the master key and starts every service:

```bash
curl -fsSL https://kaeso.app/install.sh | sh -s -- --domain kaeso.example.com
```

Before it installs, the installer starts Docker if it's stopped and checks that your domain has a DNS record. The first start takes a minute or two while the images download. Then the installer checks that Caddy got the domain's certificate, and explains the certificate authority's answer if it didn't. It ends by printing the setup link, and opens it in your browser when you run it on your own computer; `--no-open` keeps it closed. Until you [add your SMTP server](../self-hosting/email.md), the gateway only writes its emails to its log, so sign-in links and invitations aren't delivered. [Install by hand](../self-hosting/installation.md#install-by-hand) shows each step the installer takes.

Run the installer again in the install's folder to start Kaeso again; it keeps the settings and the master key. The installer also turns on the [updater](../self-hosting/upgrades.md#the-updater), so **Update to …** under **Instance** › **Updates** installs new releases. A machine runs one Kaeso, so the installer refuses a second install in another folder.

> [!IMPORTANT]
> Keep a copy of `kaeso/master-keys` somewhere safe, apart from your database backups. Without it, Kaeso can't decrypt the stored provider keys, and you have to enter them again.

## Create the first user

The setup link the installer printed creates the first user. It works once, within 24 hours. To print a new link, which retires the old one, run this in the `kaeso` folder:

```bash
docker compose exec gateway kaeso setup-link
```

Open the link and enter your name, your email, a password of 12 to 128 characters and a name for your organization. Then choose **Set up Kaeso**. You are now an instance admin and the owner of the first organization, and Kaeso opens its **Overview**. The organization has no projects yet.

## Add a provider key

Provider keys belong to the organization, and every project uses the provider's default key.

1. Open **Providers** in the sidebar and choose **Add provider key**.
2. Choose the provider and paste its key.
3. Name it, for example `Production`, and choose **Add key**.

The first key of a provider becomes its default. Kaeso never shows the key again; see [provider keys](../console/provider-keys.md).

## Create a project

Models, API keys and request logs belong to a project.

1. On the organization's **Overview**, choose **Create project**.
2. Name it after what it is for, such as `Support bot`, and choose **Create project**.

Kaeso opens the new project, and you are its admin; see [organizations and projects](../console/organizations-and-projects.md#create-a-project).

## Add a model

A request can only name a model in the project's catalogue.

1. In the project, choose **Models**, then **Add models**.
2. Choose the provider and search for a model, such as `claude-haiku-4-5` from Anthropic or `gpt-5-mini` from OpenAI.
3. Add it.

The model's prices, limits and capabilities come from [KPMR](../concepts/kpmr.md), the Kaeso Provider & Model Registry; see [models and aliases](../console/models-and-aliases.md).

## Create an API key

1. In the project, choose **API keys**, then **Create key**.
2. Name the key after the app that will use it, such as `support-bot`.
3. Keep the expiry of 90 days and the access **Inference**, and create the key.

Copy the key now. Kaeso shows it once and keeps only a hash of it, so you can't recover a lost key, only replace it. Put it in your shell:

```bash
export KAESO_API_KEY=sk-kso-…
```

## Send your first request

Call the model with curl, or with the OpenAI SDK for Python (`pip install openai`) or TypeScript (`npm install openai`). The OpenAI SDK calls the Claude model you added: Kaeso translates the request and the reply.

```bash tab="curl"
curl https://kaeso.example.com/v1/responses \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "input": "Write a haiku about gateways."}'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)

response = client.responses.create(
    model="claude-haiku-4-5",
    input="Write a haiku about gateways.",
)
print(response.output_text)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});

const response = await client.responses.create({
  model: "claude-haiku-4-5",
  input: "Write a haiku about gateways.",
});
console.log(response.output_text);
```

If you added another model, use its id instead. The reply holds the model's haiku. Open **Request logs** in the project to see the request with its status, tokens, cost and timing.

## Next steps

- [Calling models](calling-models.md): Use the Anthropic SDK too, stream replies and find a request in the logs.
- [Coding tools](coding-tools.md): Point Claude Code or Codex at your install.
- [Using the console](../console/index.md): Invite people, and set limits and budgets.
- [Self-hosting](../self-hosting/index.md): Plan sign-in, backups, upgrades and monitoring.
