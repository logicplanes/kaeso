# Changelog

All notable changes to Kaeso are documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- A changelog page on the documentation site, after the introduction, made from `CHANGELOG.md` in English, German and Simplified Chinese: every release with its date in the page's language, an address such as `/changelog#v0.1.0`, Latest and pre-release marks, and links to its GitHub release and to the changes since the release before.
- `ThemeSelect`, a select for the system, light or dark theme with its label and choices in the provider's language, beside `ThemeMenu`.
- On phones, the documentation site and the UI library's showcase open the search from a button in the header, beside the navigation toggle; `AppFrame` takes `phoneActions` for such buttons.
- Both sites list every page in every language in `sitemap.xml`, with its `hreflang` alternates and `x-default`, and name it in `robots.txt`; every page has a canonical address, Open Graph and Twitter tags with a preview image of the Kaeso logo, and its language's alternates, and the showcase has a web manifest. Not-found pages and the showcase's examples and states pages are kept out of search results.
- Web apps can call the model API from a browser: `KAESO_CORS_ALLOWED_ORIGINS` lists their origins, which then get CORS headers on `/v1`, preflights answered without a key and readable `kaeso-*` and `retry-after` headers. Nothing changes while it is empty, the default, and `*` is refused.
- `DialogScope` in `@logicplanes/kaeso-ui`: the component inside it mounts anew each time its dialog opens and unmounts once the dialog has finished closing, so a dialog's fields, steps, results and secrets never come back on the next open.
- Both sites ship their Content Security Policy: every page carries it in a `<meta>`, and the build writes it to `csp.txt` and `_headers` for the response header. The documentation runs under `script-src 'self'; style-src 'self'` with nothing inline, and the showcase allows only the inline scripts and styles of its build, by hash.
- Each release pulls both images for both platforms without signing in, and fails with what to do when ghcr.io refuses because a package is private.
- Installs that sign in only with single sign-on: the sign-in page offers the identity provider's button without asking for an address when it is the install's only one or comes from an organization's sign-in link (`/sign-in?org=…`, shown on the Security page), names it (**Continue with Acme ID**) and can start at the provider without a click, except right after signing out or a refused sign-in. `GET /platform/v1/auth/oidc/start` is a link that goes straight to the provider, for bookmarks and intranet pages.
- Setup on an install with passwords off asks for no password and opens the organization's Security page to connect the identity provider; `kaeso user sign-in-link <email>` prints a one-time sign-in link, audited, for when no other way in works.
- Documentation pages show the current version wherever they write `{{version}}`, replaced at build time from the repository's version in text, code and frontmatter, so a release never needs a page edited; a backslash before it keeps it literal.
- Ideas and questions have a home in GitHub Discussions: the issue chooser links to it, an Ideas discussion asks for the idea in a sentence or two, and the contributing page explains the categories and how an agreed idea becomes a feature request.

### Changed

- The showcase's changelog shows the same releases, marks and links as the documentation's, with its section names in the page's language, and shows changes that aren't released yet only when there are any, marked as such.
- The documentation site's sidebar shows the same language and theme selects, side by side at its foot, as the UI library's showcase.
- Every `ThemeProvider` on a page follows the same theme choice, so a `ThemeMenu` or `ThemeSelect` in one island changes the others too.
- The showcase's View source links, its `llms.txt` and the links in its changelog open the source of the release the site was built for, at its tag, instead of the `dev` branch.
- The project catalogue knows whether each model's provider serves it to the project's provider key: **Check availability** on the **Models** page (`POST …/models/check`) asks the provider, requests teach it too, and a model marked **Unavailable** is left out of `GET /v1/models`; changing a provider key resets its models to **Not checked**. Models models.dev marks deprecated show **Deprecated** and have `lifecycle: "deprecated"` in Anthropic model listings.
- Smaller console downloads: a project's overview loads 25 kB less JavaScript and the sign-in page 4 kB less, the documentation site's frame 3.6 kB less, and a page's code is requested while the console starts instead of after its first render. `AppFrame` loads its phone navigation sheet once the page is idle or on the first tap, and the charts no longer need d3-scale's colour and number-format modules.
- The install, quick start, upgrade and reverse proxy guides start Kaeso with `docker compose up -d --wait`, which returns once every service is healthy, also without the bundled Caddy, and fails when one isn't; the reverse proxy guide shows how to reach Kaeso from a proxy in Docker over a network of its own, under names that can't collide with other stacks.
- The Platform API's cross-site refusal says which `Origin` header to send, and the Platform API page documents when `Origin` is needed.
- The documentation's and the showcase's changelog pages refuse raw HTML and links other than http, https and mailto in `CHANGELOG.md`: each site checks every part it renders, such as a section or a topic, with the Markdown parser that renders it, and its build fails and names the line of the file, so a changelog entry can never run code on either site; HTML written in a code span or a code block is fine.

