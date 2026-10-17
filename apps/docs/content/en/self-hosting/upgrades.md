---
title: Upgrades
description: Move an install to a new release.
order: 6
---

Releases follow semantic versioning, and every part of Kaeso shares the release's version number. Each release's notes are on the [releases page](https://github.com/logicplanes/kaeso/releases), and its `compose.yaml` pins the gateway and console images of that release.

## Upgrade

1. Read the release notes for anything that needs action.
2. [Back up](backups.md) PostgreSQL, and ClickHouse too, so that a [downgrade](#downgrade) stays possible.
3. Download the new release's `compose.yaml` over the old one. Your settings in `.env`, `master-keys` and `compose.override.yaml` stay.

```bash
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

4. Start the new version:

```bash
docker compose pull
docker compose up -d --wait
```

`--wait` returns once the new gateway and console report healthy, and fails if one doesn't, so a script or a person running the upgrade sees a failed start at once.

The gateway migrates PostgreSQL and ClickHouse when it starts, under a lock, so several instances can start at once and only one migrates. Migrations only move forward and run in order, so an upgrade may skip versions. Kaeso 0.1.0 creates the whole schema from one initial set of migrations; later releases add theirs after it.

## Names that stay

These names are the same in every release, so downloading the new `compose.yaml` and running `docker compose pull` keeps your data:

- the download address `https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml`;
- the images `ghcr.io/logicplanes/kaeso-gateway` and `ghcr.io/logicplanes/kaeso-console`, tagged with the release's version;
- the Compose project `kaeso` and its services `caddy`, `gateway`, `console`, `postgres`, `clickhouse` and `valkey`;
- the volumes `kaeso_postgres`, `kaeso_clickhouse` and `kaeso_valkey`, which hold the data, and `kaeso_caddy_data` and `kaeso_caddy_config`, which hold the TLS certificates.

Run Compose in your install's folder without `-p` or `COMPOSE_PROJECT_NAME`, so it uses the project name `kaeso`: under another project name, Compose starts with new, empty volumes.

## Downgrade

Migrations can't be undone. A gateway refuses to start against a PostgreSQL database that a newer release migrated, and says so. Against a ClickHouse database a newer release migrated, it starts but writes no request logs, and logs `ClickHouse migrations failed; request logs are not written`.

To go back to an older release, restore the PostgreSQL backup taken before the upgrade, and the [ClickHouse backup](backups.md#clickhouse) too if the newer release migrated ClickHouse, then start the older `compose.yaml`.
