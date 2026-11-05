<div align="center">
  <br>
  <a href="https://kaeso.app#gh-light-mode-only"><img src="https://raw.githubusercontent.com/logicplanes/kaeso/dev/.github/assets/kaeso-logo-light.svg#gh-light-mode-only" alt="Kaeso" width="320"></a>
  <a href="https://kaeso.app#gh-dark-mode-only"><img src="https://raw.githubusercontent.com/logicplanes/kaeso/dev/.github/assets/kaeso-logo-dark.svg#gh-dark-mode-only" alt="Kaeso" width="320"></a>
  <br>
  <br>
  <h3>The open-source AI gateway you run yourself</h3>
  <p>One OpenAI- and Anthropic-compatible API in front of every provider, with single sign-on, roles, budgets and request logs, written in Rust.</p>
  <p>
    <a href="https://github.com/logicplanes/kaeso/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/logicplanes/kaeso/actions/workflows/ci.yml/badge.svg?branch=dev"></a>
    <a href="https://scorecard.dev/viewer/?uri=github.com/logicplanes/kaeso"><img alt="OpenSSF Scorecard" src="https://api.scorecard.dev/projects/github.com/logicplanes/kaeso/badge"></a>
    <a href="LICENSE"><img alt="License: MIT" src=".github/assets/badges/license.svg"></a>
  </p>
  <p>
    <a href="https://kaeso.app"><b>Website</b></a> ·
    <a href="https://docs.kaeso.app"><b>Documentation</b></a> ·
    <a href="https://demo.kaeso.app"><b>Live demo</b></a> ·
    <a href="https://github.com/logicplanes/kaeso/discussions"><b>Discussions</b></a> ·
    <a href="CHANGELOG.md"><b>Changelog</b></a>
  </p>
  <br>
</div>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/logicplanes/kaeso/dev/.github/assets/demo-dark.gif">
  <img alt="The Kaeso console: connecting an Anthropic provider key, sending an OpenAI-format request to claude-haiku-5-5 from a terminal, and the request arriving at the top of the request logs" src="https://raw.githubusercontent.com/logicplanes/kaeso/dev/.github/assets/demo-light.gif">
</picture>

## Install

On a server or your computer with Docker and Compose 2.24 or newer:

```sh
curl -fsSL https://kaeso.app/install.sh | sh
```

It installs Kaeso with PostgreSQL, ClickHouse and Valkey into `./kaeso`, starts it at `https://localhost` and prints the link that creates the first user. Add `-s -- --domain kaeso.example.com` to serve your own domain. The [quick start](https://docs.kaeso.app/get-started/quick-start) continues from there, and [install by hand](https://docs.kaeso.app/self-hosting/installation#install-by-hand) shows each step.

## Send your first request

Point the OpenAI SDK at Kaeso and call any model in your project, Claude included:

```python
from openai import OpenAI
client = OpenAI(base_url="https://kaeso.example.com/v1", api_key="sk-kso-…")
print(client.responses.create(model="claude-haiku-5-5", input="Hello").output_text)
```

The Anthropic SDK works the same way with `base_url="https://kaeso.example.com"`. Kaeso translates the request and the reply between the formats ([calling models](https://docs.kaeso.app/get-started/calling-models)).

## Why Kaeso

- **Single sign-on and roles.** Each organization signs in through its own OpenID Connect provider, for any number of people at no cost, with organization and project roles and single rights on top ([members and roles](https://docs.kaeso.app/console/members-and-roles)).
- **Budgets.** Hard or soft spend and token budgets and request rates per project, member or key, checked before a request reaches the provider ([limits and budgets](https://docs.kaeso.app/console/limits-and-budgets)).
- **Request logs.** Every request with its model, status, tokens, cost, timing and caller, and its content if you keep it, on your own server ([request logs](https://docs.kaeso.app/console/request-logs)).
- **Verifiable releases.** Every image and release file carries signed build provenance from the Release workflow. `gh attestation verify` checks it, and the updater installs nothing whose signatures don't match ([upgrades](https://docs.kaeso.app/self-hosting/upgrades)).
- **Rust speed.** The gateway is one Rust process with no garbage collector between your app and the provider, and [its benchmark](https://docs.kaeso.app/development/benchmark) measures the latency it adds on your own hardware.

## How it compares

Checked on 5 November 2026 against each project's repository and documentation. [Open an issue](https://github.com/logicplanes/kaeso/issues/new/choose) when a row is out of date.

|                                          | Kaeso               | LiteLLM                                                                           | Bifrost                                                 | Portkey Gateway                                                                                               |
| ---------------------------------------- | ------------------- | --------------------------------------------------------------------------------- | ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Gateway written in                       | Rust                | Python                                                                            | Go                                                      | TypeScript                                                                                                    |
| License                                  | MIT                 | MIT, except [`enterprise/`](https://github.com/BerriAI/litellm/blob/main/LICENSE) | Apache 2.0                                              | MIT                                                                                                           |
| Single sign-on when self-hosted for free | Yes, OpenID Connect | Up to 5 users ([docs](https://docs.litellm.ai/docs/proxy/admin_ui_sso))           | Enterprise ([pricing](https://www.getmaxim.ai/pricing)) | Enterprise ([docs](https://portkey.ai/docs/product/enterprise-offering/org-management/sso))                   |
| Budgets when self-hosted for free        | Yes                 | Yes ([docs](https://docs.litellm.ai/docs/proxy/users))                            | Yes ([pricing](https://www.getmaxim.ai/pricing))        | Enterprise and some Pro plans ([docs](https://portkey.ai/docs/product/ai-gateway/virtual-keys/budget-limits)) |
| Web console when self-hosted             | Yes                 | Yes                                                                               | Yes                                                     | Request logs only ([README](https://github.com/Portkey-AI/gateway#readme))                                    |

## Works with

- **[Claude Code](https://docs.kaeso.app/get-started/coding-tools#set-up-claude-code)** and **[Codex](https://docs.kaeso.app/get-started/coding-tools#set-up-codex)**, with Kaeso as their model provider.
- **[Cursor](https://docs.kaeso.app/api/mcp#cursor)** and Claude Code, which manage Kaeso through its MCP server.
- **The [OpenAI SDK](https://docs.kaeso.app/get-started/calling-models)** and **the [Anthropic SDK](https://docs.kaeso.app/get-started/calling-models#use-the-anthropic-sdk)**, in any language, and any tool that lets you set an OpenAI or Anthropic base URL.

## Community

- [Discussions](https://github.com/logicplanes/kaeso/discussions) for questions, ideas and what you built.
- [@kaesoapp on X](https://x.com/kaesoapp) for releases.
- [Good first issues](https://github.com/logicplanes/kaeso/issues?q=is%3Aissue%20state%3Aopen%20label%3A%22good%20first%20issue%22) to start contributing, with the [contributing guide](https://docs.kaeso.app/development/contributing).

## License

[MIT](LICENSE). The Kaeso name and logo are not covered by the license. All trademarks belong to their owners; Kaeso is not affiliated with any provider or client it names. Third-party notices, trademarks included, are in [NOTICE.md](NOTICE.md).
