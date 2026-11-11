# Kaeso threat model

Kaeso is a self-hosted AI gateway written in Rust. Apps send model requests to it with the official OpenAI, Anthropic, Google Gen AI and ElevenLabs SDKs and an API key Kaeso issued. Kaeso checks the key, the member's rights and the project's limits, picks one of the organization's provider keys and sends the request to a model provider. People manage it in a web console through the Platform API. One install serves many organizations, and each organization has projects, members, roles, API keys, provider keys, budgets, request logs and an MCP portal. Kaeso therefore holds secrets that are worth money (provider keys, provider account sign-ins) and data that is private (prompts, outputs, audit logs).

`SECURITY.md` at the repository root lists what we consider in scope. This file adds where untrusted input enters, which code matters most, how to run things in this image and how we rate severity.

## Trust boundaries and attackers

Assume each of these attackers. Each one controls only what is listed.

| Attacker                                                                                                              | Controls                                                                                                                                                                                                                                                                      |
| --------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Anyone on the internet                                                                                                | Any HTTP request or WebSocket to the install's public paths: `/v1`, `/v1beta`, `/v2`, `/platform/v1` (including the unauthenticated `auth/*` sign-in, sign-up, invitation, password reset and setup endpoints), `/mcp`, `/mcp/<project id>`, `/oauth/*` and the health checks |
| A holder of a project API key (`sk-kso-…`)                                                                            | Everything above, plus model requests and MCP portal calls as that key's member or service account, within that key's models, tools and limits                                                                                                                                |
| A member of an organization                                                                                           | A console session (`__Host-kaeso_session` cookie) with the rights of their organization role and project roles, possibly a custom role with single rights added or removed                                                                                                    |
| An organization admin or owner                                                                                        | Everything in their own organization. They are not trusted with any other organization or with the instance                                                                                                                                                                   |
| An instance admin                                                                                                     | Instance settings, users and updates. Instance administration gives no rights inside organizations or projects; an instance admin reading an organization's prompts, keys or logs is a vulnerability                                                                          |
| A model provider, or anyone who can answer as one (a custom provider's server, a compromised or misbehaving upstream) | Every byte of a reply, streamed or not, its headers, its error bodies, its result links and its usage numbers                                                                                                                                                                 |
| An MCP server an organization added                                                                                   | Tool definitions (names, descriptions, schemas), tool answers, OAuth metadata and token responses                                                                                                                                                                             |
| An identity provider used for single sign-on                                                                          | Its discovery document and token responses, for its own organization only                                                                                                                                                                                                     |
| Whoever writes a prompt or model output that the console later shows                                                  | Markdown, code blocks, links, images and file names that the console renders                                                                                                                                                                                                  |

Trusted: the operator of the install, who holds the master keys, the `.env` file and the database passwords; PostgreSQL, ClickHouse and Valkey, which sit on an internal network only the gateway reaches; the Docker host; GitHub's release attestations and `update.kaeso.app`'s release list as far as the updater's signature and attestation checks allow. A report that needs the attacker to control one of these is out of scope, unless it shows that a check meant to hold against them (such as the updater's verification of a release, or the signature on Valkey cache entries) can be bypassed.

## Where untrusted input enters, by component

### The model API: `crates/kaeso-server/src/model_api`

- Every request body on `/v1`, `/v1beta` (Gemini format) and `/v2` (ElevenLabs) is decoded by KPP, Kaeso's own protocol layer, before anything else looks at it. Request bodies can be large and contain base64 images, audio, PDFs and video.
- `auth.rs` and `crates/kaeso-server/src/auth/keys.rs` check the API key, its state, expiry, models and abilities. `pipeline.rs` resolves the model and alias routes, checks limits and budgets, and picks a provider key.
- `stream.rs` relays streamed replies and keeps them alive. `in_flight.rs` tracks open requests and cancellation.
- `realtime.rs` serves ElevenLabs' realtime WebSockets, where messages pass between the app and ElevenLabs as they are, except for the credentials an app's first message names. Credential replacement and the budget checks during a session matter here.
- `links.rs` fetches result links a provider returns (images, audio) through `kaeso_upstream::Guard::PublicOnly`. `videos.rs` handles long-running video jobs, downloads with `Range` and billing each job once.
- `memory.rs` and `crates/kaeso-server/src/services/continuity.rs` keep reasoning between requests. Data from one key, project or organization must never be returned to another.
- The response cache (`crates/kaeso-server/src/services/response_cache.rs`) stores sealed replies in Valkey. A cache hit must never cross projects, keys with different access, or models.

