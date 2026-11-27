---
title: Managed platforms
description: Templates for Render, Railway, DigitalOcean, Fly.io, AWS, Azure, Google Cloud, Coolify, Dokploy, CapRover and Kubernetes.
order: 12
---

The Compose package runs Kaeso on one server you manage. The templates in the repository run the same release on a managed platform instead, where the platform terminates TLS, keeps the databases and rolls out new versions. Others run it on a platform such as Coolify on your own server.

```mermaid
flowchart LR
  accTitle: Kaeso on a managed platform
  accDescr: Apps and browsers reach a public edge over the platform's TLS. The edge sends the APIs, the MCP server and its sign-in to a private gateway and every other path to a private console. The gateway uses the platform's PostgreSQL and Valkey, an external ClickHouse when request logs are on, and calls the model providers.
  clients[Apps and browsers]:::outside
  subgraph platform [The platform]
    edge[Public edge<br>platform TLS]
    gateway[Private gateway]:::accent
    console[Private console]
    postgres[(PostgreSQL)]
    valkey[(Valkey)]
  end
  clickhouse[(ClickHouse)]:::outside
  providers[Model providers]:::outside
  clients -- HTTPS --> edge
  edge -- /v1, /platform, /mcp and OAuth --> gateway
  edge -- every other path --> console
  gateway --> postgres & valkey & clickhouse
  gateway -- HTTPS --> providers
```

## Five kinds of path

| Path | What it is | Where |
| --- | --- | --- |
| Platform template | Files the platform reads, with a public edge, a private gateway and console, and PostgreSQL and Valkey on the platform | [Render](render.md), [Railway](railway.md), [DigitalOcean](digitalocean.md), [Fly.io](fly.md) |
| Quick server | One virtual machine with the Compose package, Caddy, bundled ClickHouse and the updater, as on your own server | [AWS](aws.md), [Azure](azure.md), [Google Cloud](gcp.md) |
| Managed production | The platform's containers, load balancer, PostgreSQL and Valkey, with secrets in its secret store | [AWS](aws.md), [Google Cloud](gcp.md) |
| Self-hosted platform | A Compose file or one-click app for a platform on your own server, with a public edge behind its proxy and the Compose package's PostgreSQL, ClickHouse and Valkey | [Coolify](coolify.md), [Dokploy](dokploy.md), [CapRover](caprover.md) |
| Kubernetes | A Helm chart with the gateway and console behind your Ingress or Gateway, and PostgreSQL, ClickHouse and Valkey you bring or bundled for trying it out | [Kubernetes](kubernetes.md) |

The managed stacks on AWS and Google Cloud are not one click: you choose a region, a domain and its certificate, the sizes of the data stores, where request logs go, an SMTP server and where the master key lives. Fly.io takes a few commands, since it has no button for a project of several apps.

## What every path keeps

