# AGENTS.md

Guide for coding agents and people working in the Kaeso repository. Commands run from the repository root. The nested `AGENTS.md` of the area you change adds its own rules; read it before you start.

## What this repository is

Kaeso, the open-source, self-hosted AI gateway ([kaeso.app](https://kaeso.app)): the gateway, KPP, the console, the UI library, the documentation and the self-hosting package, released together under one version.

- **Is:** a Cargo workspace (`crates/`, `fuzz/` on its own) and a pnpm workspace (`apps/`, `packages/`), the Compose package and container images (`deploy/`), and the SDK suites (`e2e/`).
- **Is not:** a place for documentation outside `apps/docs/content`. Guides and references for people who use, run, integrate or contribute to Kaeso are pages at [docs.kaeso.app](https://docs.kaeso.app); READMEs stay short and link there.
- **Is not:** a place for plans. Roadmaps, to-do lists, handover notes and open questions go into issues.

## Layout

| Path            | Holds                                                                                                       | Agent file                                           |
| --------------- | ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| `crates/`       | The gateway (`kaeso`, `kaeso-server`, `kaeso-core`, …), KPP (`kaeso-kpp`, `kaeso-codecs`), `xtask`          | [`crates/AGENTS.md`](crates/AGENTS.md)               |
| `apps/console`  | The web console                                                                                             | [`apps/console/AGENTS.md`](apps/console/AGENTS.md)   |
| `apps/docs`     | The documentation site and its pages in `apps/docs/content`                                                 | [`apps/docs/AGENTS.md`](apps/docs/AGENTS.md)         |
| `apps/showcase` | The UI library's showcase at [ui.kaeso.app](https://ui.kaeso.app)                                           | [`apps/showcase/AGENTS.md`](apps/showcase/AGENTS.md) |
| `packages/`     | `@logicplanes/kaeso-tokens` and `@logicplanes/kaeso-ui`                                                     | [`packages/AGENTS.md`](packages/AGENTS.md)           |
| `packages/ui`   | The rules for interfaces built with the library, shipped in the npm package                                 | [`packages/ui/AGENTS.md`](packages/ui/AGENTS.md)     |
| `deploy/`       | `compose.yaml` (the self-hosting package), `compose.dev.yaml` (the one development stack), both Dockerfiles |                                                      |
| `e2e/`          | The official OpenAI and Anthropic Python SDKs against KPP, the gateway and a fresh install                  |                                                      |
| `fuzz/`         | cargo-fuzz targets for every KPP decoder                                                                    |                                                      |
| `openapi.json`  | The Platform API document, generated from the gateway's code                                                |                                                      |
| `scripts/`      | The npm package and licence checks                                                                          |                                                      |
| `.github/`      | CI, Release and Pages workflows, templates                                                                  |                                                      |

## Setup

Node.js 24 (`.node-version`), pnpm 11 (pinned in `package.json`, `corepack enable`), rustup (the toolchain in `rust-toolchain.toml` installs itself), Docker with Compose, and Python 3.13.

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

| Changed                                                  | Also run                                                                                                                                                                                                                                                                                       |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| KPP (`crates/kaeso-kpp`, `crates/kaeso-codecs`, `fuzz/`) | `e2e/.venv/bin/pytest -q e2e/conformance`, `cargo bench -p kaeso-codecs --bench codecs --no-run`, `cargo fmt --all --check --manifest-path fuzz/Cargo.toml`, `cargo clippy --manifest-path fuzz/Cargo.toml --all-targets -- -D warnings`, `cargo check --manifest-path fuzz/Cargo.toml --bins` |
| The gateway                                              | `e2e/.venv/bin/pytest -q e2e/sdk`                                                                                                                                                                                                                                                              |
| `deploy/`, a Dockerfile, packaging                       | `e2e/.venv/bin/pytest -q e2e/install`, `docker build -f deploy/console.Dockerfile .`                                                                                                                                                                                                           |
| An interface                                             | Look at every screen you changed in light and dark, at desktop and phone widths, in English, German and Chinese                                                                                                                                                                                |

CI runs all of it on every pull request; the one required check is **CI**. [Testing](https://docs.kaeso.app/development/testing) explains every suite.

## Rules

1. **No code comments, anywhere.** No `//`, `///`, `//!`, `/* */`, JSDoc or JSX comments in Rust, TypeScript, JavaScript or CSS; no `--` in SQL; no `#` comments in TOML, YAML, Python, shell, Dockerfiles, `.dockerignore`, the Caddyfile, `.env.example` or the ignore and attributes files; no `<!-- -->` in HTML, Astro or Markdown; none inside the code blocks of a Markdown file. Code explains itself through names, types, small functions and tests; explanations belong in the documentation. The **No comments** CI job enforces it.
2. **pnpm only** for JavaScript: never npm, npx or yarn; `pnpm dlx` runs a one-off tool. Add a dependency at an exact version with `pnpm --filter <package> add --save-exact <name>`. **Cargo** for Rust. Lockfiles change only through their tool.
3. **Every visible word in English, German and Simplified Chinese**: interfaces, emails, error messages and documentation pages. [Translations](https://docs.kaeso.app/development/translations) has the rules and the glossary.
4. **Tests for every change**, asserting on what a person or an app sees, deterministic (injectable clocks, no sleeps). Coverage gates in `.cargo/config.toml` and every `vitest.config.ts` are never lowered; raise them when coverage rises.
5. **Change everything a change touches, in the same pull request**: the console with the Platform API and `openapi.json`, `packages/ui` when an app needs a component, the pages in `apps/docs/content` when people will notice, a row on the [dependencies](https://docs.kaeso.app/development/dependencies) page for a new dependency.
6. **Documentation lives only in `apps/docs/content`**, in all three languages. No `docs/` folders elsewhere.
7. **MIT-compatible dependencies only**, checked by `cargo deny` and `pnpm licenses:check`.
8. **Never commit secrets or personal data.** Placeholders only: `kaeso.example.com`, `sk-kso-…`, `owner@example.com`.

## Git and releases

- Work on `dev` or on a branch from `dev`, and open pull requests into `dev`. Never push to `prod`.
- Never create tags or releases and never publish crates, npm packages, images or sites: the maintainer runs the Release and Pages workflows.
- Every user-visible change gets a line under `[Unreleased]` in `CHANGELOG.md`, in the same pull request.
- Commit messages say what changed and why.