### KPP: `crates/kaeso-kpp`, `crates/kaeso-codecs`

KPP decodes every supported wire format (OpenAI Chat Completions, Responses, Embeddings, Images, Speech, Transcriptions, Videos; Anthropic Messages; Gemini and Gemini Interactions; ElevenLabs; DashScope; MiniMax; xAI; Z.ai; OpenRouter; Veo) into one internal form and encodes it again for the provider. It reads untrusted input in both directions: requests from apps and replies, streams and errors from providers. It must not panic, use unbounded memory or time, or carry a field from one format into another against its rules. One format smuggling a parameter, a tool call or a system prompt into another is in scope. `crates/kaeso-codecs/src/sse.rs` parses server-sent events from providers.

### The Platform API: `crates/kaeso-server/src/platform`

- Authentication: session cookies (`crates/kaeso-server/src/auth/sessions.rs`), API keys with platform access, organization platform keys, and agents' OAuth access tokens (`crates/kaeso-server/src/auth/agents.rs`).
- Every operation under `orgs/{org}` checks exactly one right from `crates/kaeso-core/src/rights/catalogue.rs`. `instance/*` checks the instance admin flag. `me/*` and `auth/*` need no right. Look for an endpoint that checks the wrong right, checks it on the wrong organization or project, or reads an id from the path or body without scoping it to the authenticated organization.
- `grants.rs`, `rights.rs` and `crates/kaeso-core/src/rights` enforce the grant rules: give only what you hold, act only on peers, only owners manage owners, needed rights are checked. Breaking any of them is privilege escalation.
- Requests that carry the session cookie and change state must send an `Origin` that matches the public URL. Missing or bypassable `Origin` checks are CSRF.
- `platform/auth/*`: password sign-in, magic links, sign-up, invitations, password reset, OIDC single sign-on (`auth/oidc.rs`, `services/sso.rs`), TOTP (`auth/totp.rs`), the first-run setup link, the read-only demo (`demo.rs`) and throttling (`auth/throttle.rs`). Account takeover, sign-in throttle bypass, setup link reuse and changes made through the demo user are in scope. Actions marked "recent sign-in" must need a sign-in in the last 15 minutes.
- Provider keys and provider account sign-ins (`services/credentials.rs`, `services/provider_accounts.rs`, `services/managed.rs`) are sealed with the master key (`crates/kaeso-crypto`). They must never appear in any API response, log line, metric, error message, audit event or ClickHouse row.

### The MCP server and the MCP portal: `crates/kaeso-server/src/mcp`, `crates/kaeso-server/src/portal`

- `/mcp` is Kaeso's own MCP server, which exposes the Platform API to AI agents. Agents sign in through OAuth (`mcp/oauth.rs`), including client ID metadata documents fetched from URLs the client names (`mcp/cimd.rs`). Look at redirect URI checks, PKCE, code reuse, token audience and scope, consent, and SSRF through the metadata URL.
- `/mcp/<project id>` is a project's MCP portal. It forwards tool calls to MCP servers an organization added (`services/mcp_servers.rs`, `services/mcp_tools.rs`, `services/mcp_connections.rs`, `crates/kaeso-upstream/src/mcp.rs`). Who may call which tool is decided in `portal/auth.rs` and `portal/call.rs`: the tool must be on and approved, the caller needs `tools.use`, tool access that includes it and, for a key, the Tools ability. A tool whose definition changed must stay out of every portal under the `hold` policy until an admin approves it. Kaeso signs in to MCP servers with shared or per-person connections; one person's connection must never serve another person.
- `crates/kaeso-mcp` parses JSON-RPC messages from clients and servers.

### Outbound requests and SSRF: `crates/kaeso-upstream/src/guard.rs`

Kaeso calls addresses that organization admins choose: custom provider base URLs, MCP server URLs, OIDC issuers, client metadata URLs and provider result links. These go through the guard, which checks every resolved address on each new connection, checks literal IPs before each call, and uses no proxy. Reaching loopback, private, link-local or cloud metadata addresses through any of them, including through redirects, DNS rebinding, IPv6 forms of IPv4 addresses or a built-in provider whose base URL can be pointed elsewhere, is in scope.

### The console: `apps/console`, `packages/ui`

A React app that renders data from the Platform API: model output, prompts, tool arguments and answers, MCP tool descriptions, Markdown, code blocks, names chosen by other members, and file names. Cross-site scripting or markup injection that runs in another member's session is in scope. The console runs on the same origin as the Platform API.

### The updater: `crates/kaeso-updater`, `crates/kaeso-update`

