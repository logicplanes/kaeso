# Security policy

## Supported versions

Security fixes are made for the latest release only. Please make sure the problem still occurs with the latest release before reporting it.

## Reporting a vulnerability

Do not open a public issue for a security problem. Report it privately through GitHub's private vulnerability reporting:

https://github.com/logicplanes/kaeso/security/advisories/new

Please include:

- a description of the problem and its impact;
- the affected version or commit, and how Kaeso was deployed;
- the steps, the smallest request or input, or a failing test that reproduces it;
- any suggestion for a fix you may have.

## What to expect

- An acknowledgement within three working days.
- An assessment and, if the report is confirmed, a plan for the fix within ten working days.
- A fix in a new release, followed by a published security advisory that credits the reporter unless they prefer otherwise. We agree on a disclosure date with you.

## Scope

Kaeso holds provider keys, API keys, sessions and request logs, and it decodes untrusted input from apps and providers. In scope, for example:

- reading or using another organization's or project's data, keys, provider keys or logs;
- authentication or authorization bypasses on the model API or the Platform API: missing right checks, CSRF, session fixation, key confusion;
- secrets (provider keys, master keys, API key secrets, session ids, sign-in tokens, TOTP secrets, passwords) appearing in logs, metrics, error messages, API responses, ClickHouse, the browser's storage or history, or a referrer;
- spending past a hard budget, or calling a model outside a key's access;
- server-side request forgery through OIDC discovery or provider base URLs;
- input that makes the gateway or a KPP codec panic, use unbounded memory or time, carry data from one format to another against KPP's rules, or leak data from one request into another;
- cross-site scripting or markup injection through anything the console, the UI library or the documentation site renders, such as Markdown, code blocks, model output or a page's content;
- weaknesses of the container images, the Compose package or the console's Caddy configuration.

Never put keys, tokens or personal data in a report, an issue, a fixture or a test. Use placeholders such as `sk-kso-…`, `kaeso.example.com` and `owner@example.com`.
