---
title: Quick start
description: Install Kaeso with Docker Compose, connect a provider and send your first request.
order: 1
---

Install Kaeso on a server, connect a model provider such as OpenAI or Anthropic, and send your first request through it.

## Before you begin

- A server with Docker Engine and Docker Compose 2.24 or newer, 2 vCPUs, 4 GB of memory and 20 GB of disk ([requirements](../self-hosting/requirements.md)).
- A domain, such as `kaeso.example.com`, whose DNS record points at the server. Ports 80 and 443 must be open to the internet, so Caddy can get a certificate.
- An API key from a [model provider Kaeso calls](../concepts/supported-providers.md), such as OpenAI or Anthropic.

> [!TIP]
> To try Kaeso on your own computer, use `localhost` as the domain and open `https://localhost`. Caddy then signs the certificate itself, and your browser asks you to accept it once. [Trying it on your computer](../self-hosting/installation.md#trying-it-on-your-computer) shows how to make curl and the SDKs trust it.

## Download the Compose file

Each release publishes a `compose.yaml` that pins its images by version and digest. Download it into a new folder that only you can read:

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

## Write the settings

Create `.env` with your domain and four random passwords:

```bash
cat > .env <<EOF
KAESO_DOMAIN=kaeso.example.com
POSTGRES_PASSWORD=$(openssl rand -hex 24)
POSTGRES_SUPERUSER_PASSWORD=$(openssl rand -hex 24)
CLICKHOUSE_PASSWORD=$(openssl rand -hex 24)
VALKEY_PASSWORD=$(openssl rand -hex 24)
EOF
```

The gateway sends sign-in links, invitations and alerts by email, and it doesn't start without a way to send them. Add your SMTP server. Kaeso reaches it on port 587 with STARTTLS unless [you change it](../self-hosting/email.md):

```bash
cat >> .env <<'EOF'
KAESO_SMTP_HOST=smtp.example.com
KAESO_SMTP_USER=kaeso
KAESO_SMTP_PASSWORD=…
KAESO_SMTP_FROM="Kaeso <kaeso@example.com>"
EOF
```

To try Kaeso without a mail server, let the gateway only log its emails. Password sign-in and the setup link still work, but sign-in links and invitations aren't delivered:

```bash
echo KAESO_MAIL_TRANSPORT=log >> .env
```

## Create the master key

The master key encrypts the provider keys that Kaeso stores. Generate one into `master-keys`, which Compose hands to the gateway as a Docker secret:

```bash
docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate > master-keys
chmod 644 master-keys
```

The gateway runs as an unprivileged user in its container, so the file must be readable. The folder's permissions keep other users of the server out.

> [!IMPORTANT]
> Keep a copy of `master-keys` somewhere safe, apart from your database backups. Without it, Kaeso can't decrypt the stored provider keys, and you have to enter them again.

## Start Kaeso

Start every service and wait until each one is healthy:

```bash
docker compose up -d --wait
```

The first start takes a minute or two while the images download and the gateway prepares the databases. The command returns once every service is healthy, or names the one that isn't. Caddy requests the certificate in the background, so HTTPS requests can fail until it has one. `docker compose logs caddy` shows when it gets the certificate, or why it can't.

## Create the first user

The setup link creates the first user. Print it in your terminal:

```bash
docker compose exec gateway kaeso setup-link
```

The link works once, within 24 hours. Running the command again prints a new link and retires the old one.

Open the link and enter your name, your email, a password of 12 to 128 characters and a name for your organization. Then choose **Set up Kaeso**. You are now an instance admin and the owner of the first organization, which comes with a default project.

## Add a provider key

Provider keys belong to the organization, and every project uses the provider's default key.

1. Open **Providers** in the sidebar and choose **Add provider key**.
2. Choose the provider and paste its key.
3. Name it, for example `Production`, and choose **Add key**.

The first key of a provider becomes its default. Kaeso never shows the key again; see [provider keys](../console/provider-keys.md).

## Add a model

A request can only name a model in the project's catalogue.

1. Open the default project and choose **Models**, then **Add models**.
2. Choose the provider and search for a model, such as `claude-haiku-4-5` from Anthropic or `gpt-5-mini` from OpenAI.
3. Add it.

The model's prices, limits and capabilities come from [models.dev](https://models.dev); see [models and aliases](../console/models-and-aliases.md).

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

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

response = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.")
print(response.output_text)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1", apiKey: process.env.KAESO_API_KEY });

const response = await client.responses.create({ model: "claude-haiku-4-5", input: "Write a haiku about gateways." });
console.log(response.output_text);
```

If you added another model, use its id instead. The reply holds the model's haiku. Open **Request logs** in the project to see the request with its status, tokens, cost and timing.

## Next steps

- [Calling models](calling-models.md): Use the Anthropic SDK too, stream replies and find a request in the logs.
- [Coding tools](coding-tools.md): Point Claude Code or Codex at your install.
- [Using the console](../console/index.md): Invite people, and set limits and budgets.
- [Self-hosting](../self-hosting/index.md): Plan sign-in, backups, upgrades and monitoring.
