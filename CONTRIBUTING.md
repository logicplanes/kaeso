# Contributing

Thank you for your interest in Kaeso. Issues are welcome: bug reports, questions and ideas. Pull requests from outside contributors are reviewed at the maintainer's discretion; for anything larger than a small fix, open an issue first so the approach can be agreed.

Everything about working on Kaeso is in the documentation:

- [Local setup](https://docs.kaeso.app/development/local-setup): from a fresh clone to a running stack and passing tests.
- [Contributing](https://docs.kaeso.app/development/contributing): how to make a change and the rules every change follows.
- [Testing](https://docs.kaeso.app/development/testing): every suite and what must pass before a change is done.
- [Writing documentation](https://docs.kaeso.app/development/documentation) and [translations](https://docs.kaeso.app/development/translations): pages and words in English, German and Simplified Chinese.
- [Releasing](https://docs.kaeso.app/development/releasing): versions, the changelog and what a release publishes.

The short version: branch from `dev`, write a failing test and then the code, add a line under `[Unreleased]` in [CHANGELOG.md](CHANGELOG.md) for anything people will notice, run the definition of done in [AGENTS.md](AGENTS.md#definition-of-done), and open a pull request into `dev` with the template filled in. `prod` only moves through releases, which the maintainer makes.

Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md), never in a public issue. Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md). By contributing you agree that your contributions are licensed under the [MIT License](LICENSE).
