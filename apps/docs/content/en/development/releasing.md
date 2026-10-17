---
title: Releasing
description: How versions are numbered, what a release publishes, and how the maintainer makes one.
order: 9
---

Kaeso is released as a whole: the gateway, the console, KPP's crates and the UI library's packages share one version number, and one release publishes all of them. Contributors never create tags or releases; the maintainer makes every release from `prod` with the repository's Release workflow and deploys the documentation and the showcase from the release's tag.

## Versions

Versions follow [semantic versioning](https://semver.org). A release that needs a manual step from people who run Kaeso, such as a migration that can't run by itself, is a major one. A pre-release adds a suffix, such as `0.2.0-rc.1`. The Git tag of a release is `v` and its version, such as `v0.2.0`.

The gateway and the console of one release always belong together: the release's `compose.yaml` pins both images at that version, and the console expects exactly the Platform API of its gateway.

## The version in the repository

The root `package.json` holds the repository's version, and only a release changes it, with `node scripts/version.mjs set`. That command writes the version into every file that states it: every `package.json`, the crates' manifests through `cargo set-version`, `Cargo.lock` and `fuzz/Cargo.lock`, `openapi.json`, both image tags in `deploy/compose.yaml`, and the dependency lines in the crates' READMEs, such as `kaeso-kpp = "0.2"`. Everything else takes the version from there instead of writing it: a documentation page writes `\{{version}}`, which the site's build replaces ([editing a page](documentation.md#everything-else)), the gateway reads `env!("CARGO_PKG_VERSION")`, and the showcase and the npm packages' generated files read their `package.json`.

`node scripts/version.mjs check` fails when these files disagree, or when the current version appears in any other file, and names the file and the line. `pnpm test` runs it, and so does CI's **Repository tooling** job for every pull request. History and examples may name versions: `CHANGELOG.md`, test fixtures, demo data, this page and the few lines about the first release are listed in `ALLOWED` in the script, each with its reason.

With a version, `node scripts/version.mjs check 0.2.0` lists the files that already state that version by hand, such as a test whose fixture happens to use it. The Release workflow runs it first and refuses the release until they are fixed on `dev` or added to `ALLOWED`.

## Branches

| Branch | What it is |
| --- | --- |
| `dev` | The default branch. Every pull request goes here, and the maintainer may push to it directly |
| `prod` | The latest release, or the commit about to become one. The maintainer pushes to it to release, always as a fast-forward, and the Release workflow runs only here |
| `hotfix/X.Y.Z` | A fix for the latest release, branched from its tag and pushed to `prod` |

`dev` and `prod` can't be deleted or force-pushed, and a `v*` tag can't be moved or deleted. After every release, the Release workflow brings the release into `dev`, so the next push of `dev` to `prod` is a fast-forward again.

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

A release adds the heading `## [X.Y.Z] - YYYY-MM-DD` below `## [Unreleased]`, so the lines gathered there become the version's section. At the end of the file, `[Unreleased]` then compares with the new tag, and a new link points at the version's release. That section is the release's notes. A release with nothing under `[Unreleased]` is refused, unless `CHANGELOG.md` already has a section for that version or, for a stable release, a pre-release of it.

A stable release after pre-releases of its version, such as `0.2.0` after `0.2.0-rc.1` and `0.2.0-rc.2`, folds their sections and anything new under `[Unreleased]` into the one section `## [0.2.0] - YYYY-MM-DD`: the lines under each heading, such as `### Added`, and under each topic within it, such as `#### Model API`, come together, newest first, and the pre-releases' sections and links go. `scripts/changelog.mjs` makes these changes, and the release's notes on GitHub are the section it writes. Its notes then list everything the version changes since the last stable release.

## Making a release

1. **Get `dev` ready.** Every check of the [definition of done](testing.md#before-a-change-is-done) passes, `pnpm release:check` included, CI on `dev` is green, and `CHANGELOG.md` says everything the release changes.
2. **Refresh the model data**, so a fresh install starts with current prices and models, and commit the result to `dev`:

```bash
cargo xtask models-dev
```

3. **Push `dev` to `prod`.** The push is a fast-forward. When it's refused, `dev` doesn't have the last release yet: merge `prod` into `dev` first, as in step 5.

```bash
git switch dev
git pull --ff-only
git push origin dev:prod
```

4. **Run the Release workflow on `prod`** with the version, such as `0.2.0` or `0.2.0-rc.1`; a leading `v` is dropped. In the repository's **Actions**, that's **Release**, then **Run workflow** with the branch `prod`, or from a terminal:

```bash
gh workflow run release.yml --repo logicplanes/kaeso --ref prod -f version=0.2.0
```

5. **Check that `dev` has the release.** The run's **Sync dev** job brings it in. When that job fails, the release itself is complete, and its summary says why; merge `prod` into `dev` by hand, resolve any conflict, finish the merge with `git commit --no-edit`, and push `dev`:

```bash
git fetch origin
git switch dev
git pull --ff-only
git merge --no-ff -m "Merge release 0.2.0 into dev" origin/prod
git push origin dev
```

6. **Deploy the sites** from the release's tag, such as `v0.2.0`, as described in [the sites](#the-sites). After a pre-release this is optional, since the sites then describe the pre-release.

```bash
git checkout v0.2.0
```

## Pre-releases

A pre-release, such as `0.2.0-rc.1`, is made from `prod` like any release. It gets the `next` tag on npm, is marked as a pre-release on GitHub, and gets only its version as an image tag, so `latest` stays on the newest stable release. `prod` then holds the pre-release, and the next release is another pre-release or the stable version. Each pre-release gets its own section in the changelog until the stable release folds them into one ([the changelog](#the-changelog)), so the stable release needs no new lines of its own.

## Hotfixes

A hotfix fixes the latest release without what `dev` gathered since. It works while `prod` is still at that release.

1. **Branch from the release's tag**, such as `v0.2.0` for the hotfix `0.2.1`:

```bash
git fetch origin --tags
git switch -c hotfix/0.2.1 v0.2.0
```

2. **Fix it** with a test that shows the bug, add a line under `[Unreleased]` in `CHANGELOG.md`, commit, and run the checks.
3. **Push the branch to `prod`**, a fast-forward because `prod` is at the release's commit:

```bash
git push origin hotfix/0.2.1:prod
```

4. **Run the Release workflow on `prod`** with the hotfix's version:

```bash
gh workflow run release.yml --repo logicplanes/kaeso --ref prod -f version=0.2.1
```

**Sync dev** merges the hotfix into `dev`. When `dev` has lines under `[Unreleased]`, `CHANGELOG.md` conflicts: merge by hand as in [making a release](#making-a-release), keeping the lines of `dev` under `[Unreleased]` and the hotfix's section below them.

## The Release workflow

`.github/workflows/release.yml` runs Validate, Prepare, Checks and Tag one after another; Sync dev and the publishing jobs start once the tag is pushed. The jobs that write to the repository or publish anything run in the `release` environment, which accepts runs from `prod` only. Tag and Sync dev are the only jobs that can push: they build nothing, only run git, and push with the run's own token, so their pushes don't start CI again.

| Job | What it does |
| --- | --- |
| Validate | Refuses a run on any branch but `prod`, a version that isn't semantic, a tag that already exists, a version that isn't newer than the latest tag, a `CHANGELOG.md` with nothing under `[Unreleased]` and no section for the version or, for a stable version, a pre-release of it, and a version that a file already states by hand |
| Prepare | Runs `node scripts/version.mjs set` with the version, which writes it into every file that states it ([the version in the repository](#the-version-in-the-repository)) and refuses when the version is still written by hand anywhere else; moves the changelog, folding the sections of a stable version's pre-releases into its own. It can only read the repository, holds no Git credentials, and hands these changes on to Tag as a patch |
| Checks | Runs the whole CI workflow, every job, whatever changed |
| Tag | Applies the patch, refusing one that creates, deletes or renames a file or changes a file other than `CHANGELOG.md` and those `node scripts/version.mjs files` lists. When the patch changes a file, it commits "Release X.Y.Z" to `prod`; when it changes none, as for `0.1.0`, the tag goes on `prod`'s commit itself. It refuses when `prod` moved since the run started, tags `vX.Y.Z`, and pushes `prod` and the tag together, both or neither |
| Sync dev | Brings the release into `dev`: a fast-forward when `dev` has nothing new, otherwise the merge commit "Merge release X.Y.Z into dev" when it merges cleanly. When it conflicts, when the release changes a workflow file that `dev` lacks, or when `dev` moves meanwhile, it fails and its summary says how to merge by hand |
| Image | Builds each image for each platform from the tag, with full build provenance and an SBOM |
| Image manifest | Joins both platforms under the image tags above, copies the images' `org.opencontainers.image` labels onto the joined index as annotations, and attests the build's provenance |
| Pack npm packages | Builds the libraries and runs the package check with the release's version |
| npm | Publishes `@logicplanes/kaeso-tokens` and `@logicplanes/kaeso-ui` |
| crates.io | Publishes `kaeso-kpp` and `kaeso-codecs` |
| GitHub release | Downloads the published tarballs and crates and creates the release with its files and notes |

The npm and crates.io jobs authenticate with trusted publishing, so the repository holds no registry tokens. They skip a version that is already published, so when a publishing job fails, re-running the run's failed jobs finishes the release.

## The sites

The documentation at docs.kaeso.app and the showcase at ui.kaeso.app are static sites, deployed by the maintainer from a release's tag to a static host; no workflow deploys them. Both builds check their budgets:

```bash
pnpm install --frozen-lockfile && pnpm build:packages
```

```bash
pnpm --filter kaeso-docs build
```

```bash
pnpm --filter kaeso-showcase build
```

`apps/docs/dist` holds the documentation and `apps/showcase/dist` the showcase, every page already rendered in English, German and Simplified Chinese. The sites describe the release they were built from: the showcase's View source links, its `llms.txt` and the links its changelog makes into the repository point at the release's tag, `v` and the version in the root `package.json`, while the documentation's Edit on GitHub links point at `dev`, where changes to pages are proposed.

Each folder is ready to upload as it is: `robots.txt`, which allows everything and names the sitemap, `sitemap.xml` with every page in every language and its `hreflang` alternates, a `404.html` per language marked `noindex`, `site.webmanifest`, the icons and `og.png`, the image link previews show. A static host only has to answer `/guides` with `guides.html` and an unknown address with the `404.html` of its language. Both builds end with `scripts/check-site.mjs`, which fails the build when a page that search engines may index is missing from `sitemap.xml`, the sitemap lists an address that isn't a page of the build, or a page lacks its canonical address, its alternates in the three languages and `x-default`, its own title and description, its Open Graph and Twitter tags with an image that is in the build, its icons or its web manifest. The sitemap has no `lastmod`, because nothing in the build knows when a page last changed.