### Fixed

- The console's audit logs show the actor's profile picture, in the Actor column and in the actor filter, instead of only an initial, and a project's API keys show their owner's picture.
- Anthropic Messages requests without `max_tokens` are refused with Anthropic's 400 `invalid_request_error`, as Anthropic refuses them, instead of getting the model's output limit; requests in the OpenAI formats still get it.
- OpenAI Responses requests with `max_output_tokens` below 16, the API's minimum, are refused before anything is sent, and so are requests from other formats whose output limit would reach an OpenAI Responses model below it.
- Error bodies in the Anthropic format carry the request's id in `request_id`, the same as `kaeso-request-id`, also on errors from Anthropic; the OpenAI formats have no such field and keep the id in the header.
- Claude Fable 5.1, Opus 5.5 and Sonnet 5.5 requests with a forced tool choice are refused before sending, in the app's own words (`tool_choice` `required`, `any`, `tool` or a named function), instead of returning Anthropic's sentence about `tool` and `any`; the catalogue has a new field, **Accepts a forced tool choice**, to change it per model.
- Anthropic Messages requests with adaptive thinking for a Claude model without it, such as Claude Opus 4.5, are refused before sending, and capability refusals name the app's own field, such as `reasoning.effort` for OpenAI Responses and `output_config.effort` for Anthropic Messages.
- The descriptions of the showcase's home and changelog pages are in German and Chinese on the German and Chinese pages, and the showcase's German search placeholder fits on a phone.
- Collapsible, Accordion and the sidebar no longer shift their content while they open or close, so a press during the animation is no longer lost.
- Tables in the documentation keep their words whole and scroll inside a named region when they are wider than the screen, and the previous and next links show their label above the page title; a ScrollArea with an `aria-label` is now a region screen readers announce.
- The component registry in `@logicplanes/kaeso-ui` lists every component's default variants again, without an extra `_DEFAULT` entry per component.
- Turning two-factor authentication on or off, or changing your password, no longer signs you out when the console was loading something at the same moment.
- The console's dialogs open fresh every time: Create key starts on an empty first step again after a key was created, and transfer ownership, add owner, send test email, change email, change password, export requests and add models no longer keep what was typed or chosen before they were cancelled. A key's sheet shows only the key it opened for.
- The command palette keeps its results while it closes instead of emptying first, and the documentation search opens with an empty query instead of the last search's headings.
- A request for a catalogue model that the provider does not serve to the project's provider key, such as a retired model or one the key has no access to, now fails with 404 and `param: model` in the app's format, with a message that names the provider and the model and says to check the key's access or remove the model, instead of the provider's bare `model: …`; Kaeso confirms it with the provider's model endpoint first and never retries.
- The console image sets the permissions of its files itself, so an image built from a checkout with a strict umask serves the console instead of restarting without a word; when it can't read its files, it says which and stops.
- ScrollArea and Select no longer add an inline `<style>` element, which a strict Content Security Policy blocks; the stylesheet hides their native scrollbars.
- Single sign-on brings people back to the console page they asked for, from `next`, instead of the start page; `next` must be a console path on the install, so it can't send a browser to another site.
- The description of `org` in `POST auth/oidc/start` names where the organization comes from; the sign-in link it described didn't exist until now.
- A change to a member's, service account's or role's rights is refused with `400` when it removes a right that a remaining right needs, also when that right already lacked another of its needs: removing `members.view` from a member who holds `members.invite` but no longer `org.view`, after their custom role was emptied, used to be accepted.
- Changing a custom role no longer leaves the members, project members or service accounts who hold it with a right they were given without one it needs: such a change is refused with `400` and a message that names the right, its need and how many holders it affects, in the console too, and nobody's rights change.
- The parts of `@logicplanes/kaeso-ui` that load their code on demand recover from a failed download instead of staying broken until the page is reloaded: `AppFrame`'s phone navigation and the collapsed sidebar's peek, the sidebar's menus, tooltips, date pickers' calendars, a list's checkboxes and its rows past the first screen. The next tap, hover, focus or scroll loads the code again, from a fresh address in Chromium and WebKit, which remember a failed download; nothing retries on its own. While `AppFrame`'s sheet is still on its way, Escape, a tap outside or the toggle takes the tap back and a pointer that leaves the button cancels the peek, so the sheet never opens after the person has moved on.
- When Kaeso answers that a provider does not serve a catalogue model, the request log keeps the provider's own 404 message instead of Kaeso's explanation, which the app still gets.
- A model availability check or request that was under way while an org admin replaced, assigned, made default or deleted a provider key no longer records its answer from the old key over the reset; such models stay **Not checked**, and the check reports them as `key_changed`.
- A provider that answers 404 to `GET /v1/models/{model}`, such as an OpenAI-compatible service without that endpoint, no longer makes every model **Unavailable**: Kaeso marks a model unavailable only when the provider's model listing answers and lacks it, and otherwise passes the provider's 404 through, keeps the availability and reports the check as `inconclusive`.
- A single sign-on refused by the identity provider, such as with `access_denied`, ends its sign-in flow and returns to the sign-in page with the page asked for in `next`, as other refusals do; a refused sign-in that began from an organization's sign-in link or button also keeps `org`, so the page shows that organization's button again beside the reason.

