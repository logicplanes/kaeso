---
title: Contributing
description: Discuss ideas, report issues, propose changes, and the rules every change to Kaeso follows.
order: 10
---

Ideas and questions are welcome in [Discussions](https://github.com/logicplanes/kaeso/discussions), bugs and concrete feature requests in [issues](https://github.com/logicplanes/kaeso/issues). Pull requests from outside contributors are reviewed at the maintainer's discretion; for anything larger than a small fix, open an issue first so the approach can be agreed. Everyone taking part follows the [Code of Conduct](https://github.com/logicplanes/kaeso/blob/dev/CODE_OF_CONDUCT.md).

## Discussions

[Discussions](https://github.com/logicplanes/kaeso/discussions) are for talking things through before anything is decided, with no commitment on either side:

| Category | For |
| --- | --- |
| Ideas | A feature you would like, in a sentence or two; others can add their use case or vote for it |
| Q&A | A question about installing, running or using Kaeso; mark the reply that answered it |
| Show and tell | Something you built with Kaeso |
| Announcements | News from the maintainer, such as releases |

An idea that is agreed becomes a feature request issue.

## Issues

| Template | For |
| --- | --- |
| Bug report | Kaeso does something it should not, or fails to do what it should. It asks for the version, the area, what happened, the smallest reproduction and, for anything you see, the language, theme and width |
| Feature request | A new capability, a change to an existing one, or something the documentation should explain. Link the provider's documentation or OpenAPI schema for any wire field involved |

Remove API keys, provider keys, session cookies and personal data from anything you paste. Report security problems privately, through the repository's [security advisories](https://github.com/logicplanes/kaeso/security/advisories/new), never in a public issue; [`SECURITY.md`](https://github.com/logicplanes/kaeso/blob/dev/SECURITY.md) says what to include and what to expect.

## Making a change

1. **Branch from `dev`**, the default branch. `prod` holds the latest release; only the maintainer pushes to it, to make a release ([branches](releasing.md#branches)).
2. **Write a failing test, then the code.** Every rule has a test, and every bug fix starts with a test that shows the bug.
3. **Change everything the change touches, in the same pull request.** The gateway and the console change together when the Platform API changes, with `openapi.json` written again. A KPP change starts with a [fixture](kpp/fixtures.md) and updates the dialect's mapping page. A new dependency gets a row under [dependencies](dependencies.md). A change to rights, roles or settings changes a [reference snapshot](testing.md#snapshots-and-references); review its diff and update the matching documentation page from it.
4. **Update the documentation.** When people who use, run or integrate Kaeso will notice the change, the pages in `apps/docs/content` change with it, in English, German and Simplified Chinese ([writing documentation](documentation.md)).
5. **Add a line to `CHANGELOG.md`** under `[Unreleased]` for every change people will notice ([the changelog](releasing.md#the-changelog)).
6. **Run the checks.** Everything under [before a change is done](testing.md#before-a-change-is-done) passes, and you have looked at every interface you changed in both themes, at desktop and phone widths, in all three languages.
7. **Open a pull request into `dev`.** Keep it to one change, and write commit messages that say what changed and why. The template asks what changed and why, how it was tested, and for a checklist of the steps above. The one required check is **CI**, which runs every suite the change can affect.

## Rules for every change

- **No code comments, anywhere.** No `//`, `///`, `//!`, `/* */`, JSDoc or JSX comments; no `--` in SQL; no `#` comments in TOML, YAML, Python, shell, Dockerfiles, `.dockerignore`, the Caddyfile, `.env.example` or the ignore and attributes files; no `<!-- -->` in HTML, Astro or Markdown; none in the code blocks of a page. Code explains itself through names, types, small functions and tests; explanations belong in this documentation. The **No comments** CI job enforces it.
- **Tests for every change**, asserting on what a person or an app sees, deterministic, with injectable clocks and no sleeps. Coverage thresholds are never lowered; raise them when coverage rises.
- **Interfaces use only `@logicplanes/kaeso-ui`**, with `className` for layout only, and every word in three languages ([translations](translations.md)).
- **MIT-compatible dependencies only**, checked by `cargo deny` and `pnpm licenses:check` ([license policy](dependencies.md#license-policy)).
- **No secrets or personal data** in the repository: placeholders only, such as `kaeso.example.com`, `sk-kso-…` and `owner@example.com`.
- **No plans in the repository.** Roadmaps, to-do lists and open questions go into issues.
- **pnpm is the only JavaScript package manager**; `pnpm dlx` runs a one-off tool. Add a dependency at an exact version with `pnpm --filter <package> add --save-exact <name>`. Cargo builds the Rust code. Lockfiles change only through their tool.

The gateway's own rules, such as no panics in library code, no floats for money and exactly one right per endpoint, are under [gateway](architecture/gateway.md#rules), and KPP's under [KPP internals](kpp/index.md#design-principles).

## Agents

Each part of the repository has an `AGENTS.md` with its rules and its checks, for coding agents and for people: the root, `crates`, `packages`, `apps/console`, `apps/docs` and `apps/showcase`. They say the same as these pages, shorter. `packages/ui/AGENTS.md` holds the rules for apps built with the UI library and ships in its npm package.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](https://github.com/logicplanes/kaeso/blob/dev/LICENSE).
