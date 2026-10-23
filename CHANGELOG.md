# Changelog

All notable changes to Kaeso are documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-10-23

Kaeso 0.4.0 lets organizations add ChatGPT accounts as OpenAI provider keys, and Grok accounts as an experimental SpaceX provider key, so projects can draw on those plans instead of API accounts. Nothing in this release is breaking, so existing provider keys and integrations need no change. The gateway's ctutils dependency moves to 0.4.4 to fix a timing advisory in encryption and password hashing, so upgrading is recommended for every install.

**Contributors:** @dvnold

**Full changes:** [v0.3.9...v0.4.0](https://github.com/logicplanes/kaeso/compare/v0.3.9...v0.4.0)

### Added

- An organization can add a ChatGPT account as an OpenAI provider key, so a project's requests use the account's ChatGPT plan instead of an OpenAI API account. Sign-in ends with pasting the loopback address the browser lands on, and the account's requests go to OpenAI's plan endpoint, which refuses embeddings and token counts. Its tokens renew before they expire, and an account whose sign-in is revoked shows as signed out until an admin signs it in again. ([#365](https://github.com/logicplanes/kaeso/pull/365), [#366](https://github.com/logicplanes/kaeso/pull/366), [#367](https://github.com/logicplanes/kaeso/pull/367))
- Organizations can add a Grok account as an experimental SpaceX provider key, so a project's requests use the account's Grok plan where it includes API access. The key is marked experimental in the console, the Platform API and the documentation, because it relies on xAI's device sign-in rather than a documented contract for third-party apps. A plan without API access is reported as such, and requests are not sent to xAI. ([#369](https://github.com/logicplanes/kaeso/pull/369))

### Changed

#### Documentation

- The repository runs the OpenSSF Scorecard weekly, after pushes to `dev` and on demand, and the README shows its score. The rsa timing advisory it reports, which no release of that crate fixes and which cannot affect Kaeso, is ignored in `osv-scanner.toml` with the same reason as in `deny.toml`, and a test keeps the two files in step. ([#363](https://github.com/logicplanes/kaeso/pull/363), [#364](https://github.com/logicplanes/kaeso/pull/364))

### Security

- The gateway now uses ctutils 0.4.4, which fixes RUSTSEC-2026-0385: a dependency that generated masks and compared values in non-constant time, used by envelope encryption, password hashing and signed tokens. Installs need no configuration change. ([#370](https://github.com/logicplanes/kaeso/pull/370))

## [0.3.9] - 2026-10-23

Kaeso 0.3.9 changes the sign-in-again dialog so that single sign-on is offered only when no password or sign-in link can confirm a recent sign-in, and providers that cannot confirm one are no longer offered. Integrations that read the sign-in-again, security or SSO fields, or handle the sso_error codes, must be updated because `sends_auth_time` is renamed and two codes are replaced. Operators gain the `kaeso instance admin-sso` command for choosing the organization whose single sign-on may administer an instance.

- **Breaking:** The `sends_auth_time` field of the security and SSO endpoints is renamed to `confirms_sign_in`, and the `stale_sign_in` and `no_auth_time` codes are replaced by `cannot_confirm`. Integrations should read the new field, handle the new code, and expect `sso` to be empty whenever a password, sign-in link or set-password option is available. ([#361](https://github.com/logicplanes/kaeso/pull/361))

**Contributors:** @dvnold

**Full changes:** [v0.3.8...v0.3.9](https://github.com/logicplanes/kaeso/compare/v0.3.8...v0.3.9)

### Added

- The `kaeso instance admin-sso` command sets the organization whose single sign-on may administer the instance, or clears it with `none`, from the server's shell. Installs without passwords or sign-in links could not set this before without editing the database. Each change is written to the audit log and applies at once. ([#360](https://github.com/logicplanes/kaeso/pull/360))

### Changed

#### Documentation

- The GitHub release page keeps its tagline, highlights and breaking changes, then lists every changelog entry under its section heading instead of linking to the full changelog. ([#359](https://github.com/logicplanes/kaeso/pull/359))

### Fixed

- Sign-in again offers single sign-on only when no password or sign-in link can confirm the account, so people are no longer sent back to a dialog that single sign-on cannot satisfy. A confirmation whose ID token has no `auth_time`, or a stale one, is refused with the `cannot_confirm` code, and the provider is no longer offered for recent sign-in. ([#361](https://github.com/logicplanes/kaeso/pull/361))

## [0.3.8] - 2026-10-23

Kaeso 0.3.8 contains no change that operators must act on when they upgrade. It moves the console, gateway and updater images to newer Go, Node and distroless bases, pins newer ClickHouse and Mailpit versions in the Compose files, and updates Rust and JavaScript dependencies. The console's API client no longer relies on the Redocly OpenAPI core package, and toasts in the UI library no longer catch clicks meant for buttons beside them.

**Contributors:** @dvnold

**Full changes:** [v0.3.7...v0.3.8](https://github.com/logicplanes/kaeso/compare/v0.3.7...v0.3.8)

### Changed

- Upgrades the Rust dependencies, including tokio from 1.53.2 to 1.53.3 and tower-http from 0.6.11 to 0.7.1, and updates clickhouse and aws-lc-rs. ([#351](https://github.com/logicplanes/kaeso/pull/351), [#352](https://github.com/logicplanes/kaeso/pull/352))
- Upgrades 24 JavaScript packages across the console, UI library, showcase and documentation, among them React and react-dom 19.3.1, TanStack Router, Playwright and Vitest. ([#357](https://github.com/logicplanes/kaeso/pull/357))

#### Console

- The console generates its API client without a Redocly config of its own, so it no longer depends on the Redocly OpenAPI core package. ([#354](https://github.com/logicplanes/kaeso/pull/354))

#### Documentation

- Upgrades mermaid from 11.17.2 to 12.2.0 and regenerates the documentation diagrams as SVG files. ([#355](https://github.com/logicplanes/kaeso/pull/355))

#### Self-hosting

- Moves the console, gateway and updater images to newer Go, Node and distroless base images, and pins newer ClickHouse and Mailpit versions in the Compose files. ([#348](https://github.com/logicplanes/kaeso/pull/348), [#349](https://github.com/logicplanes/kaeso/pull/349), [#350](https://github.com/logicplanes/kaeso/pull/350))

### Fixed

#### UI library

- Toasts no longer catch clicks meant for a button beside or below them. The hover area that keeps an expanded stack open now covers only the gaps between toasts, so a button in a member's sheet always receives its click. ([#358](https://github.com/logicplanes/kaeso/pull/358))

## [0.3.7] - 2026-10-22

Operators need to act on this release: compose.yaml changed beyond its image versions and must be installed by hand, and a reverse proxy of your own is trusted only when it is named in the trusted proxies setting. Single sign-on now starts only after the email, organization sign-in links are removed, and changing or deleting an SSO connection ends the sessions it signed in. The gateway also stops deeply nested requests from crashing it, keeps one key from holding the whole translation memory budget, and refuses unpriced models under a spend budget.

- **Breaking:** Organization start links are removed and sign-in starts after the email, so point links and bookmarks at `/sign-in` instead. ([#319](https://github.com/logicplanes/kaeso/pull/319))
- **Breaking:** A model a key may not use now answers 404 like an unknown model instead of 403, so clients that match on 403 must change. ([#334](https://github.com/logicplanes/kaeso/pull/334))
- **Breaking:** Metrics listen on loopback by default, so a scraper on another host needs `metrics_listen` set to a reachable address. ([#335](https://github.com/logicplanes/kaeso/pull/335))
- **Breaking:** `compose.yaml` changed beyond its image versions, so install the release's file by hand and run Compose from the install's folder. ([#342](https://github.com/logicplanes/kaeso/pull/342), [#343](https://github.com/logicplanes/kaeso/pull/343))
- **Breaking:** A reverse proxy of your own is trusted only when named in the trusted proxies setting, since the default trusts loopback and the edge network. ([#344](https://github.com/logicplanes/kaeso/pull/344))

**Contributors:** @dvnold

**Full changes:** [v0.3.6...v0.3.7](https://github.com/logicplanes/kaeso/compare/v0.3.6...v0.3.7)

### Added

- Instance administration can be opened to one organization's single sign-on through a new instance setting, which names that organization or none, the default. An instance admin's session from that provider can then use the instance API while the organization manages the account. Changing the setting still needs a password or sign-in link, and each change is audited as `instance.admin_sso_update`. ([#338](https://github.com/logicplanes/kaeso/pull/338))

#### Console

- An organization's Security page says when its identity provider's sign-ins can never count as recent, and `GET orgs/{org}/security` returns it as `sso.sends_auth_time`.

### Changed

- Prometheus metrics listen on loopback by default instead of on every interface. The gateway image still listens on all interfaces inside its container, which is what the documented port mapping reaches, and Compose publishes the port nowhere. ([#335](https://github.com/logicplanes/kaeso/pull/335))
- A sign-in through the single sign-on of the organization that manages an account now opens all of that person's organizations, and Require single sign-on applies only to those accounts. An organization manages the accounts whose address is at one of its verified domains, and guests keep their own sign-in. ([#337](https://github.com/logicplanes/kaeso/pull/337))
- The identity provider of the organization that manages an account now owns its address. `POST me/email` is refused for such an account, and a sign-in through that provider replaces the stored address with a newer verified one at a verified domain. An address another account holds is never taken. ([#339](https://github.com/logicplanes/kaeso/pull/339))
- Signing in again from a managed account's single sign-on offers that organization's identity provider first, with the password and an email link beside it. `GET me/sign-in-again` lists the managing organization first in `sso`, and a provider that doesn't send `auth_time` is left out. ([#340](https://github.com/logicplanes/kaeso/pull/340))

### Fixed

#### Console

- The sign-in-again dialog no longer keeps offering an identity provider that never sends `auth_time`, such as one run on Clerk. After one failed confirmation it explains why and offers the password or an email link, and a confirmation without `auth_time` now returns `no_auth_time` instead of `stale_sign_in`.
- Setting up an authenticator app from a single sign-on session asks to sign in another way instead of showing an error whose Try again failed the same way.

#### UI library

- A dialog that finishes closing after another dialog opened no longer takes focus from it, so a search typed into a combobox stays and its options remain. ([#346](https://github.com/logicplanes/kaeso/pull/346))

### Security

- Single sign-on starts only after the person enters their email, and the organization start link is removed. A planted link can no longer sign a signed-out browser in to another account, and links point to `/sign-in` instead. ([#319](https://github.com/logicplanes/kaeso/pull/319))
- Deleting an organization's SSO connection, or saving it with a new issuer or client id, ends every session that connection signed in, and the audit entry records `sessions_ended`. `DELETE me/sso-links/{org}` likewise ends the account's sessions from that organization's SSO and now needs a recent sign-in. ([#320](https://github.com/logicplanes/kaeso/pull/320))
- Reset links, sign-in links and the link sign-up sends to an existing account go to the account's stored address. Addresses are compared in NFKC-normalized form, so lookalike addresses no longer receive another account's links or dodge its sign-in limits. ([#321](https://github.com/logicplanes/kaeso/pull/321))
- Confirming a new email address now signs out every other session of the account. The session that opens the confirmation link stays when it belongs to the account, otherwise the session that asked for the change stays. ([#323](https://github.com/logicplanes/kaeso/pull/323))
- The authenticator-code lock also applies to account changes that ask for a code, and regenerating recovery codes needs an authenticator or recovery code. IPv4-mapped client addresses count as their IPv4 address for throttling, and a changed email address is reported as in use only after the second factor passed. ([#324](https://github.com/logicplanes/kaeso/pull/324))
- A deeply nested model API request is refused with 400 instead of aborting the gateway, which dropped every request and stream of every organization. Bodies nested more than 127 levels deep are refused before any codec parses them. ([#326](https://github.com/logicplanes/kaeso/pull/326))
- A request holds its full translation memory estimate only while it is decoded and encoded, and one key can hold at most a quarter of the `body_limits.in_flight` budget. A request that finds no room within 10 seconds gets 503 with `Retry-After: 10`, and whitespace no longer counts toward the estimate. ([#327](https://github.com/logicplanes/kaeso/pull/327))
- Request bodies have a total read deadline, `server.body_read_deadline`, 60 seconds by default, so a body sent one byte at a time gets the same 400 as a stalled one. The gateway also caps its connections with `server.max_connections`, 4096 by default. ([#328](https://github.com/logicplanes/kaeso/pull/328))
- The request log's CSV export quotes every text cell and adds an apostrophe in front of values that start a formula or contain a semicolon, pipe or tab. Spreadsheet programs can no longer run a formula from a model name or user agent. ([#329](https://github.com/logicplanes/kaeso/pull/329))
- The models catalogue that sets the prices spend budgets use is fetched only over https, or over plain http when the feed is on the local machine. Redirects are followed at most three times and only to the same host over https. ([#331](https://github.com/logicplanes/kaeso/pull/331))
- Under a spend budget, a model with no price, or a zero price from the feed, is refused with 403 naming the model instead of counting as free. A price an admin sets by hand, including zero, still counts as set. ([#332](https://github.com/logicplanes/kaeso/pull/332))
- Model ids of `.` and `..`, and ids with either between slashes, are refused in the codecs and the Platform API. They can no longer change the path of a provider request. ([#333](https://github.com/logicplanes/kaeso/pull/333))
- A model a key may not use now answers 404 with the same message as a model that doesn't exist, instead of 403. A restricted key can no longer list a project's catalogue by probing model names. ([#334](https://github.com/logicplanes/kaeso/pull/334))
- Release builds no longer restore shared caches or run dependency build scripts while holding publishing credentials. Release notes are checked for install commands, pipes into a shell and instructions to change variables. ([#341](https://github.com/logicplanes/kaeso/pull/341))
- The console and the edge run a Caddy built with Go 1.26.9 and `x/net` 0.60.0, which fix six HTTP/2 vulnerabilities. The edge now runs the console image without the capability to bind privileged ports, and PostgreSQL is pinned to a rebuild with a fixed zlib. ([#342](https://github.com/logicplanes/kaeso/pull/342))
- The updater refuses containers that change any security-relevant setting, and its Docker proxy allows bind mounts only from the install's folder. Inspect calls accept only the full or short container id that the updater itself uses. ([#343](https://github.com/logicplanes/kaeso/pull/343))
- New installs write their files with a restrictive umask and give `master-keys` to the gateway's user, so only that user can read it. The edge network has a fixed subnet that the gateway trusts for forwarded addresses, and the edge Caddy runs read-only with its admin API off. This release changes `compose.yaml` beyond its image versions, so install it by hand, and a reverse proxy of your own is trusted only when named in the trusted proxies setting. ([#344](https://github.com/logicplanes/kaeso/pull/344))
- The console refuses sign-in redirects to other hosts, forgets secrets and one-time link tokens once they are used, requires Trusted Types, and updates by hand only after every check passes. ([#345](https://github.com/logicplanes/kaeso/pull/345))

#### Gateway

- A request that touches a session now invalidates its cached entry instead of writing back the row it read, so a revoke, deactivation or demotion takes effect at once instead of lingering for up to 30 seconds. ([#322](https://github.com/logicplanes/kaeso/pull/322))
- A key restricted to some models no longer gets every model of its project during a cache invalidation race. The cached access table lists unrestricted keys and service accounts too, an unlisted key gets no models, and the gateway reloads the table once when it meets an unknown key. ([#330](https://github.com/logicplanes/kaeso/pull/330))

## [0.3.6] - 2026-10-22

### Changed

#### Console

- The sign-in page always asks for the email first, and offers single sign-on only for the organization that verified the email's domain, as large services do. Links to `/sign-in?org=…` show the same email page.

#### Gateway

- Single sign-on is set up per organization and no longer switched on for the whole install: `auth.oidc` and `KAESO_AUTH_OIDC` are gone. In a config file the old key stops startup with a clear message; as an environment variable it only logs a warning, so remove it.
- An organization's start link (`/platform/v1/auth/oidc/start?org=…`) goes straight to its identity provider only while **Start without a click** is on; otherwise it opens the email page and keeps `next`. The Sign-in link on an organization's Security page is now that start link, so share it again if you published the old address.

#### Platform API

- `auth/methods` no longer has `sso`, and names a `provider` only after an email or for a start link with Start without a click; `instance/settings` no longer has `sign_in.sso`. `POST auth/oidc/start` with `org` needs Start without a click or a signed-in member of the organization; otherwise send `email`.

## [0.3.5] - 2026-10-21

Kaeso 0.3.5 changes how the gateway limits request bodies and memory, and it tightens sign-in, invitation and project membership rules. Operators may need to raise `body_limits.model_api` for large uploads, and should review default project membership, since it no longer copies current members. Integrations that use `GET auth/oidc/start` or spend budgets should check the entries under Changed. Several security fixes are included, so upgrading is recommended.

**Full changes:** [v0.3.4...v0.3.5](https://github.com/logicplanes/kaeso/compare/v0.3.4...v0.3.5)

### Changed

#### Gateway

- The model API takes request bodies up to 8 MiB instead of 32 MiB, and the Playground up to 1 MiB; raise `body_limits.model_api` for large PDFs or many images. The new setting `body_limits.in_flight`, 512 MiB by default, bounds the memory requests hold while Kaeso translates them.

#### Platform API

- Making a project the default no longer adds the organization's current members to it; people who join from then on start there.
- `GET auth/oidc/start` hands the browser to the console's sign-in page, which goes on to the identity provider at once when nobody is signed in.
- `TotpRequired` carries `attempts` and `codes_locked`, and `auth/methods` names the provider's organization in `org_name`.

### Security

- A wrong second-factor code counts against its sign-in: after three the password or sign-in link is needed again, and after ten in a row authenticator codes stop working until a recovery code or a password reset by email, and the account's owner gets an email. ([GHSA-h2fm-vp9c-hh88](https://github.com/logicplanes/kaeso/security/advisories/GHSA-h2fm-vp9c-hh88))
- One model API request can no longer exhaust the gateway's memory: bodies are smaller, messages, content blocks, tools and embedding inputs are counted before they are decoded, and translation shares a memory budget. ([GHSA-vr6h-jrf4-8f5w](https://github.com/logicplanes/kaeso/security/advisories/GHSA-vr6h-jrf4-8f5w))
- An SSO start link no longer signs a browser in on its own: when someone is signed in, the sign-in page names the organization and waits for a click, and an SSO answer for another person never replaces or ends the browser's session. ([GHSA-r9x9-pm2q-m3vr](https://github.com/logicplanes/kaeso/security/advisories/GHSA-r9x9-pm2q-m3vr))
- Holding Update organization alone no longer adds anyone to a project by making it the default. ([GHSA-xjcx-5v89-jfx4](https://github.com/logicplanes/kaeso/security/advisories/GHSA-xjcx-5v89-jfx4))
- SSO sessions date from when the person signed in at the identity provider, so a reused provider session no longer counts as a recent sign-in, and confirmations need the provider's `auth_time`. ([GHSA-w8jw-655f-5vvg](https://github.com/logicplanes/kaeso/security/advisories/GHSA-w8jw-655f-5vvg))
- Under a spend budget, Kaeso refuses provider-side tools, MCP servers, search options and service tiers it can't price, instead of letting them spend past the budget. ([GHSA-5h99-c7qh-j7ww](https://github.com/logicplanes/kaeso/security/advisories/GHSA-5h99-c7qh-j7ww))
- Project admins can't change or remove members whose organization role gives them rights the admin lacks, and only organization owners change owners in projects. ([GHSA-hv9c-x3hp-gj93](https://github.com/logicplanes/kaeso/security/advisories/GHSA-hv9c-x3hp-gj93))
- Accepting a second invitation no longer restores project roles taken away after the first, and accepting one revokes the address's other pending invitations. ([GHSA-22ff-ww33-472c](https://github.com/logicplanes/kaeso/security/advisories/GHSA-22ff-ww33-472c))
- Invitations and the SSO default role can't give people rights in the default project that the inviter can't grant there. ([GHSA-h7qj-hxgg-pqcv](https://github.com/logicplanes/kaeso/security/advisories/GHSA-h7qj-hxgg-pqcv))
- A release feed can hold back update checks for at most an hour and no longer stops the update job, and Check now works while it waits. ([GHSA-fqpg-49fp-rpww](https://github.com/logicplanes/kaeso/security/advisories/GHSA-fqpg-49fp-rpww))
- Provider error bodies reach apps with key fragments and account ids redacted, in every format and in streams. ([GHSA-gh4q-232c-7r4p](https://github.com/logicplanes/kaeso/security/advisories/GHSA-gh4q-232c-7r4p))
- `GET me/keys` lists only the keys of organizations the session may open. ([GHSA-xq25-6hwj-5xgv](https://github.com/logicplanes/kaeso/security/advisories/GHSA-xq25-6hwj-5xgv))
- Release tooling refuses outside links and commands in release notes and shows them to the maintainer before the tag. ([GHSA-6xrm-g82c-5h3p](https://github.com/logicplanes/kaeso/security/advisories/GHSA-6xrm-g82c-5h3p))

## [0.3.4] - 2026-10-21

Kaeso 0.3.4 changes only the Console. The Organization Security page now shows whether single sign-on admits only addresses the identity provider marks verified, and sign-in prompts for single sign-on sessions no longer offer a password or email link the account may lack. No configuration or code changes are needed when upgrading.

**Full changes:** [v0.3.3...v0.3.4](https://github.com/logicplanes/kaeso/compare/v0.3.3...v0.3.4)

### Changed

#### Console

- The Organization Security page shows whether single sign-on admits only addresses the identity provider marks verified, without opening Edit.
- Pages that ask a single sign-on session to sign in another way, such as for administration or for another organization, no longer promise a password or email link the account may not have. The instance page's tab no longer repeats the word Instance.

## [0.3.3] - 2026-10-21

The notes for version 0.3.3 list a single change, to the release artwork on the changelog, which now fills its frame in the 16:9 shape it was drawn in. Nothing in these notes asks people who run or integrate Kaeso to change configuration or code when they upgrade. The release is a documentation fix.

**Full changes:** [v0.3.2...v0.3.3](https://github.com/logicplanes/kaeso/compare/v0.3.2...v0.3.3)

### Fixed

#### Documentation

- Each release's artwork on the changelog fills its frame in the shape it was drawn in, 16:9, instead of sitting between empty bars.

## [0.3.2] - 2026-10-21

### Added

#### Platform API

- `GET me/sign-in-again` lists how the signed-in account can confirm a recent sign-in, narrowed to an organization's own single sign-on when `org` requires it, and whether it can set a password by email first. `POST me/sign-in-again/sso` starts a confirmation at a linked organization's identity provider.

### Fixed

#### Console

- Confirming a recent sign-in through single sign-on no longer loops. The console opens the identity provider in a window of its own, the page and what you typed stay, and the action repeats by itself once you've signed in there; a refusal says why instead of asking again.
- **Add models** says when a model's values can't load instead of staying silent.
- An account without a password, such as one that signs in through single sign-on, gets a link by email to set one: from **Email me a link** under **Account** › **Security**, and in the dialog when an action needs another sign-in and nothing else works. Both used to end at a step it couldn't take.
- A sign-in ends the session its browser had, so signing in again to confirm an action no longer leaves a session behind each time; **Sessions** listed dozens for one browser.

#### Self-hosting

- Valkey starts again on an install that already has data. Since 0.3.0 its container started as root without capabilities, so it couldn't enter its own `appendonlydir` and exited, which rolled back an upgrade from 0.2. It now runs as the image's own user and needs no capabilities.
- The updater verifies releases again. It asked GitHub for a release's attestations at an address that read `sha256:…` as a URL of its own, so every update from the console or in a window was refused with "cannot be fetched (request failed)". Installs that use the updater update to this release by hand once; from then on it updates them itself.

### Security

- A confirmation through single sign-on only confirms the signed-in account. It asks the identity provider for a fresh sign-in (`prompt=login`, `max_age=0`), refuses another person, an identity that isn't linked and an older sign-in, and replaces the session; it used to start a new sign-in that could switch accounts.

## [0.3.1] - 2026-10-21

Kaeso 0.3.1 is a small follow-up to 0.3.0 with no breaking changes. The gateway now checks for new releases every 5 minutes, and Settings shows the last check only when it failed. The documentation changelog is split into one page per minor series, and the UI library's ReleaseArt takes an image source for stored release artwork.

**Full changes:** [v0.3.0...v0.3.1](https://github.com/logicplanes/kaeso/compare/v0.3.0...v0.3.1)

### Added

- Each release on the documentation's changelog shows its artwork, the same picture as the console's What's new dialog. The documentation's Content Security Policy allows images from the update server for it.

#### UI library

- `ReleaseArt` takes a `src` prop, an image of a release's stored artwork, shown in the same frame as the drawn picture, which stays as the fallback if the image doesn't load.

### Changed

- The gateway checks for new releases every 5 minutes instead of twice a day, and 15 minutes after a failed check. An unchanged release list costs only a `304` answer.

#### Console

- `Settings → Instance → Updates` no longer shows when the gateway last looked for releases; it says only when a check failed.

### Fixed

#### Documentation

- The changelog is split into one page per minor series, so no page outgrows the site's page budget. Older series sit on their own pages, and links to a version, including old version anchors, open the page that holds it. The 0.3.0 documentation and showcase could not be deployed until this was fixed.

## [0.3.0] - 2026-10-21

Kaeso 0.3.0 adds six built-in providers, so DeepSeek, SpaceX (xAI's API), Together AI, Groq, OpenRouter and Mistral can be added with a key and called from any app format. It tightens access: only owners transfer ownership, single sign-on admits only linked accounts and verified email domains, and forwarded addresses are trusted only from loopback by default. Five changes need action when you upgrade, listed under breaking, and most of the rest make requests, streams, budgets and the console faster or more accurate.

- **Breaking:** Only owners can transfer ownership: custom roles and individual grants lose the right, and the content logging right is renamed `logs.content.manage`, so grant it again where people switch content logging. ([#229](https://github.com/logicplanes/kaeso/pull/229))
- **Breaking:** The gateway trusts X-Forwarded-For only from loopback by default. Set `KAESO_TRUSTED_PROXIES` when a reverse proxy sits outside the Compose package, or every client shares the proxy's address in logs and throttles. ([#247](https://github.com/logicplanes/kaeso/pull/247))
- **Breaking:** Single sign-on signs in only accounts linked to the organization's identity provider. Existing members whose accounts predate their organization's single sign-on must link them once under Account, Security. ([#252](https://github.com/logicplanes/kaeso/pull/252))
- **Breaking:** Single sign-on admits only email domains the organization has verified with a DNS TXT record. Publish the record and choose Verify domains under Security before relying on single sign-on after upgrading. ([#264](https://github.com/logicplanes/kaeso/pull/264))
- **Breaking:** Single sign-on no longer trusts private-network identity providers by default, so set `KAESO_AUTH_OIDC_ALLOW_PRIVATE_NETWORKS` to true for one. Turn off required verified addresses only for providers that never send the claim. ([#270](https://github.com/logicplanes/kaeso/pull/270))

**Contributors:** @dvnold

**Full changes:** [v0.2.2...v0.3.0](https://github.com/logicplanes/kaeso/compare/v0.2.2...v0.3.0)

### Added

- Count requests for a model whose provider has no token-count endpoint are refused in the app's own format with a 400 that names the model, instead of reaching the provider. ([#197](https://github.com/logicplanes/kaeso/pull/197))
- DeepSeek, SpaceX (xAI's API, with its Grok models), Together AI, Groq, OpenRouter and Mistral are built-in providers. An organization adds a key on Providers and calls their models from any app format, under the usual keys, limits and logs. ([#200](https://github.com/logicplanes/kaeso/pull/200), [#202](https://github.com/logicplanes/kaeso/pull/202), [#203](https://github.com/logicplanes/kaeso/pull/203), [#204](https://github.com/logicplanes/kaeso/pull/204), [#209](https://github.com/logicplanes/kaeso/pull/209), [#213](https://github.com/logicplanes/kaeso/pull/213))
- Providers that speak OpenAI Chat Completions can declare the fields they rename or refuse, such as `max_completion_tokens`, the `developer` role or `safety_identifier`, so Anthropic Messages and Responses apps no longer get a 422 from them. ([#212](https://github.com/logicplanes/kaeso/pull/212))
- `cargo xtask record` takes any provider in the registry with `--provider` and records its replies into a folder of its own, so new providers can be checked against live replies. ([#198](https://github.com/logicplanes/kaeso/pull/198))
- NVIDIA NIM is not a built-in provider in this release: its support was added and then withdrawn until its calls can be checked against the live API. ([#206](https://github.com/logicplanes/kaeso/pull/206), [#210](https://github.com/logicplanes/kaeso/pull/210))
- Request bodies that stall end after `server.body_read_timeout`, apps that stop reading a stream release the provider call, and the optional `streams.max_duration` and `streams.max_bytes` settings end long replies in the app's format. ([#266](https://github.com/logicplanes/kaeso/pull/266))
- `GET orgs/{org}/providers` lists every provider with its name, key page, key prefix, formats and token counting, so clients read provider facts from the gateway instead of keeping their own table. ([#281](https://github.com/logicplanes/kaeso/pull/281))

#### UI library

- The console draws a provider's own mark wherever it names the provider, and the providers added in this release have marks of their own. ([#216](https://github.com/logicplanes/kaeso/pull/216))
- The UI library adds CommandList, a searchable list that stays fast with thousands of rows, and ProviderPicker and ModelPicker are built on it. ([#257](https://github.com/logicplanes/kaeso/pull/257))

#### Documentation

- The documentation explains how providers work in one concept page, and adds a supported providers page that lists each provider's formats, host and where its keys come from. ([#217](https://github.com/logicplanes/kaeso/pull/217))

### Changed

- The console no longer reloads every loaded page of a long list when the window regains focus, and a change under an organization no longer reloads every list inside it. ([#221](https://github.com/logicplanes/kaeso/pull/221))
- Provider logos follow each provider's brand guidelines in one colour per theme, and Anthropic and Groq show a letter where their terms don't allow the logo in an interface. ([#223](https://github.com/logicplanes/kaeso/pull/223))
- A request for a model the provider no longer serves is refused locally with the same 404, and the provider is asked again only once the last answer is five minutes old. ([#228](https://github.com/logicplanes/kaeso/pull/228))
- The console asks the gateway for less: one list per resource, no lookups one at a time, and a live request log that keeps up with bursts of more than 100 requests. ([#233](https://github.com/logicplanes/kaeso/pull/233))
- The gateway keeps serving from its caches while it cannot hear Valkey invalidations, and concurrent misses share one load instead of each querying PostgreSQL. ([#235](https://github.com/logicplanes/kaeso/pull/235))
- Large and long requests use less memory and CPU, because the gateway shares one copy of each request and translates large bodies off its async worker threads. ([#236](https://github.com/logicplanes/kaeso/pull/236))
- Every console page lines up on one edge, and boxes start and end on the gutter with their text 12 pixels further in, on phones as well as desktops. ([#237](https://github.com/logicplanes/kaeso/pull/237))
- Limit checks no longer wait behind large cache reads and writes on Valkey, because bulk transfers use their own connections. ([#242](https://github.com/logicplanes/kaeso/pull/242))
- When a provider refuses the project's provider key, apps get a 502 provider error without the key details, and the provider's reason is kept, redacted, in the request log. ([#244](https://github.com/logicplanes/kaeso/pull/244))
- The gateway trusts X-Forwarded-For only from loopback by default, so set `KAESO_TRUSTED_PROXIES` for a reverse proxy outside the Compose package. ([#247](https://github.com/logicplanes/kaeso/pull/247))
- Every model and provider choice in the console is searchable and grouped, and the Providers page lists only the providers the organization has keys for. ([#258](https://github.com/logicplanes/kaeso/pull/258))
- The key dialog links each provider's API key page, and Add models warns when a provider has no key yet or when a bulk add would be large. ([#259](https://github.com/logicplanes/kaeso/pull/259))
- The key dialog opens SpaceX's API keys page directly, and it links Mistral's key page with a hint that Mistral keys have no fixed prefix. ([#275](https://github.com/logicplanes/kaeso/pull/275))
- Every built-in provider shows its own logo in the console, including Anthropic and Groq, and SpaceX's mark stays readable at small sizes. ([#279](https://github.com/logicplanes/kaeso/pull/279))
- The console reads every provider's name, key page, key prefix and formats from the gateway's list, so providers the gateway adds appear without a console update. ([#290](https://github.com/logicplanes/kaeso/pull/290))

#### Gateway

- Streams to one provider spread over several HTTP/2 connections, so hundreds of concurrent requests no longer queue behind one connection's stream limit. ([#222](https://github.com/logicplanes/kaeso/pull/222))
- Model requests find their catalogue entry and each provider's record in constant time, however many models and providers are configured. ([#224](https://github.com/logicplanes/kaeso/pull/224), [#230](https://github.com/logicplanes/kaeso/pull/230))
- The models.dev sync check reads only its timestamps and content hash on each run, instead of loading the whole stored snapshot every five minutes. ([#225](https://github.com/logicplanes/kaeso/pull/225))
- A burst of requests that finds a lost budget counter rebuilds it from PostgreSQL once instead of once per request. ([#251](https://github.com/logicplanes/kaeso/pull/251))
- A fresh install starts with the models and prices that models.dev listed at the release, 561 models across the providers Kaeso calls. ([#316](https://github.com/logicplanes/kaeso/pull/316))

#### KPP

- Long tool loops from Chat Completions apps on Claude or the Responses API restore kept reasoning in one pass per request, so requests late in a conversation translate faster. ([#227](https://github.com/logicplanes/kaeso/pull/227))

#### Console

- The console's tests cover every branch again, so the coverage gate passes on the release branch. ([#310](https://github.com/logicplanes/kaeso/pull/310))

#### Documentation

- The Release workflow can be re-run after a partial release, and it checks its release commit against an independent run of the version script before pushing only the tag. ([#220](https://github.com/logicplanes/kaeso/pull/220))
- The published crates `kaeso-kpp` and `kaeso-codecs` are packaged, documented and checked for API breaks on every change, and the KPP decoders are fuzzed nightly. ([#231](https://github.com/logicplanes/kaeso/pull/231))
- Code may carry a comment only where a careful reader would otherwise get it wrong, such as a workaround or a security invariant, and CI rejects leftover task notes in comments. ([#254](https://github.com/logicplanes/kaeso/pull/254))

#### Self-hosting

- Release images ship a gateway binary built with link-time optimisation, which is 28 percent smaller; local builds and CI keep the faster release profile. ([#273](https://github.com/logicplanes/kaeso/pull/273))

### Fixed

- Anthropic-format streams no longer fail when an OpenAI-compatible provider sends running usage on every chunk, as vLLM-based providers do. ([#219](https://github.com/logicplanes/kaeso/pull/219))
- Hard budgets and token rates reserve a worst case for each concurrent request, so a burst of expensive requests cannot overrun them, and a long stream keeps its reservation. ([#232](https://github.com/logicplanes/kaeso/pull/232))
- Replies and streams that finish without usage are charged an estimate instead of nothing, and the request log marks the usage as estimated. ([#238](https://github.com/logicplanes/kaeso/pull/238))
- Tool calls and errors from OpenAI-compatible providers are no longer dropped, merged or taken for a finished reply, and non-object tool arguments are refused for Anthropic apps instead of changed. ([#246](https://github.com/logicplanes/kaeso/pull/246))
- Project pages say when their keys, members, service accounts or limits could not load, instead of showing an empty list. ([#248](https://github.com/logicplanes/kaeso/pull/248))
- Writes to Anthropic's one-hour prompt cache are charged at their own price, twice the input price, instead of the five-minute price. ([#249](https://github.com/logicplanes/kaeso/pull/249))
- A revoked API key or ended session stops working at once across instances, and shared cache entries are signed with a key derived from the master key, so planted entries are ignored. ([#250](https://github.com/logicplanes/kaeso/pull/250))
- A stream error from a provider stops the provider request, so it no longer keeps generating and charging after the error is reported. ([#253](https://github.com/logicplanes/kaeso/pull/253))
- Replies from OpenAI-compatible providers with unusual usage, ids or finish reasons reach every app intact, and a stream that ends before its finish is reported as an error. ([#256](https://github.com/logicplanes/kaeso/pull/256))
- Release links are https only, request logs no longer load remote images, and the docs and showcase send strict transport security, nosniff and a referrer policy. ([#263](https://github.com/logicplanes/kaeso/pull/263))
- Responses streams whose text or tool arguments arrive only in their final events reach apps in other formats instead of an empty reply. ([#267](https://github.com/logicplanes/kaeso/pull/267))
- Server-sent events whose first field name starts with a byte order mark are written so that they read back the same. ([#268](https://github.com/logicplanes/kaeso/pull/268))
- Shutdown closes connections still open after the grace period, startup retries an unreachable PostgreSQL, and log and usage writes no longer lose rows. ([#269](https://github.com/logicplanes/kaeso/pull/269))
- Three earlier changes that later merges had undone are back in place: Chat reply translation, key safeguards and signed cache entries. ([#276](https://github.com/logicplanes/kaeso/pull/276))
- DeepSeek stops at the output limit apps in every format send, because its chat profile now uses `max_tokens`, and budgets hold enough for SpaceX reasoning. ([#278](https://github.com/logicplanes/kaeso/pull/278))
- Reasoning from DeepSeek, Groq, Together AI, OpenRouter and SpaceX reaches Anthropic and Responses apps and the Playground, and the request log stores it as reasoning rather than raw blocks. ([#283](https://github.com/logicplanes/kaeso/pull/283))
- The Playground refuses a model the provider does not serve with the same 404 and message as the model API, and the request log shows that message from the first failure. ([#288](https://github.com/logicplanes/kaeso/pull/288))
- Finished Playground replies no longer show up as 499 errors, and a failed or cut-off reply says why in its column. ([#289](https://github.com/logicplanes/kaeso/pull/289))
- Certificate checks no longer trust the distrusted emSign roots and intermediates, as the advisories against the certificate stack require. ([#293](https://github.com/logicplanes/kaeso/pull/293))
- OpenRouter errors carry the upstream provider's reason, such as a shared rate limit, to Anthropic and Responses apps and the request log, instead of only Provider returned error. ([#294](https://github.com/logicplanes/kaeso/pull/294))
- Request log messages appear in the person's language, with backtick spans set as code, and the request sheet states each outcome once. ([#295](https://github.com/logicplanes/kaeso/pull/295))
- Account pages explain why an organization's only owner cannot leave, order sessions by last use with readable client names, and describe two-factor authentication the same way everywhere. ([#297](https://github.com/logicplanes/kaeso/pull/297))
- Numbers, dates, context sizes and prices read the same way across the console, in English, German and Chinese. ([#298](https://github.com/logicplanes/kaeso/pull/298))
- Members and projects can be searched, archived items have their own tab, and inviting people keeps the address that was typed. ([#299](https://github.com/logicplanes/kaeso/pull/299))
- The model picker filters by provider at any number of providers, opens with an empty search, and greys out models the provider does not serve. ([#300](https://github.com/logicplanes/kaeso/pull/300))
- Single sign-on shows the sign-in callback to register before a connection exists, each domain's verification state, and why it cannot be required yet. ([#302](https://github.com/logicplanes/kaeso/pull/302))
- Audit logs name every action and target in words, show the time zone, and keep the target column readable at every width. ([#303](https://github.com/logicplanes/kaeso/pull/303))
- The Playground keeps its conversation while the tab is open, Clear can be undone, and replies show tokens and costs in the same form as the request logs. ([#304](https://github.com/logicplanes/kaeso/pull/304))
- Add models shows prices, how many models match, and exactly how many Add every current would add, and the Models page shows each provider's logo once per group. ([#305](https://github.com/logicplanes/kaeso/pull/305))
- Instance admins see each user's organizations and can reopen What's new, and the instance pages show dates, badges and update states that make sense. ([#309](https://github.com/logicplanes/kaeso/pull/309))
- Enter confirms text in an input method without submitting a dialog, and Enter in a search field inside a dialog keeps filtering. ([#311](https://github.com/logicplanes/kaeso/pull/311))
- The Providers page keeps a search's result when a refresh fails and explains why the first key cannot be added, and folded values are labelled for screen readers. ([#312](https://github.com/logicplanes/kaeso/pull/312))
- Inviting people stops at an invalid address still being typed, splits a pasted row of addresses into one tag each, and keeps focus after a role change. ([#313](https://github.com/logicplanes/kaeso/pull/313))
- Step dialogs keep what was typed when someone clicks outside them, and confirmations opened over a sheet dim the page behind them. ([#315](https://github.com/logicplanes/kaeso/pull/315))
- Copyable values show in full, Enter submits dialogs, field edges reach 3 to 1 contrast, and sheets keep their content while they close. ([#280](https://github.com/logicplanes/kaeso/pull/280))
- Provider keys name their provider in dialogs and menus, the providers list keeps every detail on narrow screens, and large organizations can search it. ([#282](https://github.com/logicplanes/kaeso/pull/282))
- Usage charts name each UTC day by its own date in every time zone and mark the period that is still running. ([#284](https://github.com/logicplanes/kaeso/pull/284))
- The request log keeps the model and provider readable at every width, and its model filter follows the chosen provider. ([#285](https://github.com/logicplanes/kaeso/pull/285))
- Sign-in shows a wrong password under the field, opens the app when you are already signed in, and carries your address into a password reset. ([#286](https://github.com/logicplanes/kaeso/pull/286))

#### Console

- Console pages line up on the gutter and show their shape while they load, and request logs fit a phone with the Status header shown whole. ([#239](https://github.com/logicplanes/kaeso/pull/239), [#240](https://github.com/logicplanes/kaeso/pull/240))
- The sign-in page loads about 2 kB less JavaScript before its first paint, which keeps it inside its size budget. ([#272](https://github.com/logicplanes/kaeso/pull/272))
- The playground's controls are translated in the console's development server, as they already are in the production build. ([#287](https://github.com/logicplanes/kaeso/pull/287))
- The command palette searches the organization's projects, members and keys on its pages, and recent requests show their project and time to tell them apart. ([#306](https://github.com/logicplanes/kaeso/pull/306))
- Organization pages show each provider's own name, order and formats from the first frame instead of briefly showing raw ids. ([#314](https://github.com/logicplanes/kaeso/pull/314))

#### UI library

- Tables, lists, date presets and form errors work with a keyboard and a screen reader, and selected rows and menu hints meet contrast requirements. ([#261](https://github.com/logicplanes/kaeso/pull/261))
- Charts that show a loading state first draw once their data arrives, so the overview's Requests chart no longer stays empty. ([#277](https://github.com/logicplanes/kaeso/pull/277))
- Checkbox and radio edges reach 3 to 1 contrast in light and dark mode, the same as field edges. ([#307](https://github.com/logicplanes/kaeso/pull/307))
- Closing a sheet whose row has gone no longer takes the focus from a dialog opened meanwhile. ([#318](https://github.com/logicplanes/kaeso/pull/318))

#### Documentation

- The requirements page lists every host an install connects to, and the quick start creates the master key with the release's gateway image. ([#243](https://github.com/logicplanes/kaeso/pull/243))

### Security

- Each release's compose file pins the Kaeso images by digest, and manual upgrades can verify them with `gh attestation verify`. ([#218](https://github.com/logicplanes/kaeso/pull/218))
- The updater refuses to create a container with more rights than the one it replaces, and it pulls only Kaeso images by digest. ([#226](https://github.com/logicplanes/kaeso/pull/226))
- Only owners can transfer ownership, custom roles and grants can no longer include that right, and the content logging right is renamed `logs.content.manage`. ([#229](https://github.com/logicplanes/kaeso/pull/229))
- Caddy and the updater run unprivileged, the databases drop their Linux capabilities, and every service has memory, process and log limits. ([#234](https://github.com/logicplanes/kaeso/pull/234))
- Single sign-on signs in only accounts linked to the organization's identity provider, and existing members link their account once under Account, Security. ([#252](https://github.com/logicplanes/kaeso/pull/252))
- Sign-in links ask for the second factor, account changes need a code, and email changes can be cancelled from the old address. ([#255](https://github.com/logicplanes/kaeso/pull/255))
- Only a person re-enables a key that a person disabled, keys cannot create platform-access keys, and instance admin changes to people need a recent sign-in. ([#262](https://github.com/logicplanes/kaeso/pull/262))
- Single sign-on admits only addresses at email domains the organization has verified with a DNS TXT record. ([#264](https://github.com/logicplanes/kaeso/pull/264))
- Single sign-on refuses link-local and cloud metadata addresses, issuers with queries, reuse of a stored secret for a changed issuer, and unverified email addresses, and private networks are off by default. ([#270](https://github.com/logicplanes/kaeso/pull/270))
- The sign-in page shows its own message for an SSO failure instead of text taken from the address, so a crafted link cannot show arbitrary text. ([#271](https://github.com/logicplanes/kaeso/pull/271))
- Advisories against the certificate stack are resolved, so the gateway rejects certificates chained through distrusted emSign intermediates. ([#292](https://github.com/logicplanes/kaeso/pull/292))

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

[Unreleased]: https://github.com/logicplanes/kaeso/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/logicplanes/kaeso/releases/tag/v0.4.0
[0.3.9]: https://github.com/logicplanes/kaeso/releases/tag/v0.3.9
[0.3.8]: https://github.com/logicplanes/kaeso/releases/tag/v0.3.8
[0.3.7]: https://github.com/logicplanes/kaeso/releases/tag/v0.3.7
[0.3.6]: https://github.com/logicplanes/kaeso/releases/tag/v0.3.6
[0.3.5]: https://github.com/logicplanes/kaeso/releases/tag/v0.3.5
[0.3.4]: https://github.com/logicplanes/kaeso/releases/tag/v0.3.4
[0.3.3]: https://github.com/logicplanes/kaeso/releases/tag/v0.3.3
[0.3.2]: https://github.com/logicplanes/kaeso/releases/tag/v0.3.2
[0.3.1]: https://github.com/logicplanes/kaeso/releases/tag/v0.3.1
[0.3.0]: https://github.com/logicplanes/kaeso/releases/tag/v0.3.0
[0.2.2]: https://github.com/logicplanes/kaeso/releases/tag/v0.2.2
[0.2.1]: https://github.com/logicplanes/kaeso/releases/tag/v0.2.1
[0.2.0]: https://github.com/logicplanes/kaeso/releases/tag/v0.2.0
[0.1.0]: https://github.com/logicplanes/kaeso/releases/tag/v0.1.0
