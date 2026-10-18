# Changelog

All notable changes to Kaeso are documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.2] - 2026-10-18

Kaeso 0.2.2 lets installs update themselves in a daily or weekly window, and it replaces the `updates.apply` setting with `updates.mode`, so remove `apply` from any TOML file before you upgrade. Installs now look for releases at Kaeso's own update service instead of GitHub's API, so they need outgoing access to it or a mirror set in `updates.releases_url`. Each install has one time zone that the console uses for every time it shows, and users see a What's new dialog once after an update. The release also splits the instance settings into General, Logging and System pages and fixes phone zoom and the sign-in method prompt.

- **Breaking:** `updates.apply` and `KAESO_UPDATES_APPLY` are replaced by `updates.mode` and `KAESO_UPDATES_MODE`; remove `apply` from any TOML file before you upgrade. ([#160](https://github.com/logicplanes/kaeso/pull/160))

**Contributors:** @dvnold

**Full changes:** [v0.2.1...v0.2.2](https://github.com/logicplanes/kaeso/compare/v0.2.1...v0.2.2)

### Added

- Each install has one IANA time zone, set in `instance.timezone` (`KAESO_INSTANCE_TIMEZONE`, default `UTC`). The setup page takes the first user's browser zone, instance admins change it under General, and the console shows every time in it. Platform API responses carry it in the `Kaeso-Time-Zone` header, and API timestamps stay RFC 3339 in UTC. ([#159](https://github.com/logicplanes/kaeso/pull/159))
- Updates install either manually or automatically, in a daily or weekly window in the install's time zone, set with `updates.mode` (`KAESO_UPDATES_MODE`, `manual` or `automatic`). Major releases and releases with manual steps always wait for an admin, and only `updates.check` turns off looking for releases. `updates.apply` is replaced by `updates.mode`, and a TOML configuration that still sets `apply` is refused at start. ([#160](https://github.com/logicplanes/kaeso/pull/160))
- After an update, the console shows what changed once, in a What's new dialog with the release's artwork and an option to show its notes. It lists each release since the last version the user saw, up to 10, newest first, and records the version as seen when the dialog closes. The Platform API adds `GET /me/whats-new` and `POST /me/whats-new/seen` for this. ([#161](https://github.com/logicplanes/kaeso/pull/161))
- Each release gets its own artwork, chosen once by Kaeso's update service and stored with the release, so installs keep the same picture for a version. The What's new dialog draws the stored artwork and falls back to the version's own artwork when a spec is missing or unreadable. The UI library's `ReleaseArt` draws eight geometric styles in twelve palettes from a spec. ([#169](https://github.com/logicplanes/kaeso/pull/169))

#### UI library

- The UI library adds `ReleaseArt` for abstract release artwork and `releaseArtSvg` for writing the same picture as a standalone SVG file. `StatusDot` takes an optional `icon`, and `setFormatTimeZone` makes `formatDate`, `formatRelative` and chart axes show times in one IANA zone, while calendar pickers keep local days. ([#157](https://github.com/logicplanes/kaeso/pull/157))

### Changed

- Instance settings are split into General, Logging and System pages, with General first in the sidebar. The System page holds the email transport and a Send test email button, and the old `/settings/instance/settings` address is removed without a redirect. ([#158](https://github.com/logicplanes/kaeso/pull/158))
- Installs look for new releases at Kaeso's own update service instead of GitHub's API, and `updates.releases_url` now defaults to it. A mirror that answers in the same format or in GitHub's releases format still works, and installs need outgoing access to the service. ([#162](https://github.com/logicplanes/kaeso/pull/162))

#### Console

- The What's new dialog only moves forward now, with Next through missed releases and Continue on the last one, so the Back button is gone. ([#168](https://github.com/logicplanes/kaeso/pull/168))

### Fixed

- Phones no longer zoom in when a text field is tapped in the console, the documentation or the showcase. Text fields render at 16px on touch screens, quick second taps on buttons no longer zoom, and pinch zoom still works. ([#154](https://github.com/logicplanes/kaeso/pull/154))
- The recent sign-in dialog waits for the profile and asks the gateway which methods apply to the account, then offers exactly one way first. People without a password, such as single sign-on accounts, no longer see a password field, and disabled methods are not offered. ([#156](https://github.com/logicplanes/kaeso/pull/156))

## [0.2.1] - 2026-10-18

Kaeso 0.2.1 changes only how releases are published on GitHub. The release page now shows a short summary, while CHANGELOG.md and the documentation keep the full notes. The changes in this release touch only the release workflow, the release-notes script and the releasing guide, so there is nothing to act on when you upgrade a running install.

**Contributors:** @dvnold

**Full changes:** [v0.2.0...v0.2.1](https://github.com/logicplanes/kaeso/compare/v0.2.0...v0.2.1)

### Changed

#### Documentation

- The release page on GitHub now carries a short version of the notes: a tagline, up to five highlights and every breaking change in one sentence. The full notes stay in `CHANGELOG.md` and in the documentation site's changelog, and the releasing guide explains the short version. ([#147](https://github.com/logicplanes/kaeso/pull/147))

## [0.2.0] - 2026-10-18

Kaeso 0.2.0 lets self-hosted installs learn about new releases and, with the optional updater, apply them in one click. It also adds installs that sign in with single sign-on alone, CORS for browser apps, and model availability in the project catalogue. Requests that a provider or model would refuse, such as Anthropic Messages requests without max_tokens, are now refused before they are sent, so check any app that relied on Kaeso filling in the limit. The gateway no longer logs the setup link, the console no longer shows an API key's secret a second time, and the documentation is rewritten, with diagrams, code tabs and a changelog page.

- **Breaking:** Anthropic Messages requests without `max_tokens` are now refused with 400, as Anthropic refuses them. Forced tool choice for Claude Fable 5.1, Opus 5.5 and Sonnet 5.5, and OpenAI Responses output limits below 16, are also refused before sending. Apps that relied on Kaeso filling in the output limit must send `max_tokens` themselves. ([#34](https://github.com/logicplanes/kaeso/pull/34))

**Contributors:** @dvnold

**Full changes:** [v0.1.0...v0.2.0](https://github.com/logicplanes/kaeso/compare/v0.1.0...v0.2.0)

### Added

#### Gateway

- Web apps can call the model API from a browser: `KAESO_CORS_ALLOWED_ORIGINS` lists their origins, which then get CORS headers on `/v1`, preflights answered without a key, and readable `kaeso-*` and `retry-after` headers. Nothing changes while it is empty, the default, and `*` is refused. ([#33](https://github.com/logicplanes/kaeso/pull/33))

#### UI library

- `ThemeSelect`, a select for the system, light or dark theme, with its label and choices in the provider's language, beside `ThemeMenu`. The documentation site's sidebar now shows the same language and theme selects as the showcase. Every `ThemeProvider` on a page follows the same theme choice, so a `ThemeMenu` or `ThemeSelect` in one island changes the others too. ([#25](https://github.com/logicplanes/kaeso/pull/25))
- `icons.update` in the UI library's icon vocabulary.

#### Documentation

- A changelog page on the documentation site, after the introduction, made from `CHANGELOG.md` in English, German and Simplified Chinese. It shows every release with its date in the page's language, an address such as `/changelog#v0.1.0`, Latest and pre-release marks, and links to its GitHub release and to the changes since the release before. The showcase's changelog shows the same releases, marks and links, with its section names in the page's language, and shows unreleased changes only when there are any. ([#29](https://github.com/logicplanes/kaeso/pull/29))
- Both sites list every page in every language in `sitemap.xml`, with `hreflang` alternates and `x-default`, and name the sitemap in `robots.txt`. Every page has a canonical address and Open Graph and Twitter tags with a preview image. Not-found pages and the showcase's examples and states pages stay out of search results. On phones, both sites open search from a button in the header, for which `AppFrame` takes `phoneActions`. The showcase's View source links, `llms.txt` and changelog links open the release's tag instead of `dev`, and its German and Chinese page descriptions and its German search placeholder are fixed. ([#28](https://github.com/logicplanes/kaeso/pull/28))
- Diagrams in the documentation are `mermaid` code blocks drawn at build time into a light and a dark SVG in Kaeso's colours and type. They follow the theme, are named for screen readers by `accTitle` and `accDescr`, and are searchable by their labels; no Mermaid code reaches the browser. A diagram that doesn't parse, or a code block that draws with box-drawing characters, fails the build with its file and line. The UI library adds `ThemeImage`, which follows the theme and prints the light version, and `Code flow="text"`, so long inline code breaks with its sentence. A nested folder's page is named by its title in the sidebar. ([#54](https://github.com/logicplanes/kaeso/pull/54))
- Code examples in the documentation show as language tabs, such as curl, Python and TypeScript, and the reader's choice carries to every group on the page and on the next page. Next steps show as link cards, and every heading links to itself. In `@logicplanes/kaeso-ui`, `CodeBlock` takes `tab` and `onTabChange`, and `Link` gains the `heading` and `card` variants. ([#91](https://github.com/logicplanes/kaeso/pull/91))
- A new guide, Automate with the Platform API, walks from creating a project, adding a provider key and a model, and creating an app's key with a service account, to calling the model and reading usage and logs. It takes every key from the environment. ([#99](https://github.com/logicplanes/kaeso/pull/99))
- Ideas and questions have a home in GitHub Discussions: the issue chooser links to it, an Ideas discussion asks for the idea in a sentence or two, and the contributing page explains the categories and how an agreed idea becomes a feature request. ([#31](https://github.com/logicplanes/kaeso/pull/31))
- Documentation pages show the current version wherever they write `{{version}}`, replaced at build time from the repository's version in text, code and frontmatter, so a release never needs a page edited. A backslash before it keeps it literal.

#### Self-hosting

- Updates for self-hosted installs: the gateway checks GitHub for new releases twice a day and shows the running and newest versions, with release notes, under Settings › Instance › Updates. `KAESO_UPDATES_CHECK=false` turns the check off. The optional updater (`COMPOSE_PROFILES=updater`, `KAESO_UPDATER_TOKEN`) applies a release in one click: it verifies the signed build provenance, pulls images by digest, waits for migrations, and rolls back when the new version doesn't start and the database wasn't migrated. Major releases and releases with manual steps update by hand, and every update is audited. Caddy serves an updating page in three languages meanwhile, and each release carries an attested `release.json` and the `kaeso-updater` image. ([#40](https://github.com/logicplanes/kaeso/pull/40))

### Changed

- The README shows a short recording of the console, in a light and a dark version, instead of a screenshot: connecting a provider key, creating an API key, sending a request and watching it arrive in the request logs. `pnpm demo` records it again on the seeded development stack.

#### Gateway

- The project catalogue knows whether each model's provider serves it to the project's provider key. Check availability on the Models page (`POST …/models/check`) asks the provider, requests teach it too, and a model marked Unavailable is left out of `GET /v1/models`. Changing a provider key resets its models to Not checked. Models that models.dev marks deprecated show Deprecated and have `lifecycle: "deprecated"` in Anthropic listings. A request for a model the provider doesn't serve fails with 404 and `param: model` in the app's format, with a message that names the provider and the model. Kaeso confirms this with the provider's model endpoint first and never retries. ([#37](https://github.com/logicplanes/kaeso/pull/37))

#### Console

- Smaller console downloads: a project's overview loads 25 kB less JavaScript and the sign-in page 4 kB less, the documentation site's frame 3.6 kB less, and a page's code is requested while the console starts instead of after its first render. `AppFrame` loads its phone navigation sheet once the page is idle or on the first tap, and the charts no longer need d3-scale's colour and number-format modules. ([#35](https://github.com/logicplanes/kaeso/pull/35))

#### Documentation

- The documentation follows one binding style guide, with page types, voice, code examples and a checklist. It is now ordered by the reader's journey: introduction, get started, concepts, console, API, self-hosting and development. Moved pages keep their old addresses through redirects, and the concepts overview defines Kaeso's terms once and ends with its pages as next steps. ([#73](https://github.com/logicplanes/kaeso/pull/73), [#74](https://github.com/logicplanes/kaeso/pull/74), [#83](https://github.com/logicplanes/kaeso/pull/83), [#97](https://github.com/logicplanes/kaeso/pull/97))
- The introduction, quick start, calling models, coding tools and model API pages lead with requests you can run in curl, Python or TypeScript. Coding tools walks through Claude Code, Codex and other tools step by step, and the get started overview lists its pages as next steps. ([#49](https://github.com/logicplanes/kaeso/pull/49), [#90](https://github.com/logicplanes/kaeso/pull/90), [#93](https://github.com/logicplanes/kaeso/pull/93), [#94](https://github.com/logicplanes/kaeso/pull/94), [#117](https://github.com/logicplanes/kaeso/pull/117), [#133](https://github.com/logicplanes/kaeso/pull/133))
- Request flow explains each step with a diagram that fits a phone and says that Kaeso's own 503 answers reach OpenAI-format apps as 500. Format translation lists what Kaeso refuses per target, and the rights page explains its notes once and states each grant rule plainly. ([#95](https://github.com/logicplanes/kaeso/pull/95), [#96](https://github.com/logicplanes/kaeso/pull/96), [#98](https://github.com/logicplanes/kaeso/pull/98))
- The model API, errors and Platform API pages are rewritten and checked against the gateway. The errors page opens with a failing request, shows how to read its `kaeso-request-id` and lists one cause per row. The Platform API reference leads with a runnable request and corrects which actions need a recent sign-in, and the API overview is a short list of its pages. ([#48](https://github.com/logicplanes/kaeso/pull/48), [#85](https://github.com/logicplanes/kaeso/pull/85), [#100](https://github.com/logicplanes/kaeso/pull/100), [#101](https://github.com/logicplanes/kaeso/pull/101))
- The console pages are rewritten as task guides and checked against the console, with labels from its catalogs. This covers organizations and members, provider keys and models, API keys and limits, request logs and the Playground, and account and instance administration. A new section explains recent sign-in, and the overview is a short map of the console's pages. ([#51](https://github.com/logicplanes/kaeso/pull/51), [#103](https://github.com/logicplanes/kaeso/pull/103), [#104](https://github.com/logicplanes/kaeso/pull/104), [#105](https://github.com/logicplanes/kaeso/pull/105), [#106](https://github.com/logicplanes/kaeso/pull/106), [#107](https://github.com/logicplanes/kaeso/pull/107), [#108](https://github.com/logicplanes/kaeso/pull/108))
- The self-hosting pages are rewritten and corrected against the code. Configuration is the one reference for every setting, and a new page explains how an install works. Domains, email and sign-in are task guides, Upgrades leads with the update by hand and leaves updater internals to the architecture page, and Troubleshooting gives each message its own fix. Corrections include the email delivery limits and that only PostgreSQL keeps the password it was created with. ([#50](https://github.com/logicplanes/kaeso/pull/50), [#75](https://github.com/logicplanes/kaeso/pull/75), [#121](https://github.com/logicplanes/kaeso/pull/121), [#122](https://github.com/logicplanes/kaeso/pull/122), [#123](https://github.com/logicplanes/kaeso/pull/123), [#124](https://github.com/logicplanes/kaeso/pull/124), [#125](https://github.com/logicplanes/kaeso/pull/125))
- The development guides are rewritten. Local setup is one path from a fresh clone to passing tests, with a new Development stack page and Colima on macOS. Contributing follows the order in which a contributor works, and the testing and releasing pages keep to the commands and steps. ([#52](https://github.com/logicplanes/kaeso/pull/52), [#76](https://github.com/logicplanes/kaeso/pull/76), [#112](https://github.com/logicplanes/kaeso/pull/112), [#114](https://github.com/logicplanes/kaeso/pull/114), [#115](https://github.com/logicplanes/kaeso/pull/115))
- The architecture and KPP internals pages are tightened and corrected. Reasoning across formats and codec design have pages of their own, every mapping page opens with what it is for, and the longest sentences become lists and tables. ([#53](https://github.com/logicplanes/kaeso/pull/53), [#119](https://github.com/logicplanes/kaeso/pull/119), [#120](https://github.com/logicplanes/kaeso/pull/120), [#128](https://github.com/logicplanes/kaeso/pull/128), [#129](https://github.com/logicplanes/kaeso/pull/129), [#130](https://github.com/logicplanes/kaeso/pull/130), [#131](https://github.com/logicplanes/kaeso/pull/131))
- German and Chinese documentation pages use the glossary's terms and the console's labels throughout: German with the formal imperative, Chinese with 实例 for an install and 身份提供商 for the identity provider. Links follow the anchors that changed. ([#132](https://github.com/logicplanes/kaeso/pull/132), [#137](https://github.com/logicplanes/kaeso/pull/137), [#138](https://github.com/logicplanes/kaeso/pull/138))
- The documentation's and the showcase's changelog pages refuse raw HTML, and links other than http, https and mailto, in `CHANGELOG.md`. Each site checks every part it renders with the Markdown parser that renders it, and the build fails and names the line, so a changelog entry can never run code on either site. HTML written in a code span or a code block is fine.

### Removed

#### Documentation

- The performance, dependencies and KPP field classes pages are gone. The licence policy lives under Contributing and the field classes under KPP internals, and each old address leads to the page that replaces it. ([#111](https://github.com/logicplanes/kaeso/pull/111), [#113](https://github.com/logicplanes/kaeso/pull/113), [#127](https://github.com/logicplanes/kaeso/pull/127))

### Fixed

- German and Chinese read consistently across the console, the UI library, the showcase, the documentation site, the emails and the updating page. German addresses the reader as Sie and Chinese as 您, both follow the translation glossary, and every Platform API message now has a German and a Chinese translation in the console. CI fails on text left in English, on informal address and on mixed punctuation, and a gateway test fails when a Platform API message has no console translation. ([#84](https://github.com/logicplanes/kaeso/pull/84), [#86](https://github.com/logicplanes/kaeso/pull/86), [#87](https://github.com/logicplanes/kaeso/pull/87), [#88](https://github.com/logicplanes/kaeso/pull/88), [#89](https://github.com/logicplanes/kaeso/pull/89), [#92](https://github.com/logicplanes/kaeso/pull/92), [#110](https://github.com/logicplanes/kaeso/pull/110))

#### Gateway

- Requests are refused before sending wherever the API or the model would refuse them. Anthropic Messages requests without `max_tokens` get Anthropic's 400 `invalid_request_error` instead of the model's output limit, while the OpenAI formats still get it; OpenAI Responses requests with `max_output_tokens` below 16 are refused, also from other formats; Claude Fable 5.1, Opus 5.5 and Sonnet 5.5 requests with a forced tool choice are refused, with a new catalogue field, Accepts a forced tool choice, to change it per model; and adaptive thinking for a Claude model without it is refused. Anthropic-format error bodies carry the request's id in `request_id`, and capability refusals name the app's own field. ([#34](https://github.com/logicplanes/kaeso/pull/34))
- Rights changes no longer leave a right without one it needs. A change to a member's, service account's or role's rights is refused with `400` when it removes a needed right, also when that right already lacked another of its needs. A custom role change that would leave any member, project member or service account who holds the role with a right but without its need is refused with `400`, in the console too. The message names the right, its need and how many holders it affects, and nobody's rights change. ([#41](https://github.com/logicplanes/kaeso/pull/41), [#42](https://github.com/logicplanes/kaeso/pull/42))
- A single sign-on refused by the identity provider, such as with `access_denied`, ends its sign-in flow and returns to the sign-in page with `next`. When the sign-in began from an organization's sign-in link, it also keeps `org`, so that organization's button shows again. Requests refused before they reach a model in another format name the field the app sent, such as `max_completion_tokens` or `max_tokens` rather than `max_output_tokens`. The bundled Caddy has a health check, so `docker compose up -d --wait` returns only once Caddy serves too. ([#44](https://github.com/logicplanes/kaeso/pull/44))
- Model availability is recorded more reliably. The request log keeps the provider's own 404 message, while the app still gets Kaeso's explanation. A check or request under way while an org admin replaced, assigned, made default or deleted a provider key no longer records the old key's answer over the reset; the check reports such models as `key_changed`. A provider that answers 404 to `GET /v1/models/{model}`, such as an OpenAI-compatible service without that endpoint, no longer makes every model Unavailable. Kaeso marks a model unavailable only when the provider's listing answers and lacks it; otherwise it passes the 404 through and reports the check as `inconclusive`. ([#46](https://github.com/logicplanes/kaeso/pull/46))

#### Console

- The console's audit logs show the actor's profile picture, in the Actor column and in the actor filter, instead of only an initial, and a project's API keys show their owner's picture. ([#30](https://github.com/logicplanes/kaeso/pull/30))
- Everything typed after ⌘K or Ctrl+K reaches the search of the documentation site and the console's command palette, also while its code is still loading and in the moment before its field has focus. `CommandPalette` takes `query` to show a query its host keeps. ([#61](https://github.com/logicplanes/kaeso/pull/61))
- Turning two-factor authentication on or off, or changing your password, no longer signs you out when the console was loading something at the same moment.

#### UI library

- Collapsible, Accordion and the sidebar no longer shift their content while they open or close, so a press during the animation is no longer lost. ([#22](https://github.com/logicplanes/kaeso/pull/22))
- The component registry in `@logicplanes/kaeso-ui` lists every component's default variants again, without an extra `_DEFAULT` entry per component. ([#27](https://github.com/logicplanes/kaeso/pull/27))
- Dialogs whose content changes, such as a `StepDialog`'s steps or a `LayerDialog`'s layers, stay centered and resize smoothly instead of jumping. `StepDialog` takes `result` to show what the last step made in the same dialog, so creating a key or a platform key, or setting up two-factor authentication, no longer opens a second dialog. A dialog stays open while its work is underway: `Dialog` and `LayerDialog` take `busy`, and `StepDialog` and `AlertDialog` are busy while they submit. Closing Create key while the key is being created no longer loses its one-time secret, and no console action can be sent twice. ([#38](https://github.com/logicplanes/kaeso/pull/38))
- The parts of `@logicplanes/kaeso-ui` that load their code on demand recover from a failed download instead of staying broken until a reload. This covers `AppFrame`'s phone navigation and the collapsed sidebar's peek, the sidebar's menus, tooltips, date pickers, and a list's checkboxes and later rows. The next tap, hover, focus or scroll loads the code again, from a fresh address in Chromium and WebKit; nothing retries on its own. While `AppFrame`'s sheet is still on its way, Escape, a tap outside or the toggle takes the tap back, and a pointer that leaves the button cancels the peek. ([#45](https://github.com/logicplanes/kaeso/pull/45))
- `ScrollArea`, and with it the tables in the documentation, scrolls only in its own direction and only when its content overflows, so a table that fits no longer bounces or holds back the page's scroll. Inline `Code` in the documentation flows with the text around it and has 4px of padding inside the chip, so a comma or closing bracket after it stays beside it. `Code` takes `flow="cell"` for long code in a table cell. ([#58](https://github.com/logicplanes/kaeso/pull/58))
- On touch screens, code blocks keep their first line clear of the copy button, so the end of a one-line command is no longer hidden on a phone. ([#78](https://github.com/logicplanes/kaeso/pull/78))
- A step dialog that moves on while a select is still closing no longer causes ResizeObserver loop errors. Every resize observer in the library now applies its changes together on the next frame. ([#82](https://github.com/logicplanes/kaeso/pull/82))

#### Documentation

- Tables in the documentation keep their words whole and scroll inside a named region when they are wider than the screen, and the previous and next links show their label above the page title. A ScrollArea with an `aria-label` is now a region that screen readers announce. ([#24](https://github.com/logicplanes/kaeso/pull/24))
- The introduction's diagram runs top to bottom, so it fits a phone in every language. ([#80](https://github.com/logicplanes/kaeso/pull/80))
- Statements that no longer matched the gateway and the codecs are corrected. Among them are which instance and update settings need a recent sign-in, what effort `none` does on Claude, and which OpenAI error bodies reach which apps. ([#136](https://github.com/logicplanes/kaeso/pull/136))

#### Self-hosting

- The console image sets the permissions of its files itself, so an image built with a strict umask serves the console instead of restarting silently, and it names any file it can't read. Each release pulls both images for both platforms without signing in and fails with what to do when a package is private, and the guides use `docker compose up -d --wait` and show a proxy in Docker on a network of its own. Both sites ship their Content Security Policy in a `<meta>` and in `csp.txt` and `_headers`, with the documentation under `script-src 'self'; style-src 'self'`, now that ScrollArea and Select no longer add an inline `<style>` element. ([#36](https://github.com/logicplanes/kaeso/pull/36))

### Security

#### Gateway

- The gateway no longer logs the setup link, which anyone reading its logs could have used to take over an install before its first user existed; it says to run `kaeso setup-link` instead. Installs can sign in with single sign-on alone: the sign-in page offers the identity provider's button when it is the only one or comes from an organization's sign-in link (`/sign-in?org=…`), and can start at the provider without a click. `GET /platform/v1/auth/oidc/start` links straight to the provider. Setup with passwords off asks for none, and `kaeso user sign-in-link <email>` prints an audited one-time link. Single sign-on returns to the page in `next`, which must be a console path, and the cross-site refusal names the `Origin` header to send. ([#39](https://github.com/logicplanes/kaeso/pull/39))

#### Console

- The console no longer shows an API key's secret again after its dialog was closed; until a reload, the next Create key opened on the last key's secret. The secret now leaves the page and the console's memory once the dialog has closed. Every console dialog opens fresh, including transfer ownership, add owner, send test email, change email, change password, export requests and add models, through `DialogScope` in `@logicplanes/kaeso-ui`, which mounts its content anew on each open. The command palette keeps its results while it closes, and the documentation search opens with an empty query. ([#32](https://github.com/logicplanes/kaeso/pull/32))

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

[Unreleased]: https://github.com/logicplanes/kaeso/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/logicplanes/kaeso/releases/tag/v0.2.2
[0.2.1]: https://github.com/logicplanes/kaeso/releases/tag/v0.2.1
[0.2.0]: https://github.com/logicplanes/kaeso/releases/tag/v0.2.0
[0.1.0]: https://github.com/logicplanes/kaeso/releases/tag/v0.1.0
