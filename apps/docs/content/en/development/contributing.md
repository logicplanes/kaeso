---
title: Contributing
description: Report issues, propose changes, and the rules every change to Kaeso follows.
order: 10
---

Ideas and questions go to [Discussions](https://github.com/logicplanes/kaeso/discussions), bugs and concrete feature requests to [issues](https://github.com/logicplanes/kaeso/issues). The maintainer reviews pull requests from outside contributors at their discretion; for anything larger than a small fix, open an issue first so the approach can be agreed. Everyone taking part follows the [Code of Conduct](https://github.com/logicplanes/kaeso/blob/dev/CODE_OF_CONDUCT.md).

## Discussions and issues

Discussions are for talking an idea through before anything is decided: **Ideas** for a feature you would like, **Q&A** for questions about installing, running or using Kaeso, **Show and tell** for something you built. An idea that is agreed becomes a feature request.

Issues have two templates:

| Template | For |
| --- | --- |
| Bug report | Kaeso does something it should not, or fails to do what it should. It asks for the version, the area, what happened, the smallest reproduction, and how Kaeso runs. For anything you see, it also asks for the language, theme and width, and the browser |
| Feature request | A new capability, a change to an existing one, or something the documentation should explain. Link the provider's documentation or OpenAPI schema for any wire field involved |

Remove API keys, provider keys, session cookies and personal data from anything you paste.

> [!IMPORTANT]
> Report security problems privately through the repository's [security advisories](https://github.com/logicplanes/kaeso/security/advisories/new), never in a public issue. [`SECURITY.md`](https://github.com/logicplanes/kaeso/blob/dev/SECURITY.md) says what to include and what to expect.

## Making a change

1. **Branch from `dev`**, the default branch. `prod` holds the latest release and only the maintainer pushes to it ([branches](releasing.md#branches)).
2. **Write a failing test, then the code.** Every bug fix starts with a test that shows the bug.
3. **Change everything the change touches, in the same pull request.** A Platform API change updates the gateway, the console and `openapi.json` together. A KPP change starts with a [fixture](kpp/fixtures.md) and updates the dialect's mapping page. A new dependency gets a row under [dependencies](dependencies.md). A change to rights, roles or settings changes a [reference snapshot](testing.md#snapshots-and-references); review its diff and update the matching page.
4. **Update the documentation** in `apps/docs/content`, in English, German and Simplified Chinese, when people who use, run or integrate Kaeso will notice the change ([writing documentation](documentation.md)).
5. **Add a line to `CHANGELOG.md`** under `[Unreleased]` for every change people will notice ([the changelog](releasing.md#the-changelog)).
6. **Run the checks** under [before a change is done](testing.md#before-a-change-is-done), and look at every interface you changed in both themes, at desktop and phone widths, in all three languages.
7. **Open a pull request into `dev`.** Keep it to one change. The template asks what changed and why, how it was tested, and for a checklist of the steps above.

### How changes reach dev

Every change reaches `dev` through a pull request. It can be merged once the required **CI** check is green ([continuous integration](testing.md#continuous-integration)), and it is merged as a single squashed commit; merge commits and rebase merges are turned off. The pull request's title becomes the commit's subject, so write it, and every commit message, to say what changed and why. The maintainer can bypass these rules.

### Stacked pull requests

When a change depends on another that isn't merged yet, open them as a stack with GitHub's stacked pull requests instead of one large pull request. The `gh stack` command comes from the `github/gh-stack` extension (`gh extension install github/gh-stack`):

```bash
gh stack init
gh stack add store-tokens
gh stack submit
```

`gh stack init` starts a stack on `dev`, `gh stack add` adds a branch on top of the current one, and `gh stack submit` pushes every branch and opens a pull request for each, based on the branch below it. `gh stack rebase` brings the stack up to date after a branch below changes. The pull requests are reviewed and merged bottom first, each with its own green CI.

## Rules for every change

- **No code comments, anywhere.** No `//`, `///`, `//!`, `/* */`, JSDoc or JSX comments; no `--` in SQL; no `#` comments in TOML, YAML, Python, shell, Dockerfiles, `.dockerignore`, the Caddyfile, `.env.example` or the ignore and attributes files; no `<!-- -->` in HTML, Astro or Markdown; none in the code blocks of a page. Names, types, small functions and tests explain the code, and this documentation explains the rest. The **No comments** CI job enforces it.
- **Tests for every change**, asserting on what a person or an app sees, deterministic, with injectable clocks and no sleeps. Coverage thresholds are never lowered; raise them when coverage rises.
- **Interfaces use only `@logicplanes/kaeso-ui`**, with `className` for layout only, and every word in three languages ([translations](translations.md)).
- **MIT-compatible dependencies only**, checked by `cargo deny` and `pnpm licenses:check` ([license policy](dependencies.md#license-policy)).
- **No secrets or personal data** in the repository. Use placeholders such as `kaeso.example.com`, `sk-kso-…` and `owner@example.com`.
- **No plans in the repository.** Roadmaps, to-do lists and open questions go into issues.
- **pnpm only** for JavaScript; `pnpm dlx` runs a one-off tool. Add a dependency at an exact version with `pnpm --filter <package> add --save-exact <name>`. Cargo builds the Rust code. Lockfiles change only through their tool.

The gateway adds its own rules, such as no panics in library code, no floats for money and exactly one right per endpoint ([gateway](architecture/gateway.md#rules)); so does KPP ([design principles](kpp/index.md#design-principles)).

## AGENTS.md files

The root, `crates`, `packages`, `apps/console`, `apps/docs` and `apps/showcase` each have an `AGENTS.md` with the rules and checks of that part, for coding agents and for people. They summarize these pages. `packages/ui/AGENTS.md` holds the rules for apps built with the UI library and ships in its npm package.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](https://github.com/logicplanes/kaeso/blob/dev/LICENSE).
