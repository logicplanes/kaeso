---
title: Contributing
description: Report a problem, propose a change and get your pull request merged.
order: 2
---

Ask questions and share ideas in [Discussions](https://github.com/logicplanes/kaeso/discussions), and report bugs and concrete feature requests as [issues](https://github.com/logicplanes/kaeso/issues). For anything larger than a small fix, open an issue first, so that you and the maintainer agree on the approach before you write code. Everyone taking part follows the [Code of Conduct](https://github.com/logicplanes/kaeso/blob/dev/CODE_OF_CONDUCT.md).

## Start a discussion

Talk an idea through in Discussions before anything is decided. Use **Ideas** for a feature you would like, **Q&A** for questions about installing, running or using Kaeso, and **Show and tell** for something you built. An idea the maintainer agrees to becomes a feature request.

## Open an issue

Choose the template that fits:

| Template | For |
| --- | --- |
| Bug report | Kaeso does something it shouldn't, or fails to do what it should. It asks for the version, the area, what happened, the smallest reproduction and how Kaeso runs. For something you see, it also asks for the language, the theme, the width and the browser |
| Feature request | A new capability, a change to an existing one, or something the documentation should explain. Link the provider's documentation or OpenAPI schema for any wire field involved |

Remove API keys, provider keys, session cookies and personal data from anything you paste.

> [!IMPORTANT]
> Report security problems privately through the repository's [security advisories](https://github.com/logicplanes/kaeso/security/advisories/new), never in a public issue. [`SECURITY.md`](https://github.com/logicplanes/kaeso/blob/dev/SECURITY.md) says what to include and what to expect.

## Make a change

1. Branch from `dev`, the default branch.
2. Write a failing test first. A bug fix starts with a test that shows the bug.
3. Change the code until the test passes.
4. Change everything else the change touches, in the same pull request. A Platform API change updates the gateway, the console and `openapi.json` together. A KPP change starts with a [fixture](kpp/fixtures.md) and updates the dialect's mapping page.
5. Update the documentation in English, German and Simplified Chinese when people who use, run or integrate Kaeso will notice the change ([writing documentation](documentation.md)).
6. Run the checks under [before a change is done](testing.md#before-a-change-is-done). Look at every interface you changed in light and dark, at desktop and phone widths, in all three languages.
7. Open a pull request into `dev` that holds this one change. The template asks what changed and why, how you tested it, and for its checklist.

After a Platform API change, write `openapi.json` again and commit it with the change:

```bash
cargo run -q -p kaeso -- openapi > openapi.json
```

A change to rights, roles or settings changes a [reference snapshot](testing.md#snapshots-and-references). Review its diff and update the page it belongs to.

## Write the title

The title becomes the pull request's line in the changelog, so write a sentence that says what changed for people who use, run or build on Kaeso. For example: "Keep dialogs centered and resize them smoothly when their content changes". Don't edit `CHANGELOG.md`: the Release workflow writes it from the merged pull requests.

The **Labels** workflow sets the area labels, such as `area: console`, and `internal` from the files you change. The maintainer sets a type label, such as `fixed`, when the title's first word doesn't choose the right section ([labels](releasing.md#labels)).

## Get it merged

Your pull request can be merged once the required **CI** check is green ([continuous integration](testing.md#continuous-integration)). It is merged as one squashed commit, whose subject is the title, so make every commit message say what changed and why.

By contributing, you agree that your contributions are licensed under the [MIT License](https://github.com/logicplanes/kaeso/blob/dev/LICENSE).

## Stack dependent pull requests

When a change builds on another that isn't merged yet, open both as a stack instead of one large pull request. Install the `gh stack` command once:

```bash
gh extension install github/gh-stack
```

Start a stack on `dev`, add a branch for each change on top of the one below, and open the pull requests:

```bash
gh stack init
gh stack add store-tokens
gh stack submit
```

`gh stack submit` pushes every branch and opens a pull request for each, based on the branch below it. After a branch below changes, `gh stack rebase` brings the rest up to date. The pull requests are reviewed and merged bottom first, each with its own green CI.

## Follow the house rules

Every change follows these rules, and CI checks most of them:

- **Comments only where the code can't say why.** Names, types, small functions and tests explain what code does, and this documentation explains how to use it. A comment gives a reason a careful reader would otherwise miss. It explains a workaround for a bug elsewhere (and links to it), a security invariant the types can't hold, or where a limit or an order comes from. It never describes the next line, and code is never committed commented out or with `TODO` notes; open an issue instead. Doc comments belong on the public API of the published crates and packages. Code blocks on these pages never contain comments. The **Comments** job refuses `TODO`, `FIXME`, `XXX` and `HACK` notes, and review decides the rest.
- **Tests assert on what a person or an app sees.** They are deterministic, with injected clocks and no sleeps. Coverage thresholds only ever go up.
- **Interfaces use only `@logicplanes/kaeso-ui`**, with `className` for layout only, and every word in three languages ([translations](translations.md)).
- **pnpm only** for JavaScript, and `pnpm dlx` for a one-off tool. Cargo builds the Rust code, and lockfiles change only through their tool.
- **No secrets or personal data** in the repository. Kaeso itself is `kaeso.example.com`, keys are `sk-kso-…` and the reader or owner is `owner@example.com`. Anything else, such as an identity provider or an SMTP server, takes another name under the reserved `example.com` or `.example` domains ([RFC 2606](https://www.rfc-editor.org/rfc/rfc2606)), never a real host, key or person.
- **No plans in the repository.** Roadmaps, to-do lists and open questions go into issues.

The gateway adds rules of its own, such as no panics and no floats for money ([gateway rules](architecture/gateway.md#rules)). So does KPP ([design principles](kpp/index.md#design-principles)).

The root of the repository and each of `crates`, `packages`, `apps/console`, `apps/docs` and `apps/showcase` have an `AGENTS.md`. Each sums up the rules and checks of its part, for coding agents and for people. `packages/ui/AGENTS.md` holds the rules for apps that build interfaces with the UI library and ships in its npm package.

## Add a dependency

Add a JavaScript dependency at an exact version with `pnpm --filter <package> add --save-exact <name>`, and a Rust one with `cargo add`. Say in one line of the pull request's description why it is needed.

Kaeso is MIT licensed, and everything it ships depends only on permissively licensed code. GPL, LGPL and AGPL code is never allowed. Two checks enforce the policy:

| What ships | Allowed licenses | Checked by |
| --- | --- | --- |
| The gateway and the updater | The list in `deny.toml`: MIT, MIT-0, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, ISC, Unicode and Zlib | `cargo deny check` |
| `kaeso-kpp`, `kaeso-codecs` | The same list, without exceptions | `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` |
| The UI library, the console and the docs | MIT, ISC, BSD-2-Clause, BSD-3-Clause, Apache-2.0 and 0BSD | `pnpm licenses:check`, after `pnpm build` |

`cargo deny check` also refuses crates with security advisories, yanked crates, wildcard versions, and crates from unknown registries or Git repositories. A few crates that the gateway only reaches through other crates have an exception for another permissive license. `deny.toml` names each one. An advisory that cannot affect Kaeso and has no fixed version is ignored with its reason in both `deny.toml` and `osv-scanner.toml`, which the OpenSSF Scorecard reads; `pnpm test` fails when the two lists differ.

Tools that run only in tests or builds, such as axe-core, never reach a package, an image or a published site. The UI library's fonts, Inter and Geist Mono, are under the SIL Open Font License, and `pnpm release:check` refuses a package without their license files.

KPP's library code may depend only on `serde`, `serde_json`, `bytes`, `thiserror`, `indexmap`, `base64` and `mime`, and its policy bans `tokio`, `reqwest`, `hyper` and `axum`. Anything else needs the maintainer's agreement first.

## Next steps

- [Local setup](local-setup.md): Get a checkout running with sample data.
- [Testing](testing.md): Find the suite that covers your change.
- [Writing documentation](documentation.md): Update the pages your change touches.
- [Translations](translations.md): Add the German and Chinese for new text.
