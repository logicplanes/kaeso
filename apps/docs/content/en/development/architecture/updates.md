---
title: Updates
description: How the gateway learns about releases, what a release carries for the updater, and how the updater verifies and applies one.
order: 5
---

```text
GitHub releases API ◀── gateway (update_check job, instance/updates/*) ──▶ PostgreSQL
                              │  bearer token, internal network "updates"
                              ▼
                           updater ──▶ github.com (release.json, compose.yaml), api.github.com (attestations), ghcr.io
                              │  internal network "docker"
                              ▼
                         docker-proxy ──▶ /var/run/docker.sock
                              │
updater ──▶ maintenance volume ◀── Caddy (updating page, JSON 503s)
```

## The gateway

`services/updates.rs` and the `update_check` job look for releases: every five minutes the job asks whether the next check is due (`update_settings.next_check_at`), and a check fetches `releases_url?per_page=50` with `If-None-Match`. A changed list replaces `update_releases` in one transaction; for every release newer than the running version, its `release.json` is fetched and stored, or the reason it couldn't be. At most twelve manifests are fetched per check, four at a time, newest first; the rest wait for the next check, which then asks without `If-None-Match`. A manifest is fetched only from `https://github.com/logicplanes/kaeso/releases/download/`, or from the origin of `updates.releases_url` when it points at a mirror, and the client never follows a redirect from HTTPS to HTTP. The next check is 12 hours later plus a random part of an hour, after a failure at least an hour later, or later still when GitHub's `Retry-After` or `x-ratelimit-reset` says so; until then `update_settings.blocked_until` keeps **Check now** from asking too. A repeated failure is logged at debug level only.

`kaeso_update::policy::plan` chooses what to offer: the newest release on the channel that is newer than the running version, and whether it is manual: a new major version, `manual: true` in any crossed release's manifest (pre-releases included), or no manifest at all; the target's `manual_since` counts too, so a crossed release the gateway never saw still makes it manual. The Platform API under `instance/updates` is for instance admins only; every change is audited (`instance.update_check`, `instance.update_settings`, `instance.update_start`, `instance.update_finish`), and `apply` needs a sign-in within 15 minutes.

`update_jobs` holds every update; a unique index allows one `running` row at a time. `apply` commits the row before it calls the updater. When the updater refuses (any HTTP answer but `202`) or can't be connected to, the row ends `failed` at once, with `updater_refused` or `updater_unreachable`; when the request went out but no clear answer came back, the row stays `running` and the updater's own record decides. While a row runs, reading it asks the updater for its state and records the end once, with `instance.update_finish`, whichever gateway reads it first: usually the new one, after the switch. A job the updater still doesn't know two minutes after it started ends as `interrupted`; one the updater hasn't answered about for an hour ends as `updater_unreachable`.

## What a release carries

The Release workflow builds three images, `kaeso-gateway`, `kaeso-console` and `kaeso-updater`, and attests their build provenance with `actions/attest-build-provenance`. The `release-manifest` job writes `release.json` (`scripts/release-manifest.mjs`); the `attest-manifest` job, the only one besides the image manifests with `id-token: write`, sees nothing but that file and attests it; the `github-release` job then checks `release.json` and every image with `gh attestation verify` against this workflow on `refs/heads/prod` and the commit `release.json` names, and creates the release only when all of them pass:

```json
{
  "schema": 1,
  "version": "1.4.2",
  "commit": "5388ad0b98aa3d957fea99e200259842bf746680",
  "manual": false,
  "manual_since": "1.3.0",
  "updater_protocol": 1,
  "images": {
    "gateway": { "name": "ghcr.io/logicplanes/kaeso-gateway", "digest": "sha256:…" },
    "console": { "name": "ghcr.io/logicplanes/kaeso-console", "digest": "sha256:…" },
    "updater": { "name": "ghcr.io/logicplanes/kaeso-updater", "digest": "sha256:…" }
  },
  "compose": { "name": "compose.yaml", "sha256": "…" },
  "migrations": { "postgres": "20261017120001_updates", "clickhouse": "0002_usage_rollups" }
}
```

`commit` is the commit the Release workflow ran on, which the attestations name, not the tag's release commit on top of it; the script refuses any other. `manual` is true when the Release workflow's **manual** input says so, or when `compose.yaml` differs from the previous release's in anything but the image versions. `manual_since` names the newest release up to this one with `manual: true`, carried over from the previous release's `release.json`. `migrations` names the latest PostgreSQL and ClickHouse migration of the release. `kaeso_update::manifest` parses it, refuses another schema, and checks every name, digest, migration, `manual_since` and the commit.

## The updater

`crates/kaeso-updater` is a small axum server on port 8081: `GET /healthz` without a token, and `GET /v1/status`, `POST /v1/updates` (`{ "id", "version" }`) and `GET /v1/updates/{id}` with `Authorization: Bearer` and the shared token. It runs one update at a time and keeps it in `/state/job.json`, written atomically after every change; an update whose first state can't be written is refused with `503`.

An update (`engine.rs`):

