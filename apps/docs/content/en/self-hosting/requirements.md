---
title: Requirements
description: The server, network access and disk space a Kaeso install needs.
order: 1
---

Kaeso runs on one Linux server with Docker. Size the disk by how many requests you log.

## Server

| Resource | Minimum |
| --- | --- |
| System | Linux with Docker Engine and Docker Compose 2.24 or newer, on `linux/amd64` or `linux/arm64` |
| CPU | 2 vCPUs |
| Memory | 4 GB, of which ClickHouse uses about 2 GB |
| Disk | 20 GB, plus room for request logs |

[The updater](upgrades.md#the-updater) also needs Docker Engine 25 or newer.

## Network

The install needs a domain whose DNS record points at the server, and these connections:

| Direction | Ports or hosts | For |
| --- | --- | --- |
| Incoming | TCP ports 80 and 443, and UDP port 443 for HTTP/3 | The certificate and HTTPS |
| Outgoing | The host of each provider you use ([supported providers](../concepts/supported-providers.md)), or the base URL you set for it ([provider settings](configuration.md#provider-settings)) | The providers you use |
| Outgoing | `models.dev` | Model data, refreshed once a day |
| Outgoing | `update.kaeso.app` | Looking for new Kaeso releases, every 5 minutes |
| Outgoing | `acme-v02.api.letsencrypt.org`, `acme.zerossl.com` | Caddy getting and renewing the certificate |
| Outgoing | `ghcr.io`, `pkg-containers.githubusercontent.com` | Docker pulling Kaeso's images, when you install or update |
| Outgoing | `registry-1.docker.io`, `auth.docker.io`, `production.cloudflare.docker.com` | Docker pulling the images of PostgreSQL, ClickHouse, Valkey and the updater's socket proxy |
| Outgoing | `github.com`, `release-assets.githubusercontent.com` | Downloading a release's `compose.yaml` and `release.json`, by hand or by the updater |
| Outgoing | `api.github.com` | The updater, and `gh attestation verify`, fetching a release's attestations |
| Outgoing | Your SMTP server | Sign-in links, invitations, alerts and security notices ([email](email.md)) |
| Outgoing | Your organizations' identity providers | Single sign-on, when an organization connects one ([sign-in](sign-in.md)) |

Without internet access, Kaeso works with the model data built into the release, and you update by hand. Turn off the outgoing checks as [installs without internet access](upgrades.md#installs-without-internet-access) shows.

## Disk for request logs

Request logs live in ClickHouse. Plan for this much disk per million requests:

| Logging | Per million requests |
| --- | --- |
| Metadata only: model, status, tokens, cost and timing | About 0.1 GB |
| Metadata and content, at about 2.5 KB of text per request | About 0.65 GB |

Content grows with the size of your prompts and replies. Retention caps the total. At a million requests a day, the default retention keeps about 6 GB of metadata (90 days) and 16 GB of content (30 days).

Each organization chooses its retention on its **Logging** page, within the instance's maximums ([request logs](../console/request-logs.md)). The instance's **System** page shows how much the logs take.
