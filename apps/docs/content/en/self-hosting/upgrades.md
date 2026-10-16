---
title: Upgrades
description: Move an install to a new release.
order: 6
---

Releases follow semantic versioning, and every part of Kaeso shares the release's version number. Each release's notes are on the [releases page](https://github.com/logicplanes/kaeso/releases), and its `compose.yaml` pins the gateway and console images of that release.

## Upgrade

1. Read the release notes for anything that needs action.
2. [Back up](backups.md) PostgreSQL.
3. Download the new release's `compose.yaml` over the old one. Your settings in `.env`, `master-keys` and `compose.override.yaml` stay.

```bash
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

4. Start the new version:

```bash
docker compose pull
docker compose up -d
```

The gateway migrates PostgreSQL and ClickHouse when it starts, under a lock, so several instances can start at once and only one migrates. Migrations only move forward and run in order, so an upgrade may skip versions.

## Downgrade

A gateway refuses to start against a database that a newer version migrated, and says so. To go back to an older version, restore the backup taken before the upgrade, then start the older `compose.yaml`.