The optional updater is the only part of Kaeso that talks to Docker, through a socket proxy. The gateway asks it to apply a release with a bearer token over an internal network. It verifies `release.json` and every image against GitHub's build attestations, pins images by digest and rewrites `compose.yaml`. Getting it to apply anything that did not come from Kaeso's Release workflow, to run a container or mount a path the release does not name, or to accept a request from anything but the gateway is critical.

### Other parts that read untrusted input

- `crates/kaeso-core/src/kpmr.rs` reads the signed model catalogue; it must refuse a catalogue no trusted key signed.
- `crates/kaeso-server/src/services/updates.rs` reads the release feed and manifests, but only from the expected origins.
- `crates/kaeso-mail` builds emails that contain names other people chose (organization names, inviter names). Header injection or HTML injection into emails is in scope.
- `crates/kaeso-server/src/services/log_media.rs` and the logs endpoints store and serve request content. Members without `logs.content.view` must never see prompts or outputs.

## Components that matter most and least

Most: `crates/kaeso-server` (model API, Platform API, rights, sessions, MCP portal and OAuth), `crates/kaeso-codecs` and `crates/kaeso-kpp`, `crates/kaeso-upstream` (the guard), `crates/kaeso-crypto`, `crates/kaeso-store` (organization scoping in queries), `crates/kaeso-updater`, and the console's rendering of untrusted content.

Less: `crates/kaeso-mail`, `crates/kaeso-update`, `crates/kaeso-core` outside rights and limits.

Out of scope: `crates/kaeso-testkit` (mock provider, mock identity provider, test databases), `crates/kaeso-bench`, `crates/kpp-conformance`, `crates/xtask`, `e2e/`, `fuzz/` itself, `scripts/`, `.github/`, `apps/docs` and `apps/showcase` (static sites built from content the maintainers write), and `deploy/` cloud templates other than `deploy/compose.yaml` and the Dockerfiles.

## How to exercise it in this image

The image has the Rust toolchain from `rust-toolchain.toml`, nightly with `cargo-fuzz`, `cargo-nextest`, Node.js and pnpm with `node_modules` installed, Python with `e2e/.venv`, and PostgreSQL 18, ClickHouse and Valkey. The workspace, its binaries and every test binary except most of `kaeso-server`'s integration tests are built in `/src/target`; each of those compiles in about a minute the first time it runs, because all its dependencies are already built. There is no network and no Docker.

Start the databases first. The `KAESO_TEST_*` variables already point the tests at them, and each test creates and drops its own database:

```sh
.oss-scanner/services start
```

Run the whole Rust suite, or one crate or test file:

```sh
cargo nextest run --workspace --all-features
cargo nextest run -p kaeso-server --test authorization
cargo nextest run -p kaeso-codecs
```

The gateway's integration tests in `crates/kaeso-server/tests` start a real gateway in-process against fresh databases and the mock provider. They are the best place to write a reproducer. `tests/common` has helpers that create users, organizations, projects, roles, keys and provider keys. Good starting points are `authorization.rs`, `keys.rs`, `mcp_portal.rs`, `portal_signin.rs`, `agents.rs`, `oidc.rs`, `custom_providers.rs`, `limits.rs`, `logged_secrets.rs`, `logs.rs` and `cors.rs`.

The fuzz targets are built in `fuzz/target/<host triple>/release/`, one per KPP decoder plus `sse`, `mcp` and `audio_length`. Their seed corpora are `crates/kaeso-codecs/fixtures/<format>` and `crates/kaeso-mcp/fixtures`:

```sh
host="$(rustc -vV | sed -n 's/^host: //p')"
cargo +nightly fuzz run --target "$host" decode_openai_chat fuzz/corpus/decode_openai_chat crates/kaeso-codecs/fixtures/openai_chat -- -max_total_time=300 -rss_limit_mb=2048 -max_len=65536
```

To run a whole gateway, start the mock provider and the gateway the way `e2e/sdk/conftest.py` does: `target/debug/kaeso-testkit mock-provider 127.0.0.1:<port>`, `target/debug/kaeso-testkit fresh-services --temporary` for databases, `target/debug/kaeso master-key generate` for `KAESO_MASTER_KEYS`, then `target/debug/kaeso serve` with `KAESO_PUBLIC_URL`, `KAESO_LISTEN`, `KAESO_MAIL_TRANSPORT=log`, `KAESO_KPMR_SYNC=false` and `KAESO_PROVIDERS_<ID>_BASE_URL` pointed at the mock provider. `target/debug/kaeso setup-link` prints the link that creates the first user. The Python suites in `e2e/sdk` and `e2e/mcp` do all of this: `e2e/.venv/bin/pytest -q e2e/sdk`.

