---
title: Requirements
description: What a server needs to run Kaeso.
order: 1
---

## Server

- Linux with Docker Engine and Docker Compose 2.24 or newer.
- At least 2 vCPUs and 4 GB of memory. ClickHouse uses about 2 GB of it.
- At least 20 GB of disk, plus room for request logs.

The images are published for `linux/amd64` and `linux/arm64`.

## Network

- A domain whose DNS record points at the server.
- Ports 80 and 443 open to the internet, for the certificate and for HTTPS. Caddy also answers HTTP/3 on UDP port 443 where it is open.
- Outgoing HTTPS to the providers you use (`api.openai.com`, `api.anthropic.com`) and to `models.dev`, where the gateway refreshes model data once a day. An install without internet access to `models.dev` works with the model data built into the release; turn the refresh off with `KAESO_MODELS_DEV_SYNC=false` in [`compose.override.yaml`](installation.md#local-changes).
- An SMTP server for sign-in links, invitations, budget alerts and security notices. See [email](email.md).

## Disk for request logs

Request logs live in ClickHouse. Measured with the gateway's schema on synthetic traffic, 200,000 requests with about 2.5 KB of request and reply text each, after ClickHouse merged its parts:

| Data | On disk per million requests |
| --- | --- |
| Metadata: model, status, tokens, cost, timing | about 63 MB |
| Content: requests and replies, at 2.5 KB of text per request | about 545 MB, about 0.21 bytes per byte of text |
| Usage rollups | at most 40 MB, usually far less |

Plan about 0.1 GB per million requests without content logging and 0.65 GB with it, and scale the content part with the size of your prompts and replies. Retention bounds the total: at a million requests a day and the default retention of 90 days for metadata and 30 days for content, that is about 6 GB of metadata and 16 GB of content.

Retention is set per organization under [logging](../console/request-logs.md#retention), within the maximums of the instance. The organization's **Logging** settings and the instance's **System** page show the real numbers.
