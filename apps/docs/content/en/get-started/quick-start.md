---
title: Quick start
description: Run Kaeso with Docker Compose, sign in, connect a provider and send the first request.
order: 1
---

This page takes you from an empty server to an app that calls a model through Kaeso. It starts the whole install with one Compose file: the gateway, the console, PostgreSQL, ClickHouse, Valkey and Caddy, which gets a TLS certificate for your domain by itself.

## What you need

- A server with Docker Engine and Docker Compose 2.24 or newer, at least 2 vCPUs, 4 GB of memory and 20 GB of disk. [Requirements](../self-hosting/requirements.md) has the details.
- A domain, such as `kaeso.example.com`, whose DNS record points at the server.
- Ports 80 and 443 open to the internet, so Caddy can get the certificate.
- A key from OpenAI or Anthropic.

> [!TIP]
> To try Kaeso on your own computer instead, use `localhost` as the domain below and open `https://localhost`. Caddy then uses a certificate from its own local authority, which your browser asks you to accept once; [trying it on your computer](../self-hosting/installation.md#trying-it-on-your-computer) shows how curl and the SDKs trust it.

## Download the Compose file

Every Kaeso release carries one `compose.yaml` that pins the gateway and console images of that release. Put it in a folder that only you can read:

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

## Write the settings

Create `.env` next to `compose.yaml` with your domain and four random passwords:

```bash
cat > .env <<EOF
KAESO_DOMAIN=kaeso.example.com
POSTGRES_PASSWORD=$(openssl rand -hex 24)
POSTGRES_SUPERUSER_PASSWORD=$(openssl rand -hex 24)
CLICKHOUSE_PASSWORD=$(openssl rand -hex 24)
VALKEY_PASSWORD=$(openssl rand -hex 24)
EOF
```

The gateway sends sign-in links, invitations and alerts by email, and refuses to start without a way to send them. Add your SMTP server to `.env`:

```bash
KAESO_SMTP_HOST=smtp.example.com
KAESO_SMTP_USER=kaeso
KAESO_SMTP_PASSWORD=…
KAESO_SMTP_FROM=Kaeso <kaeso@example.com>
```

To try Kaeso without a mail server, add `KAESO_MAIL_TRANSPORT=log` instead. The gateway then only logs that an email would have been sent; signing in with a password still works. [Email](../self-hosting/email.md) has every option.

## Create the master key

The master key encrypts the provider keys you will store. Generate one into the file `master-keys`, which the Compose file hands to the gateway as a Docker secret:

```bash
docker run --rm ghcr.io/logicplanes/kaeso-gateway:latest master-key generate > master-keys
chmod 644 master-keys
```

The gateway runs as an unprivileged user inside its container and must be able to read the file; the folder keeps it closed to other users of the server.

> [!IMPORTANT]
> Keep a copy of `master-keys` somewhere safe, apart from your database backups. Without it, the stored provider keys can't be decrypted and have to be entered again.

## Start Kaeso

```bash
docker compose up -d --wait
```

The first start takes a minute or two: the images download, the databases start, the gateway prepares them, and Caddy requests the certificate. `--wait` makes the command return only once every service is ready, and fail with the service's name if one isn't; `docker compose ps` shows each service as healthy.

## Sign in with the setup link

While the install has no users, the gateway logs a setup link:

```bash
docker compose logs gateway | grep setup_url
```

Open the link in a browser. It works once, for 24 hours. Enter your name, your email, a password of 12 to 128 characters and the name of your organization, then choose **Set up Kaeso**.

You are now the install's first instance admin and the owner of its first organization, which starts with a default project.

If the link expired or got lost, print a new one; older links stop working:

```bash
docker compose exec gateway kaeso setup-link
```

## Add a provider key

Provider keys belong to the organization. Open **Providers** in the sidebar and choose **Add key**. Choose OpenAI or Anthropic, paste the key from the provider, give it a name such as `Production`, and choose **Add key**.

The first key of a provider becomes its default, which every project uses. Kaeso never shows the secret again. More in [provider keys](../console/provider-keys.md).

## Add a model

Requests may only name models in the project's catalogue. Open the default project, choose **Models**, then **Add models**. Choose the provider, search for a model, such as `claude-haiku-4-5` under Anthropic or `gpt-5-mini` under OpenAI, select it and add it. Its prices, limits and capabilities come from [models.dev](https://models.dev). More in [models and aliases](../console/models-and-aliases.md).

## Create an API key

In the project, choose **API keys**, then **Create key**. Name it after the app that will use it, such as `support-bot`, keep the expiry of 90 days and the access **Inference**, which calls the model API, and create it.

> [!IMPORTANT]
> The key is shown once. Kaeso keeps only a hash of it, so a lost key can't be recovered; create a new one instead.

## Send the first request

Apps use the official SDKs; only the base URL and the key change. Any SDK can call any model in the catalogue: Kaeso translates the request and the reply.

| SDK | Base URL |
| --- | --- |
| OpenAI | `https://kaeso.example.com/v1` |
| Anthropic | `https://kaeso.example.com` |

With the OpenAI SDK for Python (`pip install openai`):

```python
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key="sk-kso-…")

response = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.")
print(response.output_text)
```

With the Anthropic SDK for Python (`pip install anthropic`):

```python
from anthropic import Anthropic

client = Anthropic(base_url="https://kaeso.example.com", api_key="sk-kso-…")

message = client.messages.create(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(message.content[0].text)
```

Both examples call `claude-haiku-4-5`, so the OpenAI SDK talks to a Claude model; use any model you added to the project. Open **Request logs** in the project: every request is there with its status, model, tokens, cost and timing.

## Next

| To | Read |
| --- | --- |
| Call models from TypeScript or curl, stream replies and find a request again | [Calling models](calling-models.md) |
| Point Claude Code or Codex at your install | [Coding tools](coding-tools.md) |
| Invite people, set limits and budgets | [Using the console](../console/index.md) |
| Plan backups, upgrades and monitoring | [Self-hosting](../self-hosting/index.md) |