1. **Verifying.** It downloads `release.json` and verifies it before anything else, refuses an updater protocol it doesn't speak, finds the gateway and console containers of its own Compose project (from its own container's labels; one-off `docker compose run` containers don't count, and a `…-previous` container left from an earlier update stops it), reads the running version from the gateway image's `org.opencontainers.image.version`, refuses an older, equal or new major version and a release whose `manual` or `manual_since` asks for steps by hand after the running version, checks that the local `compose.yaml` with the new image versions equals the release's byte for byte and keeps a copy of it in `/state`, and verifies the index of every image (`verify.rs`): the attestations from `api.github.com/repos/logicplanes/kaeso/attestations/sha256:…`, each a Sigstore bundle, verified with [sigstore-rs](https://github.com/sigstore/sigstore-rs) against the trust root in `data/trusted_root.json` (the certificate chain to Fulcio, the SCT, the identity `https://github.com/logicplanes/kaeso/.github/workflows/release.yml@refs/heads/prod` from the issuer `https://token.actions.githubusercontent.com`, the DSSE signature, the transparency log entry), plus the log's signed entry timestamp, which the updater checks itself with the Rekor key, and the in-toto statement: SLSA provenance v1, the subject's name and digest, the workflow and the commit `release.json` names. Every Fulcio authority, CT log and Rekor key counts only within its `validFor` in the trust root, at the entry's integrated time. A bundle without a signed entry timestamp, as Rekor v2 writes them, is refused with a message that says so. For images, the subject is the image index fetched from `ghcr.io` by digest.
2. **Downloading.** It pulls the three images by digest; each pulled image's version label must equal the release. It then fetches the running release's `release.json`, verifies it the same way and compares the migrations; when they differ, or that manifest can't be had, the update counts as migrating the database.
3. **Switching.** It writes the maintenance flag, then for the console and then the gateway records the container, stops it, renames it `…-previous`, records that it is creating the new one, creates it from the old one's settings (`clone.rs`: everything the old image brought is dropped, so the new image's environment, command and health check apply; the gateway's health check gets a start period as long as the migration timeout, marked with a label so the next update can take it off again and compare the check with the image's) and starts it.
4. **Migrating.** It waits for each health check: 2 minutes for the console, 10 for the gateway's migrations. A crash, a restart or a stopped container fails at once; `unhealthy` fails the console at once but not the gateway, which may report it during a long migration and only fails when the 10 minutes are over.
5. **Online.** It reads `compose.yaml` again and writes the new versions when the file is still the one it checked (or already written); otherwise it leaves it and says so. After a restart it also repairs a file that is empty or the beginning of the copy or of the new file, as a torn write leaves it, but no other edit. It tags the images, removes the previous containers and clears the flag.

A failure before the switch changes nothing (`failed`). A failure after it starts the previous containers again in reverse order and waits for them (`rolled_back`, or `rollback_failed` with the reason); a container created before the updater could record it is found by its name and removed. When the gateway failed after it started, or Docker started it though the start call failed, and the update counts as migrating the database, nothing is restored: the new gateway and the new console, which speaks its API, stay, the previous ones stay stopped as `…-previous`, and the update ends `needs_attention`. When `compose.yaml` can't be written or changed meanwhile, the update still ends `succeeded`, with `compose_unwritable` or `compose_changed` as its reason. On start, the updater waits up to two minutes for Docker, then finishes an update that was `online` (restoring a half-written `compose.yaml` from its copy and tagging the images from the saved manifest), finishes one whose new gateway turns healthy, and rolls back any other; when Docker doesn't answer, the update ends `needs_attention`, or `failed` when nothing was switched yet.

The updater reaches Docker only at `http://docker-proxy:2375/v1.44/`, whose allow-list in `compose.yaml` names every path it uses. The proxy filters by method and path only: the body of `containers/create` is the updater's to choose, which is why the updater is host-root equivalent and opt-in.

## The edge

Caddy reads the maintenance flag from the `maintenance` volume. While it exists, every request gets the maintenance answer at once; otherwise a `502`, `503` or `504` from the gateway or the console does. `/kaeso-maintenance.json` serves the flag, `{"state":"updating","step":"migrating"}`, so the page can name the step without naming versions. The page is inline in `compose.yaml` with a Content-Security-Policy that allows exactly its own style and script by hash. `crates/kaeso-server/tests/edge.rs` checks that the JSON answers are what `ApiError` and `kaeso_codecs::dispatch::encode_error` produce for the same message, that the hashes match and that the page loads nothing from elsewhere.

## Tests

| Where | What |
| --- | --- |
| `crates/kaeso-update` | Version tags, channels, the plan with majors, manual steps and pre-releases (with a property test), the manifest with its migrations and `manual_since`, the protocol |
| `crates/kaeso-server/tests/updates.rs` | Against a mock GitHub and a mock updater: conditional requests, failures and their backoff, GitHub's wait for manual checks, manifests from elsewhere, a dozen manifests per check, offline installs, rights, the audit log, the one-update lock, refused and unconfirmed starts, the grace period, progress and every ending |
| `crates/kaeso-updater/tests/verify.rs` | The real attestations of Kaeso 0.1.0 verify; another image, commit, workflow, signature or log entry doesn't, nor a key or authority outside its validity, nor a bundle without a signed entry timestamp |
| `crates/kaeso-updater/tests/engine.rs`, `server.rs` | Against a fake Docker API: a full update, verification before any download, every refusal, a slow migration, rollbacks, a gateway left for a person, a failed rollback, `compose.yaml` changed or unwritable at the end, restarts at every step, Docker coming back late or not at all, the state file, the token and the API |
| `scripts/release-manifest.test.mjs` | The manifest, `manual_since`, the latest migrations, and the commit with and without a release commit on top |
| `e2e/install` | The install with the updater profile: the gateway reaches the updater, the updater reaches Docker only through the proxy, and Caddy's answers in every format while Kaeso is away, through the official SDKs; the Release workflow's jobs, permissions and attestation gate |
| `apps/console` | The Updates page in every state, in unit tests and against the stack's mock releases and updater; the updating page in three languages, light and dark, with axe |
