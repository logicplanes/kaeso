---
title: Backups and master keys
description: What to back up, how to restore it, and how to rotate the master key.
order: 7
---

Back up three things, and keep the master keys apart from the database backups.

| What | Holds | Without it |
| --- | --- | --- |
| PostgreSQL | Users, organizations, projects, keys, encrypted provider keys, catalogues, limits, usage records, audit logs | The install is gone |
| ClickHouse | Request logs and usage rollups | Log history is gone; limits and budgets don't depend on it |
| Master keys | The keys that encrypt stored provider keys | Provider keys must be entered again |

Valkey holds only counters and caches. Each limit has counters of its own; budget counters are rebuilt from PostgreSQL when they are missing, and rate windows start over.

## PostgreSQL

Dump the database while Kaeso runs:

```bash
docker compose exec -T postgres pg_dump -U kaeso -Fc kaeso > kaeso-$(date +%F).dump
```

Restore into an empty database, with the gateway stopped:

```bash
docker compose stop gateway
docker compose exec -T postgres pg_restore -U kaeso -d kaeso --clean < kaeso-2026-10-15.dump
docker compose start gateway
```

## ClickHouse

Use ClickHouse's `BACKUP` command to a disk configured for backups, or a tool such as `clickhouse-backup`. For a small install, stopping ClickHouse and copying its volume also works:

```bash
docker compose stop clickhouse
```

Copy the `clickhouse` volume with your usual volume backup, then start it again with `docker compose start clickhouse`. While ClickHouse is down, requests keep working and log rows wait in the gateway's memory, up to `KAESO_LOGS_BUFFER_SIZE`.

## Master keys

The master keys are the file `master-keys` next to `compose.yaml`, which the gateway reads as the Docker secret `master_keys`. Each stored provider key records the id of the master key that encrypts it, never the key itself, so a database backup without its master key can't decrypt provider keys.

Keep a copy in a password manager or a secrets store, not next to the database dumps.

## Rotating the master key

`master-keys` holds one or more keys, separated by commas, the active key first: `k7c21e9d04a:…,k3f9a01bc2d:…`. New secrets use the first key; older ones still decrypt with theirs.

1. Generate a new key and put it first, before the old one:

```bash
printf '%s,%s\n' "$(docker compose exec -T gateway kaeso master-key generate)" "$(cat master-keys)" > master-keys.new
mv master-keys.new master-keys
chmod 644 master-keys
```

2. Restart the gateway, so it reads the new file:

```bash
docker compose up -d --force-recreate gateway
```

3. Re-encrypt every stored secret with the new key:

```bash
docker compose exec gateway kaeso master-key rewrap
```

It reports how many secrets each key id still encrypts and which old ids nothing uses any more.

4. When the old id is unused, remove it from `master-keys` and restart the gateway again. With several gateway instances, remove it only once every instance runs with the new list.

> [!WARNING]
> The gateway refuses to start when stored secrets use a key id that isn't configured, and names the id. Never remove a key before `rewrap` reports it unused.