The console's unit tests run with `pnpm --filter kaeso-console test`, and the UI library's with `pnpm --filter @logicplanes/kaeso-ui test`.

## How we rate severity

Critical:

- Remote code execution in the gateway, the console's Caddy, or the updater.
- Getting the updater to apply an image, a `compose.yaml` or a command that did not come from Kaeso's Release workflow.
- Without any account in the target organization: reading or using its provider keys, provider account sign-ins, API key secrets, master keys or session ids; reading its prompts or outputs; or taking over its accounts.
- Becoming an instance admin, or an owner of an organization you are not an owner of, from an unauthenticated position or from an ordinary account.
- Reusing or forging the setup link, or a sign-in, magic link, invitation or password reset token, so that it takes over an account.

High:

- Any access across organizations, or across projects within an organization, that a member's rights do not allow: reading, changing or using another project's keys, models, limits, logs, tools or MCP connections.
- Privilege escalation inside an organization: getting a right you do not hold, giving rights you do not hold, acting on someone who is not your peer, or bypassing "owners manage owners".
- An instance admin reading or changing an organization's data.
- Calling a model outside a key's allowed models, or spending past a hard budget or a token rate. Kaeso reserves what each running request can cost at most and refuses a request that could overrun what is left ([limits and budgets](../apps/docs/content/en/console/limits-and-budgets.md)); the one documented exception is a request with no output limit to a model with neither an output limit nor a context window.
- Calling an MCP tool that is off, not approved, held after a change, outside your tool access, or through another person's connection.
- A secret listed in `SECURITY.md` appearing in a log line, a metric, an error message, an API response, ClickHouse, the browser's storage or history, or a referrer.
- SSRF that reaches loopback, private, link-local or metadata addresses.
- Stored cross-site scripting in the console that runs in another member's session.
- CSRF that changes rights, keys, provider keys, members, single sign-on or MCP servers.
- Bypassing two-factor authentication, an organization's required single sign-on, or the recent sign-in requirement.
- A single unauthenticated request that crashes the gateway or makes it use unbounded memory.
- A member without `logs.content.view` reading prompts or outputs.
- Cached responses, kept reasoning or streamed data served to a different key, project or organization.

Medium:

- A request with a valid API key that crashes the gateway or makes it use unbounded memory or CPU. A KPP decoder panic or unbounded allocation on provider replies.
- KPP carrying a field from one format into another against its rules, where it changes what the provider does (a tool, a system instruction, a parameter that costs money).
- CSRF on other state-changing endpoints.
- Bypassing the sign-in throttle or a rate limit (not a hard budget).
- Changes made through the read-only demo.
- Email header or HTML injection.
- Costs counted lower than the provider bills them (tokens, media units or per-call features left out), so that budgets and usage under-report.

Low:

- Reading metadata you should not see that holds no secret and no content: names, ids, counts, timestamps.
- Missing security headers or cookie attributes without a working exploit.
- Timing differences on comparisons of values that are not secret.
- A panic in `crates/kaeso` command-line handling that only the operator can trigger.

When a report needs a non-default setting, rate it as it would be with that setting on and say so. A finding that needs the attacker to be an organization owner and only affects their own organization is not a vulnerability unless it breaks a rule that holds even for owners (such as instance isolation, the updater's checks or the secrecy of provider keys toward the API).

## What to leave alone

- Organization owners and admins seeing and changing everything in their own organization. Auditors read the whole organization but never prompts or outputs; that one is in scope.
- An operator with access to the database, Valkey, the master keys or the Docker host.
- A provider answering with wrong or harmful model output. KPP must handle it safely, but the content itself is not a vulnerability.
- An MCP tool whose answer tells a model to do something. Prompt injection through tool answers is the client's problem. A changed tool definition getting past `hold` is in scope.
- Self-XSS that needs the victim to paste something into their own browser console.
- Denial of service that needs a flood of requests from an attacker with a valid key, which the project's own rate limits and budgets are meant to limit.
- Spending your own organization's budget or provider credit.
- Rates and soft budgets letting requests through while Valkey can't be reached; that is documented behaviour, and hard budgets refuse then.
- Findings only in tests, mocks, benchmarks, fixtures, build scripts or CI workflows.
- Missing rate limits on endpoints that need a signed-in session and only affect the caller's own account.
- Dependency advisories that `cargo deny` and Dependabot already track, unless the vulnerable code is reachable from untrusted input in Kaeso.
