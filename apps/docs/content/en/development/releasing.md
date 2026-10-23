---
title: Releasing
description: How versions and the changelog work, and how the maintainer makes, publishes and deploys a release.
order: 11
---

Kaeso is released as a whole. The gateway, the console, KPP's crates and the UI library's packages share one version, and one run of the Release workflow publishes all of them. Only the maintainer makes releases; contributors never create tags or releases.

## Versions

Versions follow [semantic versioning](https://semver.org). A release that needs a manual step from people who run Kaeso, such as a migration that can't run by itself, is a major one. A pre-release adds a suffix, such as `0.2.0-rc.1`. The Git tag is `v` and the version, such as `v0.2.0`.

The gateway and the console of one release belong together. The release's `compose.yaml` pins both images to that version and the digests the release built, and the console expects exactly the Platform API of its gateway.

## The version in the repository

The root `package.json` holds the version, and only a release changes it, with `node scripts/version.mjs set <version>`. That command writes the version into every file that has to state it, such as the other `package.json` files, the crates' manifests, `openapi.json` and `deploy/compose.yaml`. `node scripts/version.mjs files` lists them.

Everything else reads the version instead of stating it. A page writes `\{{version}}`, the gateway reads `env!("CARGO_PKG_VERSION")`, and generated files read their `package.json`. `node scripts/version.mjs check`, part of `pnpm test`, fails on the current version anywhere else and names the file and line.

## Branches

| Branch | What it is | Who changes it |
| --- | --- | --- |
| `dev` | The default branch, where all work lands | Pull requests only ([get it merged](contributing.md#get-it-merged)) |
| `prod` | The commit the latest release ran on, or the commit about to become a release | The maintainer, always as a fast-forward |
| `hotfix/X.Y.Z` | A fix for the latest release, branched from its tag | The maintainer, who pushes it to `prod` |

`dev` and `prod` can't be deleted or force-pushed, only the maintainer can push to `prod`, and a `v*` tag can't be moved or deleted.

## The changelog

`CHANGELOG.md` follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and pull requests never change it. The Release workflow writes each version's section from the pull requests merged since the last release. Each pull request's title as merged becomes its line, with a link to the pull request, and its labels choose where the line goes. The title as merged is the subject of the pull request's squash commit on `prod`, so editing a pull request after its merge changes nothing. For a pull request merged another way, it is the title at the moment of the merge.

Pull requests labelled `internal` are left out. Dependabot's updates become one line, "Dependencies updated". A pull request labelled `breaking` is also named at the top of the section.

To write the notes yourself, put lines under `[Unreleased]` in `CHANGELOG.md` on `dev`. The release puts those lines first, then a line for each pull request that none of them links as `[#42](…/pull/42)`. This suits a release whose notes need more than titles, and a hotfix, whose commits have no pull request.

When the repository's `release` environment has the secret `LOG_AI_KEY`, the workflow has a model draft the section through the maintainer's Kaeso install, from the merged pull requests. The model reads each one as merged: the squash commit's subject and body, or the commits of a pull request merged another way, without their HTML comments. It sees a linked issue's title only when the issue's author can write to the repository. It keeps your lines under `[Unreleased]` as written. If the draft fails a check or the request fails, the release falls back to the titles. To preview the notes of the next release without changing a file:

```bash
pnpm release:notes --dry-run --since v{{version}} --head origin/dev
```

A stable release after pre-releases of its version folds their sections into one, so its notes cover everything since the last stable release.

The GitHub release opens with one sentence, up to five highlights and the breaking changes, then lists every entry of the section under its own heading, such as **Added**, **Changed**, **Removed** or **Fixed**, one line each with its topic and pull requests. The contributors and links to the full changelog and the compare view close it. The model writes the opening with the section; without the model, the release has no highlights, and its breaking changes are cut to their first sentence. The preview above prints the GitHub release after the notes.

### What the notes may say

Every line of the notes, from the model or from a title, passes these checks:

- No addresses: no URLs, email addresses or IP addresses.
- No host names but `kaeso.app` and its subdomains, `github.com/logicplanes/kaeso` and Kaeso's images on `ghcr.io/logicplanes`. `crates.io` and `models.dev` may appear without a path. A name ending in a common top-level domain, such as `.com` or `.example`, counts as a host name, in code spans too.
- No image references outside `ghcr.io/logicplanes/`. A name counts as an image when it starts with a registry, such as `docker.io/example/kaeso-gateway:1.4.0`, has a digest, such as `example/kaeso-gateway@sha256:…`, or follows `FROM` or `image:`. A model id such as `google/gemma-4-31b-it:free` is fine.
- No shell pipelines such as `| sh` or `curl … | jq`, and no download commands such as `curl -fsSL` or `wget`.
- No commands that pull, install or run something: `docker run`, `docker pull`, `npx`, `pnpm dlx`, `npm install`, `npm i` or `pip install`.
- No instructions to set an environment variable, such as `set KAESO_…`, `export KAESO_…` or `KAESO_…=…`. Naming a setting is fine.

A draft that still fails after one correction falls back to the titles. A title that fails stops the **Prepare** job and names the pull request. Write a line for it under `[Unreleased]` on `dev`, linking the pull request, and run Release again.

### Review before the tag

Every job that uses a secret, writes to the repository or publishes runs in the `release` environment, so it waits for the maintainer's approval. Approving **Release notes** lets the model draft. **Prepare** then writes the changelog and shows the section and the GitHub release in its summary, exactly as the release publishes them. **Tag** waits for your approval after that, so read the summary first. If anything in it is wrong, reject **Tag**, fix the cause on `dev` and run Release again.

### Labels

| Label | In the changelog |
| --- | --- |
| `added`, `changed`, `deprecated`, `removed`, `fixed`, `security` | The section, such as Added. Without one, the title's first word decides: Add, Fix, Remove and so on, otherwise Changed |
| `breaking` | Also named at the top, as a breaking change |
| `internal` | Left out |
| `area: gateway`, `area: kpp`, `area: console`, `area: ui library`, `area: documentation`, `area: self-hosting` | The topic within the section. Exactly one area label sets it |

The **Labels** workflow sets the area labels and `internal` from the files a pull request changes, following `.github/labeler.yml`, on every push. A label you set by hand holds until the next push. The maintainer sets the type label when the title's first word doesn't choose the right section, and `breaking` when people must change something to upgrade.

## Make a release

1. Check that `dev` is ready: CI is green, every merged pull request has the labels its line needs, and the checks under [before a change is done](testing.md#before-a-change-is-done) pass.
2. Refresh the model data, so a fresh install starts with current models and prices, and merge the result into `dev` through a pull request:

   ```bash
   cargo xtask models-dev
   ```

3. Push `dev` to `prod` as a fast-forward:

   ```bash
   git switch dev
   git pull --ff-only
   git push origin dev:prod
   ```

4. Run the Release workflow on `prod` with the version. Tick **manual** in the workflow's inputs when the release needs steps by hand, so the updater never installs it:

   ```bash
   gh workflow run release.yml --repo logicplanes/kaeso --ref prod -f version=0.2.0
   ```

   Approve the run's jobs as they wait. Before you approve **Tag**, read the notes in the summary of **Prepare** ([review before the tag](#review-before-the-tag)).

5. Bring the release into `dev`. The run's **Sync dev** job tries, but `dev` only takes pull requests, so it usually finishes with a warning and shows the command to run. Merge by hand, which your bypass of the `dev` rules allows:

   ```bash
   git fetch origin --tags
   git switch dev
   git pull --ff-only
   git merge --no-ff -m "Merge release 0.2.0 into dev" v0.2.0
   git push origin dev
   ```

6. Deploy the sites from the release's tag ([deploy the sites](#deploy-the-sites)).

If the run stops because `prod` doesn't have the last release yet, merge its tag into `dev` as in step 5, then repeat step 3. When a publishing job fails, re-run the run's failed jobs. Each skips what is already published, and the GitHub release job replaces the assets and notes of a release it already created.

A release publishes:

| Where | What |
| --- | --- |
| GitHub releases | `vX.Y.Z` with its notes, `compose.yaml`, `kaeso.example.toml`, `env.example`, `release.json`, the npm tarballs and the `.crate` files |
| GitHub Container Registry | `kaeso-gateway`, `kaeso-console` and `kaeso-updater` for `linux/amd64` and `linux/arm64`, with build provenance and a software bill of materials |
| crates.io | `kaeso-kpp` and `kaeso-codecs` |
| npm | `@logicplanes/kaeso-ui` and `@logicplanes/kaeso-tokens` |

Images are tagged with the version and, for stable releases, with `MAJOR.MINOR`, `latest` and, from 1.0.0 on, `MAJOR`.

### Clean builds

Release jobs restore no cache that another run could have written. The images build with `no-cache`, and pnpm and Cargo start from an empty store. The npm packages are packed from an install without lifecycle scripts. The **crates.io build** job packages and compiles `kaeso-kpp` and `kaeso-codecs` without a publishing token. The **crates.io** job then holds the token and uploads them with `cargo publish --no-verify`, so it runs no build script.

### Commits and attestations

The run starts on `prod`'s commit. When the version changes, the Tag job commits `Release X.Y.Z` on top of it and tags that commit `vX.Y.Z`. The release commit changes only `CHANGELOG.md` and the files `node scripts/version.mjs files` lists. Before it commits, the Tag job checks that these changes, apart from `CHANGELOG.md`, equal what `node scripts/version.mjs set` writes in a job of its own.

The Tag job pushes only the tag: `prod` gets the release commit with the next push from `dev`, which has it from step 5. Every image, package and asset is built from the tag.

`gh attestation verify` and the updater check the attestations of the images and of `release.json`. They name the commit the run started on, since GitHub signs them for the run, and `release.json` names the same commit. The build provenance inside each image, which BuildKit writes, names the tag's commit as its source. The tag's commit is therefore the attested commit, or a release commit directly on top of it that changes only the version and the changelog.

## Make a pre-release

Run the Release workflow on `prod` with a version such as `0.2.0-rc.1`. A pre-release gets the `next` tag on npm and only its version as an image tag, and GitHub never marks it as the latest release. Deploying the sites after it is optional.

## Make a hotfix

A hotfix fixes the latest release without what `dev` has gathered since. It works while `prod` is still at the commit that release ran on.

1. Branch from the release's tag:

   ```bash
   git fetch origin --tags
   git switch -c hotfix/0.2.1 v0.2.0
   ```

2. Fix the bug with a test that shows it, and add a line under `[Unreleased]` in `CHANGELOG.md`. That line is the hotfix's changelog.
3. Push the branch to `prod`:

   ```bash
   git push origin hotfix/0.2.1:prod
   ```

4. Run the Release workflow on `prod` with the hotfix's version.
5. Bring the hotfix into `dev` as in [make a release](#make-a-release). If `CHANGELOG.md` conflicts, keep the lines of `dev` under `[Unreleased]` and the hotfix's section below them.

## Deploy the sites

The documentation at docs.kaeso.app and the showcase at ui.kaeso.app are static sites, built from a release's tag:

```bash
git fetch origin --tags
git checkout v0.2.0
pnpm install --frozen-lockfile && pnpm build:packages
pnpm --filter kaeso-docs build
pnpm --filter kaeso-showcase build
```

Upload `apps/docs/dist` and `apps/showcase/dist` as they are. They hold every page in every language, `robots.txt`, `sitemap.xml` and a `404.html` per language. The host has to answer `/console` with `console.html`, and an unknown address with the `404.html` of its language. Moved pages have a page at their old address, and `_redirects` lists the same moves for hosts that read it.

### Content Security Policy

Each build writes its site's policy into every page as a `<meta>` tag, and into `csp.txt` and `_headers`. Also send it as the `Content-Security-Policy` header, because only the header can add `frame-ancestors 'none'`. The documentation's header never changes; it allows images from `update.kaeso.app`, where the changelog's release artwork comes from:

```text
default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data: https://update.kaeso.app; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'
```

The showcase's policy lists a hash for each of its inline scripts and styles, which change with them. Take its header from the `csp.txt` of the build you deploy.
