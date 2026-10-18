---
title: Backups and master keys
description: Back up the databases and master keys, restore them, and rotate the master key.
nav: Backups
order: 9
---

Back up three things, and keep the master keys apart from the database backups:

| What | Holds | Without a backup |
| --- | --- | --- |
| PostgreSQL | Users, organizations, projects, keys, encrypted provider keys, catalogues, limits, usage and audit logs | The install is gone |
| ClickHouse | Request logs and usage rollups | Log history is gone; limits and budgets don't depend on it |
| Master keys | The keys that encrypt stored provider keys | Every provider key must be entered again |

Valkey holds only counters, caches and short-lived data, and needs no backup. The gateway rebuilds missing budget counters from PostgreSQL, and rate windows start over.

## Back up PostgreSQL

Dump the database while Kaeso runs:

```bash
docker compose exec -T postgres pg_dump -U kaeso -Fc kaeso > kaeso-$(date +%F).dump
```

## Restore PostgreSQL

Stop the gateway, restore the dump and start the gateway again. `--clean --if-exists` replaces whatever the database holds, so the same commands work on an existing install and on an empty database:

```bash
docker compose stop gateway
docker compose exec -T postgres pg_restore -U kaeso -d kaeso --clean --if-exists < kaeso-2026-10-15.dump
docker compose start gateway
```

To restore on a new server:

1. Put `compose.yaml`, `.env` and `master-keys` back in the install's folder.
2. Start only PostgreSQL with `docker compose up -d --wait postgres`.
3. Restore the dump with the `pg_restore` command above.
4. Start the rest with `docker compose up -d --wait`.

## Back up ClickHouse

Use ClickHouse's `BACKUP` command with a disk set up for backups, or a tool such as `clickhouse-backup`. On a small install, you can also stop ClickHouse and copy its volume:

```bash
docker compose stop clickhouse
```

Copy the `clickhouse` volume with your usual volume backup, then run `docker compose start clickhouse`. While ClickHouse is down, requests keep working, and their log rows wait in the gateway's memory, up to `KAESO_LOGS_BUFFER_SIZE`.

## Keep the master keys safe

The master keys are the file `master-keys` next to `compose.yaml`. A stored provider key records only the id of the master key that encrypts it, so a database backup can't decrypt provider keys without its master keys. Keep a copy in a password manager or a secrets store, not next to the database dumps.

## Rotate the master key

`master-keys` holds one or more keys separated by commas, the active key first: `k7c21e9d04a:…,k3f9a01bc2d:…`. Kaeso encrypts new secrets with the first key and decrypts older ones with theirs.

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

The command reports how many secrets each key id still encrypts, and which old ids nothing uses any more.

4. Remove the unused old key from `master-keys`, and recreate the gateway with the command from step 2. With several gateway instances, wait until every instance runs with the new list first.

> [!WARNING]
> The gateway doesn't start while stored secrets use a key id that isn't configured. Never remove a key before `rewrap` reports it unused.

## Next steps

- [Upgrades](upgrades.md): Update to a new release, with a backup to go back to.
- [Health and metrics](monitoring.md): Watch the install for problems.
