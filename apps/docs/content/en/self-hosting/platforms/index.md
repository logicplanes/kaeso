---
title: Managed platforms
description: Run Kaeso on Render, Railway, AWS or Google Cloud with the templates in the repository.
order: 12
---

The Compose package runs Kaeso on one server you manage. The templates in the repository run the same release on a managed platform instead, where the platform terminates TLS, keeps the databases and rolls out new versions.

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

## Two kinds of path

| Path | What it is | Where |
| --- | --- | --- |
| Platform template | One file the platform reads, with a public edge, a private gateway and console, and the platform's PostgreSQL and Valkey | [Render](render.md), [Railway](railway.md) |
| Quick server | One virtual machine with the Compose package, Caddy, bundled ClickHouse and the updater, as on your own server | [AWS](aws.md), [Google Cloud](gcp.md) |
| Managed production | The platform's containers, load balancer, PostgreSQL and Valkey, with secrets in its secret store | [AWS](aws.md), [Google Cloud](gcp.md) |

AWS and Google Cloud are not one click: you choose a region, a domain and its certificate, the sizes of the data stores, where request logs go, an SMTP server and where the master key lives.

## What every path keeps

- **One origin.** The console and the APIs answer on one domain. `/v1/*`, `/platform/*`, `/mcp`, `/mcp/*`, `/oauth/*` and `/.well-known/oauth-*` reach the gateway; every other path reaches the console. On Render and Railway, `kaeso-edge`, a second command in the console image, does this; on AWS and Google Cloud, the load balancer's path rules do.
- **Streams stay unbuffered.** The edge flushes every chunk, and a load balancer's idle timeout is longer than the gateway's keepalive interval ([long streams](../domains-and-tls.md#long-streams)).
- **The client address is believed only from the platform's proxy.** The edge takes `X-Forwarded-For` only from the platform's private network and sends the gateway one address; the gateway trusts the edge alone ([`KAESO_TRUSTED_PROXIES`](../configuration.md#settings)).
- **Health checks** use `/healthz` for the process and `/readyz` for PostgreSQL, Valkey and ClickHouse ([health and metrics](../monitoring.md)). The metrics port is never public.
- **The updater is off.** The platform rolls a new release out and back; a release's migrations only move forward ([upgrades](../upgrades.md)).
- **One version.** Gateway, console and edge are pinned to the same release; a release updates the templates.

## What every path needs

| Need | How |
| --- | --- |
| A master key | `docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate`, kept in the platform's secret store and in a password manager ([backups and master keys](../backups.md)) |
| Email | An SMTP server; `KAESO_MAIL_TRANSPORT=log` only for trying Kaeso ([email](../email.md)) |
| The first user | `KAESO_SETUP_EMAIL`: the gateway emails the one-time setup link at every start until the first user exists, since the gateway image has no shell for `kaeso setup-link` ([configuration](../configuration.md#settings)) |
| Request logs | An external ClickHouse, such as ClickHouse Cloud, in `KAESO_CLICKHOUSE_URL`; without it, request logs are off and usage, limits and budgets work the same ([choose how to keep request logs](../installation.md#choose-how-to-keep-request-logs)) |
| A public address | `KAESO_PUBLIC_URL`, the `https` address people and apps use |

## What was validated

The edge, the gateway and the console were tested in this layout with the release images, locally with Docker, on 2026-11-06: path routing, chunked streams, forged client addresses and each API's error format while the gateway is away (`e2e/install/test_edge.py`). The Render blueprint and the Railway file were checked against the platforms' specifications, the CloudFormation templates with `cfn-lint`, and the Terraform with `tofu validate`; CI repeats every check.

None of the four paths has yet been deployed on its platform from an empty account. Until that test exists, there is no deploy button, and each page says what it is based on.

## Next steps

- [Render](render.md): Deploy the blueprint with Render Postgres and Key Value.
- [Railway](railway.md): Apply the infrastructure file with Railway Postgres and Redis.
- [AWS](aws.md): Launch one EC2 server, or ECS Fargate with an Application Load Balancer, RDS and ElastiCache.
- [Google Cloud](gcp.md): Launch one Compute Engine server, or Cloud Run with a load balancer, Cloud SQL and Memorystore.
