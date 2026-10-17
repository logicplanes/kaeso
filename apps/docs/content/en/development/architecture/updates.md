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

`services/updates.rs` and the `update_check` job look for releases: every five minutes the job asks whether the next check is due (`update_settings.next_check_at`), and a check fetches `releases_url?per_page=50` with `If-None-Match`. A changed list replaces `update_releases` in one transaction; for every release newer than the running version, its `release.json` is fetched and stored, or the reason it couldn't be. The next check is 12 hours later plus a random part of an hour, after a failure at least an hour later, or later still when GitHub's `Retry-After` or `x-ratelimit-reset` says so. A repeated failure is logged at debug level only.

`kaeso_update::policy::plan` chooses what to offer: the newest release on the channel that is newer than the running version, and whether it is manual: a new major version, `manual: true` in any crossed release's manifest (pre-releases included), or no manifest at all. The Platform API under `instance/updates` is for instance admins only; every change is audited (`instance.update_check`, `instance.update_settings`, `instance.update_start`, `instance.update_finish`), and `apply` needs a sign-in within 15 minutes.

`update_jobs` holds every update; a unique index allows one `running` row at a time. While a row runs, reading it asks the updater for its state and records the end once, with `instance.update_finish`, whichever gateway reads it first: usually the new one, after the switch. A job the updater doesn't know, or that hasn't answered for an hour, ends as `interrupted` or `updater_unreachable`.

## What a release carries

The Release workflow builds three images, `kaeso-gateway`, `kaeso-console` and `kaeso-updater`, attests their build provenance with `actions/attest-build-provenance`, and attaches `release.json` (`scripts/release-manifest.mjs`), whose provenance it attests too:

```json
{
  "schema": 1,
  "version": "1.4.2",
  "commit": "5388ad0b98aa3d957fea99e200259842bf746680",
  "manual": false,
  "updater_protocol": 1,
  "images": {
    "gateway": { "name": "ghcr.io/logicplanes/kaeso-gateway", "digest": "sha256:…" },
    "console": { "name": "ghcr.io/logicplanes/kaeso-console", "digest": "sha256:…" },
    "updater": { "name": "ghcr.io/logicplanes/kaeso-updater", "digest": "sha256:…" }
  },
  "compose": { "name": "compose.yaml", "sha256": "…" }
}
```

`manual` is true when the Release workflow's **manual** input says so, or when `compose.yaml` differs from the previous release's in anything but the image versions. `kaeso_update::manifest` parses it, refuses another schema, and checks every name, digest and the commit.

## The updater

`crates/kaeso-updater` is a small axum server on port 8081: `GET /healthz` without a token, and `GET /v1/status`, `POST /v1/updates` (`{ "id", "version" }`) and `GET /v1/updates/{id}` with `Authorization: Bearer` and the shared token. It runs one update at a time and keeps it in `/state/job.json`, written atomically after every step.

An update (`engine.rs`):

1. downloads `release.json` and `compose.yaml` from the release, checks the manifest, finds the gateway and console containers of its own Compose project (from its own container's labels), reads the running version from the gateway image's `org.opencontainers.image.version`, refuses an older, equal or new major version, and checks that the local `compose.yaml` with the new image versions equals the release's byte for byte;
2. pulls the three images by digest and verifies `release.json` and every image (`verify.rs`): the attestations from `api.github.com/repos/logicplanes/kaeso/attestations/sha256:…`, each a Sigstore bundle, verified with [sigstore-rs](https://github.com/sigstore/sigstore-rs) against the trust root in `data/trusted_root.json` (the certificate chain to Fulcio, the SCT, the identity `https://github.com/logicplanes/kaeso/.github/workflows/release.yml@refs/heads/prod` from the issuer `https://token.actions.githubusercontent.com`, the DSSE signature, the transparency log entry), plus the log's signed entry timestamp, which the updater checks itself with the Rekor key, and the in-toto statement: SLSA provenance v1, the subject's name and digest, the workflow and the commit `release.json` names. For images, the subject is the image index fetched from `ghcr.io` by digest; the pulled image's version label must equal the release;
3. writes the maintenance flag, then for the console and then the gateway stops the container, renames it `…-previous`, creates the new one from the old one's settings (`clone.rs`: everything the old image brought is dropped, so the new image's environment, command and health check apply) and starts it;
4. waits for each health check: 2 minutes for the console, 10 for the gateway's migrations; a crash, a restart or `unhealthy` fails at once;
5. writes `compose.yaml` in place, tags the images, removes the previous containers and clears the flag.

A failure before the switch changes nothing (`failed`). A failure after it starts the previous containers again in reverse order and waits for them (`rolled_back`, or `rollback_failed` with the reason). On start, an update the file says is running is rolled back, or finished when only the cleanup was left.

The updater reaches Docker only at `http://docker-proxy:2375/v1.44/`, whose allow-list in `compose.yaml` names every path it uses.

## The edge

Caddy reads the maintenance flag from the `maintenance` volume. While it exists, every request gets the maintenance answer at once; otherwise a `502`, `503` or `504` from the gateway or the console does. `/kaeso-maintenance.json` serves the flag, `{"state":"updating","step":"migrating"}`, so the page can name the step without naming versions. The page is inline in `compose.yaml` with a Content-Security-Policy that allows exactly its own style and script by hash. `crates/kaeso-server/tests/edge.rs` checks that the JSON answers are what `ApiError` and `kaeso_codecs::dispatch::encode_error` produce for the same message, that the hashes match and that the page loads nothing from elsewhere.

## Tests

| Where | What |
| --- | --- |
| `crates/kaeso-update` | Version tags, channels, the plan with majors, manual steps and pre-releases (with a property test), the manifest, the protocol |
| `crates/kaeso-server/tests/updates.rs` | Against a mock GitHub and a mock updater: conditional requests, failures and their backoff, offline installs, rights, the audit log, the one-update lock, progress and every ending |
| `crates/kaeso-updater/tests/verify.rs` | The real attestations of Kaeso 0.1.0 verify; another image, commit, workflow, signature or log entry doesn't |
| `crates/kaeso-updater/tests/engine.rs`, `server.rs` | Against a fake Docker API: a full update, every refusal, rollbacks, a failed rollback, restarts mid-update, the token and the API |
| `e2e/install` | The install with the updater profile: the gateway reaches the updater, the updater reaches Docker only through the proxy, and Caddy's answers in every format while Kaeso is away, through the official SDKs |
| `apps/console` | The Updates page in every state, in unit tests and against the stack's mock releases and updater; the updating page in three languages, light and dark, with axe |