### Security

- The gateway no longer logs the setup link, which anyone who could read its logs could have used to take an install over before its first user existed; the log says that the install isn't set up yet and to run `kaeso setup-link`, which prints the link in the terminal.
- The console no longer shows an API key's secret again after its dialog was closed: until the page was reloaded, the next Create key opened on the last key's secret. The secret now leaves the page and the console's memory once the dialog has closed.

## [0.1.0] - 2026-10-16

The first release of Kaeso, the open-source AI gateway you run yourself: the gateway, KPP, the console, the UI library and the documentation, from one repository under one version.

### Added

#### Model API

- OpenAI- and Anthropic-compatible endpoints under `/v1`: `chat/completions`, `responses` and `responses/input_tokens`, `embeddings`, `messages` and `messages/count_tokens`, and `models` and `models/{id}` in both formats, with streaming. Every request is translated through KPP, so each generation format reaches every chat model in the project's catalogue, whatever its provider.
- OpenAI and Anthropic as providers, OpenAI through Responses by default; pooled HTTP clients with rustls and HTTP/2; connect, total, first-byte and stream-idle timeouts, with a per-model timeout for the total and first-byte limits; cancellation of the provider request when the app goes away.
- Errors in the app's own format, no retries and no passthrough; `kaeso-request-id` on every response, and `kaeso-model` and `kaeso-provider` once the model is known, errors included.
- Keepalive frames on long streams, readable reasoning across formats, and reasoning kept between tool calls for OpenAI Chat apps on other providers.
- Tool results nested more than four levels deep are refused with an error that says so, instead of being translated.
- Usage for streams that end early, estimated and marked as such, and honest outcomes for streams that fail after their headers were sent.

#### Catalogue, keys and limits

