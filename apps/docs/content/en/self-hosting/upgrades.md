---
title: Upgrades
description: Learn about new releases, update by hand or in one click with the updater, and what happens while Kaeso updates.
order: 6
---

Releases follow semantic versioning, and every part of Kaeso shares the release's version number. Each release's notes are on the [releases page](https://github.com/logicplanes/kaeso/releases) and, for instance admins, in the console under **Instance settings → Updates**. Each release's `compose.yaml` pins the gateway, console and updater images of that release.

There are two ways to update: [by hand](#update-by-hand), which always works, and [in one click](#one-click-updates) from the console, once you add the optional updater.

## New releases

The gateway looks for new releases twice a day, at a random minute within the hour, on GitHub's releases API. It asks with the `ETag` of the last answer, so an unchanged list costs GitHub nothing; after a failure it waits at least an hour, longer when GitHub asks for it, and it never retries at once. The request carries nothing about your install but the gateway's version in its `User-Agent`.

Under **Instance settings → Updates** instance admins see the version that runs, the newest release with the notes of every release since, when the gateway last looked and, when it failed, why. **Check now** looks at once, even when checks are off. A **New** mark beside **Updates** in the settings sidebar is the only reminder; Kaeso never sends emails or banners about releases.

| Setting | Default | What it does |
| --- | --- | --- |
| Look for new releases (`updates.check`) | on | Turn it off for installs without internet access. |
| Releases (`updates.channel`) | stable only | `prerelease` also offers release candidates. A pre-release is never offered on the stable channel, but an install that runs one is offered the stable release that follows it. |
| One-click updates (`updates.apply`) | off | Whether instance admins may apply updates from the console through the updater. |

The three settings start from `KAESO_UPDATES_CHECK`, `KAESO_UPDATES_CHANNEL` and `KAESO_UPDATES_APPLY` on a new install; after that, instance admins change them in the console, and each change is in the instance audit log. Turning one-click updates on needs a sign-in within the last 15 minutes.

### Installs without internet access

Turn **Look for new releases** off, or set `KAESO_UPDATES_CHECK=false` in [`compose.override.yaml`](installation.md#local-changes) before the first start. The page then shows the running version and updates by hand as below. `KAESO_UPDATES_RELEASES_URL` points the gateway at a mirror that answers like GitHub's releases API, if you keep one.

## Which releases update in one click

A release updates in one click only when all of these hold; otherwise the console shows its notes and a link to this page:

- It is not a new major version. A migration that needs a manual step makes a release a major one.
- No release on the way asks for steps by hand. A release does when its `compose.yaml` changes beyond the image versions, or when the maintainers mark it so.
- The release carries `release.json`, the manifest the updater reads: every release from the first one with the updater on.

## Update by hand

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

The console shows these commands with the exact version whenever one click isn't possible.

## One-click updates

The updater is an optional service of `compose.yaml`, in the Compose profile `updater`, with its Docker socket proxy `docker-proxy`. It is the only part of Kaeso that talks to Docker.

### Turn it on

Give the gateway and the updater a shared token and turn the profile on, in the install's folder:

```bash
echo "KAESO_UPDATER_TOKEN=$(openssl rand -hex 32)" >> .env
echo "COMPOSE_PROFILES=updater" >> .env
docker compose up -d --wait
```

Then turn on **One-click updates** under **Instance settings → Updates**. The page says when the updater isn't installed, doesn't answer or is too old for a release. Kaeso releases before the updater update by hand once.

The updater mounts `/var/run/docker.sock` through its proxy; set `DOCKER_SOCKET` in `.env` when Docker's socket is elsewhere, as with rootless Docker.

### What happens

**Update to …** asks for a confirmation that names the version and reminds you to back up first. The gateway records the update, writes `instance.update_start` to the instance audit log and hands the version, and nothing else, to the updater. The console then follows these steps:

1. **Download.** The updater fetches `release.json` and `compose.yaml` from the release on github.com over TLS and pulls the gateway, console and updater images by the digests in `release.json`. Kaeso keeps running.
2. **Check the signatures.** For `release.json` and every image, the updater fetches the [build provenance attestations](https://docs.github.com/actions/security-for-github-actions/using-artifact-attestations) of the Release workflow and verifies them with Sigstore: a certificate the Sigstore certificate authority issued to `logicplanes/kaeso`'s Release workflow on `prod`, its certificate transparency proof, the transparency log's signed entry, the signature over the statement, the digest it names and the commit `release.json` names. The image's own version label has to match too. The trust root ships inside the updater; nothing is installed when any check fails.
3. **Switch.** Caddy answers with the [updating page](#while-kaeso-updates). The updater stops the console, starts it again from its own settings with the new image and waits for its health check; then it does the same with the gateway.
4. **Migrate.** The new gateway migrates the databases when it starts; the updater waits up to 10 minutes for its health check.
5. **Back online.** The updater writes the new versions into `compose.yaml`, so a later `docker compose up -d --wait` starts the same images, tags them with the version, removes the old containers and ends the maintenance.

The whole update takes about a minute. Only one update runs at a time. The result is recorded as `instance.update_finish` in the instance audit log and in the page's history: **Updated**, **Not installed** (nothing changed), **Rolled back** or **Rollback failed**.

### Rollback

When a new container doesn't become healthy, the updater removes it and starts the previous containers again: the update is **Rolled back**, and Kaeso runs the version from before. When the new gateway has already migrated the database, the previous gateway refuses it, as every gateway refuses a database a newer version migrated, and the update is **Rollback failed**. Then either start the new version by hand and read its log (`docker compose logs gateway`), or restore the backup and run the old version, as in [downgrade](#downgrade).

When the updater restarts during an update, it rolls the update back when it starts again, or finishes it when only the cleanup was left.

### Security model

| Part | What it can do |
| --- | --- |
| Gateway | Asks the updater to apply a version, by number, and reads its progress. It never gets Docker access, and it can't name images, options or containers. |
| Updater | Reachable only on the internal network `updates`, which only the gateway shares, with the token from `.env`: at least 32 characters, compared in constant time, never logged or returned. It only replaces the gateway and console containers of its own Compose project, from their own settings, with images it pinned by digest and verified, and it only changes the image versions in `compose.yaml`, which it refuses to touch when the file differs from the release's in anything else. |
| `docker-proxy` | [socket-proxy](https://github.com/wollomatic/socket-proxy) on the internal network `docker`. It answers only the updater, and only the Docker API calls the updater makes: listing and inspecting containers, inspecting, pulling and tagging Kaeso images, and creating, starting, stopping, renaming and removing containers. |

Whoever controls Docker controls the server, so the updater is opt-in, runs with every capability dropped except the one it needs to write `compose.yaml`, and shares those networks only with the gateway and the proxy. Without the `updater` profile nothing in Kaeso has Docker access.

## While Kaeso updates

While the updater switches versions, and whenever the gateway or the console doesn't answer, for an update by hand too, Caddy answers in their place:

| Request | Answer |
| --- | --- |
| A page | `503` with a page in English, German or Simplified Chinese, after the browser's language, in light or dark after the system. It names the step while the updater works, checks every 5 seconds and returns to the console when Kaeso is back. It loads nothing from elsewhere. |
| `/platform/v1/*` | `503` with the Platform API's error, type `unavailable` |
| `/v1/messages`, and other `/v1/*` with an `anthropic-version` header | `529` with Anthropic's `overloaded_error`, as the gateway answers when a provider is overloaded |
| Other `/v1/*` | `503` with OpenAI's `service_unavailable_error` |

Every answer carries `Retry-After: 30`, which the OpenAI and Anthropic SDKs honour when they retry. The message says whether Kaeso is being updated or can't be reached right now.

## Names that stay

These names are the same in every release, so downloading the new `compose.yaml` and running `docker compose pull` keeps your data:

- the download address `https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml`;
- the images `ghcr.io/logicplanes/kaeso-gateway`, `ghcr.io/logicplanes/kaeso-console` and `ghcr.io/logicplanes/kaeso-updater`, tagged with the release's version;
- the Compose project `kaeso` and its services `caddy`, `gateway`, `console`, `postgres`, `clickhouse` and `valkey`, and `updater` and `docker-proxy` in the `updater` profile;
- the volumes `kaeso_postgres`, `kaeso_clickhouse` and `kaeso_valkey`, which hold the data, `kaeso_caddy_data` and `kaeso_caddy_config`, which hold the TLS certificates, and `kaeso_maintenance` and `kaeso_updater`, which hold the maintenance flag and the updater's progress.

Run Compose in your install's folder without `-p` or `COMPOSE_PROJECT_NAME`, so it uses the project name `kaeso`: under another project name, Compose starts with new, empty volumes.

## Downgrade

Migrations can't be undone. A gateway refuses to start against a PostgreSQL database that a newer release migrated, and says so. Against a ClickHouse database a newer release migrated, it starts but writes no request logs, and logs `ClickHouse migrations failed; request logs are not written`.

To go back to an older release, restore the PostgreSQL backup taken before the upgrade, and the [ClickHouse backup](backups.md#clickhouse) too if the newer release migrated ClickHouse, then start the older `compose.yaml`.