- **One origin.** The console and the APIs answer on one domain. `/v1/*`, `/v1beta/*`, `/v2/*`, `/platform/*`, `/mcp`, `/mcp/*`, `/oauth/*`, `/ingest/*` and `/.well-known/oauth-*` reach the gateway; every other path reaches the console. On Render, Railway, Fly.io, Coolify, Dokploy and CapRover, `kaeso-edge`, a second command in the console image, does this; on DigitalOcean, the app's ingress rules; on the managed stacks of AWS and Google Cloud, the load balancer's path rules; on Kubernetes, the chart's Ingress or HTTPRoute; on a quick server, Caddy.
- **Streams stay unbuffered.** The edge flushes every chunk, and a load balancer's idle timeout is longer than the gateway's keepalive interval ([long streams](../domains-and-tls.md#long-streams)).
- **The client address is believed only from the platform's proxy.** The edge takes the client's address only from the platform's proxy and sends the gateway one address ([settings of the edge](#settings-of-the-edge)); the gateway trusts the edge alone ([`KAESO_TRUSTED_PROXIES`](../configuration.md#settings)).
- **Health checks** use `/healthz` for the process and `/readyz` for PostgreSQL, Valkey and ClickHouse ([health and metrics](../monitoring.md)). The metrics port is never public.
- **The updater is off.** The platform rolls a new release out and back; a release's migrations only move forward ([upgrades](../upgrades.md)).
- **One version.** Gateway, console and edge are pinned to the same release; a release updates the templates.
- **Search engines stay out** until `KAESO_SEARCH_ENGINES` is `true` for the gateway and the console ([search engines](../domains-and-tls.md#search-engines)). Each template sets it in one place: on the gateway, whose value the console takes, on Render and in Railway's infrastructure file; as a variable of the whole app on DigitalOcean; as the stack's variable on Coolify, Dokploy and CapRover; as the `SearchEngines` parameter on AWS and the `search_engines` variable on Google Cloud; and in both `gateway.toml` and `console.toml` on Fly.io.

## What every path needs

| Need | How |
| --- | --- |
| A master key | `docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate`, kept in the platform's secret store and in a password manager; Coolify and Dokploy generate it, and you copy it into a password manager ([backups and master keys](../backups.md)) |
| Email | An SMTP server; `KAESO_MAIL_TRANSPORT=log` only for trying Kaeso ([email](../email.md)) |
| The first user | `KAESO_SETUP_EMAIL`: the gateway emails the one-time setup link at every start until the first user exists, since the gateway image has no shell for `kaeso setup-link`; on Coolify, Dokploy and CapRover, `docker exec` on the server also runs it ([configuration](../configuration.md#settings)) |
| Request logs | An external ClickHouse, such as ClickHouse Cloud, in `KAESO_CLICKHOUSE_URL`; without it, request logs are off and usage, limits and budgets work the same; Coolify, Dokploy and CapRover run ClickHouse beside Kaeso ([choose how to keep request logs](../installation.md#choose-how-to-keep-request-logs)) |
| A public address | `KAESO_PUBLIC_URL`, the `https` address people and apps use |

## Settings of the edge

`kaeso-edge` listens on `PORT`, 8080 by default, and reads these variables. Each template sets what its platform needs; set them yourself when you run the edge elsewhere.

| Variable | Default | What it does |
| --- | --- | --- |
| `KAESO_GATEWAY_UPSTREAM` | Required | The gateway's host and port, such as `kaeso-gateway:8080`; it gets the gateway's paths |
| `KAESO_CONSOLE_UPSTREAM` | Required | The console's host and port, such as `kaeso-console:3000`; it gets every other path |
| `KAESO_EDGE_TRUSTED_PROXIES` | `private_ranges` | The networks of the proxies in front of the edge, separated by spaces, such as `private_ranges 192.0.2.0/24`; `private_ranges` covers the private IPv4 and IPv6 ranges. From any other address, the edge takes that address as the client's |
| `KAESO_EDGE_CLIENT_IP_HEADERS` | `X-Forwarded-For` | One or more headers with the client's address, separated by spaces, read on requests from those networks; the edge uses the first one a request has |

The edge reads `X-Forwarded-For` from the right and skips the trusted networks, so the first other address is the client, even behind a proxy that appends to the header. A client inside the trusted networks shows up as the proxy, so narrow `KAESO_EDGE_TRUSTED_PROXIES` to the proxy's network when people reach Kaeso from a private network. When a CDN in front of the platform's proxy shows up in the header too, add the CDN's networks. Where the platform's proxy overwrites a header of its own, set `KAESO_EDGE_CLIENT_IP_HEADERS` to it: `CF-Connecting-IP` on Render, `Fly-Client-IP` on Fly.io, `X-Real-IP` on CapRover and Railway.

## What was validated

The edge, gateway and console were tested locally with Docker on 2026-11-06: path routing, chunked streams, forged client addresses and each API's error format while the gateway is away (`e2e/install/test_edge.py`). CI repeats the template, lint and format checks.

The quick servers were then deployed from empty cloud accounts on 2026-11-07. The AWS template ran as a CloudFormation stack in `us-east-1` and the Google Cloud configuration in a new project in `europe-west3`; both installed the eighth release candidate. The Google Cloud deployment created the first user, routed the console and APIs on one HTTPS origin, streamed five SSE events whose text was exactly `Kaeso stream works.`, wrote the request to ClickHouse, and ignored a forged `X-Forwarded-For` address. The AWS deployment created its first user and passed the same checks only after the key file was put back under the installer's ownership on the test instance. The template now leaves that ownership alone, but the corrected template has not been redeployed from an empty account.

The Render Blueprint was created from an empty Render project in Frankfurt on the same day. It created all five resources from the release-pinned `render.yaml`, emailed the one-time setup link, passed the edge, gateway and readiness checks, streamed the same completion and logged the request. The live test found that Render appends to the caller's `X-Forwarded-For`; the edge now reads Cloudflare's overwritten `CF-Connecting-IP` header on Render. The corrected header behavior passes the local edge integration test and the Render configuration check, but the corrected edge image has not been retested on a live Render deployment.

| Platform | Live empty-account test |
| --- | --- |
| Render | Blueprint, setup email, routing, streaming and logging passed on 2026-11-07; the corrected client-address path has local regression coverage, but no live retest |
| AWS quick server | Stack and app checks passed after correcting master-key ownership on the test instance on 2026-11-07; the corrected template has not been redeployed |
| Google Cloud quick server | Terraform configuration and setup-link path passed on 2026-11-07 |
| AWS and Google Cloud managed stacks | Linted and formatted by CI; not deployed in this pass |
| Railway | The file and the button's template each deployed to an empty project on 2026-11-09, with setup and sign-in through the edge; email delivery and streaming not tested; the edge's client-address settings pass the template check, but no live retest |
| DigitalOcean | Not deployed; App Platform's validation and proposal API accepted the button's spec on 2026-11-09 |
| Fly.io | Not deployed; the files pass the SchemaStore schema for `fly.toml` |
| Azure quick server | Not deployed; the template passes the ARM template test toolkit |
| Coolify, Dokploy | Not deployed on the platforms; both files ran locally with Docker on 2026-11-09, with healthy services, path routing and the setup link |
| CapRover | Not deployed; the template parses and uses only fields CapRover reads |
| Kubernetes | Installed with the bundled stores, served a request and upgraded without a failed request on a kind cluster in CI; not installed on a managed cluster |

## Next steps

- [Render](render.md): Deploy the blueprint with Render Postgres and Key Value.
- [Railway](railway.md): Deploy with the button, or apply the infrastructure file with the Railway CLI.
- [DigitalOcean](digitalocean.md): Deploy with the button, or create the app with a managed Valkey with `doctl`.
- [Fly.io](fly.md): Deploy four apps on the private network with `fly`, and Fly Managed Postgres.
- [AWS](aws.md): Launch one EC2 server, or ECS Fargate with an Application Load Balancer, RDS and ElastiCache.
- [Azure](azure.md): Launch one virtual machine from the Deploy to Azure button or the Azure CLI.
- [Google Cloud](gcp.md): Launch one Compute Engine server, or Cloud Run with a load balancer, Cloud SQL and Memorystore.
- [Coolify](coolify.md): Deploy the Compose file with Coolify's proxy and generated passwords.
- [Kubernetes](kubernetes.md): Install the Helm chart with stores you bring, or bundled ones for trying Kaeso out.
- [Dokploy](dokploy.md): Import the template with Traefik and generated passwords.
- [CapRover](caprover.md): Deploy the one-click app from the template.