- A per-project model catalogue filled from a shipped [models.dev](https://models.dev) snapshot, synced every 24 hours, with overrides, custom models, bulk adds and aliases.
- Provider keys owned by the organization: several per provider, one default, assignment to projects, envelope encryption with AES-256-GCM under a rotatable master key ring, write-only.
- API keys per project for members and service accounts, with expiry, inference and platform access, disabling and archiving; organization platform keys; expiry warnings.
- Model access and limits on projects, members and keys: request and token rates, token and spend budgets per day, month or in total, hard or soft, with alerts for budgets; counts up to 10^12 and spend budgets up to 1,000,000,000 USD.
- Hard budgets are never skipped: when a project's limits can't be loaded, the request is refused with 503, and budget counters that may be wrong after a failed update are dropped and rebuilt from PostgreSQL before the next request they limit.
- Exact decimal prices and amounts, which the Platform API accepts only as decimal strings or whole numbers, as documented; costs at the prices of request time; usage by day, week, month, model, key, member and project.

#### Request logs

- One metadata row per model request and, when the organization and project allow it, the request and the full reply in ClickHouse, batched and bounded; retention per organization; filters, export as JSON Lines or CSV in which cells that a spreadsheet would read as formulas are neutralised, deletion by time range and a storage overview.

#### Platform API

- Everything the console does, under `/platform/v1`, described by a generated [OpenAPI document](https://github.com/logicplanes/kaeso/blob/dev/openapi.json): organizations, members, invitations, projects, service accounts, keys, provider keys, models, limits, usage, logs, the audit log, security settings and instance administration.
- Fine-grained rights with built-in and custom roles ([rights](https://docs.kaeso.app/concepts/rights)); every organization and project endpoint checks exactly one right, and a right is granted and removed together with the rights it needs.
- Revisions for catalogue models and limits: `ETag` on reads and changes, and `If-Match` on changes and deletions, answered with 412 when the entry changed since, as the OpenAPI document declares.
- A 503 that names the cause while PostgreSQL, ClickHouse or Valkey can't be reached, and normal answers again as soon as it is back, without a restart.
- One naming and one shape throughout: single sign-on is `sso` in instance settings and in the sign-in methods, and every list, the roles of an organization included, answers with the same envelope of `data`, `has_more` and `next_cursor`.
- Audit logs for the instance, each organization and each project, and an export of the organization's log.
- Streamed Playground requests to a project's models as the signed-in member.

#### Accounts and sign-in

- First-start setup, email and password with TOTP and recovery codes, sign-in links, password resets, single sign-on through each organization's OIDC provider with email domains in Unicode or ASCII form, and sign-up by instance setting with allowed email domains.
- HttpOnly session cookies, session lists, recent sign-in for sensitive actions, required two-factor authentication and single sign-on per organization, and throttled sign-in attempts, each counted before it is checked and per /64 for IPv6 clients.
- Changing the password, resetting it or changing the email address retires older sign-in links, reset links and two-factor challenges; changing the password or two-factor authentication keeps the session's original sign-in time and method.
- Identity providers must use https; plain http is accepted only for providers on private addresses.
- Pictures of people, organizations and projects; the user's own account, language and notifications, email changes with confirmation, leaving an organization and deleting the account; ownership transfer of an organization.
- Instance administration: users (invite, deactivate, reset two-factor authentication, end sessions, make instance admin), organizations, settings, model sources with a manual sync, system status and the instance audit log; organization deletion with a grace period during which the organization stays readable.
- Emails for every account, security, organization and alert event, in English, German and Simplified Chinese.

#### Console

- The web console for a Kaeso install, built with React 19 and TanStack Router and Query, drawn only with `@logicplanes/kaeso-ui`, and talking to the gateway through a client generated from the Platform API document.
- First-run setup, sign-in with a password, a sign-in link, an authenticator code or single sign-on, sign-up where the install allows it, password resets and invitations.
- The organization overview with usage by project and the providers page; the project overview: requests, tokens, cost and errors over a period, charts by day, week or month, a breakdown by model, key or member, limits and recent errors.
- API keys with expiry, access, models and limits and their one-time reveal; the model catalogue with capabilities, prices, aliases and overrides; members, service accounts and roles with their rights; limits and budgets at every level. Edits to models and limits send their revision, and an edit that comes after someone else's says so and loads the current version.
- Request logs in a dense table with filters, a live mode, an inspector for each request and its content, and exports as JSON Lines or CSV.
- The Playground: one prompt to several models side by side, streamed, with their parameters.
- Settings for the account (profile, password and two-factor authentication, sessions, own API keys, memberships, notifications, appearance and language), the organization (general with deletion and restore, members with invitations and roles, security and single sign-on, platform keys, logging and retention, audit log) and the instance (users, organizations, settings, model sources, system status, audit log).
- The command palette and keyboard shortcuts throughout, and a toast while the gateway can't be reached.
- Every word in English, German and Simplified Chinese, following the account's language or the browser's; light and dark themes; phone and desktop layouts; a visible label and an accessible name for every control, a Skip to content link, and lists that work from the keyboard.

#### KPP

- `kaeso-kpp` on crates.io, for Rust 1.85 and later: canonical, stateless types for generation, token counting, embeddings and model listings, with messages of ordered blocks, tools, output formats, reasoning and sampling settings, usage with the provider's raw usage, and `KppError`; stream events with one block lifecycle; model listings paged the same way for every format; origin-bound `Extras`, raw unknown blocks and opaque reasoning, so data KPP does not model is kept and replayed only to its own format; `validate()` and `required_capabilities()`; a public API that can grow without breaking callers.
- `kaeso-codecs` on crates.io, for Rust 1.88 and later: sans-IO codecs for Anthropic Messages (with token counting), OpenAI Chat Completions, OpenAI Responses (with input token counting) and OpenAI Embeddings, covering requests, responses, server-sent event streams, errors and model listings.
- Lossless same-format round trips for every fixture, including exact spellings of `null`, defaults and stream chunking.
- `check()` refuses everything a target cannot express with an `Unsupported` error that names the feature, before anything is sent; errors for coding tools' pre-release fields say which setting to change.
- Cross-format rules: a scoped allowlist of fields that may be dropped, origin-bound file ids and reasoning, reasoning settings mapped by a fixed effort and budget table, and reasoning that travels with the conversation and comes back to its provider unchanged.
- Each crate's README opens with an example that runs as a doctest and links to the reference at [docs.kaeso.app](https://docs.kaeso.app/development/kpp/).
- `dispatch` selects every codec function by format at run time; `sse` parses and writes event streams incrementally; `testing` compares JSON and event streams structurally for round-trip tests in other crates.

#### UI library

- `@logicplanes/kaeso-tokens` on npm: every design value of Kaeso, from the pure white and pure black canvases, a tint-free grey ramp, status and chart colours and the type scale to sizes, radii, elevation, control material and motion; as CSS variables, a Tailwind CSS v4 theme in which only token values exist, TypeScript constants and JSON; with contrast tested for every text role, status colour and chart colour in both modes.
- `@logicplanes/kaeso-ui` on npm: 73 component entry points on Base UI and Tailwind CSS v4, from the app frame, sidebar, buttons and inputs to menus, dialogs, tables, settings groups, code blocks, Markdown and a command palette, with popups, such as the sidebar menus and date pickers, that load their code in the background once the page is idle, so the first press opens them at once; charts drawn as React SVG; Kaeso's own composites for models, providers, keys, status codes and the Playground.
- Keyboard access throughout: a Skip to content link in every app frame, dialogs that open on their first field or their safe action, `LayerDialog` moving focus into the layer it shows and Back returning it to the row or button that opened the layer (lower layers keep their state and scroll position, and ⌘/Ctrl+Enter keeps submitting), and `List` as a WAI-ARIA grid with one tab stop, where the arrows or `j` and `k` move, Space or `x` selects, Shift extends, Enter opens and the right arrow reaches a row's actions.
- `@logicplanes/kaeso-ui/i18n` with every library text in English, German and Simplified Chinese, `@logicplanes/kaeso-ui/format` for numbers, money, tokens and dates (compact token counts keep three significant digits in every language, and amounts too small to show read as less than the smallest one, such as "< $0.000001"), and `@logicplanes/kaeso-ui/theme` for light, dark and system themes without a flash on load.
- Inter and Geist Mono included, one entry point per component, and rules for coding agents shipped in the package: `AGENTS.md`, the design foundations and a generated registry of every component, prop and variant.
- The showcase at [ui.kaeso.app](https://ui.kaeso.app): every component with a live demo and its props, in light and dark, with every page built in English, German and Simplified Chinese at its own address.

#### Documentation

- [docs.kaeso.app](https://docs.kaeso.app) in English, German and Simplified Chinese: getting started, self-hosting, the console, the model API, the Platform API and errors, concepts such as the request flow, format translation and rights, and development of Kaeso itself.
- Pages are Markdown files with links between files checked at build time, GitHub's heading anchors and alerts, images from the pages' own folder, raw HTML kept as text, and only http, https and mailto links out.
- Search with ⌘K or Ctrl+K, the console's frame and themes, an HTML file per page and language with canonical addresses and alternates, a 404 page per language and a sitemap, for any static host.
- Bundle budgets of 216 kB for the frame and 24 kB for each page's content.

#### Self-hosting

- One binary, `kaeso`: `serve`, `migrate`, `master-key generate` and `rewrap`, `setup-link`, `user reset-2fa`, `healthcheck`, which probes the configured listen address, and `openapi`.
- Configuration through `KAESO_*` variables or a TOML file, with secrets from files, validated at startup ([configuration](https://docs.kaeso.app/self-hosting/configuration)): `server.header_read_timeout` (default 30s), which closes connections that are slow to send headers or sit idle, trusted proxies, limits on request bodies and on provider responses, a throttle on failed API key lookups per client address, and email over SMTP or only logged.
- Several gateway instances behind one proxy, sharing PostgreSQL, ClickHouse and Valkey.
- One initial schema of nine PostgreSQL and two ClickHouse migrations, run at startup under a lock; migrations only move forward.
- `/healthz`, `/readyz`, Prometheus metrics on a separate port, including `kaeso_usage_rows_dropped_total` for usage rows PostgreSQL refuses for good, and structured JSON logs.
- A graceful shutdown that finishes requests in progress, writes the last usage and request logs and sends queued emails.
- Non-root images for linux/amd64 and linux/arm64, with provenance and a software bill of materials: `ghcr.io/logicplanes/kaeso-gateway` on a distroless base, and `ghcr.io/logicplanes/kaeso-console`, served by Caddy with a strict content security policy.
- A one-file Docker Compose package with Caddy, PostgreSQL, ClickHouse and Valkey, attached to every release with an example configuration file ([self-hosting](https://docs.kaeso.app/self-hosting)). It asks for a password for each store, `POSTGRES_PASSWORD`, `POSTGRES_SUPERUSER_PASSWORD`, `CLICKHOUSE_PASSWORD` and `VALKEY_PASSWORD`, and for `KAESO_DOMAIN`.
- A hardened package: the gateway signs in to PostgreSQL as an application role that is not a superuser, ClickHouse's user can't manage users and grants, Valkey requires a password, containers can't gain privileges and run without Linux capabilities (Caddy keeps only the right to bind ports 80 and 443, the gateway has a read-only file system), third-party images are pinned by version and digest, and Caddy sends `Strict-Transport-Security`.

#### Quality

- Unit and integration tests against real PostgreSQL, ClickHouse and Valkey, with line coverage gates for the gateway and KPP and coverage thresholds for the UI library, the tokens, the console and the documentation site.
- The official `openai` and `anthropic` Python SDKs against KPP's encoders, against the gateway with a mock provider built from KPP's fixtures, and against a fresh install of the Compose package.
- A fixture corpus with provenance for every format, a cross-format golden matrix, property tests, fuzz targets for every decoder and benchmarks.
- Playwright suites for the console against a stack of its own, and for the documentation site and the showcase with accessibility checks in both themes.
- KPP's crates checked and tested with the oldest Rust version they declare, and the examples in their READMEs run as doctests.
- Dependency audits, MIT-compatible licences checked for every dependency and every shipped bundle, and a check that the code has no comments.

#### Release

- A Release workflow that runs only from `prod`: it prepares the version changes and the changelog without write access to the repository, lets only jobs that run nothing but git push the release commit and the tag, syncs the release commit back to `dev`, and pins every action by commit SHA.

[Unreleased]: https://github.com/logicplanes/kaeso/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/logicplanes/kaeso/releases/tag/v0.1.0
