---
title: Contributing
description: Report issues, propose changes, and the rules every change to Kaeso follows.
order: 10
---

Issues are welcome in [the repository](https://github.com/logicplanes/kaeso/issues): bugs, questions and ideas. Pull requests from outside contributors are reviewed at the maintainer's discretion; for anything larger than a small fix, open an issue first so the approach can be agreed. Report security problems privately, through the repository's [security advisories](https://github.com/logicplanes/kaeso/security/advisories/new), never in a public issue.

## Making a change

1. **Branch from `dev`**, the default branch. `prod` only moves through releases.
2. **Write a failing test, then the code.** Every rule has a test, and every bug fix starts with a test that shows the bug.
3. **Change everything the change touches, in the same pull request.** The gateway and the console change together when the Platform API changes, with `openapi.json` written again. A KPP change starts with a [fixture](kpp/fixtures.md) and updates the dialect's mapping page. A new dependency gets a row under [dependencies](dependencies.md). A change to rights, roles or settings changes a reference snapshot; review its diff and update the matching documentation page from it.
4. **Update the documentation.** When people who use, run or integrate Kaeso will notice the change, the pages in `apps/docs/content` change with it, in English, German and Simplified Chinese ([writing documentation](documentation.md)).
5. **Add a line to `CHANGELOG.md`** under `[Unreleased]` for every change people will notice ([the changelog](releasing.md#the-changelog)).
6. **Run the checks.** Everything under [before a change is done](testing.md#before-a-change-is-done) passes, and you have looked at every interface you changed in both themes, at desktop and phone widths, in all three languages.
7. **Open a pull request into `dev`.** Keep it to one change, fill in the template, and say what changed and why.

## Rules for every change

- **No code comments.** No `//`, `///`, `/* */`, JSDoc or JSX comments, no `--` in SQL, no `#` comments in TOML, YAML, Python, shell, Dockerfiles or the Caddyfile, and none in the code blocks of a page. Code explains itself through names, types, small functions and tests; explanations belong in this documentation.
- **Tests for every change**, asserting on what a person or an app sees. Coverage thresholds are never lowered; raise them when coverage rises.
- **Interfaces use only `@logicplanes/kaeso-ui`**, with `className` for layout only, and every word in three languages ([translations](translations.md)).
- **No secrets or personal data** in the repository: placeholders only, such as `kaeso.example.com`, `sk-kso-…` and `owner@example.com`.
- **No plans in the repository.** Roadmaps, to-do lists and open questions go into issues.
- **pnpm is the only JavaScript package manager.** Add a dependency at an exact version with `pnpm --filter <package> add --save-exact <name>`.

The gateway's own rules, such as no panics in library code, no floats for money and exactly one right per endpoint, are under [gateway](architecture/gateway.md#rules), and KPP's under [KPP internals](kpp/index.md#design-principles).

## Agents

Each part of the repository has an `AGENTS.md` with its rules and its checks, for coding agents and for people: the root, `crates`, `packages`, `apps/console`, `apps/docs` and `apps/showcase`. They say the same as these pages, shorter. `packages/ui/AGENTS.md` holds the rules for apps built with the UI library and ships in its npm package.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](https://github.com/logicplanes/kaeso/blob/dev/LICENSE).
