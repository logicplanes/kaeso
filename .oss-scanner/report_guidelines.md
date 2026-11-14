# How we would like reports written

## Layout and length

One root cause per report. When the same mistake appears at several call sites, report it once and list every site.

Use these sections, in this order, and keep the whole report under two pages:

1. **Summary**: two or three sentences that say what an attacker can do, not which function is wrong.
2. **Severity**: the rating from `.oss-scanner/threat_model.md` and the line of the rubric it matches.
3. **Attacker**: the starting position, using the attacker names from the threat model's table (anyone on the internet, a holder of a project API key, a member with a given role or rights, an organization admin, an instance admin, a provider, an MCP server, an identity provider). Say what they must already have, such as a key with the Tools ability or a custom role with one extra right.
4. **Affected code**: the commit or version, and the files and functions, as `path:line`. Name the public route or entry point that reaches the faulty code, such as `POST /platform/v1/orgs/{org}/projects/{project}/keys` or `POST /v1/chat/completions`.
5. **Configuration**: the settings it needs, if any differ from the defaults (for example request logs on, a custom provider, an MCP server with a per-person connection, single sign-on required).
6. **Reproduction**: preferably a failing test in `crates/kaeso-server/tests/server` that uses the helpers in `tests/server/common`, or a failing unit test next to the code. Otherwise, the exact HTTP requests in order, with their bodies. Show the output that proves the impact.
7. **Suggested fix**: one paragraph. The patch goes in its own attachment.

## What every report should state

- Whether the issue crosses an organization, a project, a key or a person, and which one.
- Whether it was confirmed by running the reproduction in the built image, or found by reading the code only.
- For a panic or a resource problem: the input size, memory or time it takes, and whether one request is enough.
- For a secret that leaks: which secret, where it ends up and who can read it there.

## Style

- Plain English, short sentences. No marketing language and no speculation beyond what the reproduction shows.
- Use Kaeso's own words for its concepts: organization, project, member, role, right, API key, platform key, provider key, alias route, MCP portal.
- Never put real keys, tokens, hosts or personal data in a report. Use `sk-kso-…` for Kaeso keys, `kaeso.example.com` for the install and `owner@example.com` for people.
- Start the title with the area in brackets, such as `[model API]`, `[platform API]`, `[rights]`, `[MCP portal]`, `[OAuth]`, `[KPP openai_chat]`, `[guard]`, `[console]` or `[updater]`.
