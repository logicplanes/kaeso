---
title: Releasing
description: How versions are numbered, what a release publishes, and how the maintainer makes one.
order: 9
---

Kaeso is released as a whole: the gateway, the console, KPP's crates and the UI library's packages share one version number, and one release publishes all of them. Contributors never create tags or releases; the maintainer makes every release with the repository's Release workflow and publishes the documentation with its Pages workflow.

## Versions

Versions follow [semantic versioning](https://semver.org). A release that needs a manual step from people who run Kaeso, such as a migration that can't run by itself, is a major one. A pre-release adds a suffix, such as `0.2.0-rc.1`. The Git tag of a release is `v` and its version, such as `v0.1.0`.

The gateway and the console of one release always belong together: the release's `compose.yaml` pins both images at that version, and the console expects exactly the Platform API of its gateway.

## Branches

| Branch | What it is |
| --- | --- |
| `dev` | The default branch. Every pull request goes here, and releases are made from it |
| `prod` | The latest stable release. Only the Release workflow moves it, to the commit of a release that isn't a pre-release |
| `hotfix/*` | A fix for the latest release, branched from `prod` and released from the branch itself |

After a hotfix release, `prod` has a commit that `dev` doesn't; merge `prod` into `dev`, or the next release from `dev` is refused.

## What a release publishes

| Where | What |
| --- | --- |
| GitHub releases | The release `vX.Y.Z`, titled "Kaeso X.Y.Z", with its notes from `CHANGELOG.md` and these files: `compose.yaml`, the one [installations](../self-hosting/installation.md) download; `kaeso.example.toml`; `env.example`; the two npm tarballs and the two `.crate` files |
| GitHub Container Registry | `ghcr.io/logicplanes/kaeso-gateway` and `ghcr.io/logicplanes/kaeso-console` for `linux/amd64` and `linux/arm64`, with build provenance and a software bill of materials (SBOM) |
| crates.io | `kaeso-kpp` and `kaeso-codecs` |
| npm | `@logicplanes/kaeso-ui` and `@logicplanes/kaeso-tokens`, with provenance |

| Image tag | Given to |
| --- | --- |
| The version, such as `0.2.0` or `0.2.0-rc.1` | Every release |
| `MAJOR.MINOR`, such as `0.2`, and `latest` | Every release that isn't a pre-release |
| `MAJOR`, such as `1` | Every release from `1.0.0` on that isn't a pre-release |

On npm, a release gets the `latest` tag and a pre-release the `next` tag. A pre-release is marked as one on GitHub and never becomes the latest release, so `releases/latest/download/compose.yaml` always points at the newest stable release.

## The changelog

`CHANGELOG.md` at the repository root follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Every change people will notice adds a line under `[Unreleased]` in the same pull request: what changed, from the point of view of someone who uses, runs or builds on Kaeso.

A release adds the heading `## [X.Y.Z] - YYYY-MM-DD` below `## [Unreleased]`, so the lines gathered there become the version's section. At the end of the file, `[Unreleased]` then compares with the new tag, and a new link points at the version's release. That section is the release's notes. A release with nothing under `[Unreleased]` is refused, unless `CHANGELOG.md` already has a section for that version.

## Making a release

1. **Get `dev` ready.** Every check of the [definition of done](testing.md#before-a-change-is-done) passes, `pnpm release:check` included, and `CHANGELOG.md` says everything the release changes.
2. **Refresh the model data**, so a fresh install starts with current prices and models, and commit the result:

```bash
cargo xtask models-dev
```

3. **Run the Release workflow.** In the repository's **Actions**, choose **Release**, then **Run workflow**. Choose the branch, `dev` or a `hotfix/*` branch, and type the version, such as `0.2.0` or `0.2.0-rc.1`; a leading `v` is dropped.
4. **Publish the documentation.** Choose **Pages**, then **Run workflow**, and type the release's tag, such as `v0.2.0`. The default, `prod`, is the latest stable release; after a pre-release, type its tag to publish its documentation.

## The Release workflow

`.github/workflows/release.yml` runs these jobs in order. The jobs that write to the repository or publish anything run in the `release` environment.

| Job | What it does |
| --- | --- |
| Validate | Refuses a branch other than `dev` or `hotfix/*`, a version that isn't semantic, a tag that already exists, a version that isn't newer than the latest tag, and an empty `[Unreleased]` |
| Checks | Runs the whole CI workflow, every job, whatever changed |
| Tag | Sets the version in every crate with `cargo set-version`, in `Cargo.lock` and `fuzz/Cargo.lock`, and in every `package.json`; writes `openapi.json` again; pins both images in `deploy/compose.yaml`; moves the changelog. It commits "Release X.Y.Z" to the branch, refuses when `prod` has commits the branch lacks, tags `vX.Y.Z`, and pushes the branch and the tag together, with `prod` too unless the version is a pre-release |
| Image | Builds each image for each platform from the tag, with full build provenance and an SBOM |
| Image manifest | Joins both platforms under the image tags above and attests the build's provenance |
| Pack npm packages | Builds the libraries and runs the package check with the release's version |
| npm | Publishes `@logicplanes/kaeso-tokens` and `@logicplanes/kaeso-ui` |
| crates.io | Publishes `kaeso-kpp` and `kaeso-codecs` |
| GitHub release | Downloads the published tarballs and crates and creates the release with its files and notes |

The npm and crates.io jobs skip a version that is already published, so when a publishing job fails, re-running the run's failed jobs finishes the release.

## The Pages workflow

`.github/workflows/pages.yml` takes one input, the tag or branch to publish. It installs the workspace, builds the libraries with `pnpm build:packages`, builds this site with `pnpm --filter kaeso-docs build`, budgets included, and deploys `apps/docs/dist` to GitHub Pages at docs.kaeso.app. The documentation describes the release it was published from.
