# AGENTS.md

Guide for coding agents and people working in the Kaeso repository. Commands run from the repository root. The nested `AGENTS.md` of the area you change adds its own rules; read it before you start.

## What this repository is

Kaeso, the open-source, self-hosted AI gateway ([kaeso.app](https://kaeso.app)): the gateway, KPP, the console, the UI library, the documentation and the self-hosting package, released together under one version.

- **Is:** a Cargo workspace (`crates/`, `fuzz/` on its own) and a pnpm workspace (`apps/`, `packages/`), the Compose package and container images (`deploy/`), and the SDK suites (`e2e/`).
- **Is not:** a place for documentation outside `apps/docs/content`. Guides and references for people who use, run, integrate or contribute to Kaeso are pages at [docs.kaeso.app](https://docs.kaeso.app); READMEs stay short and link there.
- **Is not:** a place for plans. Roadmaps, to-do lists, handover notes and open questions go into issues.

## Layout

| Path            | Holds                                                                                                                                             | Agent file                                           |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| `crates/`       | The gateway (`kaeso`, `kaeso-server`, `kaeso-core`, …), the updater (`kaeso-update`, `kaeso-updater`), KPP (`kaeso-kpp`, `kaeso-codecs`), `xtask` | [`crates/AGENTS.md`](crates/AGENTS.md)               |
| `apps/console`  | The web console                                                                                                                                   | [`apps/console/AGENTS.md`](apps/console/AGENTS.md)   |
| `apps/docs`     | The documentation site and its pages in `apps/docs/content`                                                                                       | [`apps/docs/AGENTS.md`](apps/docs/AGENTS.md)         |
| `apps/showcase` | The UI library's showcase at [ui.kaeso.app](https://ui.kaeso.app)                                                                                 | [`apps/showcase/AGENTS.md`](apps/showcase/AGENTS.md) |
| `apps/update`   | The update service at update.kaeso.app: the release feed installs read, mirrored release manifests, release artwork and the releases page         | [`apps/update/AGENTS.md`](apps/update/AGENTS.md)     |
| `packages/`     | `@logicplanes/kaeso-tokens` and `@logicplanes/kaeso-ui`                                                                                           | [`packages/AGENTS.md`](packages/AGENTS.md)           |
| `packages/ui`   | The rules for interfaces built with the library, shipped in the npm package                                                                       | [`packages/ui/AGENTS.md`](packages/ui/AGENTS.md)     |
| `deploy/`       | `compose.yaml` (the self-hosting package), `compose.dev.yaml` (the one development stack), the gateway, console and updater Dockerfiles           |                                                      |
| `e2e/`          | The official OpenAI and Anthropic Python SDKs against KPP, the gateway and a fresh install                                                        |                                                      |
| `fuzz/`         | cargo-fuzz targets for every KPP decoder                                                                                                          |                                                      |
| `openapi.json`  | The Platform API document, generated from the gateway's code                                                                                      |                                                      |
| `scripts/`      | The package, licence and site checks, and the release's steps: version, changelog, release notes and `release.json`                               |                                                      |
| `.github/`      | The CI, Release and Labels workflows, the labeler's rules, Dependabot, and the issue and pull request templates                                   |                                                      |

## Setup

Node.js 24 (`.node-version`), pnpm 11 (pinned in `package.json`, `corepack enable`), rustup (the toolchain in `rust-toolchain.toml` installs itself), a Docker engine with Compose, and Python 3.13; `actionlint` for workflow changes, and the GitHub CLI with the `github/gh-stack` extension for stacked pull requests.

```sh
pnpm install
pnpm --filter kaeso-console exec playwright install chromium
cargo install cargo-deny cargo-llvm-cov sqlx-cli --locked
docker compose -f deploy/compose.dev.yaml up -d --wait
python3 -m venv e2e/.venv && e2e/.venv/bin/pip install -r e2e/requirements.txt
```

The development stack runs PostgreSQL, ClickHouse, Valkey and Mailpit for the Rust tests; `pnpm stack:up` adds the gateway built from the checkout, the mock provider and the mock identity provider for the console. [Local setup](https://docs.kaeso.app/development/local-setup) has every step.

## Definition of done

Every command passes, with the development stack running, before a change is complete:

```sh
pnpm lint
pnpm format:check
pnpm typecheck
pnpm test
pnpm i18n
pnpm knip
pnpm build
pnpm licenses:check
pnpm release:check
pnpm e2e
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo coverage-kpp
cargo coverage-gateway
cargo test -p xtask -p kpp-conformance
cargo deny check
cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check
cargo sqlx prepare --workspace --check
cargo run -q -p kaeso -- openapi | diff - openapi.json
```

Then, for the parts you changed:

| Changed                                                  | Also run                                                                                                                                                                                                                                                                                 |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| KPP (`crates/kaeso-kpp`, `crates/kaeso-codecs`, `fuzz/`) | `e2e/.venv/bin/pytest -q e2e/conformance`, `cargo test --doc -p kaeso-kpp -p kaeso-codecs`, `cargo fmt --all --check --manifest-path fuzz/Cargo.toml`, `cargo clippy --manifest-path fuzz/Cargo.toml --all-targets -- -D warnings`, `cargo check --manifest-path fuzz/Cargo.toml --bins` |
| The gateway                                              | `e2e/.venv/bin/pytest -q e2e/sdk`                                                                                                                                                                                                                                                        |
| `deploy/`, a Dockerfile, packaging                       | `e2e/.venv/bin/pytest -q e2e/install`, `docker build -f deploy/console.Dockerfile .`                                                                                                                                                                                                     |
| `.github/workflows`                                      | `actionlint`                                                                                                                                                                                                                                                                             |
| An interface                                             | Look at every screen you changed in light and dark, at desktop and phone widths, in English, German and Chinese                                                                                                                                                                          |

CI runs these checks in parallel jobs, the longest suites split into shards, for every area a pull request changes, and all of them on `dev`, `prod` and releases; the one required check is **CI**. [Testing](https://docs.kaeso.app/development/testing) explains every suite.

## Rules

1. **No code comments, anywhere.** No `//`, `///`, `//!`, `/* */`, JSDoc or JSX comments in Rust, TypeScript, JavaScript or CSS; no `--` in SQL; no `#` comments in TOML, YAML, Python, shell, Dockerfiles, `.dockerignore`, the Caddyfile, `.env.example` or the ignore and attributes files; no `<!-- -->` in HTML, Astro or Markdown; none inside the code blocks of a Markdown file. Code explains itself through names, types, small functions and tests; explanations belong in the documentation. The **No comments** CI job enforces it.
2. **pnpm only** for JavaScript: never npm, npx or yarn; `pnpm dlx` runs a one-off tool. Add a dependency at an exact version with `pnpm --filter <package> add --save-exact <name>`. **Cargo** for Rust. Lockfiles change only through their tool.
3. **Every visible word in English, German and Simplified Chinese**: interfaces, emails, error messages and documentation pages. [Translations](https://docs.kaeso.app/development/translations) has the rules and the glossary.
4. **Tests for every change**, asserting on what a person or an app sees, deterministic (injectable clocks, no sleeps). Coverage gates in `.cargo/config.toml` and every `vitest.config.ts` are never lowered; raise them when coverage rises.
5. **Change everything a change touches, in the same pull request**: the console with the Platform API and `openapi.json`, `packages/ui` when an app needs a component, the pages in `apps/docs/content` when people will notice. A new dependency needs an MIT-compatible license and a one-line reason in the pull request's description ([add a dependency](https://docs.kaeso.app/development/contributing#add-a-dependency)).
6. **Documentation lives only in `apps/docs/content`**, in all three languages. No `docs/` folders elsewhere. Every diagram of a system, an architecture, a request or data flow, a sequence, a state or a lifecycle is a Mermaid block that the docs build draws; never ASCII or box-drawing art, never a screenshot or an image of a diagram. The content check fails on a code block that draws one.
7. **MIT-compatible dependencies only**, checked by `cargo deny` and `pnpm licenses:check`.
8. **Never commit secrets or personal data.** Placeholders only: `kaeso.example.com`, `sk-kso-…`, `owner@example.com`.
9. **No leftovers when something changes.** Rename or remove a name, page, heading, route, setting, field or message everywhere in the same change: code, tests, translations and every page in all three languages. Never keep the old name, an anchor, an alias, a redirect or a compatibility path for it; the docs and the interface always describe Kaeso as it is now.

## Git and releases

- Branch from `dev` and open pull requests into `dev`, one change each; `dev` takes changes only through pull requests. A change that builds on an unmerged pull request goes on top of it as a stack with `gh stack` ([stacked pull requests](https://docs.kaeso.app/development/contributing#stack-dependent-pull-requests)). Never push to `prod`: the maintainer pushes `dev` there to make a release.
- Never create tags or releases and never publish crates, npm packages, images or sites: the maintainer runs the Release workflow and deploys the sites.
- Never edit `CHANGELOG.md` in a pull request: the Release workflow writes it, and the release notes, from the merged pull requests. The title is the changelog line, so write it as a sentence that says what changed for people who use, run or build on Kaeso. The Labels workflow sets the `area: …` labels and `internal` from the changed files; a type label such as `added` or `fixed` chooses the section ([labels](https://docs.kaeso.app/development/releasing#labels)).
- Never write the current version by hand. Only a release changes it, with `node scripts/version.mjs set`; pages write `{{version}}`, code reads `package.json` or `env!("CARGO_PKG_VERSION")`. `node scripts/version.mjs check`, part of `pnpm test`, fails on a version anywhere else and names the file and line.
- Commit messages say what changed and why.
