# AGENTS.md

Rules for the update service, `apps/update`. The root [`AGENTS.md`](../../AGENTS.md) holds the definition of done and the rules for the whole repository; commands run from the repository root.

## What the service is

**Kaeso's update service** at [update.kaeso.app](https://update.kaeso.app): a static site the maintainer deploys that answers everything about Kaeso's versions. Gateways read its release feed instead of GitHub's API.

- **Is:** a build step (`scripts/feed.ts`) that reads the releases of `logicplanes/kaeso` and writes `public/`, and a one-page React site, drawn only with [`@logicplanes/kaeso-ui`](../../packages/ui), that shows them.
- **Is not:** a server. Nothing runs but static files; the feed changes only when the site is built again.
- **Is not:** a place that changes a release. A release manifest is mirrored byte for byte, since the updater verifies its signature itself.
- **Not a place for plans.** Roadmaps, to-do lists and open questions go into issues.

## What the build writes

| Path                               | Holds                                                                                                                   |
| ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `v1/releases.json`                 | The newest releases with a semver tag, in the shape of GitHub's releases API that the gateway reads, under 3 MiB        |
| `v1/history.json`                  | Every release with its full notes, newest first, for the releases page                                                  |
| `v1/latest.json`                   | `stable`, the newest stable version, and `prerelease`, the newest version of all; `null` when there is none             |
| `download/v<version>/release.json` | The release's manifest, unchanged; the feed's `release.json` asset points here, because installs trust only this origin |
| `art/<version>.svg`, `-dark.svg`   | The release's artwork from `releaseArtSvg`, the same picture the console's What's new dialog draws from the version     |

## Setup

```
corepack enable
pnpm install
pnpm build:packages
pnpm --filter kaeso-update dev
```

`dev` writes the feed and serves the site at http://localhost:4490 (`KAESO_UPDATE_PORT` chooses another port). The feed asks GitHub without a token and stops at its rate limit; `GITHUB_TOKEN` raises it. Offline, `KAESO_RELEASES_FILE=apps/update/fixtures/releases.json KAESO_RELEASE_ASSETS_DIR=apps/update/fixtures/assets` reads a list in GitHub's format and the manifests (`v<version>/release.json`) from files instead.

## Checks

```
pnpm --filter kaeso-update lint
pnpm --filter kaeso-update typecheck
pnpm --filter kaeso-update test
pnpm --filter kaeso-update build
pnpm --filter kaeso-update licenses:check
```

`test` runs Vitest with coverage thresholds: the feed against the fixtures with no network, and the page in happy-dom. `licenses:check` checks every package in the build. Then look at the page in light and dark, at desktop and phone widths, in English, German and Chinese.

## Rules

1. **The feed keeps GitHub's shape.** `v1/releases.json` holds exactly the fields `GitHubRelease` and `GitHubAsset` in `crates/kaeso-server/src/services/updates.rs` read; a change there changes `scripts/feed.ts` in the same pull request.
2. **Manifests are never touched.** They are copied as bytes; a build that cannot fetch one fails and writes nothing.
3. **The build is deterministic.** The same releases write the same files, and no test touches the network.
4. **Only `@logicplanes/kaeso-ui`** for anything visible. **`className` holds layout only**; `pnpm lint` enforces it (`eslint/kaeso.js`).
5. **Every word in English, German and Simplified Chinese**: `t("key", "English")` with the catalogs in `src/locales`, German with „Sie“, Chinese with 您. Release notes stay as written.
6. **Fixtures use versions far from Kaeso's own**, so `node scripts/version.mjs check` never mistakes them for the current version.
