---
title: Quick start
description: Install Kaeso with Docker Compose, connect a provider and send the first request.
order: 1
---

One Compose file runs the whole install: the gateway, the console, PostgreSQL, ClickHouse, Valkey, and Caddy, which gets a TLS certificate for your domain. This page takes you from an empty server to a first request.

## What you need

- A server with Docker Engine and Docker Compose 2.24 or newer, 2 vCPUs, 4 GB of memory and 20 GB of disk ([requirements](../self-hosting/requirements.md)).
- A domain, such as `kaeso.example.com`, whose DNS record points at the server, with ports 80 and 443 open to the internet so Caddy can get a certificate.
- An API key from OpenAI or Anthropic.

> [!TIP]
> To try Kaeso on your own computer, use `localhost` as the domain and open `https://localhost`. Caddy then signs the certificate with its own local authority. Your browser asks you to accept it once; [trying it on your computer](../self-hosting/installation.md#trying-it-on-your-computer) shows how to make curl and the SDKs trust it.

## Download the Compose file

Each release publishes a `compose.yaml` that pins its gateway and console images. Put it in a new folder that only you can read:

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

The gateway sends sign-in links, invitations and alerts by email, and doesn't start without a way to send them. Add your SMTP server, which Kaeso reaches on port 587 with STARTTLS unless [you change it](../self-hosting/email.md):

```bash
cat >> .env <<'EOF'
KAESO_SMTP_HOST=smtp.example.com
KAESO_SMTP_USER=kaeso
KAESO_SMTP_PASSWORD=…
KAESO_SMTP_FROM="Kaeso <kaeso@example.com>"
EOF
```

To try Kaeso without a mail server, have the gateway only log its emails instead. Password sign-in and the setup link still work; sign-in links and invitations aren't delivered.

```bash
echo KAESO_MAIL_TRANSPORT=log >> .env
```

## Create the master key

The master key encrypts the provider keys that Kaeso stores. Generate one into `master-keys`, which Compose hands to the gateway as a Docker secret:

```bash
docker run --rm ghcr.io/logicplanes/kaeso-gateway:latest master-key generate > master-keys
chmod 644 master-keys
```

The gateway runs as an unprivileged user in its container, so the file has to be readable; the folder keeps other users of the server out.

> [!IMPORTANT]
> Keep a copy of `master-keys` somewhere safe, apart from your database backups. Without it, the stored provider keys can't be decrypted and have to be entered again.

## Start Kaeso

```bash
docker compose up -d --wait
```

The first start takes a minute or two while the images download and the gateway prepares the databases. The command returns once every service reports healthy, Caddy included, or fails and names the one that doesn't. Caddy is healthy as soon as it serves and requests the certificate for a new domain in the background, so HTTPS requests can still fail until it has one; `docker compose logs caddy` shows when it gets it, or why it can't.

## Create the first user

The setup link creates the first user. Print it in your terminal:

```bash
docker compose exec gateway kaeso setup-link
```

The link works once, within 24 hours. Running the command again prints a new link and retires the old one.

Open the link, enter your name, your email, a password of 12 to 128 characters and a name for your organization, and choose **Set up Kaeso**. You are now an instance admin and the owner of the first organization, which comes with a default project.

## Add a provider key

Provider keys belong to the organization. Open **Providers** in the sidebar and choose **Add key**. Choose OpenAI or Anthropic, paste the provider's key, name it, for example `Production`, and choose **Add key**.

The first key of a provider becomes its default, which every project uses. Kaeso never shows the key again. See [provider keys](../console/provider-keys.md).

## Add a model

A request can only name a model in the project's catalogue. Open the default project, choose **Models**, then **Add models**. Choose the provider, search for a model, such as `claude-haiku-4-5` from Anthropic or `gpt-5-mini` from OpenAI, and add it. Its prices, limits and capabilities come from [models.dev](https://models.dev); see [models and aliases](../console/models-and-aliases.md).

## Create an API key

In the project, choose **API keys**, then **Create key**. Name the key after the app that will use it, such as `support-bot`, keep the expiry of 90 days and the access **Inference**, and create it.

Copy the key now. It is shown once, and Kaeso keeps only a hash of it: a lost key can't be recovered, only replaced.

## Send the first request

Put the key in your shell and call the model with curl:

```bash
export KAESO_API_KEY=sk-kso-…
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "messages": [{"role": "user", "content": "Write a haiku about gateways."}]}'
```

Apps keep their official SDK and change only the base URL and the key. The OpenAI SDK for Python (`pip install openai`) calls the same Claude model; Kaeso translates the request and the reply.

```python
import os
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

response = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.")
print(response.output_text)
```

Use whichever model you added. Then open **Request logs** in the project: both requests are there with their status, tokens, cost and timing.

## Next

| To | Read |
| --- | --- |
| Call models from TypeScript or with the Anthropic SDK, and stream replies | [Calling models](calling-models.md) |
| Point Claude Code or Codex at your install | [Coding tools](coding-tools.md) |
| Invite people and set limits and budgets | [Using the console](../console/index.md) |
| Sign in through your identity provider | [Sign-in and single sign-on](../self-hosting/sign-in.md) |
| Plan backups, upgrades and monitoring | [Self-hosting](../self-hosting/index.md) |
