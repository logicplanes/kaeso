---
title: Releasing
description: How versions are numbered, what a release publishes, and how the maintainer makes one.
order: 9
---

Kaeso is released as a whole: the gateway, the console, KPP's crates and the UI library's packages share one version number, and one release publishes all of them. Contributors never create tags or releases; the maintainer makes every release with the repository's Release workflow.

## Versions

Versions follow [semantic versioning](https://semver.org). A release that needs a manual step from people who run Kaeso, such as a migration that can't run by itself, is a major one. The Git tag of a release is `v` and its version, such as `v0.1.0`.

The gateway and the console of one release always belong together: the release's `compose.yaml` pins both images at that version, and the console expects exactly the Platform API of its gateway.

## What a release publishes

| Where | What |
| --- | --- |
| GitHub releases | The release `vX.Y.Z` with its notes from `CHANGELOG.md` and the file `compose.yaml` attached, the one [installations](../self-hosting/installation.md) download |
| GitHub Container Registry | `ghcr.io/logicplanes/kaeso-gateway` and `ghcr.io/logicplanes/kaeso-console`, tagged with the version |
| crates.io | `kaeso-kpp` and `kaeso-codecs` |
| npm | `@logicplanes/kaeso-ui` and `@logicplanes/kaeso-tokens`, published with pnpm |

The documentation at docs.kaeso.app and the showcase at ui.kaeso.app are published from the same repository and describe the latest release.

## The changelog

`CHANGELOG.md` at the repository root follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Every change people will notice adds a line under `[Unreleased]` in the same pull request: what changed, from the point of view of someone who uses, runs or builds on Kaeso. A release turns `[Unreleased]` into the version and its date, and its section becomes the release's notes. A release with nothing under `[Unreleased]` is refused.

## Making a release

1. **Get `dev` ready.** Every check of the [definition of done](testing.md#before-a-change-is-done) passes, and `CHANGELOG.md` says everything the release changes.
2. **Refresh the model data**, so a fresh install starts with current prices and models, and commit the result:

```bash
cargo xtask models-dev
```

3. **Check the packages.** After `pnpm build`, `pnpm release:check` packs the npm packages and checks what they contain, such as the font license files.
4. **Run the Release workflow.** In the repository's **Actions**, choose **Release**, then **Run workflow**, and type the version, such as `0.2.0`. It must be newer than the last release.

The workflow runs the checks again, sets the version, tags the release, and publishes everything in the table above. When it has finished, the release's `compose.yaml` is what [upgrades](../self-hosting/upgrades.md) download.
