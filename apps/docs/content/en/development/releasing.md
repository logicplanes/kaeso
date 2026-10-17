---
title: Releasing
description: How versions are numbered, what a release publishes, and how the maintainer makes one.
order: 9
---

Kaeso is released as a whole: the gateway, the console, KPP's crates and the UI library's packages share one version, and one release publishes all of them. The maintainer makes every release from `prod` with the Release workflow, then deploys the documentation and the showcase from the release's tag. Contributors never create tags or releases.

## Versions

Versions follow [semantic versioning](https://semver.org). A release that needs a manual step from people who run Kaeso, such as a migration that can't run by itself, is a major one. A pre-release adds a suffix, such as `0.2.0-rc.1`. A release's Git tag is `v` and its version, such as `v0.2.0`.

The gateway and the console of one release belong together: the release's `compose.yaml` pins both images at that version, and the console expects exactly the Platform API of its gateway.

## The version in the repository

The root `package.json` holds the repository's version, and only a release changes it, with `node scripts/version.mjs set <version>`. That command writes the version into every file that states it: every `package.json`, the crates' manifests (through `cargo set-version`), `Cargo.lock` and `fuzz/Cargo.lock`, `openapi.json`, both image tags in `deploy/compose.yaml`, and the dependency lines in the crates' READMEs, such as `kaeso-kpp = "0.2"`. `node scripts/version.mjs files` lists these files.

Everything else reads the version instead of stating it: a documentation page writes `\{{version}}`, which the site's build replaces ([everything else](documentation.md#everything-else)), the gateway reads `env!("CARGO_PKG_VERSION")`, and the showcase and the npm packages' generated files read their `package.json`.

| Command | Fails when |
| --- | --- |
| `node scripts/version.mjs check` | The files above disagree, or the current version appears in any other file. It names the file and the line. `pnpm test` runs it, and so does CI's **Repository tooling** job |
| `node scripts/version.mjs check 0.2.0` | Any other file already states `0.2.0` by hand, such as a test whose fixture happens to use it. The Release workflow runs it first and refuses the release until the files are fixed on `dev` |

History and examples may name versions: `CHANGELOG.md`, test fixtures, demo data, this page and the few lines about the first release are listed in `ALLOWED` in the script, each with its reason.

## Branches

| Branch | What it is | Who changes it |
| --- | --- | --- |
| `dev` | The default branch, where all work lands | Pull requests only, merged as one squashed commit once the **CI** check is green ([how changes reach dev](contributing.md#how-changes-reach-dev)) |
| `prod` | The latest release, or the commit about to become one; the Release workflow runs only here | The maintainer, always as a fast-forward |
| `hotfix/X.Y.Z` | A fix for the latest release, branched from its tag | The maintainer, who pushes it to `prod` |

`dev` and `prod` can't be deleted or force-pushed, and a `v*` tag can't be moved or deleted. The maintainer can bypass these rules. After every release, the Release workflow tries to bring the release into `dev`, so that the next push of `dev` to `prod` is a fast-forward again.

## What a release publishes

| Where | What |
| --- | --- |
| GitHub releases | The release `vX.Y.Z`, titled "Kaeso X.Y.Z", with its notes from `CHANGELOG.md` and these files: `compose.yaml`, which [installations](../self-hosting/installation.md) download; `kaeso.example.toml`; `env.example`; the two npm tarballs and the two `.crate` files |
| GitHub Container Registry | `ghcr.io/logicplanes/kaeso-gateway` and `ghcr.io/logicplanes/kaeso-console` for `linux/amd64` and `linux/arm64`, with build provenance and a software bill of materials (SBOM) |
| crates.io | `kaeso-kpp` and `kaeso-codecs` |
| npm | `@logicplanes/kaeso-ui` and `@logicplanes/kaeso-tokens`, with provenance |

| Image tag | Given to |
| --- | --- |
| The version, such as `0.2.0` or `0.2.0-rc.1` | Every release |
| `MAJOR.MINOR`, such as `0.2`, and `latest` | Every stable release |
| `MAJOR`, such as `1` | Every stable release from `1.0.0` on |

On npm, a stable release gets the `latest` tag and a pre-release the `next` tag. On GitHub, a pre-release is marked as one and never becomes the latest release, so `releases/latest/download/compose.yaml` always points at the newest stable release.

## The changelog

`CHANGELOG.md` at the repository root follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Every change people will notice adds a line under `[Unreleased]` in the same pull request, saying what changed from the point of view of someone who uses, runs or builds on Kaeso.

A release turns the lines under `[Unreleased]` into the version's section, `## [X.Y.Z] - YYYY-MM-DD`, leaves `## [Unreleased]` empty above it, and updates the links at the end of the file: `[Unreleased]` compares with the new tag, and a new link points at the version's release. That section becomes the release's notes on GitHub. A release with nothing under `[Unreleased]` is refused, unless `CHANGELOG.md` already has a section for that version or, for a stable release, for a pre-release of it.

A stable release after pre-releases of its version, such as `0.2.0` after `0.2.0-rc.1` and `0.2.0-rc.2`, folds their sections and anything new under `[Unreleased]` into the one section `## [0.2.0] - YYYY-MM-DD`. The lines under each heading (such as `### Added`) and each topic within it (such as `#### Model API`) come together, newest first, and the pre-releases' sections and links are removed. The stable release's notes therefore list everything the version changes since the last stable release. `scripts/changelog.mjs` makes these changes.

Both sites show the changelog at `/changelog`. The documentation ([the changelog page](architecture/docs-site.md#the-changelog)) and the showcase read `CHANGELOG.md` through `history` in `scripts/changelog.mjs`, so a release needs no extra step for them. Built from a release's tag, they have nothing under `[Unreleased]` and leave it out; built from `dev`, they show those lines first, marked as not released yet.

## Making a release

1. **Get `dev` ready.** CI on `dev` is green, every check under [before a change is done](testing.md#before-a-change-is-done) passes, `pnpm release:check` included, and `CHANGELOG.md` describes everything the release changes.
2. **Refresh the model data**, so a fresh install starts with current models and prices, and merge the result into `dev` through a pull request:

   ```bash
   cargo xtask models-dev
   ```

3. **Push `dev` to `prod`.** The push is a fast-forward. If it is refused, `dev` doesn't have the last release yet; merge `prod` into `dev` first, as in step 5.

   ```bash
   git switch dev
   git pull --ff-only
   git push origin dev:prod
   ```

4. **Run the Release workflow on `prod`** with the version, such as `0.2.0` or `0.2.0-rc.1`; a leading `v` is dropped. In the repository's **Actions**, choose **Release**, then **Run workflow** with the branch `prod`, or from a terminal:

   ```bash
   gh workflow run release.yml --repo logicplanes/kaeso --ref prod -f version=0.2.0
   ```

5. **Check that `dev` has the release.** The run's **Sync dev** job tries to bring it in. If that job fails, the release itself is still complete, and the job's summary says why. Merge `prod` into `dev` by hand and push it, which the maintainer's bypass of the `dev` rules allows. When the merge stops at a conflict, resolve it and finish with `git commit --no-edit` before pushing:

   ```bash
   git fetch origin
   git switch dev
   git pull --ff-only
   git merge --no-ff -m "Merge release 0.2.0 into dev" origin/prod
   git push origin dev
   ```

6. **Deploy the sites** from the release's tag, as described under [the sites](#the-sites). After a pre-release this is optional, since the sites would then describe the pre-release.

   ```bash
   git fetch origin --tags
   git checkout v0.2.0
   ```

## Pre-releases

A pre-release, such as `0.2.0-rc.1`, is made from `prod` like any release. It gets the `next` tag on npm, is marked as a pre-release on GitHub, and gets only its version as an image tag, so `latest` stays on the newest stable release. Each pre-release gets its own changelog section until the stable release folds them into one ([the changelog](#the-changelog)), so the stable release needs no new lines of its own.

## Hotfixes

A hotfix fixes the latest release without what `dev` has gathered since. It works while `prod` is still at that release.

1. **Branch from the release's tag**, such as `v0.2.0` for the hotfix `0.2.1`:

   ```bash
   git fetch origin --tags
   git switch -c hotfix/0.2.1 v0.2.0
   ```

2. **Fix it** with a test that shows the bug, add a line under `[Unreleased]` in `CHANGELOG.md`, commit, and run the checks.
3. **Push the branch to `prod`.** This is a fast-forward, because `prod` is at the release's commit:

   ```bash
   git push origin hotfix/0.2.1:prod
   ```

4. **Run the Release workflow on `prod`** with the hotfix's version:

   ```bash
   gh workflow run release.yml --repo logicplanes/kaeso --ref prod -f version=0.2.1
   ```

5. **Bring the hotfix into `dev`.** If **Sync dev** fails, merge by hand as in [making a release](#making-a-release). When `dev` has lines under `[Unreleased]`, `CHANGELOG.md` conflicts: keep the lines of `dev` under `[Unreleased]` and the hotfix's section below them.

## The Release workflow

`.github/workflows/release.yml` is started by hand with a version, and only one run goes at a time; a second waits for the first. Validate, Prepare, Checks and Tag run one after another. Once the tag is pushed, Sync dev and the publishing jobs start.

```mermaid
flowchart TD
  accTitle: The order of the Release workflow's jobs
  accDescr: Validate, Prepare, Checks and Tag run one after another. After Tag, Sync dev, the images, the npm packages and crates.io start. The images are joined in Image manifest and pulled without credentials, the npm packages are packed and published, and GitHub release runs once the anonymous pull, npm and crates.io have finished.
  validate[Validate] --> prepare[Prepare] --> checks[Checks] --> tag[Tag]
  tag --> sync[Sync dev]
  tag --> image[Image] --> manifest[Image manifest] --> pull[Anonymous pull]
  tag --> pack[Pack npm packages] --> npm[npm]
  tag --> crates[crates.io]
  pull --> release[GitHub release]
  npm --> release
  crates --> release
```

The jobs that write to the repository or publish anything run in the `release` environment, which accepts runs from `prod` only. Tag and Sync dev are the only jobs that push to the repository: they build nothing, only run git, and push with the run's own token, so their pushes don't start CI again.

| Job | What it does |
| --- | --- |
| Validate | Refuses a run on any branch but `prod`, a version that isn't semantic, a tag that already exists, a version that isn't newer than the latest tag, a `CHANGELOG.md` with nothing under `[Unreleased]` and no section for the version (or, for a stable version, a pre-release of it), and a version that a file already states by hand |
| Prepare | Runs `node scripts/version.mjs set` with the version, which writes it into every file that states it ([the version in the repository](#the-version-in-the-repository)) and fails if the version is still written by hand anywhere else. Then moves the changelog, folding the sections of a stable version's pre-releases into its own. It can only read the repository, holds no Git credentials, and hands its changes to Tag as a patch |
| Checks | Runs the whole CI workflow on `prod`'s commit, every job, whatever changed |
| Tag | Applies the patch, refusing one that creates, deletes or renames a file, changes a file's mode, or changes any file other than `CHANGELOG.md` and those `node scripts/version.mjs files` lists. If the patch changes files, commits "Release X.Y.Z" to `prod`; if it changes none, tags `prod`'s commit itself. Refuses when `prod` moved since the run started. Tags `vX.Y.Z` and pushes `prod` and the tag together, both or neither |
| Sync dev | Brings the release into `dev`: nothing to do when `dev` already has it, a fast-forward when `dev` has nothing new, otherwise the merge commit "Merge release X.Y.Z into dev" when it merges cleanly. Fails, and its summary says how to merge by hand, when the merge conflicts, when the release changes a file in `.github/workflows` that `dev` doesn't have, or when the push to `dev` is refused |
| Image | Builds each image for each platform from the tag, with full build provenance and an SBOM |
| Image manifest | Joins both platforms under the image tags above, copies the images' `org.opencontainers.image` labels onto the joined index as annotations, and attests the build's provenance |
| Anonymous pull | Pulls both images of the version for `linux/amd64` and `linux/arm64` without credentials, as an install does. If ghcr.io refuses with 401 or 403, the packages are private: the job fails and says to make them public in the package's settings, after which re-running the job finishes the release |
| Pack npm packages | Builds the libraries from the tag and packs them with the package check at the release's version |
| npm | Publishes `@logicplanes/kaeso-tokens` and `@logicplanes/kaeso-ui` |
| crates.io | Publishes `kaeso-kpp` and `kaeso-codecs` |
| GitHub release | Once anyone can pull the images and both registries have the packages, downloads the published tarballs and crates and creates the release with its files and notes |

The npm and crates.io jobs authenticate with trusted publishing, so the repository holds no registry tokens. Both skip a version that is already published, so when a publishing job fails, re-running the run's failed jobs finishes the release.

## The sites

The documentation at docs.kaeso.app and the showcase at ui.kaeso.app are static sites. The maintainer builds them from a release's tag and uploads them to a static host; no workflow deploys them.

```bash
pnpm install --frozen-lockfile && pnpm build:packages
pnpm --filter kaeso-docs build
pnpm --filter kaeso-showcase build
```

`apps/docs/dist` then holds the documentation and `apps/showcase/dist` the showcase, every page already rendered in English, German and Simplified Chinese. The showcase's **View source** links, its `llms.txt` and its changelog's links into the repository point at the release's tag (`v` and the version in the root `package.json`). The documentation's **Edit on GitHub** links point at `dev`, where changes to pages are proposed.

Each folder is ready to upload as it is. It contains `robots.txt`, which allows everything and names the sitemap; `sitemap.xml`, with every page in every language and its `hreflang` alternates; a `404.html` per language, marked `noindex`; `site.webmanifest`, the icons, and `og.png`, the image link previews show. The host only has to answer `/guides` with `guides.html`, and an unknown address with the `404.html` of its language.

Both builds end with `scripts/check-site.mjs`, which fails the build when:

- a page search engines may index is missing from `sitemap.xml`, or the sitemap lists an address that isn't a page of the build;
- a page lacks its canonical address, its alternates in the three languages and `x-default`, its own title and description, its Open Graph and Twitter tags with an image that is in the build, its icons or its web manifest.

The sitemap has no `lastmod`, because nothing in the build knows when a page last changed.

### Content Security Policy

Both builds end by writing their site's policy with `scripts/csp.mjs`. It goes into every page as a `<meta http-equiv="content-security-policy">` right after the charset, so it holds on any host, and into `csp.txt` and `_headers` (for hosts that read that file). Send it as the `Content-Security-Policy` header as well: only the header can add `frame-ancestors 'none'`, which keeps the sites out of other pages' frames. The build fails on an inline event handler or a `javascript:` URL, which no policy without `'unsafe-inline'` allows, and on a policy over 4 KB, which is too long for some servers' headers.

The documentation has no inline scripts or styles, and its build fails on one, so its header is always:

```text
default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'
```

The showcase's pages carry inline scripts: those Astro and React write (the islands' loader and the end of streamed Suspense boundaries), and the showcase's own language and theme scripts, which must run before the first paint. Its policy lists the SHA-256 hash of each, and of its few inline `<style>` elements, in `script-src` and `style-src`. It also allows `style` attributes with `style-src-attr 'unsafe-inline'`, because the components' server-rendered HTML sets them and CSS in an attribute can't run code. The hashes change whenever those scripts do, so take the showcase's header from the `csp.txt` of the build you deploy.

The end-to-end suites of both sites load their pages under the header from `csp.txt`, in both themes and every language, and fail on any `securitypolicyviolation`.
