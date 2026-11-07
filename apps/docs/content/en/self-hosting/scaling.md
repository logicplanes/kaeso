---
title: Several gateways
description: Run more than one gateway behind Caddy or a load balancer, and know what they share.
order: 13
---

One gateway serves thousands of connections. When one isn't enough, or one must keep serving while another restarts, run several. They share everything a request depends on, so any gateway can answer any request.

## What gateways share

Every gateway of an install reads the same PostgreSQL, Valkey and ClickHouse, and keeps nothing a request needs to itself:

| Shared through | What |
| --- | --- |
| PostgreSQL | Users, sessions, organizations, projects, keys, catalogues, limits, usage and audit logs; a session signed in on one gateway is valid on every other |
| Valkey | Rate, token and budget counters, so a limit holds across all gateways; the [response cache](../console/response-cache.md); reasoning continuity; the shared caches of keys, catalogues and settings, with invalidations every gateway receives within a second; which [alias route](../concepts/alias-routes.md#steps-that-are-skipped) steps just failed, so one gateway's failed call spares the others |
| PostgreSQL locks | Background jobs: the KPMR sync, update checks, notifications and cleanups run on one gateway at a time, never once per gateway |

Each gateway keeps to itself only what belongs to its process: the request log rows it writes every second and the usage it records every 10 seconds, its memory budget for translating streams, and its metrics. Scrape every gateway's `/metrics`; the counters add up ([health and metrics](monitoring.md#metrics)).

## In the Compose package

Set the number of gateways in `.env` and apply it:

```bash
echo GATEWAY_REPLICAS=2 >> .env
docker compose up -d --wait
```

Compose starts `gateway-1`, `gateway-2` and so on, each with its own memory and process limits ([container limits](configuration.md#container-limits)). Caddy resolves `gateway` through Docker's DNS every second, sends each request to the gateway with the fewest open connections, and skips one that fails for 5 seconds. `docker compose exec gateway` reaches the first one, which is enough for `kaeso setup-link` and the other commands.

The updater handles every replica: it stops all of them, then starts the new ones one by one and waits for each to be healthy, so no old gateway runs against a database the new one is migrating ([the updater](upgrades.md#the-updater)).

## What it brings

Measured on 2026-11-07 with Docker 29.5 on an Apple M-series Mac, each gateway limited to one CPU, Caddy in front, the mock provider behind, and 64 connections sending chat completions for 20 seconds: non-streamed in three runs (the middle one), streamed in two (the lower one):

| Gateways | Requests per second | Median | 99th percentile | Streams per second | Stream median |
| --- | --- | --- | --- | --- | --- |
| 1 | 7,951 | 7.7 ms | 16.6 ms | 2,614 | 23.9 ms |
| 2 | 11,910 | 5.1 ms | 11.2 ms | 4,561 | 13.8 ms |
| 3 | 16,143 | 3.6 ms | 9.5 ms | 7,355 | 8.3 ms |

Three gateways serve twice the completions of one at half the latency, and 2.8 times the streams, whose translation costs CPU on every event. Completions gain less than the number of gateways because Caddy, the mock provider and the load generator shared the same machine; on separate hosts, or with a CPU each, the gain stays close to it. Long streams that wait on a real provider gain capacity as well: each gateway holds its own `KAESO_SERVER_MAX_CONNECTIONS` ([configuration](configuration.md#settings)).

A gateway that restarts, for an update or after a crash, costs nothing while the others serve. Caddy stops sending to it within 5 seconds and returns once it is healthy.

## On a managed platform

The [platform templates](platforms/index.md) take the number of gateways as `GatewayCount` on AWS and `gateway_min_instances` on Google Cloud and `instance_count` on DigitalOcean; Render and Railway scale a service from their dashboards. The load balancer in front needs nothing sticky, because nothing a request depends on is kept on one gateway.

## Next steps

- [Configuration](configuration.md): `GATEWAY_REPLICAS` and the limits each gateway gets.
- [Health and metrics](monitoring.md): Scrape every gateway and read the counters that add up.
- [Upgrades](upgrades.md): How the updater replaces every replica.
