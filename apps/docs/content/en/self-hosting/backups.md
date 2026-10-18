---
title: Backups and master keys
description: What to back up, how to restore it, and how to rotate the master key.
order: 9
---

Back up three things, and keep the master keys apart from the database backups.

| What | Holds | Without it |
| --- | --- | --- |
| PostgreSQL | Users, organizations, projects, keys, encrypted provider keys, catalogues, limits, usage records, audit logs | The install is gone |
| ClickHouse | Request logs and usage rollups | Log history is gone; limits and budgets don't depend on it |
| Master keys | The keys that encrypt stored provider keys | Every provider key must be entered again |

Valkey holds only counters, caches and short-lived data, and needs no backup: budget counters are rebuilt from PostgreSQL when they are missing, and rate windows start over.

## PostgreSQL

Dump the database while Kaeso runs:

```bash
docker compose exec -T postgres pg_dump -U kaeso -Fc kaeso > kaeso-$(date +%F).dump
```

Restore with the gateway stopped. `--clean --if-exists` replaces whatever the database holds, so the same command works on an existing install and on an empty database:

```bash
docker compose stop gateway
docker compose exec -T postgres pg_restore -U kaeso -d kaeso --clean --if-exists < kaeso-2026-10-15.dump
docker compose start gateway
```

On a new server, put `compose.yaml`, `.env` and `master-keys` back first, start only PostgreSQL with `docker compose up -d --wait postgres`, restore, then start the rest with `docker compose up -d --wait`.

## ClickHouse

Use ClickHouse's `BACKUP` command with a disk configured for backups, or a tool such as `clickhouse-backup`. On a small install, stopping ClickHouse and copying its volume also works:

```bash
docker compose stop clickhouse
```

Copy the `clickhouse` volume with your usual volume backup, then run `docker compose start clickhouse`. While ClickHouse is down, requests keep working and request log rows wait in the gateway's memory, up to `KAESO_LOGS_BUFFER_SIZE`.

## Master keys

The master keys are the file `master-keys` next to `compose.yaml`, which the gateway reads as the Docker secret `master_keys`. A stored provider key records only the id of the master key that encrypts it, never the key itself, so a database backup without its master keys can't decrypt provider keys. Keep a copy in a password manager or a secrets store, not next to the database dumps.

## Rotating the master key

`master-keys` holds one or more keys separated by commas, the active key first: `k7c21e9d04a:…,k3f9a01bc2d:…`. New secrets are encrypted with the first key; older ones still decrypt with theirs.

1. Generate a new key and put it in front of the old one:

```bash
printf '%s,%s\n' "$(docker compose exec -T gateway kaeso master-key generate)" "$(cat master-keys)" > master-keys.new
mv master-keys.new master-keys
chmod 644 master-keys
```

2. Recreate the gateway, so it reads the new file:

```bash
docker compose up -d --wait --force-recreate gateway
```

3. Re-encrypt every stored secret with the new key:

```bash
docker compose exec gateway kaeso master-key rewrap
```

It reports how many secrets each key id still encrypts, and which old ids nothing uses any more.

4. Once the old id is unused, remove it from `master-keys` and recreate the gateway with the same command as in step 2. With several gateway instances, remove it only after every instance runs with the new list.

> [!WARNING]
> The gateway refuses to start when stored secrets use a key id that isn't configured, and names the id. Never remove a key before `rewrap` reports it unused.
