# Changelog

All notable changes to Kaeso are documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.5-rc.2] - 2026-11-26

Kaeso 0.7.5-rc.2 fixes a gateway CPU cost increase measured in 0.7.4. A reply waits for its sources note again, which brings CPU per request at capacity back to 0.7.3's level in the benchmark. At one client, the median latency is about 50 µs higher than 0.7.4's. No change is marked as breaking.

**Contributors:** @dvnold

**Full changes:** [v0.7.5-rc.1...v0.7.5-rc.2](https://github.com/logicplanes/kaeso/compare/v0.7.5-rc.1...v0.7.5-rc.2)

### Fixed

#### Gateway

- A reply waits for its sources note again, which reverts the earlier change that removed this wait. In the benchmark at capacity (64 closed-loop clients, gateway on 2 pinned cores), CPU per request is 210 µs, against 229–236 µs in 0.7.4 and 211–214 µs in 0.7.3. At one client the median is about 50 µs higher than 0.7.4's, which is 0.7.3's latency again. ([#1521](https://github.com/logicplanes/kaeso/pull/1521))

## [0.7.5-rc.1] - 2026-11-26

Kaeso 0.7.5-rc.1 adds a new release artwork style, `glow`, and the gateway accepts it in the feed's `art`. Stored specs of earlier releases still draw their flat style as before. No change is marked as breaking.

**Contributors:** @dvnold

**Full changes:** [v0.7.4...v0.7.5-rc.1](https://github.com/logicplanes/kaeso/compare/v0.7.4...v0.7.5-rc.1)

### Added

- Release artwork has a new `glow` style, the look of the release articles' covers: the release's name in white on a near-black ground, lit by one bloom of light. `nextReleaseArt` always picks a glow in a light the last four releases didn't have, and `releaseArtFor`, the console's fallback when a release has no stored artwork, draws a glow. A style the UI library doesn't know draws as a glow instead of throwing, and stored specs of earlier releases still draw their flat style as before. The gateway accepts `glow` in the feed's `art` and keeps it; a gateway before this one keeps no artwork for a glow release and draws one from the version until it is updated, when it takes the feed's. ([#1519](https://github.com/logicplanes/kaeso/pull/1519))

## [0.7.4] - 2026-11-25

Kaeso 0.7.4 closes a CPU exhaustion problem in the merge of a tool schema's root union: each definition is now expanded once, and a merge stops after 4096 schemas. It also removes a wait that every reply had in organizations without a source since the previous release. The performance page now shows the 0.7.3 benchmark run. No change is marked as breaking.

**Contributors:** @dvnold

**Full changes:** [v0.7.3...v0.7.4](https://github.com/logicplanes/kaeso/compare/v0.7.3...v0.7.4)

### Changed

#### Documentation

- The performance page shows the benchmark run of Kaeso 0.7.3 in English, German and Simplified Chinese. The section on changes since 0.7.0 is replaced by one on changes since 0.7.2, and the page says how the run was made and how to run it yourself. ([#1516](https://github.com/logicplanes/kaeso/pull/1516))

### Fixed

#### Gateway

- A reply no longer waits for a background task when its organization has no source. Since the previous release every reply waited for that task, which only read a cached `false` in such organizations; a request of an organization known to have no source, or a successful one without usage, now starts no task. Organizations with sources, or not known yet, behave as before. ([#1517](https://github.com/logicplanes/kaeso/pull/1517))

### Security

#### KPP

- Merging a tool schema's root union into one object expands each definition a `$ref` reaches once, where it was expanded again at every reference and took exponential time for a valid API key's schema before any provider call. A merge may now expand at most 4096 schemas, counting every reference it follows. Past that, the tool is refused as `Unsupported` in the app's format, before anything is sent. The cross-format page lists the new refusal. ([#1515](https://github.com/logicplanes/kaeso/pull/1515))

## [0.7.3] - 2026-11-25

Kaeso 0.7.3 sends an Anthropic app's request to a provider's own Messages API without translating it, when the provider lists it for the model and the model's catalogue entry names no egress dialect. It also ends a ChatGPT plan's reply at the app's output limit and holds an account at its plan's usage limit for longer between retries. When Kaeso can't check a project's hard budgets or load its limits, it refuses the request as Kaeso being busy, and a request sent through the gateway and also reported to a source counts once across an organization's projects. No change is marked as breaking.

**Full changes:** [v0.7.2...v0.7.3](https://github.com/logicplanes/kaeso/compare/v0.7.2...v0.7.3)

### Changed

- An Anthropic app's request goes as it came to a provider's own Messages API instead of being translated into Chat Completions and back, when the provider lists it for the model and the model's catalogue entry names no egress dialect. This covers DeepSeek, Moonshot (K3 models), QwenCloud (Qwen models), Baidu Qianfan, DeepInfra, OpenRouter, Hugging Face, Ollama Cloud, Tencent TokenHub, Fireworks AI, SambaNova and Vercel. KPMR's or the organization's Sends as setting always wins, so sending a model as Chat Completions keeps the translation; the Anthropic stream decoder also reads DeepSeek's timeout frame as DeepSeek's error with its message, where the stream failed with "ended before `message_stop`" before. ([#1506](https://github.com/logicplanes/kaeso/pull/1506))
- A request after a pause no longer waits for its project's settings to load one after another: every entry that ran out, such as limits, log settings or the response cache, now loads in parallel with the others. Entries stay warm for an hour after their last use instead of 15 minutes. `stream_processing` now adds up only the decoding, translating and encoding of each batch, no longer the time spent waiting. The warm path is unchanged. ([#1511](https://github.com/logicplanes/kaeso/pull/1511))

#### Documentation

- The performance page shows the benchmark run of Kaeso 0.7.2 and a new section on what changed since 0.7.0, in English, German and Simplified Chinese. Peak memory at full load with request logs on fell from 450 to 212 MB, which fits 0.7.2 bounding the rows that wait for ClickHouse by `logs.buffer_size`. The median at 16 clients and the databases' peak memory rose, and the page says the runs can't tell whether 0.7.2 or the machine caused that. ([#1508](https://github.com/logicplanes/kaeso/pull/1508))

### Fixed

- A ChatGPT plan's reply now stops at the app's output limit. The plan's Responses route refuses `max_output_tokens`, so Kaeso remembers the app's limit and ends the reply itself with the stop reason of the app's format: `max_tokens` for Anthropic, `length` for Chat Completions, `incomplete` with `max_output_tokens` for Responses. Usage is logged as an estimate. Tool call arguments from another format now stream to Anthropic apps as they arrive instead of as one `input_json_delta` when the block ends; only leading blank text waits, and the arguments the app accumulates are unchanged. ([#1505](https://github.com/logicplanes/kaeso/pull/1505))
- An account at its plan's usage limit, such as a ChatGPT plan, is held longer with each limit in a row instead of being tried again every 5 minutes. Without a stated wait the hold is 5, 15 and 45 minutes, then 2 hours at most. A stated `Retry-After` holds the account that long, at least 5 minutes and now up to a week (it was capped at an hour). Any answer from the account starts the backoff over at 5 minutes. ([#1507](https://github.com/logicplanes/kaeso/pull/1507))
- When Kaeso can't check a project's hard budgets because its limit counters are unreachable, or can't load the project's limits, it refuses the request before any provider is called as Kaeso being busy: 503 for OpenAI formats and Gemini, 529 for Anthropic apps, with `Retry-After: 5`. The request log shows `overloaded` instead of `provider_error` against a provider that was never called, and OpenAI Chat Completions and Responses apps get 503 instead of 500. An alias route still stops at this refusal. ([#1509](https://github.com/logicplanes/kaeso/pull/1509))
- A request that a client sends through the gateway and also reports to a source (Claude Code, Codex, Gemini CLI, Antigravity or LiteLLM, for example) now counts once across an organization's projects, where matching was per project. The gateway notes its requests before the reply ends, and failed gateway requests are noted too. Models named with an effort level log as one model, and the request log keeps each request's reasoning effort. A provider a client only derives from its own setup is priced like the source kind's usual provider first, and the model's maker when that provider doesn't sell the model. ([#1510](https://github.com/logicplanes/kaeso/pull/1510))
- QwenCloud's Messages API errors now reach apps with QwenCloud's message and kind. The Messages error decoder reads DashScope's error body by its code, as it does for Chat Completions, and takes the data of an error body sent as an SSE `event:error` frame with a non-2xx status. A refusal for a spent free quota is now a billing refusal with QwenCloud's message; before, apps got `permission_denied` and a message that the provider returned an HTTP 403 with a body that is not an Anthropic error. ([#1514](https://github.com/logicplanes/kaeso/pull/1514))

#### KPP

- The validation error for a tool call or result in the wrong message now names that message's role, such as "message 2 has the role `assistant`", instead of "message 2 is a assistant message". ([#1512](https://github.com/logicplanes/kaeso/pull/1512))

## [0.7.2] - 2026-11-25

Kaeso 0.7.2 is mostly about console speed, lower gateway memory and a long list of fixes. Console pages prefetch their data, large tables and the model picker do less work, and the Platform API gzip-compresses text answers to reads under set conditions. Open streams and the request log buffer hold less memory, and several security fixes tighten guardrails, limits, sign-in and the updater's release check. No change is marked as breaking.

**Contributors:** @dvnold

**Full changes:** [v0.7.1...v0.7.2](https://github.com/logicplanes/kaeso/compare/v0.7.1...v0.7.2)

### Changed

#### Gateway

- The Platform API gzip-compresses text answers (JSON, JSON lines and CSV downloads) to reads (`GET`, `HEAD`) from clients that send `accept-encoding: gzip`. A request with a cookie is compressed only when the browser marks it `same-origin`, as the console's own requests are; a request without a cookie, such as a platform key's, always is, and answers to changes never are. Server-sent events, media, files, ranges, bodies under 32 bytes and anything already encoded go out as they are, and the model API, MCP and the portal are unchanged. An install that serves the console from another origin than the gateway gets no compression for the console. ([#1470](https://github.com/logicplanes/kaeso/pull/1470))
- An open stream takes about a third less of the gateway's memory: the state of the work that ends a stream is now allocated only when the stream ends. In the pull request's measurements on a development machine, memory per open stream fell from about 151 KiB to about 105 KiB at 5,000 streams, and time to first byte and chunk gap stayed within the spread between runs. The same code runs in the same order. ([#1500](https://github.com/logicplanes/kaeso/pull/1500))

#### Console

- Console pages start loading their data when you point at, focus or touch a link to them, so most open without a skeleton. This covers project tools and settings, organization analytics, MCP servers, providers and projects, and the account, organization and instance settings pages, and the request log's loader now also covers tool calls. The prefetch checks your rights the way the page does and nothing is awaited, so navigation is never held up. `/settings/account` now opens Profile, as the other settings areas do. ([#1501](https://github.com/logicplanes/kaeso/pull/1501))
- Tables in request logs, tool calls and analytics mount, resize and scroll faster. Cell text no longer measures itself while drawing, and text that fits no longer mounts a tooltip. Cut text is still a tab stop and shows its full text on hover and keyboard focus, and text that fits gets no tooltip and no tab stop. ([#1471](https://github.com/logicplanes/kaeso/pull/1471))
- The project overview's Latest requests cell asks every five seconds for the 7 requests it shows instead of 100. In a measurement on a seeded install, one poll drops from 114.4 KB to 7.7 KB. Live in the request logs is unchanged. ([#1472](https://github.com/logicplanes/kaeso/pull/1472))

#### UI library

- `DataTable` now draws only the rows in view above 50 rows by default, down from 100, so request logs, tool calls and analytics breakdowns open faster. `virtualizeAbove` still overrides the default, and tables of up to 50 rows render as before. In tables of 51 to 100 rows, find-in-page and printing see only the rows drawn, and analytics breakdowns and tool calls keep their CSV export. ([#1468](https://github.com/logicplanes/kaeso/pull/1468))

### Fixed

- Two MCP servers of an organization can no longer expose a tool under the same name in the portal. A server handle that ends in a hyphen or holds two in a row now keeps its hyphens in the tool names, for example `a-__x` instead of `a___x`. Every other handle is unchanged, and only servers created through the API with such a handle see their tools renamed, since the console already refuses them. ([#1480](https://github.com/logicplanes/kaeso/pull/1480))
- A single sign-on group rule that gives a since-deleted custom role in a project no longer makes the sign-ins it matches fail. The rule gives nothing in that project and the sign-in goes on with everything else the rules give. Saving group rules still refuses a rule that names a deleted role. ([#1486](https://github.com/logicplanes/kaeso/pull/1486))
- An OpenAI Responses request or reply with a field name that is empty, contains a dot or starts with `header:`, `@` or `~` keeps that field and its other unknown fields, such as `store` and `prompt_cache_key`. Before, one such key removed itself and every other unknown field of its object. The key is still never sent as a header, and other dialects still refuse it as an extras field. ([#1493](https://github.com/logicplanes/kaeso/pull/1493))
- An OpenAI Chat Completions tool call whose `type` is neither `function` nor `custom` stays in `tool_calls`: a reply no longer loses it while `finish_reason` stays `tool_calls`, and a request no longer sends it inside the assistant message's `content`. The stream codec was not affected. ([#1497](https://github.com/logicplanes/kaeso/pull/1497))

#### Gateway

- An amount too large to convert between currencies, such as a spend budget near the largest decimal value in an organization that counts in euros, is refused with the limit's normal message instead of aborting the request. ([#1475](https://github.com/logicplanes/kaeso/pull/1475))
- The top-level object of a body sent to `POST /mcp` or `POST /mcp/{project}`, and of a message an upstream MCP server answers with, is read in time linear in its member count. A body within the default 1 MiB limit with about 100,000 distinct member names took about 8 seconds of CPU in a release build before. Duplicate member names are refused as before. ([#1477](https://github.com/logicplanes/kaeso/pull/1477))
- Guardrails find a card number followed by its security code, a card after a digit, and an IBAN followed by a word in capitals; before, such values were neither masked, blocked nor counted. Still not found, as before: an IBAN directly followed by a group of digits, a 13-digit card printed in groups next to other digits, and the second of two grouped cards in a row. ([#1479](https://github.com/logicplanes/kaeso/pull/1479))
- The `batches` and `webhooks` background jobs no longer share one lock, so a running `batches` job no longer makes every `webhooks` run (every 5 seconds) skip, and a `batches` run is no longer skipped when it meets a `webhooks` run. During a rolling upgrade, an instance on the old release and one on this release can run `batches` at the same moment; the pull request did not check whether that is safe. ([#1485](https://github.com/logicplanes/kaeso/pull/1485))
- A realtime session whose app or provider stops reading its socket now ends as idle after the idle timeout and is written to the request log and billed. Before, a blocked write could keep the session open, hold its limit reservation and leave it unlogged. Closing messages get five seconds. ([#1496](https://github.com/logicplanes/kaeso/pull/1496))
- Request log rows that wait for ClickHouse hold no more memory than `logs.buffer_size` allows. In the pull request's test, a full 1 MiB buffer held 3,831,344 bytes before and about 1.07 to 1.08 million bytes after. The buffer now drops its oldest rows at the memory `logs.buffer_size` names instead of far beyond it. ([#1503](https://github.com/logicplanes/kaeso/pull/1503))

#### KPP

- A Gemini request with many function calls is decoded in time linear in its size. A 9 MB body with 80,000 calls took about 14 s of CPU before and about 0.35 s now. The order in which responses are matched to calls is unchanged. ([#1484](https://github.com/logicplanes/kaeso/pull/1484))
- A Bedrock `ConverseStream` reply keeps the events that arrived before a damaged message, however the transport split the bytes, and the error follows them. Before, a chunk holding complete messages and then a malformed one returned only the error. A 3.5 MB chunk of small messages is also read in about 16 ms instead of about 1.4 s. ([#1488](https://github.com/logicplanes/kaeso/pull/1488))
- ElevenLabs' streamed speech with timestamps is read in time linear in its size. One 32 MiB line in 16 KiB chunks took 12.3 s before and takes 21 ms now. The events are the same as before. ([#1494](https://github.com/logicplanes/kaeso/pull/1494))
- A streamed OpenAI Chat Completions tool call keeps its `index` when it follows a tool call of an unknown type or one at a higher index, so two calls are no longer written at the same index and merged into one slot by clients. ([#1499](https://github.com/logicplanes/kaeso/pull/1499))

#### Console

- Opening a request or a tool call in the log no longer draws every row of the table again. In a measurement on a seeded install at 1440 px, a row click to the sheet showing went from 442 ms to 181 ms. ([#1469](https://github.com/logicplanes/kaeso/pull/1469))
- The model picker and the request log's Model filter open with far less delay and filter without freezing, also with thousands of models. Lists draw only the rows in view above 40 rows, the picker builds its items once per opening, and the request log builds the model choices only once the Model filter is added. ([#1473](https://github.com/logicplanes/kaeso/pull/1473))
- The organization and project overviews and Analytics jump less while they load: the release banner, trend lines, the Tool calls tab, metric tab notes and header buttons keep their place. The retiring-models warning, the month's cost split and the spend limit can still move the content below them when their data arrives after the first frame. ([#1474](https://github.com/logicplanes/kaeso/pull/1474))
- The console shows exchange-rate days and model release dates as the gateway sent them, instead of the day before for people whose time zone is west of UTC. This covers the rates note under converted costs, the exchange-rate hint under a model price, the instance's exchange-rate settings and "Released" on new-model cards and the overview's list of more models. ([#1487](https://github.com/logicplanes/kaeso/pull/1487))
- The playground's Markdown export writes where a box answer is in German and Simplified Chinese too, not only in English. ([#1498](https://github.com/logicplanes/kaeso/pull/1498))

### Security

- A guardrail rule that blocks or masks a value still acts when a longer value of another rule overlaps it, such as a secret followed by `@host` that an email rule also finds. A blocking find always counts, and a masking find is added when it overlaps no kept masking or blocking find. Finds that were kept before stay as they were, and the built-in rules guide says so. ([#1478](https://github.com/logicplanes/kaeso/pull/1478))
- Sign-in and password reset links sent to an address stop working once the identity provider of the managing organization moves the account to another address; before, they worked until they expired 15 minutes after being sent. ([#1492](https://github.com/logicplanes/kaeso/pull/1492))

#### Gateway

- A video's file link sends the provider's key only when the link's scheme, host and port match the provider's base URL. A link that differs, such as `http` for an `https` provider, is fetched without the key and with a guard that refuses plain HTTP to public addresses. A provider reached over plain `http` on an allowed network still gets its key for `http` links on its own host and port. ([#1481](https://github.com/logicplanes/kaeso/pull/1481))
- The error for a wrongly written `otel.headers` pair names the pair by its position and no longer quotes it, so a token in a pair written with a colon is not repeated in the startup error or the logs. ([#1482](https://github.com/logicplanes/kaeso/pull/1482))
- An agent that creates a shared link can share only the usage, costs and member names its own grant lets it see; before, the content was decided from the rights of the person behind the agent. Names the grant hides are stored as `hide_members` and `hide_keys` on the link. Links made in a console session are unchanged. ([#1483](https://github.com/logicplanes/kaeso/pull/1483))
- Requiring single sign-on now also refuses, for the accounts the organization manages, a password or sign-in link sign-in that waits for its TOTP code and a signup link sent before, answering that the organization requires single sign-on. The challenge of a link from `kaeso user sign-in-link` keeps working, as that link itself does. ([#1489](https://github.com/logicplanes/kaeso/pull/1489))
- Rate, token and spend limits count a request that moved on from a plan to an API key of the same pool, such as a ChatGPT plan that gives way for a media request. Before, that request counted against no limit. ([#1495](https://github.com/logicplanes/kaeso/pull/1495))

#### Self-hosting

- The updater refuses a release whose transparency log entry names a time or body the log didn't sign, when a bundle carries a field under both its camelCase and snake_case names. All checks now run on the bundle as it was parsed. The pull request states that forging a release still needs a leaked signing key and that no release is known to be affected. ([#1476](https://github.com/logicplanes/kaeso/pull/1476))

## [0.7.1] - 2026-11-24

Kaeso 0.7.1 adds sources, keys that let clients report the calls they make straight to their provider so Kaeso can log and count them as observed requests. MCP servers, existing ones included, now auto-approve detected tool changes by default, and streaming latency and per-chunk CPU are cut; apps written for 0.7.0 can still set and read an MCP server's `change_policy`. The release also carries security fixes and fixes for Gemini apps on Google's Go SDK, OpenRouter-style `reasoning` requests and the installer's settings check, and no change in it is marked as breaking.

**Contributors:** @dvnold

**Full changes:** [v0.7.0...v0.7.1](https://github.com/logicplanes/kaeso/compare/v0.7.0...v0.7.1)

### Added

- Projects can add sources: keys that let clients such as Claude Code, Codex, Gemini CLI and Antigravity, other gateways and scripts report the calls they make straight to their provider. Kaeso logs and counts them as observed requests but is not in their path and cannot refuse them. Each source has its own `sk-kso-src-…` key, which never works as an API key, managed through Platform API endpoints under `/orgs/{org}/projects/{project}/sources` and the rights `sources.view` and `sources.manage`; observed requests count toward project, member and budget limits. ([#1438](https://github.com/logicplanes/kaeso/pull/1438))
- Sources report over `/ingest/otlp/v1/logs`, `/traces` and `/metrics` (OTLP/HTTP, protobuf or JSON), `/ingest/v1/events`, `/ingest/v1/litellm` and `/ingest/v1/antigravity`, with the source key in `Authorization: Bearer` or `x-kaeso-source-key`. No or an unknown key gets 401, a disabled or archived source or project 403, an oversized body 413 and too many batches 429 with `Retry-After`. Compose, Helm and the AWS, GCP and DigitalOcean templates route `/ingest/*` to the gateway. ([#1441](https://github.com/logicplanes/kaeso/pull/1441))
- Usage and the timeline group by `source` and filter by `source`, where the key `gateway` stands for the gateway's own requests; request and tool-call logs carry `source_id` and filter by it. A spend or token limit shows `observed`, the part of its use that sources reported, and it is `null` for rate limits. `gemini-cli`, `antigravity` and `litellm` user agents name the client app. ([#1443](https://github.com/logicplanes/kaeso/pull/1443))
- The console has a Sources page in the project sidebar, shown with `sources.view`, with a list, Active, Archived and All tabs, and a sheet for each source. Add source (`sources.manage`) picks the client, shows the key once with the setup for that client and polls until the first event arrives. The UI library gains `SourceBadge`. ([#1446](https://github.com/logicplanes/kaeso/pull/1446))
- A source's batch is read within bounded memory: a batch holds at most 1,000 records (was 5,000), and a source may send 6,000 records a minute and 200,000 a day besides its 600 batches a minute, past which it gets 429 with `Retry-After`. The events, LiteLLM and Antigravity endpoints answer too many records with 413 before reading them; JSON may be 8 MiB once unzipped, and protobuf stays at 16 MiB. A prompt that Claude Code or Gemini CLI reports once for a turn is kept with the first request it led to, not copied onto every request of a tool loop. ([#1454](https://github.com/logicplanes/kaeso/pull/1454))
- A source counts only while its own member or service account does: once the member can no longer see the project, or the service account is archived, every record is refused as `inactive`. Matching members by email is opt-in, with `match_members` defaulting to `false` in the API and the Add source dialog, and a record is capped at 10 million of each token count (was 100 million) and $100 of reported cost (was $10,000). ([#1455](https://github.com/logicplanes/kaeso/pull/1455))
- Grok Build can be added as a source, with its own kind `grok`, setup lines in the Add source dialog, xAI's provider mark on the source badge and a guide in the docs. A migration adds `grok` to the allowed source kinds, and its usual provider is xAI. ([#1459](https://github.com/logicplanes/kaeso/pull/1459))
- MCP servers get an `auto_approve_tool_changes` setting, on by default for new servers and, through a migration, for existing ones. When on, detected tool changes apply right away, are recorded in the server's tool-change history (`GET /platform/v1/orgs/{org}/mcp-servers/{server}/tool-changes`) and audit-logged as `tool.auto_approve`, without review notifications; it never turns a tool on in a project that has it off. When off, changed tools are held for review and reported to MCP server managers, as before; switching it on approves the changes that are waiting. ([#1450](https://github.com/logicplanes/kaeso/pull/1450))

#### Console

- Observed requests show in the request log with an Observed tag, a Reported by filter and an optional Source column, in analytics under By source for people with `sources.view`, and in the tool-call log. Limits with an observed part note the amount sources reported, which Kaeso counts but cannot stop. ([#1447](https://github.com/logicplanes/kaeso/pull/1447))

#### Documentation

- A Performance page in the self-hosting docs compares Kaeso 0.7.0 with LiteLLM, Bifrost and Portkey on BerriAI's ai-gateway-bench. With 64 clients, a zero-delay mock and two dedicated cores per gateway, Kaeso serves 6,338 requests a second and LiteLLM Rust 3,211; the page says the run was single-host and run by Kaeso's author. ([#1432](https://github.com/logicplanes/kaeso/pull/1432))
- A KPMR section in the docs explains the registry, how the gateway syncs it, the catalogue format, the public API, and retirements and model lines, in English, German and Chinese. It replaces the former `concepts/kpmr` page, and links to it now point into the section. ([#1434](https://github.com/logicplanes/kaeso/pull/1434))
- A Sources concept page and a Sources section explain observed requests, whose requests they are, limits and refusals, with a setup guide for Claude Code, Codex, Gemini CLI, Antigravity, LiteLLM, OpenRouter, OpenTelemetry and HTTP, in English, German and Chinese. ([#1449](https://github.com/logicplanes/kaeso/pull/1449))

### Changed

- The Helm chart, the gateway image's description and the security policy describe Kaeso as it is in 0.7. The chart gains a description, icon, keywords, Artifact Hub fields and a README, the image label names Gemini and ElevenLabs as well as alias routes, budgets, guardrails, request logs and the MCP portal, and `SECURITY.md` lists the supported versions. ([#1431](https://github.com/logicplanes/kaeso/pull/1431))
- Account › Notifications no longer has a Send a test email button; instance admins test mail under Instance › Settings › Email or with `kaeso mail test`, both unchanged. The `POST /platform/v1/me/notifications/test` endpoint is removed. ([#1440](https://github.com/logicplanes/kaeso/pull/1440))
- Background jobs log runs that found nothing to do at `debug` instead of `info`, while runs that did work or hit failures stay at `info` and job errors at `error`. A quiet live install had produced about 1,070 lines an hour; the filter `info,kaeso_server::jobs=debug` shows idle runs. ([#1444](https://github.com/logicplanes/kaeso/pull/1444))
- Streaming latency and per-chunk CPU are cut. Same-dialect SSE streams of OpenAI Chat Completions, OpenAI Responses and Anthropic Messages are forwarded byte for byte when content logging, reply guardrails, reasoning continuity and built-in unmasking are inactive. An OpenAI Responses stream translated to another format skips the wire-override diffing that foreign encoders ignore, and ready chunks after the first byte are written together. ([#1453](https://github.com/logicplanes/kaeso/pull/1453))

#### Gateway

- A fresh install starts with KPMR's model data as of 21 November 2026, 02:59 UTC: 2,936 models from the 50 providers Kaeso calls, up from 2,934, with no model removed, none newly deprecated and no status changed. `minimax-latest` gives way to `minimax-m-latest` and `minimax-m-flash-latest` at 14 providers; a project that follows `minimax-latest` keeps its current model, and the line shows that KPMR no longer lists it. ([#1465](https://github.com/logicplanes/kaeso/pull/1465))

### Fixed

- The installer checks the install's settings before every start of an existing install, not only right after it writes a new `compose.yaml`; a fresh install still skips the check. When the gateway container still runs the release `compose.yaml.previous` names while `compose.yaml` names a newer one, the run counts as an update from that release, and if the new release refuses the settings the installer puts `compose.yaml` back, removes `compose.yaml.previous` and the old release keeps running. A re-run that is not an update stops before starting and changes no files. ([#1426](https://github.com/logicplanes/kaeso/pull/1426))
- Versions and names with a hyphen, such as 0.7.0-rc.1 in the update history, read as written instead of spaced out. Text with a hyphen next to a letter keeps proportional figures in charts and UI library components, while dates, times and numbers keep tabular ones, and time zone offsets west of Greenwich are written with a minus sign. ([#1427](https://github.com/logicplanes/kaeso/pull/1427))
- After Load more, focus moves to the first new row and a polite status announces how many rows came. A page that arrives while the button only has focus, without being pressed, keeps the button in view instead, and focus that has moved on is left alone. In the request logs, the near-end loader waits while Load more has focus and loads again once focus leaves it. ([#1433](https://github.com/logicplanes/kaeso/pull/1433))
- Gemini apps using Google's Go SDK stream and return tool results cleanly: the SSE keepalive frame is two newlines instead of one, which had broken Go clients with an invalid stream chunk error. A Gemini content whose parts are all `functionResponse` without a `thoughtSignature` decodes as a user turn whatever its wire `role`, as upstreams such as Anthropic Messages require, and the original role is kept so same-dialect Gemini round trips stay exact. ([#1436](https://github.com/logicplanes/kaeso/pull/1436))
- OpenRouter-compatible apps can send the `reasoning` object (`effort`, `max_tokens`, `enabled`, `exclude`) across Claude, Gemini and OpenAI models; it was refused whenever a request crossed to Anthropic Messages, Gemini or OpenAI Responses. OpenRouter, Vercel and Ollama Cloud keep the object, other Chat Completions providers receive `reasoning_effort` or their thinking switch, and unknown `reasoning.*` keys are still refused for providers that don't take the object and across formats. `exclude: true` hides reasoning from replies and streams, and on Chat Completions usage still reports the reasoning tokens. ([#1442](https://github.com/logicplanes/kaeso/pull/1442))
- Apps written for 0.7.0 can still set and read an MCP server's `change_policy`, which now stands for `auto_approve_tool_changes`: `POST` and `PATCH` accept it again, where `hold` means `false` and `accept` means `true`, and sent together with `auto_approve_tool_changes` they must agree, else 400 with `param: change_policy`. The server view shows `change_policy` again, and `POST …/refresh` answers with `accepted` beside `auto_approved`. Their descriptions in `openapi.json` call them deprecated, and switching to `accept` behaves as switching auto-approve on, which approves the changes held for review. ([#1456](https://github.com/logicplanes/kaeso/pull/1456))
- No provider or model tile shows a letter any more; Amazon Bedrock models no longer show a grey letter. Rows about one model show the mark of the company that made it, read from the model id, on the new-models card, the request log's model column, the route editor's model tree and the model picker; a model under a namespace no maker owns shows the host's mark. Amazon Bedrock, fal and Baidu Qianfan, whose owners' terms don't allow their logos, show the provider icon, as do Amazon's and Baidu's own models, and KoboldCpp and TabbyAPI get marks. ([#1445](https://github.com/logicplanes/kaeso/pull/1445))
- SGLang and MLX LM show the provider icon: SGLang's logo is licensed CC BY-NC-ND 4.0, so a redrawn copy can't ship under MIT, and MLX LM showed Apple's logo, which Apple's guidelines reserve. DeepSeek keeps its published blue on the dark theme too, and SpaceX shows xAI's square mark again. Source badges and client buttons for Claude Code, Gemini CLI, Antigravity and OpenTelemetry, and Claude's connect button, show their marks in their makers' colours, while Codex, OpenRouter, Cursor, GitHub Copilot and ChatGPT stay black and white. ([#1451](https://github.com/logicplanes/kaeso/pull/1451), [#1457](https://github.com/logicplanes/kaeso/pull/1457))
- Sources count each request once: a request a client sends through the gateway and also reports to a source is matched to the gateway's request and counted as a duplicate, and a GenAI call exported as a span and as a log event counts once. A prompt, Codex's request start and Codex's conversation provider that arrive one export before the request are kept for it across export batches. Requests are priced at the provider they used, not a hard-coded one, and a LiteLLM `response_cost` of 0 with `response_cost_failure_debug_info` is an unknown cost, not $0. MCP tool calls a source reports now count in Analytics' daily tool-call counts. ([#1458](https://github.com/logicplanes/kaeso/pull/1458))
- A link in a section's actions keeps its words on one line, so "On GitHub" beside a release on Instance › Updates no longer wraps its last word. The Updates history columns are resized so the When column shows the whole time in English, German and Chinese. ([#1460](https://github.com/logicplanes/kaeso/pull/1460))
- A status keeps a Chinese or Japanese word whole in a narrow row; on a phone in Chinese, source statuses such as connected and waiting no longer split across two lines. A status with several Latin words can still wrap between them. ([#1464](https://github.com/logicplanes/kaeso/pull/1464))
- The Claude plugin declares no MCP server again and asks for no address, because the plugin directory refuses a server whose address is a setting; its skill connects your install in Claude Code with a command and everywhere else with the custom connector link, and the console and documentation no longer mention `/plugin configure`.

#### Gateway

- A video whose API key is gone before it ends is still logged as the app's video, with that key, its tags and its client, instead of as a Playground video. Videos and image generations keep the id of the key that started them; videos that had already lost their key id can't be told apart from Playground videos, and the migration leaves them as they are. ([#1429](https://github.com/logicplanes/kaeso/pull/1429))
- Webhook signing secrets are re-wrapped by `kaeso master-key rewrap` during master key rotation, and startup checks catch an unconfigured master key that still wraps webhook secrets. Token prefixes are also redacted in OpenTelemetry span error messages and in fallback and tool-call log error messages. ([#1437](https://github.com/logicplanes/kaeso/pull/1437))

#### Console

- Buttons on the sign-in, sign-up, invitation, setup, password reset and agent authorization pages, the recent sign-in dialog and the sidebar's Create organization now say what they still need. They stay focusable, show the reason on hover and focus, and screen readers hear it; Authorize asks for a name in Customize, and Enter in a field doesn't send a form whose button is waiting. ([#1448](https://github.com/logicplanes/kaeso/pull/1448))
- The audit log names an approved or auto-approved MCP tool instead of showing its id for `tool.approve` and `tool.auto_approve` entries. A `tool.reject` entry keeps only the hash and still shows the id. ([#1462](https://github.com/logicplanes/kaeso/pull/1462))
- Table headers leave room for the sort arrow, so a header that fills its column, such as "Änderung" in German analytics breakdowns and "缓存命中" in Chinese project analytics tables, isn't cut. A header that already had room is unchanged. ([#1463](https://github.com/logicplanes/kaeso/pull/1463))

#### UI library

- The UI showcase's View source and its `llms.txt` source link open the commit the site was built from instead of a release tag, which returned a 404 for a pre-release build. Its search finds components by their parts, props, description and examples and shows why each result matched, in English, German and Chinese. ([#1430](https://github.com/logicplanes/kaeso/pull/1430))
- View source on the showcase opens the file each component is written in; 45 of 113 component pages have no folder of their own because their component is written beside others, so the old link answered 404. The build now stops when a page's source isn't in the checkout. ([#1461](https://github.com/logicplanes/kaeso/pull/1461))

### Security

- `url` is updated to 2.5.9, which fixes RUSTSEC-2026-0592: `Url::origin()` recursed once per nested `blob:` prefix and could overflow the stack. Kaeso calls `origin()` on URLs from configuration, update feeds, sign-in issuers and MCP server addresses, none of which is a `blob:` URL in practice, so nothing changes for them. ([#1435](https://github.com/logicplanes/kaeso/pull/1435))
- A started video or image generation holds its highest possible cost against hard budgets until it is billed, and a video nobody asks about again is billed when it ends, so jobs started one after another no longer run past a hard budget. ([GHSA-7vfp-ggg2-hhwm](https://github.com/logicplanes/kaeso/security/advisories/GHSA-7vfp-ggg2-hhwm))
- Transcriptions, translations and voice changes hold the longest their uploaded audio can last against hard budgets while they run, so uploads sent side by side no longer run past a hard budget. ([GHSA-g48f-v4f7-4m95](https://github.com/logicplanes/kaeso/security/advisories/GHSA-g48f-v4f7-4m95))
- Batches check and hold hard token and spend budgets before they start, and refuse features that have no price. ([GHSA-5hx3-cg7v-33c7](https://github.com/logicplanes/kaeso/security/advisories/GHSA-5hx3-cg7v-33c7))
- kaeso-edge takes the client address from the nearest untrusted hop of `X-Forwarded-For`, and on Railway from `X-Real-IP`. ([GHSA-rwrp-468m-3jv3](https://github.com/logicplanes/kaeso/security/advisories/GHSA-rwrp-468m-3jv3))
- Replacing or removing an organization's single sign-on connection, unlinking an account from it, requiring single sign-on or removing a member through SCIM also ends the agents and portal clients approved in the sessions that end. ([GHSA-v89c-fm8p-fh9j](https://github.com/logicplanes/kaeso/security/advisories/GHSA-v89c-fm8p-fh9j))
- Account › Agents and agent grants follow the session's open organizations and sign-in rules. ([GHSA-qgxw-mmg2-92qp](https://github.com/logicplanes/kaeso/security/advisories/GHSA-qgxw-mmg2-92qp))
- Account › Connections and muting an organization's emails follow the organization's sign-in rules, and an MCP sign-in finishes only while the person still belongs to the organization. ([GHSA-gwf4-xf89-jr57](https://github.com/logicplanes/kaeso/security/advisories/GHSA-gwf4-xf89-jr57))
- MCP server sign-ins accept only `https` sign-in addresses, or `http` ones for an `http` server, and the console opens only web pages for them. ([GHSA-25wg-9wmh-9rcg](https://github.com/logicplanes/kaeso/security/advisories/GHSA-25wg-9wmh-9rcg))
- Turning automatic updates on, or changing their channel, window or time zone while they are on, needs `instance.updates.apply` and a recent sign-in. ([GHSA-c92c-rh4f-h5hm](https://github.com/logicplanes/kaeso/security/advisories/GHSA-c92c-rh4f-h5hm))
- Members with `webhooks.view` alone see only each webhook address's origin, and the audit log masks webhook addresses. ([GHSA-rw8m-ggwj-8mmc](https://github.com/logicplanes/kaeso/security/advisories/GHSA-rw8m-ggwj-8mmc))
- The weekly summary email shows a project's cost only to members who may see costs. ([GHSA-3mc2-72rx-4792](https://github.com/logicplanes/kaeso/security/advisories/GHSA-3mc2-72rx-4792))
- Provider errors on batches, media jobs and realtime sockets no longer carry key or account tokens. ([GHSA-wx5j-mvf7-crwx](https://github.com/logicplanes/kaeso/security/advisories/GHSA-wx5j-mvf7-crwx))
- Responses requests refuse `item_reference` items, send `store: false` when Kaeso keeps no stored responses, and batch lines are checked the same way. ([GHSA-2x35-h4qp-x2r3](https://github.com/logicplanes/kaeso/security/advisories/GHSA-2x35-h4qp-x2r3))
- Batch uploads hold the translation memory budget, and request bodies are checked for depth before their fields are read. ([GHSA-469r-9qrh-476q](https://github.com/logicplanes/kaeso/security/advisories/GHSA-469r-9qrh-476q))
- Realtime speech-to-text checks `audio_format` before it connects, defaults to `pcm_16000`, and bills the audio by that format. ([GHSA-w6r4-vxjx-5g7r](https://github.com/logicplanes/kaeso/security/advisories/GHSA-w6r4-vxjx-5g7r))
- The AWS managed stack passes the PostgreSQL and Valkey addresses to ECS through Secrets Manager instead of the task definition's environment. ([GHSA-x9fh-c3x2-88fh](https://github.com/logicplanes/kaeso/security/advisories/GHSA-x9fh-c3x2-88fh))

## [0.7.0] - 2026-11-21

Kaeso 0.7.0 adds decision models in three API formats, a rerank endpoint, guardrails, webhooks, SCIM 2.0 provisioning, key pools, batches and a Helm chart, new built-in providers from Amazon Bedrock and Azure to TypeSafe, and brings the console, documentation and UI library into the Kaeso Vanguard design. Updates from the console or the installer now check the install's settings before switching anything and stop before changing files when the new gateway would refuse one. Three settings that 0.6.0 accepted are refused at startup, so check `kpmr.interval`, `mcp_servers.sync_interval` and `smtp.user` before you upgrade.

- **Breaking:** The gateway stops at startup when `kpmr.interval` is below 5 minutes, which 0.6.0 accepted; set it to `5m` or longer, or remove it for `auto`. ([#1120](https://github.com/logicplanes/kaeso/pull/1120), [#1407](https://github.com/logicplanes/kaeso/pull/1407))
- **Breaking:** The gateway stops at startup when `mcp_servers.sync_interval` is below a minute, which 0.6.0 raised to a minute; set it to `1m` or longer, or remove it. ([#1120](https://github.com/logicplanes/kaeso/pull/1120), [#1407](https://github.com/logicplanes/kaeso/pull/1407))
- **Breaking:** The gateway stops at startup when `smtp.user` is set without `smtp.password`, where 0.6.0 sent email without authentication; add the password, or remove the user for a server without authentication. ([#1121](https://github.com/logicplanes/kaeso/pull/1121), [#1407](https://github.com/logicplanes/kaeso/pull/1407))

**Contributors:** @dvnold

**Full changes:** [v0.6.0...v0.7.0](https://github.com/logicplanes/kaeso/compare/v0.6.0...v0.7.0)

### Added

- Asking Kaeso about usage in a client that supports MCP Apps opens an interactive view with the period's totals, a chart and the top models, providers or keys, through the new `show_usage_stats` tool; clients without MCP Apps get a one-line summary plus the same data as `structuredContent`. ([#909](https://github.com/logicplanes/kaeso/pull/909))
- MCP servers that ship MCP Apps views show them when a client reaches them through a project's MCP portal: the portal answers `resources/list`, `resources/templates/list` and `resources/read`, and reading a view needs the same right as calling a tool that names it. ([#910](https://github.com/logicplanes/kaeso/pull/910))
- An identity provider can add and remove an organization's members through SCIM 2.0 at `/platform/v1/orgs/{org}/scim/v2`, with its own bearer token per organization created under Security › Provisioning; removing a member at the identity provider removes them as an admin would, and only accounts at the organization's verified domains are touched. ([#1244](https://github.com/logicplanes/kaeso/pull/1244))
- Single sign-on sets members' roles and projects from the identity provider's groups, mapped to an organization role and project memberships in Add only or Keep in sync mode, and for Microsoft Entra ID users in more than 200 groups Kaeso reads their groups from Microsoft Graph. ([#1245](https://github.com/logicplanes/kaeso/pull/1245), [#1340](https://github.com/logicplanes/kaeso/pull/1340))
- Prices can be entered and spend read in other currencies at the ECB's daily reference rates; an organization sets the currency Kaeso shows all its money in, while everything is still stored and costed in US dollars. ([#1328](https://github.com/logicplanes/kaeso/pull/1328), [#1376](https://github.com/logicplanes/kaeso/pull/1376))
- Projects see when their models shut down, who still calls them and what replaces them: replies from models with announced dates carry `Deprecation`, `Sunset` and `Link` headers, a request naming a retired model fails with 410, and aliases can move to the replacement in one click or on the shutdown day, both off by default. ([#1331](https://github.com/logicplanes/kaeso/pull/1331))
- A project can add model lines such as `claude-opus-latest` that follow the provider's newest stable model with a policy of `follow` (the default), `ask`, `delay` or `pin`; a move never happens when the project has no key for the provider or when a caller that may use the line may not call the new target. ([#1353](https://github.com/logicplanes/kaeso/pull/1353))
- Usage, a member's stats, a project's budgets and its summary can be shared through read-only links that people without a Kaeso account can open; each link is live or a snapshot, can hide costs and member, key and project names, expires after 1, 7, 30 or 90 days or never, and is checked against its maker's current rights each time it is opened, while request logs are never shareable; the rights `usage.share` and `usage.share.manage` govern them. ([#1347](https://github.com/logicplanes/kaeso/pull/1347))
- Requiring two-factor authentication first names the members it would lock out, and members hear what to do when an organization starts requiring two-factor authentication or single sign-on of them, through the new notification kind A sign-in rule affects you. ([#1355](https://github.com/logicplanes/kaeso/pull/1355), [#1360](https://github.com/logicplanes/kaeso/pull/1360))
- Kaeso emails 7 days and 1 day before an organization's platform key expires, to members who hold `platform_keys.create` and to the key's creator while they are still a member. ([#1349](https://github.com/logicplanes/kaeso/pull/1349))
- Test key on Providers asks the provider whether it takes a key by reading its model listing, without billing a request; `POST /orgs/{org}/credentials/{credential}/check` answers `ok`, `refused`, `unreachable` or `unknown`. ([#1350](https://github.com/logicplanes/kaeso/pull/1350))
- Instance admins can list, resend and revoke invitations to the install from Users, and see when each user last signed in on the Users list and in the user sheet. ([#1342](https://github.com/logicplanes/kaeso/pull/1342), [#1351](https://github.com/logicplanes/kaeso/pull/1351))
- Analytics breakdown tables sort by any number and download as CSV, and a project's rows open the requests they count; project and account API keys can be searched and sorted, project members searched and filtered by role, and an organization's projects sorted by their requests or cost in the last 30 days. ([#1233](https://github.com/logicplanes/kaeso/pull/1233), [#1329](https://github.com/logicplanes/kaeso/pull/1329), [#1341](https://github.com/logicplanes/kaeso/pull/1341))
- The model catalogue filters by status, an alias can be duplicated into a new one with the same route, and the route editor shows an alias's unsaved changes as a diff before saving. ([#1309](https://github.com/logicplanes/kaeso/pull/1309), [#1314](https://github.com/logicplanes/kaeso/pull/1314), [#1327](https://github.com/logicplanes/kaeso/pull/1327))
- A member's or key's limit can be copied to someone else with Copy to…, and limits sort by how much is used. ([#1317](https://github.com/logicplanes/kaeso/pull/1317))
- A platform key opens its details with who created it, when and the keys a rotation linked it to, and API keys can be renamed while the secret keeps working. ([#1321](https://github.com/logicplanes/kaeso/pull/1321), [#1326](https://github.com/logicplanes/kaeso/pull/1326))
- Instance admins export the instance audit log as JSON Lines; the project audit log names the keys, models, aliases, service accounts and limits it lists and opens each, and member and user sheets open the audit log at their changes or the changes made to them. ([#1315](https://github.com/logicplanes/kaeso/pull/1315), [#1332](https://github.com/logicplanes/kaeso/pull/1332), [#1337](https://github.com/logicplanes/kaeso/pull/1337))
- The command palette opens a pasted request id in the project's request log, and the shortcut sheet lists every keyboard shortcut that works where you are, with a go-to key for every project page. ([#1320](https://github.com/logicplanes/kaeso/pull/1320), [#1335](https://github.com/logicplanes/kaeso/pull/1335))
- Every finished Playground reply links to its request in the request log, a conversation exports as Markdown or JSON, and the last prompt can be edited and sent again to every model. ([#1322](https://github.com/logicplanes/kaeso/pull/1322), [#1323](https://github.com/logicplanes/kaeso/pull/1323), [#1324](https://github.com/logicplanes/kaeso/pull/1324))
- Expired invitations can be resent and revoked ones sent again, pending invitations can be revoked several at a time, the instance's Users list shows who has two-factor, and Organizations shows each one's sign-in requirements and filters by single sign-on. ([#1333](https://github.com/logicplanes/kaeso/pull/1333), [#1334](https://github.com/logicplanes/kaeso/pull/1334))
- Account › Connections says when each MCP connection was last used and shows when you signed in and when it renews, and an agent's Activity names the organization and project each call acted in. ([#1336](https://github.com/logicplanes/kaeso/pull/1336), [#1348](https://github.com/logicplanes/kaeso/pull/1348))

#### Gateway

- Apps make decisions through Kaeso at `/v1/decisions`, `/v1/systemone` and `/v1beta/decisions` on OpenAI, Google, OpenRouter, Vercel, Baseten, Fireworks and Perplexity models, with Kaeso's keys, limits, budgets, logs, costs, alias routes and response cache; a new setting `body_limits.decisions` (default 64 MiB) bounds their bodies, and chat, token counts and embeddings on a decision-only model are refused with a message that names the decision endpoints. ([#969](https://github.com/logicplanes/kaeso/pull/969))
- TypeSafe is a built-in provider: add its key and call Jev from any decision format, with images and box questions, which are refused before sending on Jev 1.13, and Cloudflare's Clef decision models (`@cf/cloudflare/clef`, `clef-flash` and `clef-omni`) answer decisions in any decision format through the Cloudflare provider with the same key it uses for chat. ([#970](https://github.com/logicplanes/kaeso/pull/970), [#974](https://github.com/logicplanes/kaeso/pull/974))
- Custom providers serve decisions in System One's or OpenAI's format once an org admin turns them on, with the path and, for System One, the image placement their server takes (`assets`, `state`, `root` or `none`); detection never turns decisions on, and a decision model can be added only while its decisions are on. ([#972](https://github.com/logicplanes/kaeso/pull/972))
- Standard Compute and Perplexity are built-in providers: Standard Compute calls its smart routing or a pinned model from any app format, with routers listed first and marked Router in Add models, and Perplexity calls its Sonar models, with search fields such as `search_mode` passing for Chat Completions apps only. ([#959](https://github.com/logicplanes/kaeso/pull/959), [#966](https://github.com/logicplanes/kaeso/pull/966))
- DeepInfra, Upstage, IONOS AI Model Hub, Novita AI, OVHcloud AI Endpoints, Tencent TokenHub and Venice are built-in providers: add a key and call their models and embeddings from any app format, and TokenHub's other regions work as base URL overrides. ([#1105](https://github.com/logicplanes/kaeso/pull/1105), [#1107](https://github.com/logicplanes/kaeso/pull/1107), [#1109](https://github.com/logicplanes/kaeso/pull/1109), [#1110](https://github.com/logicplanes/kaeso/pull/1110), [#1111](https://github.com/logicplanes/kaeso/pull/1111), [#1117](https://github.com/logicplanes/kaeso/pull/1117), [#1118](https://github.com/logicplanes/kaeso/pull/1118))
- Volcengine Ark, StepFun and Baidu Qianfan are built-in providers, with a new provider-level API root so Ark's `/api/v3` and Qianfan's `/v2` work like every other provider, Qianfan's embeddings, and StepFun's China service, Qianfan's international service and BytePlus ModelArk as base URL overrides. ([#1108](https://github.com/logicplanes/kaeso/pull/1108), [#1116](https://github.com/logicplanes/kaeso/pull/1116), [#1363](https://github.com/logicplanes/kaeso/pull/1363))
- Amazon Bedrock is a built-in provider: a Bedrock API key with its AWS region calls Claude models through the Messages route and other models through Chat Completions, requests can be signed with AWS Signature Version 4 from an access key or the gateway's own AWS role via `aws.ambient_credentials` (instance admins only), and models that answer only Converse, such as Amazon Nova and Meta Llama, are reached through it. ([#1174](https://github.com/logicplanes/kaeso/pull/1174), [#1246](https://github.com/logicplanes/kaeso/pull/1246), [#1247](https://github.com/logicplanes/kaeso/pull/1247))
- Azure is a built-in provider: add the key of an Azure OpenAI or Microsoft Foundry resource with its endpoint and call the resource's deployments from any app format, a key can instead be a Microsoft Entra ID service principal for resources that turn key access off, and a per-model Provider model ID sets the id Kaeso sends to the provider. ([#1187](https://github.com/logicplanes/kaeso/pull/1187), [#1254](https://github.com/logicplanes/kaeso/pull/1254))
- Cloudflare AI Gateway is a built-in provider: a key calls one Cloudflare account through one of its AI Gateways, with Workers AI, routed models, custom models and dynamic routes, and a key without a gateway calls Workers AI directly, including its embedding models. ([#962](https://github.com/logicplanes/kaeso/pull/962), [#1106](https://github.com/logicplanes/kaeso/pull/1106))
- Apps rerank at `/v1/rerank` and `/v2/rerank` in Cohere's rerank format, which Cohere's Rerank 5 models answer, and Voyage AI and Jina AI are built-in providers for embeddings from any app format and rerankers through the same endpoint. ([#1211](https://github.com/logicplanes/kaeso/pull/1211), [#1212](https://github.com/logicplanes/kaeso/pull/1212), [#1213](https://github.com/logicplanes/kaeso/pull/1213))
- fal and Replicate are built-in providers for media: add a key (a token for Replicate) and make images with FLUX and Seedream and videos with Veo 3.1 and Kling from OpenAI's image and video formats, with fal also mapping Wan 3.0 and Replicate Nano Banana Pro; only the models they map by hand are accepted, and other models are refused with the list. ([#1356](https://github.com/logicplanes/kaeso/pull/1356), [#1357](https://github.com/logicplanes/kaeso/pull/1357))
- Deepgram, Cartesia and AssemblyAI are built-in providers for speech from OpenAI's audio format: Deepgram transcribes with Nova-3 and Whisper and speaks with Aura voices, Cartesia speaks with Sonic and transcribes with Ink Whisper, and AssemblyAI transcribes with Universal-3.6 Pro through its Sync API; fields a provider has no place for are refused before sending. ([#1361](https://github.com/logicplanes/kaeso/pull/1361), [#1362](https://github.com/logicplanes/kaeso/pull/1362), [#1366](https://github.com/logicplanes/kaeso/pull/1366))
- The docs, the UI library's site and the console link a legal notice and a privacy policy set with `legal.imprint_url` and `legal.privacy_url`, each an `https` address; the gateway refuses to start with anything else, and an install that sets neither shows no legal links. ([#961](https://github.com/logicplanes/kaeso/pull/961))
- Kaeso reads request bodies sent with `Content-Encoding: gzip`, `x-gzip` or `zstd`, as TypeSafe's SDKs send them, on `/v1`, `/v1beta` and `/v2`; other encodings such as `br` or `deflate`, and stacked ones, get 415 in the app's format, a body that decodes past the route's body limit gets 413, and a corrupt one 400. ([#963](https://github.com/logicplanes/kaeso/pull/963))
- Model API replies carry the provider's request id in `kaeso-upstream-request-id`, non-streamed replies carry what they cost in `kaeso-cost` when the key's owner holds `costs.view`, and every answer, streams included, carries `kaeso-limits`, an RFC 9651 list of what is left of the key's rate limits and hard budgets, when the key's owner may see the project's limits. ([#1188](https://github.com/logicplanes/kaeso/pull/1188), [#1189](https://github.com/logicplanes/kaeso/pull/1189), [#1318](https://github.com/logicplanes/kaeso/pull/1318))
- A request can shorten how long Kaeso waits for each provider with `kaeso-timeout` (1 to 3600 seconds, never raising a wait), keep its content out of the request log with `kaeso-log-content: off`, and tag itself with up to 10 `kaeso-tags` that the request log filters by; browser apps may send all three, and another value answers 400. ([#1215](https://github.com/logicplanes/kaeso/pull/1215), [#1218](https://github.com/logicplanes/kaeso/pull/1218), [#1230](https://github.com/logicplanes/kaeso/pull/1230))
- `cors.allowed_headers` lists header names that preflights from listed origins also allow, so a browser app can send a header an alias route splits on, and `server-timing` is exposed to browser apps; `*`, `cookie`, `forwarded`, `x-forwarded-…` and names that aren't header names stop the gateway at startup. ([#1220](https://github.com/logicplanes/kaeso/pull/1220))
- The response cache honours `Cache-Control: max-age`, taking a stored reply only when it is at most that many seconds old, and `only-if-cached`, which answers from the cache or with 504 without calling the provider. ([#1308](https://github.com/logicplanes/kaeso/pull/1308))
- `/metrics` gains `kaeso_update_available`, the KPMR fetch and attempt timestamps including `kaeso_kpmr_last_attempt_failed`, `kaeso_connections_open`, `kaeso_connections_max`, `kaeso_model_requests_in_flight`, and `kaeso_tool_calls_total` with `kaeso_tool_call_duration_seconds` for MCP portal tool calls. ([#1207](https://github.com/logicplanes/kaeso/pull/1207), [#1208](https://github.com/logicplanes/kaeso/pull/1208), [#1209](https://github.com/logicplanes/kaeso/pull/1209), [#1210](https://github.com/logicplanes/kaeso/pull/1210))
- With `otel.endpoint` set, each model request is exported as an OpenTelemetry trace over OTLP/HTTP JSON: a `kaeso.request` server span that continues the app's `traceparent`, a client span per route step that called a provider, and MCP portal tool calls as `execute_tool` spans; `otel.content` adds the prompt and reply where the request log keeps content, and spans never delay or fail a request. ([#1235](https://github.com/logicplanes/kaeso/pull/1235))
- The request log stores each long message an agent conversation repeats once per project (ClickHouse migration `0017_content_parts`), so a session's stored content grows with its turns instead of their square, and every reader gets the body back byte for byte. ([#1248](https://github.com/logicplanes/kaeso/pull/1248))
- A route step tries its model again after a dropped connection, a 5xx or an overloaded provider, never after a timeout, a rate limit or a refused request; steps take `retries` from 0 to 3 and projects a Retries setting, the request log lists each try made again as retried, and `kaeso-route` adds `tries=N`. ([#1236](https://github.com/logicplanes/kaeso/pull/1236))
- Organizations send operational notices, such as budget alerts, limits reached, expiring keys and MCP server changes, to up to 10 webhooks signed as Standard Webhooks specifies, as Kaeso's JSON event or a Slack line; deliveries are at least once and tried again after 1, 5, 15 and 60 minutes, a receiver that fails 20 times in a row is turned off, and the rights `webhooks.view` and `webhooks.manage` govern them. ([#1237](https://github.com/logicplanes/kaeso/pull/1237))
- Organizations register guardrails, HTTP services of their own that Kaeso asks before a request goes to a model and, if chosen, before the reply goes back; a guardrail can allow, block or replace messages, requests are signed like Standard Webhooks, a block answers `content_filtered` with status 400 in the app's format, and only chat requests are checked. ([#1238](https://github.com/logicplanes/kaeso/pull/1238))
- Built-in guardrail rules find secrets and personal data inside Kaeso, such as provider keys, tokens, email addresses, IBANs and card numbers, plus up to 20 custom patterns, and mask them, block the request or only count them; Mask in the request log keeps the values out of the log, and Restore in the reply puts masked values back in the model's answer. ([#1240](https://github.com/logicplanes/kaeso/pull/1240))
- A project can spread its requests over several keys of one provider: assigning a key adds it to the project's pool with a weight from 1 to 100, a caller stays on one key, and a key gives way to the next when the failure is the key's own, such as a refused key, billing or a rate limit; video jobs, image generations, speech, realtime and ElevenLabs still use the primary key. ([#1241](https://github.com/logicplanes/kaeso/pull/1241))
- Responses apps can store and continue responses at OpenAI where the organization and project allow it; `store: true` and `previous_response_id` are refused everywhere else with a message that says why, `conversation` and `background: true` stay refused, and `GET` and `DELETE /v1/responses/{response_id}` work for stored responses. ([#1242](https://github.com/logicplanes/kaeso/pull/1242))
- Apps can run OpenAI-format batches through Kaeso at providers whose batch API takes them as they are, today OpenAI; `/v1/files` and `/v1/batches` check each line against the key and model access before anything reaches the provider, a finished batch is charged at the batch price, 50 percent for OpenAI, with one usage and request log row per line. ([#1243](https://github.com/logicplanes/kaeso/pull/1243))

#### KPP

- KPP reads and writes decision requests, typed questions about an input answered with probabilities, in TypeSafe's System One API, OpenAI's Decisions API and Gemini's Decisions API; requests and replies cross between all three, and what a format cannot express is refused before sending. ([#964](https://github.com/logicplanes/kaeso/pull/964), [#967](https://github.com/logicplanes/kaeso/pull/967), [#968](https://github.com/logicplanes/kaeso/pull/968))

#### Console

- The Playground asks decision models typed questions and shows every answer with its probabilities beside chat models in the same comparison, and the Platform API gains `POST …/playground/decisions` and a `decision` mode in `playground/models`. ([#965](https://github.com/logicplanes/kaeso/pull/965), [#973](https://github.com/logicplanes/kaeso/pull/973))
- The request log filters by alias, fallback, endpoint, minimum cost, the model or alias name the app sent (`requested_model`), outcome, response cache, duration and error message; `GET …/logs/errors` lists the most common failures, which the console shows above the list when Failed is on, and an hour under Slowest hours opens the slow requests. ([#1222](https://github.com/logicplanes/kaeso/pull/1222), [#1224](https://github.com/logicplanes/kaeso/pull/1224), [#1225](https://github.com/logicplanes/kaeso/pull/1225), [#1231](https://github.com/logicplanes/kaeso/pull/1231))
- The request log offers client IP, alias, formats, response cache, fallbacks and upstream request columns, hidden at first, and the request inspector can copy a JSON request whose content and media were kept whole as a command that sends it again. ([#1226](https://github.com/logicplanes/kaeso/pull/1226), [#1311](https://github.com/logicplanes/kaeso/pull/1311))
- The request log's `cost_basis` says `none` for requests no provider billed and `estimated` when Kaeso estimated the tokens; entries logged before still say `reported`. ([#1312](https://github.com/logicplanes/kaeso/pull/1312))
- The request log names each request's client app from its user agent and filters by it, the usage timeline groups by `client`, and both also group by `alias`, `endpoint`, `status` and `response_cache`. ([#1234](https://github.com/logicplanes/kaeso/pull/1234), [#1344](https://github.com/logicplanes/kaeso/pull/1344))
- The log inspector links to the model, alias, key and sender it names and filters the list by each; a logged request in the Anthropic, Chat Completions, Responses or Gemini format opens in the Playground with its settings and last message, a catalogue model opens there from its sheet, and key, member, service account and model sheets open their requests and audit history. ([#1227](https://github.com/logicplanes/kaeso/pull/1227), [#1228](https://github.com/logicplanes/kaeso/pull/1228), [#1229](https://github.com/logicplanes/kaeso/pull/1229))
- The Kaeso plugin asks for the install's MCP address when it is installed in Claude Code, and for Claude on the web, desktop and mobile and for Cowork, Account › Agents has an Add to Claude button, and the plugin's skill a link, that opens Claude's custom connector form with the install's MCP address filled in. ([#1006](https://github.com/logicplanes/kaeso/pull/1006))
- Decision answers line their bars up under each question without repeating the answer, and the request log lists each question with what its answers mean. ([#1004](https://github.com/logicplanes/kaeso/pull/1004))

#### UI library

- The UI library gains `ProbabilityBar`, `DecisionResponse` and `QuestionEditor`. ([#976](https://github.com/logicplanes/kaeso/pull/976))

#### Self-hosting

- Kaeso installs on Kubernetes with a Helm chart that each release publishes; it runs the gateway and console hardened as Compose does, migrates once in a Helm hook, and offers an Ingress or HTTPRoute, a PodDisruptionBudget, an optional ServiceMonitor and bundled single stores for trying it out, and the new setting `updates.installed_with` makes the console's Updates page show the Helm update command on such an install. ([#1232](https://github.com/logicplanes/kaeso/pull/1232))
- `kaeso healthcheck --ready` asks `/readyz` from inside the gateway's container, prints its answer and exits 0 only on a 200, for platforms whose readiness check runs a command; plain `healthcheck` stays on `/healthz`. ([#1205](https://github.com/logicplanes/kaeso/pull/1205))
- Operators can print the accounts with `kaeso user list`, check the settings with `kaeso config check`, send a test email with `kaeso mail test` and fetch model data with `kaeso kpmr sync`; Sync now on the System page does the same through `POST /instance/kpmr/sync`. ([#1310](https://github.com/logicplanes/kaeso/pull/1310), [#1319](https://github.com/logicplanes/kaeso/pull/1319))

### Changed

- An install takes less memory, CPU and disk: ClickHouse writes only the diagnostics an operator reads, request bodies are compressed in 16–32 MiB blocks so agent sessions are stored several times smaller, and the gateway allocates with mimalloc without transparent huge pages. ([#1217](https://github.com/logicplanes/kaeso/pull/1217))
- Kaeso Vanguard brings the console, docs, UI library and KPMR into the design of the redesigned kaeso.app, with cut plates, hairline structure and flat surfaces; no prop, export or variant key was removed or renamed. ([#1252](https://github.com/logicplanes/kaeso/pull/1252))
- Rust dependencies are updated, among them `toml` 1.2.0, `sigstore` 0.15.0, `resvg` 0.50.0, `redis`, `jiff`, `flate2`, `uuid` and `rustls-pki-types`. ([#926](https://github.com/logicplanes/kaeso/pull/926), [#927](https://github.com/logicplanes/kaeso/pull/927), [#987](https://github.com/logicplanes/kaeso/pull/987), [#988](https://github.com/logicplanes/kaeso/pull/988), [#1002](https://github.com/logicplanes/kaeso/pull/1002), [#1399](https://github.com/logicplanes/kaeso/pull/1399), [#1400](https://github.com/logicplanes/kaeso/pull/1400))
- JavaScript dependencies are updated across the console, the documentation, the UI library and the MCP view packages. ([#929](https://github.com/logicplanes/kaeso/pull/929), [#1007](https://github.com/logicplanes/kaeso/pull/1007), [#1345](https://github.com/logicplanes/kaeso/pull/1345), [#1398](https://github.com/logicplanes/kaeso/pull/1398))

#### Gateway

- The gateway stops at startup when `kpmr.interval` is below 5 minutes, when `mcp_servers.sync_interval` is below a minute, or when `smtp.user` is set without `smtp.password`, where 0.6.0 accepted the first two and sent email without authentication; an empty password counts as unset, and an empty `KAESO_CONFIG` now counts as unset instead of stopping the gateway. ([#1120](https://github.com/logicplanes/kaeso/pull/1120), [#1121](https://github.com/logicplanes/kaeso/pull/1121))
- A request after a pause no longer waits for the database or an account renewal: caches stay warm while they are used, ChatGPT and Grok accounts are renewed ahead of time by a new `account_renewal` job, bodies are checked and decoded in fewer passes, and a fallback's `overhead_us` now leaves out the failed steps' wait for their providers. ([#1216](https://github.com/logicplanes/kaeso/pull/1216))
- `@modelcontextprotocol/ext-apps` is updated from 1.7.5 to 2.0.4 for the usage view the gateway serves. ([#930](https://github.com/logicplanes/kaeso/pull/930))
- A fresh install starts with KPMR's model data as of 20 November 2026, 08:44 UTC: 2,934 models from the 50 providers Kaeso calls, with no model removed or newly deprecated. ([#1393](https://github.com/logicplanes/kaeso/pull/1393))

#### KPP

- Large OpenAI Responses requests, such as a coding agent's resent conversation, decode in about three quarters of the time; a 3 MB body decoded in 10.2–10.7 ms against 13.2–14.2 ms before. ([#1249](https://github.com/logicplanes/kaeso/pull/1249))

#### UI library

- `astro` is updated to 7.4.7. ([#991](https://github.com/logicplanes/kaeso/pull/991))

#### Self-hosting

- Docker image tags are updated across the base images and deployment files, and the `hashicorp/google` OpenTofu provider of the GCP templates moves from 6.50.0 to 8.11.0. ([#923](https://github.com/logicplanes/kaeso/pull/923), [#985](https://github.com/logicplanes/kaeso/pull/985), [#986](https://github.com/logicplanes/kaeso/pull/986), [#1301](https://github.com/logicplanes/kaeso/pull/1301), [#1302](https://github.com/logicplanes/kaeso/pull/1302), [#1354](https://github.com/logicplanes/kaeso/pull/1354), [#1395](https://github.com/logicplanes/kaeso/pull/1395), [#1396](https://github.com/logicplanes/kaeso/pull/1396))
- An update from the console or the update window that changes `compose.yaml`, as every update from 0.6.0 does, first checks that the new gateway takes the install's settings; when it would refuse one at startup, the update stops before switching anything, the running release keeps serving, `compose.yaml` is put back and the update's detail names the setting, and the installer's `--update` does the same check. ([#1407](https://github.com/logicplanes/kaeso/pull/1407))

### Fixed

- Pages that render Kaeso UI on a server hydrate without an error when a table shows dates or relative times, a chart highlights a day, or a list starts long or with rows selected. ([#931](https://github.com/logicplanes/kaeso/pull/931))
- UI library formatters follow each page's language on a server through new `useFormatters()` and `createFormatters`; `StatusCode` shows a focus ring, wrapped `CodeBlock` lines break only at spaces, and dark-theme provider marks stay legible. ([#960](https://github.com/logicplanes/kaeso/pull/960))
- TypeSafe's image refusals (wrong format, too small, too large) are one sentence each, so the Playground shows their reason. ([#978](https://github.com/logicplanes/kaeso/pull/978))
- A lazily loaded component asks for its code only at a fresh address after a download fails in Safari or Firefox. ([#1010](https://github.com/logicplanes/kaeso/pull/1010))
- Lowering the install's maximum log retention brings organizations above it down to it, and entries logged before a retention was shortened expire by the shorter retention. ([#1268](https://github.com/logicplanes/kaeso/pull/1268), [#1286](https://github.com/logicplanes/kaeso/pull/1286))
- Emails work in more cases: a failed run of the monthly cost report or weekly summary is retried a minute later instead of skipping the month or week, with several gateways every emailed setup link works until setup is done, budgets with the period `total` send their alert emails, and unsubscribe links in emails sent before a master-key rotation keep working while the old key is configured. ([#1282](https://github.com/logicplanes/kaeso/pull/1282), [#1283](https://github.com/logicplanes/kaeso/pull/1283), [#1289](https://github.com/logicplanes/kaeso/pull/1289), [#1296](https://github.com/logicplanes/kaeso/pull/1296))
- Reading one of your own keys needs only `account.keys.view`, as listing them does. ([#1278](https://github.com/logicplanes/kaeso/pull/1278))
- SSO group rules match Microsoft Entra ID directory roles for users whose groups are read from Microsoft Graph. ([#1343](https://github.com/logicplanes/kaeso/pull/1343))
- A finished video or image generation shows the tags, client app, alias, model line and paying key of the request that started it in the request log and usage, so filtering by them counts its cost. ([#1421](https://github.com/logicplanes/kaeso/pull/1421))

#### Gateway

- Gemini apps reach Kaeso under `/v1` as well as `/v1beta`, so google-genai 3 works with its defaults, and `GET /v1/models`, `GET /v1/models/{model}` and `POST /v1/decisions` answer in Gemini's format when the request carries `x-goog-api-key` or `?key=` and no `Authorization` header. ([#971](https://github.com/logicplanes/kaeso/pull/971))
- Browser apps can send `Cache-Control` to the response cache; CORS preflights allow it. ([#1113](https://github.com/logicplanes/kaeso/pull/1113))
- Web searches that Anthropic and OpenAI bill per call are part of a request's cost from a `search` unit price in the catalogue; searches without a price log the cost as partly unknown. ([#1206](https://github.com/logicplanes/kaeso/pull/1206))
- Kaeso's own MCP server speaks protocol 2026-07-28, so newer clients connect without a failed first request: `/mcp` answers `server/discover`, `resources/read` carries `ttlMs` and `cacheScope`, an unknown resource answers `-32602`, and MCP Apps is declared as an extension. ([#1239](https://github.com/logicplanes/kaeso/pull/1239), [#1250](https://github.com/logicplanes/kaeso/pull/1250))
- A budget counter rebuilt while a request finishes counts that request exactly once. ([#1251](https://github.com/logicplanes/kaeso/pull/1251))
- Costs are billed more exactly: cached tokens of a model without a cache price cost its input price instead of nothing, OpenAI's 12-hour prompt-cache writes cost their 12-hour rate, and reasoning tokens cost the catalogue's reasoning price when it lists one. ([#1253](https://github.com/logicplanes/kaeso/pull/1253), [#1276](https://github.com/logicplanes/kaeso/pull/1276), [#1281](https://github.com/logicplanes/kaeso/pull/1281))
- Thinking and sampling settings match what models accept: DeepSeek models turn thinking off when an app turns it off, Claude Opus 4.7 and later leave out `top_p` and `top_k` as they do a temperature, and a thinking budget reaches Claude models that refuse budgets as adaptive thinking, each change named in `kaeso-adapted` or `kaeso-left-out`. ([#1258](https://github.com/logicplanes/kaeso/pull/1258), [#1383](https://github.com/logicplanes/kaeso/pull/1383), [#1385](https://github.com/logicplanes/kaeso/pull/1385))
- Gemini and OpenAI Images no longer count cached image and audio tokens again as uncached input. ([#1261](https://github.com/logicplanes/kaeso/pull/1261), [#1277](https://github.com/logicplanes/kaeso/pull/1277))
- Google's 403 for a file the request names reaches the app as Google's answer instead of a refused key, and Google's 402 that prepaid credits are depleted reaches apps as a billing refusal instead of a rate limit to retry. ([#1262](https://github.com/logicplanes/kaeso/pull/1262), [#1388](https://github.com/logicplanes/kaeso/pull/1388))
- Gemini apps get an object in `functionCall.args` when another format's model writes other tool arguments, and a streamed Gemini reply whose prompt Google blocks ends with a content-filter stop instead of a server error. ([#1263](https://github.com/logicplanes/kaeso/pull/1263), [#1264](https://github.com/logicplanes/kaeso/pull/1264))
- Request logs are written again after a ClickHouse migration instead of failing until the gateway restarts, and `/readyz` reports degraded while writes fail. ([#1267](https://github.com/logicplanes/kaeso/pull/1267))
- The MCP server no longer lists the Playground's tools, which always refused agents, the MCP usage view says 1 request for a row with one request, and agents that follow OAuth 2.1 finish signing in without sending `redirect_uri` to the token endpoint. ([#1269](https://github.com/logicplanes/kaeso/pull/1269), [#1270](https://github.com/logicplanes/kaeso/pull/1270), [#1275](https://github.com/logicplanes/kaeso/pull/1275))
- A model request for a provider with no key in the project answers 502 like a refused key, not 400, and a provider's 404 about a file the request names no longer holds an alias route's model for 5 minutes. ([#1272](https://github.com/logicplanes/kaeso/pull/1272), [#1273](https://github.com/logicplanes/kaeso/pull/1273))
- Audit log, member, user and usage filters with a NUL character answer 400 naming the filter instead of 500 or 503. ([#1274](https://github.com/logicplanes/kaeso/pull/1274))
- A streamed reply that fails before any output falls back to the alias's next step, and a direct request answers with the error's status instead of a 200 stream with an error event, including an OpenAI Responses stream that sends a bare `error` event and ends. ([#1279](https://github.com/logicplanes/kaeso/pull/1279), [#1378](https://github.com/logicplanes/kaeso/pull/1378))
- A ChatGPT account at its plan's usage limit keeps every model of the account out of routes for at least 5 minutes, or as long as the provider says, up to an hour. ([#1280](https://github.com/logicplanes/kaeso/pull/1280))
- The usage timeline counts a finished video or ElevenLabs image once and without a 0 ms request. ([#1288](https://github.com/logicplanes/kaeso/pull/1288))
- Groq json and text transcriptions are priced by Groq's own length, and audio sent as a link shows an unknown cost instead of $0. ([#1291](https://github.com/logicplanes/kaeso/pull/1291))
- A request body over the size limit gets Kaeso's 413 in the app's format behind a proxy such as Traefik instead of the proxy's 502, and so does an answer Kaeso sends before it has read a large body, such as for an invalid key, a wrong method or a body over the MCP limit; HTTP/2 is left alone, and a client that sends the body without waiting for `100 Continue` can still get the proxy's 502. ([#1387](https://github.com/logicplanes/kaeso/pull/1387), [#1409](https://github.com/logicplanes/kaeso/pull/1409))
- The request log shows logged JSON with its keys in the order the app sent them, so a decision's answers sit under their own questions and options. ([#1000](https://github.com/logicplanes/kaeso/pull/1000))
- The README names every built-in provider, and a test keeps the list in step. ([#1122](https://github.com/logicplanes/kaeso/pull/1122))
- `kaeso instance admin-sso` and `kaeso sso verify-domain` send the email the same change sends from the console. ([#1257](https://github.com/logicplanes/kaeso/pull/1257))

#### KPP

- Gemini no longer bills cached image and audio tokens again as uncached input. ([#994](https://github.com/logicplanes/kaeso/pull/994))
- A Gemini token count with a field another format lacks is refused under the name the app sent. ([#1112](https://github.com/logicplanes/kaeso/pull/1112))
- OpenAI Videos' checked decoder refuses the request kinds its plain decoder refuses. ([#1123](https://github.com/logicplanes/kaeso/pull/1123))
- A DashScope or Gemini Interactions video job with an out-of-range time reads it as no time instead of panicking. ([#1298](https://github.com/logicplanes/kaeso/pull/1298))
- An image's `detail` reaches the other OpenAI format: Chat's `image_url.detail` becomes Responses' `detail` and back. ([#1265](https://github.com/logicplanes/kaeso/pull/1265))
- System One refuses a score level of null as it comes in instead of accepting a request it can't send, and OpenAI's Decisions API confidence follows TypeSafe's choice and score formulas. ([#1287](https://github.com/logicplanes/kaeso/pull/1287))

#### Console

- The request log's format filters list every format Kaeso logs, from the list they accept. ([#1221](https://github.com/logicplanes/kaeso/pull/1221))
- The Playground keeps models whose id starts with `~` or `@`, such as OpenRouter's `~…-latest` and Cloudflare's `@cf/…` models. ([#1114](https://github.com/logicplanes/kaeso/pull/1114))
- Clearing a project's response cache shows its success message instead of reporting an empty answer. ([#1256](https://github.com/logicplanes/kaeso/pull/1256))
- The month forecast takes the last seven days' pace in the month's first week too. ([#1271](https://github.com/logicplanes/kaeso/pull/1271))
- Analytics' links to the request log open the UTC days Analytics counted. ([#1299](https://github.com/logicplanes/kaeso/pull/1299))
- Updating to a release that is no longer the newest shows the newer one on Instance › Updates instead of asking for a check the console can't do. ([#1297](https://github.com/logicplanes/kaeso/pull/1297))
- The key expiry picker offers no day whose start in UTC has passed, and a refused date names `expires_on`. ([#1259](https://github.com/logicplanes/kaeso/pull/1259))
- Guardrails has its own icon in the settings navigation instead of Security's. ([#1367](https://github.com/logicplanes/kaeso/pull/1367))
- Organization pages and settings fit their content at every width and language: the SCIM address shows once, the agent preset fits a phone, webhook URLs and MCP handles end in an ellipsis, and role descriptions get a text area. ([#1368](https://github.com/logicplanes/kaeso/pull/1368))
- Owners and admins who reach a project through their organization role, not as members, can choose which models their keys may call from a key's sheet, project pages name people from the organization who aren't members of the project instead of showing their user id, and a tool's sheet no longer sets its MCP server's name in a code font. ([#1413](https://github.com/logicplanes/kaeso/pull/1413), [#1415](https://github.com/logicplanes/kaeso/pull/1415), [#1417](https://github.com/logicplanes/kaeso/pull/1417))
- Project pages fit and line up at every width and language: the alias editor keeps Save on a phone, header actions wrap, tables, dialogs and sheets fit phones and tablets, values end on their column's edge, and touch targets are finger-sized. ([#1373](https://github.com/logicplanes/kaeso/pull/1373), [#1386](https://github.com/logicplanes/kaeso/pull/1386), [#1397](https://github.com/logicplanes/kaeso/pull/1397))
- Analytics fits phones and tablets: charts answer a tap, grouping is a menu on a phone, Requests stays beside the cost, and the custom period's dates show whole. ([#1401](https://github.com/logicplanes/kaeso/pull/1401))
- Organization, account and instance settings fit phones and tablets and line up at every width and language: Providers no longer scrolls sideways, sheet actions stay in reach, popups, filters and tables stay on screen, and switches and section actions keep their place. ([#1375](https://github.com/logicplanes/kaeso/pull/1375), [#1382](https://github.com/logicplanes/kaeso/pull/1382), [#1384](https://github.com/logicplanes/kaeso/pull/1384), [#1402](https://github.com/logicplanes/kaeso/pull/1402), [#1414](https://github.com/logicplanes/kaeso/pull/1414))
- Sign-in, invitations, shared links and the shell fit long addresses and names on a phone: the console no longer scrolls sideways, phone navigation gives way to dialogs, and the links under sign-in forms are big enough to tap. ([#1374](https://github.com/logicplanes/kaeso/pull/1374), [#1391](https://github.com/logicplanes/kaeso/pull/1391), [#1403](https://github.com/logicplanes/kaeso/pull/1403))
- Empty states line up with the page and take a smaller title, limits with large amounts fit their row, and the guardrail rules' sample text is a full text area again. ([#1369](https://github.com/logicplanes/kaeso/pull/1369), [#1370](https://github.com/logicplanes/kaeso/pull/1370), [#1371](https://github.com/logicplanes/kaeso/pull/1371))
- In the request logs, Tab always reaches the filters after the Models and Tools switch, and switching with the arrow keys keeps the focus on the switch. ([#1410](https://github.com/logicplanes/kaeso/pull/1410))
- Bar charts and chart tooltips set names such as `gpt-5-mini` in proportional figures, page header tabs stay whole beside a long title, and a word alone on a line of wrapped code stays whole. ([#1418](https://github.com/logicplanes/kaeso/pull/1418), [#1419](https://github.com/logicplanes/kaeso/pull/1419), [#1422](https://github.com/logicplanes/kaeso/pull/1422))

#### UI library

- Stat tiles that load their numbers show them at full size on phones instead of the smallest size. ([#1406](https://github.com/logicplanes/kaeso/pull/1406))
- Tables whose rows have a right-click menu take one Tab stop instead of one per row, and screen readers no longer meet focus stops between the rows. ([#1408](https://github.com/logicplanes/kaeso/pull/1408))
- On touch screens, the model and provider pickers answer a finger-sized tap around them, like every select. ([#1412](https://github.com/logicplanes/kaeso/pull/1412))
- Long tables and lists such as the request logs stay where you scrolled when the next page of rows loads, instead of jumping back near the top. ([#1416](https://github.com/logicplanes/kaeso/pull/1416))

#### Documentation

- The Platform API page and the tool-call route no longer say the Playground runs tools, the refusal for a caller without a person says who may call tools, and the configuration page says `updates.check` is read on every check and that background jobs can hold up to 11 connections outside the pool. ([#1115](https://github.com/logicplanes/kaeso/pull/1115), [#1119](https://github.com/logicplanes/kaeso/pull/1119))
- The documentation describes what is new in 0.7.0 and what to check before upgrading to it, and its tables fit a phone. ([#1392](https://github.com/logicplanes/kaeso/pull/1392))
- The KPP docs record that OpenAI's Decisions API confidence follows TypeSafe's choice and score formulas. ([#1338](https://github.com/logicplanes/kaeso/pull/1338))
- Supported providers names the regional endpoints of QwenCloud, xAI and ElevenLabs, and the configuration page says a provider pointed at another region keeps its default region's prices. ([#1358](https://github.com/logicplanes/kaeso/pull/1358), [#1359](https://github.com/logicplanes/kaeso/pull/1359))

#### Self-hosting

- With `--domain :80` the installer writes `KAESO_PUBLIC_URL` and `KAESO_SMTP_FROM` for a port, from the new `--public-url` or, without it, a local address with the port, so the install it writes starts, and it repairs an install it wrote before. ([#1266](https://github.com/logicplanes/kaeso/pull/1266))
- An installer update that the new release refuses leaves `compose.yaml.previous` as it was, instead of a copy of the running `compose.yaml`. ([#1420](https://github.com/logicplanes/kaeso/pull/1420))

### Security

- A key with platform access can no longer widen its own model list, Tools list or limits, or leave another key looser than itself; such changes are refused with 403 `permission_denied` and need a console session. ([#920](https://github.com/logicplanes/kaeso/pull/920))

#### Gateway

- Email goes out through lettre 0.11.24, which fixes six SMTP advisories, RUSTSEC-2026-0560 to RUSTSEC-2026-0565, among them replies a man-in-the-middle could inject after STARTTLS. ([#1346](https://github.com/logicplanes/kaeso/pull/1346))

## [0.6.0] - 2026-11-10

This release makes Kaeso one gateway for every kind of model, not only chat: images, video, speech, transcripts, music and sound effects, Gemini-format apps, thirteen new built-in providers, alias routes with fallbacks and splits, and an MCP portal per project. The updater now installs releases that change `compose.yaml`. These notes cover everything since the last stable release, the release candidates included. Read the breaking changes before you upgrade: installs that update from the console need a one-time installer run, and installs behind their own reverse proxy need new routes.

- **Breaking:** An install of any earlier release, the release candidates included, has an updater that speaks protocol 1, so Instance › Updates calls it outdated: run the installer once with `--update` in the install's folder, and Update in the console works again from then on. The installer replaces `compose.yaml` only when its hash matches the release's, keeps the old file and turns the updater on if it was off; the Upgrades page of the documentation has the command. ([#769](https://github.com/logicplanes/kaeso/pull/769), [#780](https://github.com/logicplanes/kaeso/pull/780))
- **Breaking:** A reverse proxy of your own in front of Kaeso must also send `/v1beta/` (the Gemini API, with response buffering off) and `/v2/` (ElevenLabs' voice listing) to the gateway, and pass WebSocket upgrades on for ElevenLabs' realtime sessions. The bundled Caddy and the platform templates already do; the Domains and TLS page lists every path. ([#829](https://github.com/logicplanes/kaeso/pull/829), [#834](https://github.com/logicplanes/kaeso/pull/834))
- **Breaking:** Projects in an organization are all alike: there is no default project, Kaeso opens on the organization's overview, and new members join only the projects their invitation names. ([#647](https://github.com/logicplanes/kaeso/pull/647))
- **Breaking:** Kaeso no longer reads the `models_dev` settings: rename a `[models_dev]` section to `[kpmr]` and `KAESO_MODELS_DEV_SYNC` to `KAESO_KPMR_SYNC` before you upgrade, because a leftover `[models_dev]` section stops the gateway. ([#710](https://github.com/logicplanes/kaeso/pull/710))
- **Breaking:** The request log keeps short timings in microseconds, so the Platform API and the CSV export rename `request_upload_ms`, `overhead_ms` and `stream_progress.processing_ms` to `request_upload_us`, `overhead_us` and `stream_progress.processing_us`, and the CSV's `stream_processing_ms` to `stream_processing_us`, with no alias for the old names. Code that reads them divides by 1,000 for milliseconds; `duration_ms`, `ttfb_ms` and `stream_progress.first_byte_ms` stay in milliseconds. ([#868](https://github.com/logicplanes/kaeso/pull/868))
- **Breaking:** The UI library exports only its documented components and folds many of them into variants: aliases and internal parts are no longer public, StatusDot is now Status, and IconButton, SearchInput, SensitiveInput, FilterToggle, Gauge, ParameterPopover, DeleteResource, PromptSuggestions and the components that only wrapped another one are removed; ResponseMetrics no longer takes `reasoning`. ([#664](https://github.com/logicplanes/kaeso/pull/664), [#665](https://github.com/logicplanes/kaeso/pull/665), [#666](https://github.com/logicplanes/kaeso/pull/666), [#667](https://github.com/logicplanes/kaeso/pull/667), [#668](https://github.com/logicplanes/kaeso/pull/668), [#669](https://github.com/logicplanes/kaeso/pull/669), [#672](https://github.com/logicplanes/kaeso/pull/672), [#760](https://github.com/logicplanes/kaeso/pull/760), [#893](https://github.com/logicplanes/kaeso/pull/893))
- **Breaking:** In `kaeso-kpp`, `Base64Source::data` holds the decoded bytes as `bytes::Bytes` instead of base64 text, so code that builds or reads media sources changes; KPP's JSON form is unchanged. ([#849](https://github.com/logicplanes/kaeso/pull/849))

**Contributors:** @dvnold

**Full changes:** [v0.5.6...v0.6.0](https://github.com/logicplanes/kaeso/compare/v0.5.6...v0.6.0)

### Added

- Aliases route requests: fallbacks, conditions and weighted splits by request, caller or header, built in a full-page route editor with templates and a Test tab that sends a request through the draft and colors each step by its answer. A request's trace shows the path it took, and the `kaeso-route` header names every step it tried. ([#649](https://github.com/logicplanes/kaeso/pull/649), [#650](https://github.com/logicplanes/kaeso/pull/650))
- Every project has an MCP portal at `/mcp/{project}` that serves its enabled tools to MCP clients, through an API key with the `tools` ability or a client that signs in with a Client ID Metadata Document and the person's consent. Portal tokens work only at their own portal, disconnecting a client revokes its tokens at once, and held, switched-off or denied tools are refused. ([#690](https://github.com/logicplanes/kaeso/pull/690), [#691](https://github.com/logicplanes/kaeso/pull/691))
- Organizations connect remote MCP servers over Streamable HTTP, signed in with a sealed secret header or OAuth with PKCE, and Kaeso reads and writes MCP messages of every protocol version from `2025-03-26` to `2026-07-28`. Private networks are reached only when `mcp_servers.allowed_networks` lists them, and secrets and tokens are sealed by the master key. ([#682](https://github.com/logicplanes/kaeso/pull/682), [#683](https://github.com/logicplanes/kaeso/pull/683), [#686](https://github.com/logicplanes/kaeso/pull/686), [#687](https://github.com/logicplanes/kaeso/pull/687))
- Kaeso keeps each MCP tool's definition and syncs it on `mcp_servers.sync_interval`; with the default `hold` policy a changed tool is hidden and refused until someone approves the new definition, `accept` approves changes on its own, and rejecting a tool switches it off in every project. ([#688](https://github.com/logicplanes/kaeso/pull/688))
- New `mcp_servers.*` and `tools.*` rights, in every role and agent preset, govern server management, tool review, access and use. The Platform API manages MCP servers, project tools, per-principal access and connected portal clients, with an audit entry for every change, and its Playground routes call a project's tool with `POST …/tools/{tool}/call` under the portal's approval, access and rate rules. ([#685](https://github.com/logicplanes/kaeso/pull/685), [#689](https://github.com/logicplanes/kaeso/pull/689), [#696](https://github.com/logicplanes/kaeso/pull/696))
- Organization › MCP servers adds servers, probes their address, signs in and reviews definition changes with a diff. Project › Tools turns servers and tools on, one by one or a whole server at once, sets who may call them, connects or disconnects clients and sorts servers by name, calls, last use or tools on, and the consent page sums up a client's tools per server. ([#693](https://github.com/logicplanes/kaeso/pull/693), [#694](https://github.com/logicplanes/kaeso/pull/694), [#738](https://github.com/logicplanes/kaeso/pull/738), [#745](https://github.com/logicplanes/kaeso/pull/745))
- Servers that need a personal account work through Account › Connections: each person connects their own, the server's sync uses the latest working connection, and leaving an organization removes that person's connections and portal grants. ([#695](https://github.com/logicplanes/kaeso/pull/695))
- Tool calls appear in the request logs, usage and the project overview, and a `tool_call_rate` limit caps them per sliding minute for a project, member or key. The Tools tab of the request logs has live updates, every filter the Models tab has and an export to CSV or JSON Lines with each call's arguments and results. ([#692](https://github.com/logicplanes/kaeso/pull/692), [#844](https://github.com/logicplanes/kaeso/pull/844), [#870](https://github.com/logicplanes/kaeso/pull/870))
- Kaeso emails the people responsible when something changes in an instance, organization, project or portal. Account › Notifications offers now, daily or off for each email a person's rights allow, a switch for the weekly summary and the monthly cost report, a mute per organization, a test email and a signed unsubscribe link, while security emails stay on. ([#699](https://github.com/logicplanes/kaeso/pull/699), [#700](https://github.com/logicplanes/kaeso/pull/700), [#746](https://github.com/logicplanes/kaeso/pull/746))
- Every email names its organization and project, alerts name the provider, key, budget and answer behind them, announcements open with a cover and reports with their figures and tables, and the footer names the right a person holds as the console does. ([#756](https://github.com/logicplanes/kaeso/pull/756), [#761](https://github.com/logicplanes/kaeso/pull/761), [#764](https://github.com/logicplanes/kaeso/pull/764))
- An install can run as a public, read-only demo: `KAESO_AUTH_DEMO_USER`, under the gateway's environment in `compose.override.yaml`, signs every visitor in as one user and refuses every change. Every console page then says it is read-only with sample data, each visitor sees only their own session, and `models/offered` is refused before any provider is asked. ([#670](https://github.com/logicplanes/kaeso/pull/670), [#718](https://github.com/logicplanes/kaeso/pull/718), [#719](https://github.com/logicplanes/kaeso/pull/719))
- Star on GitHub links to Kaeso's repository from the documentation's sidebar and from Instance › Updates, with the star count once it reaches 100. ([#675](https://github.com/logicplanes/kaeso/pull/675))

#### Gateway

- Apps generate and edit images through OpenAI's Images API, with uploads and streamed partial images, on OpenAI, SpaceX, Google, OpenRouter, Vercel, Together AI, QwenCloud, Zhipu AI, MiniMax and ElevenLabs models, including QwenCloud's Wan and Z-Image models and ElevenLabs' GPT Image, Gemini and Seedream models. Keys, aliases, limits, budgets, logs and costs work as for every other model request. ([#799](https://github.com/logicplanes/kaeso/pull/799), [#801](https://github.com/logicplanes/kaeso/pull/801), [#835](https://github.com/logicplanes/kaeso/pull/835), [#859](https://github.com/logicplanes/kaeso/pull/859))
- Apps make speech and transcribe or translate audio through OpenAI's Audio API on every built-in provider that offers it, with speech streamed as events when asked, and sound effects reach QwenCloud's Qwen-Audio-TTS-Next. ([#806](https://github.com/logicplanes/kaeso/pull/806), [#807](https://github.com/logicplanes/kaeso/pull/807), [#857](https://github.com/logicplanes/kaeso/pull/857))
- Apps make videos through OpenAI's Videos API on SpaceX, Google AI Studio, Gemini Enterprise Agent Platform, MiniMax, Zhipu AI, OpenRouter, Together AI, QwenCloud and ElevenLabs. Kaeso keeps each job under an id of its own, asks the provider while it runs, streams the finished file and bills the job once, when it ends. ([#815](https://github.com/logicplanes/kaeso/pull/815), [#816](https://github.com/logicplanes/kaeso/pull/816), [#818](https://github.com/logicplanes/kaeso/pull/818), [#822](https://github.com/logicplanes/kaeso/pull/822))
- QwenCloud's image, speech, transcription and video models that answer only on DashScope are reached in DashScope's own API, with images and audio fetched from its result links. ([#820](https://github.com/logicplanes/kaeso/pull/820), [#821](https://github.com/logicplanes/kaeso/pull/821))
- Apps send audio and video to every provider that takes them, from any API format, Chat and Gemini apps get audio replies from each other's models, and images that Gemini models make reach Gemini apps. A format or provider with no place for them refuses before anything is sent. ([#791](https://github.com/logicplanes/kaeso/pull/791), [#794](https://github.com/logicplanes/kaeso/pull/794))
- Gemini-format apps, such as the google-genai SDKs or plain Gemini REST calls, use Kaeso as their base URL under `/v1beta/` and call any model of the project, with streams, thinking, tool calls, token counts, embeddings and model listings. ([#783](https://github.com/logicplanes/kaeso/pull/783), [#787](https://github.com/logicplanes/kaeso/pull/787))
- Zhipu AI's GLM platform (`zai`) and MiniMax are built-in providers: add a key and call GLM and MiniMax-M3 models from any app format. Fields MiniMax documents but ignores are refused before sending instead of dropped silently. ([#781](https://github.com/logicplanes/kaeso/pull/781), [#782](https://github.com/logicplanes/kaeso/pull/782))
- Google AI Studio and Gemini Enterprise Agent Platform are built-in providers. AI Studio takes an API key; Agent Platform takes an express mode API key or a service account key, which also reaches its Claude, partner and open models. Kaeso calls Gemini in its own format, so thought signatures survive tool loops in every app format. ([#786](https://github.com/logicplanes/kaeso/pull/786), [#789](https://github.com/logicplanes/kaeso/pull/789), [#790](https://github.com/logicplanes/kaeso/pull/790))
- ElevenLabs is a built-in provider. OpenAI-format apps reach its text to speech and Scribe through the Audio API, and apps built with the official ElevenLabs SDKs call its speech, voice changer, sound effects, music, transcription and image endpoints with a Kaeso key and open its realtime speech and transcription sessions, with Kaeso's limits, logs and costs. ([#808](https://github.com/logicplanes/kaeso/pull/808), [#837](https://github.com/logicplanes/kaeso/pull/837), [#901](https://github.com/logicplanes/kaeso/pull/901))
- Fireworks AI, Nebius Token Factory, SambaNova and Scaleway are built-in providers: add a key and call their models from any app format, with embeddings where they offer them and Scaleway's transcriptions. ([#871](https://github.com/logicplanes/kaeso/pull/871), [#872](https://github.com/logicplanes/kaeso/pull/872), [#873](https://github.com/logicplanes/kaeso/pull/873), [#874](https://github.com/logicplanes/kaeso/pull/874))
- Hugging Face Inference Providers, NVIDIA's hosted NIM API, Baseten and SiliconFlow are built-in providers: add a key and call their models from every app format, with NVIDIA's and SiliconFlow's embeddings and SiliconFlow's speech and transcriptions. ([#896](https://github.com/logicplanes/kaeso/pull/896), [#897](https://github.com/logicplanes/kaeso/pull/897), [#898](https://github.com/logicplanes/kaeso/pull/898), [#899](https://github.com/logicplanes/kaeso/pull/899))
- Custom providers name the server software they run, one of fifteen such as vLLM, SGLang, Ollama, LM Studio, llama.cpp, LocalAI, Text Generation Inference, NVIDIA NIM or Docker Model Runner, so Kaeso sends each one what it expects. Models added from such a provider start with the context window the server lists and prices of 0; a provider with no named server software still gets no prices. ([#804](https://github.com/logicplanes/kaeso/pull/804), [#812](https://github.com/logicplanes/kaeso/pull/812))
- Custom providers serve OpenAI's images, speech, transcription and videos APIs once an admin turns each one on; a new provider for LocalAI, Xinference or vLLM starts with the APIs that software documents, and an API that is off is refused before sending. ([#838](https://github.com/logicplanes/kaeso/pull/838))
- Organizations and projects can keep the images, audio and files of logged requests, up to 10 MiB per request, with Keep media beside content logging, and the request inspector shows, plays and downloads them. It is off by default, and the organization's setting is the ceiling for its projects. ([#883](https://github.com/logicplanes/kaeso/pull/883))
- Usage and its timeline filter by model, provider, key, member, route and project, the timeline groups by them and reports latency, and the request log finds slow requests, so agents connected over MCP can analyse usage in depth. ([#763](https://github.com/logicplanes/kaeso/pull/763))
- Long prompts are billed at the model's long-context prices from KPMR, and a price override that sets a token price without its own tiers keeps one flat price for every prompt length. ([#798](https://github.com/logicplanes/kaeso/pull/798))

#### Console

- Overview pages become homes, with the release the install runs, activity, spend, projects, members, keys, MCP servers and recent changes, and a new Analytics page shows usage by the minute and hour, per project, model, key and member. It leads with the period's totals and one readable chart, then what drives usage, reliability, efficiency and spend. ([#762](https://github.com/logicplanes/kaeso/pull/762), [#886](https://github.com/logicplanes/kaeso/pull/886))
- The Playground tries every kind of model, side by side from one prompt: chat, images, speech, sound effects, music, transcription with recorded or attached audio, and video with progress and Cancel. Chat takes images and PDFs by attaching, dropping or pasting them, and the Platform API sends each of these requests for a member. ([#877](https://github.com/logicplanes/kaeso/pull/877), [#878](https://github.com/logicplanes/kaeso/pull/878), [#879](https://github.com/logicplanes/kaeso/pull/879))
- The console shows what media requests made, such as images, characters spoken and audio or video length, and calls a cost without a price Unknown instead of $0. ([#842](https://github.com/logicplanes/kaeso/pull/842), [#863](https://github.com/logicplanes/kaeso/pull/863))

#### UI library

- FlowCanvas and FlowWorkspace draw and edit routes as trees, with fallback chains, traced paths, badges and the answer of each tried step. ([#648](https://github.com/logicplanes/kaeso/pull/648))
- Custom providers that run vLLM, Ollama, LM Studio, llama.cpp, Text Generation Inference, NVIDIA NIM, Docker Model Runner, Xinference, SGLang, LMDeploy, LocalAI, MLX LM or Jan show that software's logo, and the server picker shows every name in full. ([#811](https://github.com/logicplanes/kaeso/pull/811), [#869](https://github.com/logicplanes/kaeso/pull/869))

#### Documentation

- A new APIs by provider page shows which model APIs reach each of the built-in providers, from chat and embeddings to images, speech, transcription, video, music and sound effects, and says why where one is refused. ([#840](https://github.com/logicplanes/kaeso/pull/840))
- The MCP portal has its own concept page, guides for servers, tools and client sign-in and a portal API reference, in English, German and Chinese. ([#697](https://github.com/logicplanes/kaeso/pull/697))
- The `bootstrap` script takes a fresh clone to a seeded development stack in one command and says how to fix what's missing, and the seed reads like a real organization's history. ([#720](https://github.com/logicplanes/kaeso/pull/720), [#768](https://github.com/logicplanes/kaeso/pull/768))

#### Self-hosting

- Kaeso installs with one installer command that downloads the newest release, writes its settings and master key and starts it. The installer starts a stopped Docker, checks the domain's DNS record and certificate, opens the setup link in your browser and turns the updater on, so Update in the console works from the first start. ([#662](https://github.com/logicplanes/kaeso/pull/662), [#765](https://github.com/logicplanes/kaeso/pull/765), [#767](https://github.com/logicplanes/kaeso/pull/767))
- The updater installs releases that change `compose.yaml` and then replaces itself and Caddy, so Update and automatic updates install every release but a new major version or one that changes `docker-proxy`; for those, the console shows the one installer command with `--update` that does it. ([#769](https://github.com/logicplanes/kaeso/pull/769), [#780](https://github.com/logicplanes/kaeso/pull/780), [#793](https://github.com/logicplanes/kaeso/pull/793))
- `GATEWAY_REPLICAS` runs several gateways behind Caddy in the Compose package, the updater replaces every replica, and a page says what gateways share. ([#727](https://github.com/logicplanes/kaeso/pull/727))
- The gateway connects to Valkey over TLS, with `KAESO_VALKEY_CA` for a server whose authority is private. ([#740](https://github.com/logicplanes/kaeso/pull/740))
- `KAESO_SETUP_EMAIL` emails a one-time setup link at every start until the first user exists, for installs without a shell; it needs SMTP and never writes the link to the logs. ([#723](https://github.com/logicplanes/kaeso/pull/723))
- The console image carries `kaeso-edge`, a public edge for platforms that terminate TLS themselves and run the gateway and console as private services. ([#724](https://github.com/logicplanes/kaeso/pull/724))
- Kaeso deploys from buttons in the README to Render, Railway, DigitalOcean, AWS, Google Cloud and Azure, and has templates and guides for Fly, Coolify, Dokploy and CapRover. Every template pins Kaeso's images to its release, the AWS templates are published with every release, the AWS quick server keeps its master key readable, and each guide says which paths were tested live. ([#725](https://github.com/logicplanes/kaeso/pull/725), [#741](https://github.com/logicplanes/kaeso/pull/741), [#751](https://github.com/logicplanes/kaeso/pull/751), [#752](https://github.com/logicplanes/kaeso/pull/752), [#754](https://github.com/logicplanes/kaeso/pull/754), [#814](https://github.com/logicplanes/kaeso/pull/814), [#819](https://github.com/logicplanes/kaeso/pull/819), [#885](https://github.com/logicplanes/kaeso/pull/885))

### Changed

- The Claude Code plugin adds only Kaeso's skill, which connects your install's MCP server when it isn't connected yet, so the plugin no longer asks for your install's address; the Cursor plugin keeps its MCP configuration in a folder of its own. ([#903](https://github.com/logicplanes/kaeso/pull/903))
- Dependencies are updated, among them `resvg` 0.49.0 and `ed25519-dalek` 3.0.0, with minor and patch updates of the Rust crates, the console, the documentation and the UI packages, and newer ClickHouse, Valkey, socket-proxy and distroless base images; the OSV scanner no longer ignores an advisory that matches nothing. ([#701](https://github.com/logicplanes/kaeso/pull/701), [#702](https://github.com/logicplanes/kaeso/pull/702), [#705](https://github.com/logicplanes/kaeso/pull/705), [#706](https://github.com/logicplanes/kaeso/pull/706), [#707](https://github.com/logicplanes/kaeso/pull/707), [#708](https://github.com/logicplanes/kaeso/pull/708), [#729](https://github.com/logicplanes/kaeso/pull/729))

#### Gateway

- Alias routes hold a step after a lasting failure, such as a refused key, an empty balance or a rate limit, and skip it while a later step is ready when the step's `fall_back_on` includes the reason; every gateway of the install shares which steps are held. A route with a single step no longer asks Valkey, and skipped steps show in routes, logs, exports and metrics. ([#717](https://github.com/logicplanes/kaeso/pull/717), [#722](https://github.com/logicplanes/kaeso/pull/722), [#890](https://github.com/logicplanes/kaeso/pull/890))
- Kaeso adapts what a model can't take instead of refusing the request: turning thinking off on a model that always reasons sends its lowest effort, an effort the model doesn't list becomes the nearest one it does, adaptive thinking becomes a thinking budget on Claude models without it, and a temperature a model can't take is left out, each named in `kaeso-adapted` or `kaeso-left-out`. An effort Kaeso doesn't know is still refused. ([#712](https://github.com/logicplanes/kaeso/pull/712), [#759](https://github.com/logicplanes/kaeso/pull/759))
- When a provider answers 403, apps hear that the provider key may not make this request, for example for this model, plan or region, instead of that the key was refused; a 401 keeps the refused-key message. Apps in another format get 404 not found for a model the provider retired with a 410, where Anthropic- and Gemini-format apps got a server error; the provider's own format still gets its 410, and the model turns Unavailable in the catalogue when the provider no longer lists it. ([#828](https://github.com/logicplanes/kaeso/pull/828), [#847](https://github.com/logicplanes/kaeso/pull/847))
- Request logs no longer keep the images, audio and files inside chat requests, replies and tool calls, and show each as its type and size instead; Keep media keeps them where an organization wants them. ([#882](https://github.com/logicplanes/kaeso/pull/882))
- Requests no longer queue on shared locks for request-log flushes, usage rows or request ids, carry a 12 KB future instead of 33.5 KB, load a cold project's settings, models and limits at the same time while the body arrives, and never wait for the database for provider credentials, which refresh in the background. ([#770](https://github.com/logicplanes/kaeso/pull/770), [#771](https://github.com/logicplanes/kaeso/pull/771), [#772](https://github.com/logicplanes/kaeso/pull/772), [#775](https://github.com/logicplanes/kaeso/pull/775), [#891](https://github.com/logicplanes/kaeso/pull/891), [#892](https://github.com/logicplanes/kaeso/pull/892))
- Media uploads are held once in memory, so a 25 MiB transcription upload raises the gateway's peak memory by twice its size instead of six times, and a chat request with a large inline image takes about 1.75 times its size instead of up to 7.8 times. ([#849](https://github.com/logicplanes/kaeso/pull/849), [#895](https://github.com/logicplanes/kaeso/pull/895))
- A fresh install starts with a newer KPMR snapshot of more than 1,400 models, including Claude Fable 5.2, Liquid LFM 3, long-context price tiers and the image, speech, transcription and video models of the providers Kaeso calls. ([#652](https://github.com/logicplanes/kaeso/pull/652), [#657](https://github.com/logicplanes/kaeso/pull/657), [#674](https://github.com/logicplanes/kaeso/pull/674), [#709](https://github.com/logicplanes/kaeso/pull/709), [#737](https://github.com/logicplanes/kaeso/pull/737), [#748](https://github.com/logicplanes/kaeso/pull/748), [#755](https://github.com/logicplanes/kaeso/pull/755), [#800](https://github.com/logicplanes/kaeso/pull/800), [#825](https://github.com/logicplanes/kaeso/pull/825), [#858](https://github.com/logicplanes/kaeso/pull/858), [#900](https://github.com/logicplanes/kaeso/pull/900))

#### KPP

- Kaeso checks and decodes large agent requests much faster, about 35% and then another 26%, so a 200 KB Claude Code request takes under a millisecond, and checks them 25% faster before decoding. ([#777](https://github.com/logicplanes/kaeso/pull/777), [#778](https://github.com/logicplanes/kaeso/pull/778), [#779](https://github.com/logicplanes/kaeso/pull/779))
- Kaeso decodes OpenAI Responses replies about 18% faster and encodes them about 12% faster, translates their streams about 6% faster per event, and adapts Codex requests with MCP tools about 30% faster and those with union schemas a little faster. ([#774](https://github.com/logicplanes/kaeso/pull/774), [#776](https://github.com/logicplanes/kaeso/pull/776), [#784](https://github.com/logicplanes/kaeso/pull/784), [#785](https://github.com/logicplanes/kaeso/pull/785), [#796](https://github.com/logicplanes/kaeso/pull/796))

#### Console

- The Playground is a calm chat, with a compact model picker, a modern composer, messages in bubbles and metrics that show only what is known, and it no longer draws a caret before the first token; it doesn't offer tools, which are called through the MCP portal. ([#760](https://github.com/logicplanes/kaeso/pull/760), [#864](https://github.com/logicplanes/kaeso/pull/864), [#893](https://github.com/logicplanes/kaeso/pull/893))
- The request log shows Kaeso's overhead, request upload and stream processing under 1 ms in microseconds. ([#868](https://github.com/logicplanes/kaeso/pull/868))
- Creating a key offers Inference, Platform and Tools as three checkboxes in one box and needs at least one API. ([#735](https://github.com/logicplanes/kaeso/pull/735), [#739](https://github.com/logicplanes/kaeso/pull/739))
- The route canvas hangs a model's fallbacks under it, so a route fits beside the inspector, and the route editor's Audit log tab replaces Versions. ([#653](https://github.com/logicplanes/kaeso/pull/653), [#654](https://github.com/logicplanes/kaeso/pull/654))
- The docs and console name KPMR as the source of Kaeso's model data. ([#881](https://github.com/logicplanes/kaeso/pull/881))

#### UI library

- Buttons press to 0.96, icons draw a 1.5px stroke and cross-fade when they change, and pictures get a translucent edge in their own shape. ([#660](https://github.com/logicplanes/kaeso/pull/660))
- Many UI library components take variants: Tabs, RadioGroup, Section, Skeleton and Kbd replace PillTabs, RadioCards, SettingsSection, SkeletonLine and KbdSequence; Stat, StatGroup, CopyButton and Avatar replace StatTile, StatGrid, MetricRow, ClipboardText, InlineCopyText, AgentAvatar, AgentAttribution and AvatarGroup; Button takes `pressed`, Meter a gauge shape and ParameterPanel a trigger; an icon-only button is a Button with an icon that shows its name as a tooltip; and search, password and secret fields are types of Input. Aliases are gone, internal parts are no longer public, and StatusDot is now Status. ([#664](https://github.com/logicplanes/kaeso/pull/664), [#665](https://github.com/logicplanes/kaeso/pull/665), [#666](https://github.com/logicplanes/kaeso/pull/666), [#667](https://github.com/logicplanes/kaeso/pull/667), [#668](https://github.com/logicplanes/kaeso/pull/668), [#669](https://github.com/logicplanes/kaeso/pull/669))
- Code blocks highlight 41 languages, loaded as they are needed, with a syntax palette that reads well in light and dark. ([#773](https://github.com/logicplanes/kaeso/pull/773))

#### Documentation

- The README shows how to install Kaeso in one command and send a first request, how it compares with LiteLLM, Bifrost and Portkey, the console's pages in a theme-aware grid, the live demo, the OpenSSF Best Practices badge, one set of deploy buttons with the platforms' logos, and every built-in provider, with screenshots of the console as it is now. ([#673](https://github.com/logicplanes/kaeso/pull/673), [#679](https://github.com/logicplanes/kaeso/pull/679), [#730](https://github.com/logicplanes/kaeso/pull/730), [#734](https://github.com/logicplanes/kaeso/pull/734), [#749](https://github.com/logicplanes/kaeso/pull/749), [#757](https://github.com/logicplanes/kaeso/pull/757), [#797](https://github.com/logicplanes/kaeso/pull/797), [#906](https://github.com/logicplanes/kaeso/pull/906))
- The scaling page shows what one, two and three gateways serve for streamed and non-streamed requests, and the benchmark reports CPU time per request, the latency Kaeso adds and an agent session's streams through an alias. ([#744](https://github.com/logicplanes/kaeso/pull/744), [#766](https://github.com/logicplanes/kaeso/pull/766), [#792](https://github.com/logicplanes/kaeso/pull/792), [#889](https://github.com/logicplanes/kaeso/pull/889))
- The docs describe the Gemini and ElevenLabs paths behind your own proxy, the one-time installer step for older installs and the Platform API's newer routes, and the Images API page reads in shorter paragraphs. ([#805](https://github.com/logicplanes/kaeso/pull/805), [#834](https://github.com/logicplanes/kaeso/pull/834))
- The Images, Audio and Videos API pages name every newer provider, and the documentation's home page lists media, alias routes, the Playground and the MCP portal. ([#904](https://github.com/logicplanes/kaeso/pull/904))
- The READMEs of the UI library, the tokens, the codecs, the Claude Code plugin and the console describe what each one holds now, such as one import per UI library component. ([#905](https://github.com/logicplanes/kaeso/pull/905))
- GitHub release notes lead with two or three highlights, a small release goes out every week, and the releasing page names the secrets the notes step reads. ([#671](https://github.com/logicplanes/kaeso/pull/671), [#856](https://github.com/logicplanes/kaeso/pull/856))
- The development pages describe a faster CI, which runs the gateway tests in four shards and each app's own suite only when it changed, and ask contributors to keep details of their own installs and provider accounts out of pull requests, issues and commits, as fixture notes and provider pages now do. ([#824](https://github.com/logicplanes/kaeso/pull/824), [#827](https://github.com/logicplanes/kaeso/pull/827), [#831](https://github.com/logicplanes/kaeso/pull/831), [#836](https://github.com/logicplanes/kaeso/pull/836), [#855](https://github.com/logicplanes/kaeso/pull/855), [#862](https://github.com/logicplanes/kaeso/pull/862))

#### Self-hosting

- The Compose package gives the gateway time to drain on stop and room for connections, keeps steady connections from Caddy and sizes PostgreSQL from its container's memory. ([#714](https://github.com/logicplanes/kaeso/pull/714))

### Removed

#### Gateway

- Kaeso no longer reads the `models_dev` settings; a `[models_dev]` section stops the gateway until it is renamed to `[kpmr]`, and the old environment variables are reported as unknown. ([#710](https://github.com/logicplanes/kaeso/pull/710))

#### Console

- Projects in an organization are all alike: there is no default project, Kaeso opens on the organization's overview, and new members join only the projects their invitation names. ([#647](https://github.com/logicplanes/kaeso/pull/647))

#### UI library

- The UI library drops the components that only wrapped another one: Menubar, Accordion, HoverCard, ProviderCard, EnvironmentSnippet, Grid, Stack, Inline, ThemeSelect, the page blocks and ResourceList. ([#672](https://github.com/logicplanes/kaeso/pull/672))

### Fixed

#### Gateway

- Claude Code sessions that connect an MCP server mid-session reach models in other formats, because Kaeso applies its inline tool changes to their tool list, and DeepSeek gets tool patterns it can compile. ([#753](https://github.com/logicplanes/kaeso/pull/753))
- Requests that Kaeso or the provider refused cost nothing, free models are logged at a known cost of $0, and transcripts and the ElevenLabs voice changer are billed by the length their file states when the provider reports none. ([#848](https://github.com/logicplanes/kaeso/pull/848), [#852](https://github.com/logicplanes/kaeso/pull/852), [#854](https://github.com/logicplanes/kaeso/pull/854))
- Speech that fails before its first byte answers with an error in the app's format instead of an empty 200, and audio that breaks off ends in an error. ([#851](https://github.com/logicplanes/kaeso/pull/851))
- An image, speech or transcription model whose project's OpenAI key is a ChatGPT account says it needs an API key, in the Playground and on `/v1`. ([#894](https://github.com/logicplanes/kaeso/pull/894))
- The request log counts every keepalive a stream wrote, also while Kaeso holds back a tool call or leaves events out. A provider's own keepalive replayed to an app of the same format is not counted. ([#888](https://github.com/logicplanes/kaeso/pull/888))
- Tool calls from newest-version clients reach older MCP servers without the newest version's `_meta` claims, and an MCP client known by its metadata document signs in again after registration churn trimmed its registration. ([#736](https://github.com/logicplanes/kaeso/pull/736), [#743](https://github.com/logicplanes/kaeso/pull/743))
- The response cache and every other Valkey user reconnect on their own after Valkey refused the gateway's password for a while, and a response cache clear keeps the replies stored since the clear. ([#711](https://github.com/logicplanes/kaeso/pull/711), [#732](https://github.com/logicplanes/kaeso/pull/732))
- An alias whose route names a removed model can be renamed, and its 404 says no model is left for the request. ([#658](https://github.com/logicplanes/kaeso/pull/658))

#### KPP

- An MP3 whose ID3v1 tag reaches back over its first frame no longer crashes the audio length reader. ([#876](https://github.com/logicplanes/kaeso/pull/876))

#### Console

- The console patches the router's code-split wrapper, so a route replayed after its module arrived works as React 19 expects. ([#731](https://github.com/logicplanes/kaeso/pull/731))
- The Security page lists an organization's agents with access in a section of its own, and the Projects page shows its Active, Archived and All tabs beside the title like every other page. ([#655](https://github.com/logicplanes/kaeso/pull/655), [#758](https://github.com/logicplanes/kaeso/pull/758))
- On a phone, the update button on Instance › Updates moves under See what's new instead of past the screen's edge. ([#676](https://github.com/logicplanes/kaeso/pull/676))
- A request's reply tab lines up its stop tag with the first line of a wrapped note, every right in an access view lines up with its heading, and the copy and retry buttons under a Playground reply line up with its text. ([#865](https://github.com/logicplanes/kaeso/pull/865), [#866](https://github.com/logicplanes/kaeso/pull/866), [#867](https://github.com/logicplanes/kaeso/pull/867))

#### UI library

- The route editor stays usable on short screens and phones, a Segmented wider than its row scrolls, links in a paragraph no longer overlap each other's touch areas, a small select gets a full hit area, and a system theme change no longer replays transitions. ([#659](https://github.com/logicplanes/kaeso/pull/659), [#661](https://github.com/logicplanes/kaeso/pull/661))
- An empty table whose columns don't fit no longer scrolls sideways, and table column headers stay flush under the page header while you scroll. ([#713](https://github.com/logicplanes/kaeso/pull/713), [#902](https://github.com/logicplanes/kaeso/pull/902))
- Diagrams in the docs no longer have a thin box drawn around them inside their card. ([#830](https://github.com/logicplanes/kaeso/pull/830))

#### Documentation

- The request flow page gives the right answers for limits: no Retry-After for a used-up budget, 403 for a spend budget that can't price a feature, and 529 in the Anthropic format when the gateway is busy. ([#646](https://github.com/logicplanes/kaeso/pull/646))

#### Self-hosting

- The installer refuses to take over an existing install, moves a local install off taken ports, checks the typed domain and installs Docker on Linux when asked, and picks a free network range when it runs again on an install that never started. ([#677](https://github.com/logicplanes/kaeso/pull/677), [#680](https://github.com/logicplanes/kaeso/pull/680))

### Security

- Kaeso fetches an MCP client's metadata document only from a public address, whatever networks MCP servers may reach. ([#826](https://github.com/logicplanes/kaeso/pull/826))
- Voice and video ids can no longer steer provider calls, links on a host other than the provider's own are fetched from public addresses only, video and voice lookups count against rates, and `/v2` reaches the gateway. ([#829](https://github.com/logicplanes/kaeso/pull/829))
- The Render template keeps the client's address behind Render by reading the header Cloudflare sets instead of a forwarded one the caller can extend. ([#747](https://github.com/logicplanes/kaeso/pull/747))
- The gateway shares one memory budget, the size of `body_limits.in_flight`, among all providers' answers, so large answers from a provider or a custom provider can no longer run it out of memory: a request that gets no room within 10 seconds is answered 503, and a stream that would need more than the budget ends with the stream limit's error. ([#912](https://github.com/logicplanes/kaeso/pull/912))
- MCP portal clients meet the organization's two-factor and single sign-on rules and need the person's `tools.use` right. Their grants and tokens stop working when the project is archived or the organization is scheduled for deletion, and a password change that signs agents out, or signing out everywhere, ends them too. ([#911](https://github.com/logicplanes/kaeso/pull/911))
- A key with platform access creates and rotates keys only within its own model list, expiry and Tools access, and creates none while limits apply to it; a key can still rotate itself. ([#914](https://github.com/logicplanes/kaeso/pull/914))
- A key limited to some models no longer lists, reads, downloads, polls or deletes videos and image generations made with other models of its project. ([#915](https://github.com/logicplanes/kaeso/pull/915))
- Pending agent sign-ins for a client are capped per network, so requests from one network no longer end other people's, and client names drop invisible and direction-changing characters. ([#916](https://github.com/logicplanes/kaeso/pull/916))
- With Keep media off, a file a request sent where a media type belongs no longer stays in the request log inside the note that stands for it. ([#918](https://github.com/logicplanes/kaeso/pull/918))

## [0.5.6] - 2026-11-02

### Added

- Cerebras is a built-in provider: add its key and call its models from any app format ([#642](https://github.com/logicplanes/kaeso/pull/642))

### Changed

- The model API takes request bodies up to 32 MiB by default, as Anthropic does, instead of 8 MiB ([#617](https://github.com/logicplanes/kaeso/pull/617))
- Kaeso decodes large requests about a third faster by counting each body once ([#623](https://github.com/logicplanes/kaeso/pull/623))

#### Gateway

- The gateway serves more requests per second under heavy load and spends less CPU on each ([#600](https://github.com/logicplanes/kaeso/pull/600))
- A response cache lookup on a large request no longer holds up other requests on the same gateway thread ([#626](https://github.com/logicplanes/kaeso/pull/626))
- A fresh install starts with KPMR's OpenRouter prices as of 2 November 2026, 22:15 UTC ([#644](https://github.com/logicplanes/kaeso/pull/644))

#### KPP

- A Chat Completions tool call streamed without an id no longer slows the stream down as it grows ([#615](https://github.com/logicplanes/kaeso/pull/615))
- Kaeso translates large Responses requests, such as long Codex threads, with less work ([#627](https://github.com/logicplanes/kaeso/pull/627))
- Kaeso translates streamed replies with less work per chunk, 1.3 to 1.6 times faster for the common pairs ([#641](https://github.com/logicplanes/kaeso/pull/641))

#### Self-hosting

- Updates finish 10 to 15 seconds sooner, because new containers report healthy as soon as they are ([#636](https://github.com/logicplanes/kaeso/pull/636))

### Fixed

- Streamed tool calls keep the tool name the app defined ([#598](https://github.com/logicplanes/kaeso/pull/598))
- Shortening an organization's log retention asks first, and saving one retention keeps an unsaved edit of the other ([#602](https://github.com/logicplanes/kaeso/pull/602))
- Streams that break off log the tool call Kaeso was still holding back and count its tokens in the usage estimate ([#603](https://github.com/logicplanes/kaeso/pull/603))
- Tool call arguments, citations and unknown parts that a Responses stream sends only in the finished item reach every app ([#606](https://github.com/logicplanes/kaeso/pull/606))
- A system or developer message between two tool results reaches Chat Completions models after the last result ([#608](https://github.com/logicplanes/kaeso/pull/608))
- An Anthropic stream error in another shape or in plain text keeps the provider's message ([#609](https://github.com/logicplanes/kaeso/pull/609))
- An ended session sends you to sign in from any page, the request log stops refetching a page that keeps failing, and the project overview explains what couldn't load ([#610](https://github.com/logicplanes/kaeso/pull/610))
- A Chat Completions stream's second finish chunk no longer adds an empty text block after the stop ([#612](https://github.com/logicplanes/kaeso/pull/612))
- Date pickers name the day you picked, and the request and audit logs filter by days in the install's time zone ([#613](https://github.com/logicplanes/kaeso/pull/613))
- A Combobox lists every option again after a pick, and a data table you can open rows in is a grid that doesn't open a row when you select its text ([#621](https://github.com/logicplanes/kaeso/pull/621))
- The project's forms show why a name was refused under the name, and the key and service account dialogs go back to it ([#624](https://github.com/logicplanes/kaeso/pull/624))
- Request logs say a stream ended because Kaeso restarted, not because the app left, and a broken-off upload is no longer called too large ([#625](https://github.com/logicplanes/kaeso/pull/625))
- The `postgres.pool_size` setting now says it sizes each instance's pool and that background jobs open up to 8 connections outside it ([#633](https://github.com/logicplanes/kaeso/pull/633))
- The gateway raises its open-file limit at startup, so heavy traffic no longer runs it out of sockets at about 500 streams ([#635](https://github.com/logicplanes/kaeso/pull/635))
- Automatic updates retry within the window's hour when they can't start, and no longer retry a release whose update rolled back ([#639](https://github.com/logicplanes/kaeso/pull/639))
- The updater gives each Docker call its own timeout, so a hung Docker no longer holds an update for 10 minutes past its health deadline ([#640](https://github.com/logicplanes/kaeso/pull/640))
- The password hint wraps under its field instead of running past the dialog and off a phone's screen ([#645](https://github.com/logicplanes/kaeso/pull/645))

#### Gateway

- Long agent conversations no longer hit "busy translating" at half the memory budget ([#601](https://github.com/logicplanes/kaeso/pull/601))
- Limit checks cost the same however many requests are open, instead of slowing Valkey down as streams pile up ([#604](https://github.com/logicplanes/kaeso/pull/604))
- Limited streams no longer pause for Valkey once a minute, and a limited reply ends as soon as it is charged ([#607](https://github.com/logicplanes/kaeso/pull/607))
- Requests no longer wait for PostgreSQL every 30 seconds when a key's cached entries expire ([#616](https://github.com/logicplanes/kaeso/pull/616))
- Budgets no longer lose up to 10 seconds of usage when their counter is rebuilt during a usage write ([#619](https://github.com/logicplanes/kaeso/pull/619))
- A restart no longer loses the last seconds of usage and request logs when Docker stops the gateway before its requests have drained ([#631](https://github.com/logicplanes/kaeso/pull/631))
- Scheduling an organization's deletion no longer reads every user of the install, and no longer reports an error once the organization is scheduled ([#632](https://github.com/logicplanes/kaeso/pull/632))
- Request logs keep flowing after ClickHouse recovers from a stall, instead of the writer hanging until a restart ([#634](https://github.com/logicplanes/kaeso/pull/634))

#### Console

- Instance users and organizations stay in the gateway's order instead of sorting only the rows loaded so far ([#597](https://github.com/logicplanes/kaeso/pull/597))
- The playground's model list follows catalogue changes, the audit log's Clear search removes the target filter, and deleting your account forgets what the browser kept ([#599](https://github.com/logicplanes/kaeso/pull/599))

### Security

- Changing your password can also disconnect your agents, for when someone else may have connected one ([#605](https://github.com/logicplanes/kaeso/pull/605))
- A disabled key can no longer be rotated into a working one; enable it first ([#618](https://github.com/logicplanes/kaeso/pull/618))
- Ending your own sessions is now recorded in the instance's audit log ([#620](https://github.com/logicplanes/kaeso/pull/620))
- An agent no longer sees organizations and projects it was shut out of in your memberships or a member's rights ([#622](https://github.com/logicplanes/kaeso/pull/622))
- An organization past its deletion date can no longer be restored, and a restore under way stops its final deletion instead of losing its logs ([#628](https://github.com/logicplanes/kaeso/pull/628))
- Changing an organization's log retention or content logging now needs a recent sign-in, and the right is marked sensitive ([#629](https://github.com/logicplanes/kaeso/pull/629))
- Agent sign-ins are limited per address and capped, unknown agent tokens count toward key throttling, and sign-in errors go back only to approved agents ([#638](https://github.com/logicplanes/kaeso/pull/638))

#### Gateway

- An ownership transfer racing with a demotion, removal or leave no longer leaves an organization without an owner ([#611](https://github.com/logicplanes/kaeso/pull/611))
- A key created while its member is removed, its account deleted or its project or service account archived is refused or archived with the rest ([#614](https://github.com/logicplanes/kaeso/pull/614))
- A burst of sign-in attempts can no longer run the gateway out of memory ([#630](https://github.com/logicplanes/kaeso/pull/630))

## [0.5.5] - 2026-11-02

Kaeso 0.5.5 fixes the verification code input so its cells line up with the Verify button and the fields around them on every screen that asks for a code. Fresh installs start with KPMR's OpenRouter models as of 2 November 2026, 14:35 UTC, and running installs get the same data from KPMR on their own. There are no breaking changes in this release.

**Contributors:** @dvnold

**Full changes:** [v0.5.4...v0.5.5](https://github.com/logicplanes/kaeso/compare/v0.5.4...v0.5.5)

### Changed

#### Gateway

- A fresh install starts with KPMR's OpenRouter models as of 2 November 2026, 14:35 UTC. The snapshot adds `z-ai/glm-5.3-flashx-legal`, removes `nousresearch/hermes-3-llama-3.1-405b` and `nousresearch/hermes-3-llama-3.1-70b`, which OpenRouter no longer lists, and changes the price of `google/gemma-4-26b-a4b-it`. Running installs get the same data from KPMR on their own. ([#594](https://github.com/logicplanes/kaeso/pull/594))

### Fixed

#### UI library

- The verification code cells line up with the Verify button and the fields around them instead of hugging the left edge. The `OTPInput` row now takes its container's width, up to the 400px sign-in column, and the cells share it at 48px high (`md`) or 40px (`sm`). This applies to the sign-in two-step step, the sign-in-again dialog, two-factor set-up and the second-factor field. The empty focused cell also shows a blinking caret, which the global reduced-motion rule keeps still. ([#593](https://github.com/logicplanes/kaeso/pull/593))

## [0.5.4] - 2026-11-02

Kaeso 0.5.4 fixes Claude Code requests to models whose output limit is below 32,000 tokens, which were refused before. Fresh installs start with KPMR model data as of 2 November 2026 and current prices for two OpenRouter models. Running installs get the same data from KPMR on their own. There are no breaking changes in this release.

**Contributors:** @dvnold

**Full changes:** [v0.5.3...v0.5.4](https://github.com/logicplanes/kaeso/compare/v0.5.3...v0.5.4)

### Changed

#### Gateway

- A fresh install starts with KPMR's model data as of 2 November 2026, 12:30 UTC, with OpenRouter's current prices for DeepSeek V4.1 Flash 1017 and Gemma 4 26B A4B. Running installs get the same data from KPMR on their own. ([#591](https://github.com/logicplanes/kaeso/pull/591))

### Fixed

- Claude Code reaches models whose output limit is below 32,000 tokens, such as Command A. A request from another format that asks for more output than the model's maximum now gets that maximum, and `kaeso-adapted` names the app's field, such as `max_tokens`. Requests in the provider's own format are unchanged. ([#590](https://github.com/logicplanes/kaeso/pull/590))

## [0.5.3] - 2026-11-02

Kaeso 0.5.3 widens what the gateway accepts from Codex and Claude Code. Codex requests now reach Groq, Moonshot and Cohere models, and Claude Code requests reach OpenAI models, with the fields a provider refuses left out or adapted and named in response headers. Fresh installs start with KPMR's model data as of 2 November 2026, which removes `openai/gpt-5.2-chat` from OpenRouter, and running installs get the same data from KPMR on their own. There are no breaking changes in this release.

**Contributors:** @dvnold

**Full changes:** [v0.5.2...v0.5.3](https://github.com/logicplanes/kaeso/compare/v0.5.2...v0.5.3)

### Changed

#### Gateway

- A fresh install starts with KPMR's model data as of 2 November 2026, 09:30 UTC: OpenRouter no longer lists `openai/gpt-5.2-chat`, and five of its prices and three of its limits changed. Running installs get the same data from KPMR on their own. ([#587](https://github.com/logicplanes/kaeso/pull/587))

### Fixed

- Codex requests reach Groq models. Groq refuses `prompt_cache_key`, which Codex sends on every request, so Kaeso leaves a Responses app's key out for Groq and names it in the `kaeso-left-out` header; a Chat app's own key is refused before sending, as Groq would refuse it. ([#584](https://github.com/logicplanes/kaeso/pull/584))
- Claude Code reaches OpenAI models. Its `metadata.user_id` is 150 characters long, and OpenAI takes at most 128 in `safety_identifier`, so an Anthropic app's longer id reaches OpenAI as its SHA-256 hash, the same for the same id, and the `kaeso-adapted` header names it. Ids that fit, and every other provider, are unchanged. ([#585](https://github.com/logicplanes/kaeso/pull/585))
- Codex requests reach Moonshot and Cohere models. Their profiles refuse `parallel_tool_calls`, which Codex sends as `true` on every request; `true` only allows parallel calls, so Kaeso leaves it out and names it in the `kaeso-left-out` header, while `false` is still refused. ([#586](https://github.com/logicplanes/kaeso/pull/586))

## [0.5.2] - 2026-11-02

Kaeso 0.5.2 lets Claude Code and Codex work with models that list no effort levels or don't reason, instead of failing those requests. Kaeso now leaves out the unsupported effort or adaptive thinking and reports it in the `kaeso-left-out` header, while an unsupported effort value or enabled thinking still returns an error. Fresh installs start with KPMR's models and prices as of 2 November 2026, and four DeepSeek models now refuse JSON schemas before sending. There are no breaking changes in this release.

**Contributors:** @dvnold

**Full changes:** [v0.5.1...v0.5.2](https://github.com/logicplanes/kaeso/compare/v0.5.1...v0.5.2)

### Changed

#### Gateway

- A fresh install starts with KPMR's models and prices as of 2 November 2026, which add OpenRouter's `openai/gpt-5.2-chat` and update several OpenRouter prices and limits. `deepseek-flash`, `deepseek-v4-pro`, `deepseek-v4-flash` and `deepseek-v4-flash-vision-exp` no longer list structured output, so Kaeso refuses a JSON schema for them before sending and names `response_format`, while JSON mode (`json_object`) works as before. Running installs get the same data from KPMR on their own. ([#581](https://github.com/logicplanes/kaeso/pull/581))

### Fixed

- Claude Code and Codex now reach models that list no effort levels or don't reason. Kaeso leaves out the effort (`output_config.effort`, `reasoning.effort` or `reasoning_effort`) or adaptive `thinking` that the model cannot use, and names it in the `kaeso-left-out` header. An effort outside the model's levels, and enabled thinking or a budget for a non-reasoning model, are still refused. ([#580](https://github.com/logicplanes/kaeso/pull/580))

## [0.5.1] - 2026-11-02

This release fixes several ways that requests from coding agents failed when sent to models in another API format. Tool results with images now reach Chat Completions models, and Codex tool namespaces no longer repeat their descriptions. Anthropic, DeepSeek, Mistral and Groq targets now accept requests from OpenAI apps, Claude Code and MCP clients that failed against them before. A fresh install starts with KPMR's models and prices as of 1 November 2026.

**Contributors:** @dvnold

**Full changes:** [v0.5.0...v0.5.1](https://github.com/logicplanes/kaeso/compare/v0.5.0...v0.5.1)

### Changed

#### Gateway

- A fresh install starts with KPMR's models and prices as of 1 November 2026. The snapshot adds Ollama Cloud's MiMo V2.6 Flash and Pro, and changes OpenRouter's prices for five models, the output limit of GPT OSS 20B and Mistral Large 4's effort levels. ([#578](https://github.com/logicplanes/kaeso/pull/578))

### Fixed

- Codex requests to models outside OpenAI now send each tool namespace's description once, in front of its first tool, instead of in front of every tool. MCP servers group many tools under one namespace, so the repeated text could push a single turn past Claude's 1M context window. ([#573](https://github.com/logicplanes/kaeso/pull/573))
- Images and files a tool returns reach Chat Completions models in the user message after the tool results, instead of the whole request being refused, so Claude Code's Read and Codex's `view_image` work through DeepSeek, Mistral and OpenRouter. The tool message keeps the result's text and says that the rest follows, and the user message names the tool call before its images and files. ([#574](https://github.com/logicplanes/kaeso/pull/574))
- Anthropic and DeepSeek refused tool schemas whose root has no `type: "object"`, and Mistral refused functions without `parameters`. Kaeso now writes `type: "object"` first in such roots for Anthropic and, through the ToolSchemaRoot::TypedObjectUnions profile setting, for DeepSeek, and Mistral's profile sends the empty object schema for a function without parameters. Requests from OpenAI apps and MCP clients no longer fail only because of the target. ([#575](https://github.com/logicplanes/kaeso/pull/575))
- Kaeso sends the end user to Chat Completions providers as `safety_identifier`, which Groq rejects with a 400 error. Groq's profile now sends the id as `user` through the new `end_user_as_user()` setting, so Claude Code and other Anthropic apps no longer fail on Groq models. ([#576](https://github.com/logicplanes/kaeso/pull/576))

## [0.5.0] - 2026-10-31

- **Breaking:** Kaeso reads its model data from KPMR, the Kaeso Provider & Model Registry, as signed catalogues instead of fetching models.dev ([#571](https://github.com/logicplanes/kaeso/pull/571))

### Added

- ClickHouse is optional: an install can turn request logs off or use its own ClickHouse ([#569](https://github.com/logicplanes/kaeso/pull/569))
- Projects can answer repeated identical requests from an opt-in response cache ([#570](https://github.com/logicplanes/kaeso/pull/570))

### Changed

- Kaeso reads its model data from KPMR, the Kaeso Provider & Model Registry, as signed catalogues instead of fetching models.dev ([#571](https://github.com/logicplanes/kaeso/pull/571))
- Dependencies updated ([#564](https://github.com/logicplanes/kaeso/pull/564))

### Fixed

#### UI library

- Disclosure rows line up their chevron and hint with the title ([#563](https://github.com/logicplanes/kaeso/pull/563))

## [0.4.8] - 2026-10-30

### Added

- Moonshot (Kimi) is a built-in provider: add its key and call its models from any app format ([#561](https://github.com/logicplanes/kaeso/pull/561))
- QwenCloud, Alibaba's model platform outside mainland China, formerly Model Studio, is a built-in provider: add its key and call its models and embeddings from any app format ([#562](https://github.com/logicplanes/kaeso/pull/562))
- Every provider's mark shows in its owner's colours, in light and dark mode
- Cohere is a built-in provider: add its key and call its models from any app format ([#557](https://github.com/logicplanes/kaeso/pull/557))
- Ollama Cloud is a built-in provider: add its key and call its models from any app format ([#560](https://github.com/logicplanes/kaeso/pull/560))

### Changed

- On-or-off filters are pills that match the selects beside them, the audit log's agent filter is called Agents, and the provider picker's groups drop their counts
- Underline tabs start on the content edge, under the page heading, and preview the selection line on hover
- A dialog's layers and steps crossfade in place instead of sliding sideways
- Add models shows each model's name over its full id
- Kaeso names every provider mark's owner and says it is not affiliated with any of them ([#558](https://github.com/logicplanes/kaeso/pull/558))
- Dependencies updated ([#547](https://github.com/logicplanes/kaeso/pull/547), [#548](https://github.com/logicplanes/kaeso/pull/548), [#549](https://github.com/logicplanes/kaeso/pull/549), [#550](https://github.com/logicplanes/kaeso/pull/550), [#554](https://github.com/logicplanes/kaeso/pull/554), [#555](https://github.com/logicplanes/kaeso/pull/555), [#556](https://github.com/logicplanes/kaeso/pull/556))

#### UI library

- The UI library's site uses the same favicon as the documentation and the console ([#545](https://github.com/logicplanes/kaeso/pull/545))
- The UI library's changelog now links to kaeso.app/changelog instead of a page of its own ([#546](https://github.com/logicplanes/kaeso/pull/546))

### Fixed

- Rows with a second line no longer overflow into the next row in menus and comboboxes, and time zones show their offset at the row's end
- A button at the edge of a settings row, such as See what's new, is no longer cut short

### Security

#### Gateway

- Custom providers and SSO discovery never reach Alibaba Cloud's metadata service ([#544](https://github.com/logicplanes/kaeso/pull/544))

## [0.4.7] - 2026-10-30

### Added

- Vercel AI Gateway is a built-in provider: add its key and call its models from any app format ([#537](https://github.com/logicplanes/kaeso/pull/537))

### Changed

- Dependencies updated ([#522](https://github.com/logicplanes/kaeso/pull/522))

#### Gateway

- A fresh install starts with models.dev's models and prices as of 30 October 2026 ([#542](https://github.com/logicplanes/kaeso/pull/542))

### Fixed

- Names refuse invisible characters everywhere, and a NUL byte no longer causes a server error ([#523](https://github.com/logicplanes/kaeso/pull/523))
- The console says when the catalogue, aliases or a chart couldn't load, and charts add no invisible tab stops ([#526](https://github.com/logicplanes/kaeso/pull/526))
- Usage tiles say when they couldn't load, Add member shows its whole message, and docs tables get names of their own ([#528](https://github.com/logicplanes/kaeso/pull/528))
- Console and UI library: status not by colour alone, wrapping list rows on phones, a clean theme switch, Undo for key disables and a Save button for project names ([#529](https://github.com/logicplanes/kaeso/pull/529))
- Toast actions close their toast, errors don't pile up or outlive a retry, and list focus survives a removed row ([#531](https://github.com/logicplanes/kaeso/pull/531))
- The service account sheet keeps Rename on screen on phones, and tables no longer redraw every row while scrolling ([#532](https://github.com/logicplanes/kaeso/pull/532))
- Rate-limited answers say how long to wait in the right unit, and "1 minute" rather than "1 minutes" ([#533](https://github.com/logicplanes/kaeso/pull/533))
- Codex sessions with subagents work on Grok and every other non-OpenAI model, and a tool_choice without tools no longer reaches providers that refuse it ([#535](https://github.com/logicplanes/kaeso/pull/535))
- Settings pages share the phone bar's edges on tablets, and access rows, the timing bar and the providers table line up ([#536](https://github.com/logicplanes/kaeso/pull/536))
- After an upgrade that adds a built-in provider, its models.dev models appear at once instead of after models.dev next changes ([#540](https://github.com/logicplanes/kaeso/pull/540))

#### Gateway

- Keys expire on the date chosen, the Platform API refuses unknown fields, both log retentions shrink together, and unreadable settings are named ([#521](https://github.com/logicplanes/kaeso/pull/521))
- A replaced provider key is no longer used for up to 30 seconds after the change ([#524](https://github.com/logicplanes/kaeso/pull/524))
- Key expiry and organization deletion emails say the time, on the install's clock, instead of implying the whole day ([#534](https://github.com/logicplanes/kaeso/pull/534))

#### Console

- The System page keeps saying the status couldn't load while it refreshes, instead of flickering back to loading ([#530](https://github.com/logicplanes/kaeso/pull/530))

### Security

- Invitations send one address at most five emails an hour ([#527](https://github.com/logicplanes/kaeso/pull/527))

## [0.4.6] - 2026-10-28

### Changed

#### Gateway

- A fresh install starts with models.dev's models and prices as of 28 October 2026 ([#520](https://github.com/logicplanes/kaeso/pull/520))

### Fixed

- Kaeso overhead no longer counts the time the app takes to upload the request body ([#519](https://github.com/logicplanes/kaeso/pull/519))

#### Console

- The console no longer logs a React error when a page's loading view appears before its code has loaded ([#517](https://github.com/logicplanes/kaeso/pull/517))

## [0.4.5] - 2026-10-27

### Added

- The request log filters by format and shows each request's cached share, and usage reports a cache hit rate by route ([#511](https://github.com/logicplanes/kaeso/pull/511))

### Changed

- Every control shows its hover, press and open states, dark text reads clearly and the library works in forced colours ([#502](https://github.com/logicplanes/kaeso/pull/502))
- Buttons, links, key caps and text fields show every state, fit long labels and say what changed ([#503](https://github.com/logicplanes/kaeso/pull/503))
- Dialogs, menus, tooltips, toasts and the command palette keep their place and describe their controls once ([#505](https://github.com/logicplanes/kaeso/pull/505))
- Tabs, pagination, the sidebar and settings rows keep focus, mirror right to left and stay visible in forced colours ([#506](https://github.com/logicplanes/kaeso/pull/506))
- Data tables are one tab stop with range selection, and lists, badges, avatars and rights read clearly on every row ([#507](https://github.com/logicplanes/kaeso/pull/507))
- Charts handle empty and extreme data, keep their labels inside and survive forced colours, and code views are one tab stop ([#508](https://github.com/logicplanes/kaeso/pull/508))

#### Gateway

- A fresh install starts with models.dev's models and prices as of 27 October 2026 ([#515](https://github.com/logicplanes/kaeso/pull/515))

#### UI library

- Checkboxes, radios, switches, sliders, selects and pickers show every state and keep their popups on screen ([#504](https://github.com/logicplanes/kaeso/pull/504))
- The playground keeps focus, reserves its room and says when a response ends ([#509](https://github.com/logicplanes/kaeso/pull/509))

### Fixed

- Claude's calls to tools without parameters reach Codex and Chat apps as {}, and tool calls whose arguments aren't an object no longer stop a thread on Claude ([#501](https://github.com/logicplanes/kaeso/pull/501))
- Buttons and switches keep focus when their tooltip turns on or off, highlighted rows read clearly in forced colours, and a typed date is reported once ([#513](https://github.com/logicplanes/kaeso/pull/513))
- The request log warns about a missing cache breakpoint only when nothing was cached, counts breakpoints passed through to Anthropic, and leaves embedding routes' cache hit rate empty ([#514](https://github.com/logicplanes/kaeso/pull/514))

#### UI library

- Data tables keep focus after the last page and shrink range selections, page links recover from a page past the end, and charts write missing values as a dash ([#512](https://github.com/logicplanes/kaeso/pull/512))

## [0.4.4] - 2026-10-26

Kaeso 0.4.4 makes Claude work better behind OpenAI-format apps such as Codex: their prompts are cached, and tools whose schema has a root union reach Claude and Grok. Fresh installs start from the model catalogue of 26 October 2026, with 554 models. The release has no breaking changes, and the audit log now shows changes as a highlighted diff.

**Contributors:** @dvnold

**Full changes:** [v0.4.3...v0.4.4](https://github.com/logicplanes/kaeso/compare/v0.4.3...v0.4.4)

### Changed

- The audit log shows a change as a diff, with added and removed lines tinted, only the changed words marked, and unchanged lines folded away behind a button. A small edit to a long event now stands out at a glance. ([#496](https://github.com/logicplanes/kaeso/pull/496))

#### Gateway

- A fresh install starts with the model catalogue snapshot of 26 October 2026, which holds 554 models and their prices for the providers Kaeso supports. OpenRouter drops one model, and fourteen OpenRouter models have new prices. ([#498](https://github.com/logicplanes/kaeso/pull/498))

### Fixed

- Tools whose parameter schema has a union at its root, such as Codex Desktop's `automation_update`, now work on Claude and Grok models instead of failing every turn. Kaeso merges the union into one object schema, and the tool description lists each variant. Each merged tool is named in the `kaeso-adapted` response header. ([#494](https://github.com/logicplanes/kaeso/pull/494))
- Requests to Claude from Chat Completions and Responses apps, such as Codex, now cache the tool list, system prompt and conversation, so repeated turns cost less input. Cache breakpoints stay within Anthropic's limit of four, and `prompt_cache_retention` set to `24h` asks for a one-hour cache lifetime instead of failing. Cache reads are counted in the usage the app receives. ([#495](https://github.com/logicplanes/kaeso/pull/495), [#497](https://github.com/logicplanes/kaeso/pull/497))

## [0.4.3] - 2026-10-25

Kaeso 0.4.3 lets agents such as Claude Code and Codex connect to an instance's remote MCP server, sign in with OAuth and act under rights that the user and the instance and organization policies allow. A new custom provider now needs only a base URL and a key, because Kaeso detects its format, key style, embeddings, token counting and models. Codex, Claude Code and Grok CLI work with Claude, GPT, Grok and Mistral models in more cases, and Mistral's answers reach Chat Completions apps as plain text. The design tokens package no longer exports `brandVersion`, the one change that asks integrators to act, and a fresh install starts from the model catalogue's data as of 25 October 2026.

- **Breaking:** The design tokens package no longer exports `brandVersion`, so code that imports it must drop that import. ([#467](https://github.com/logicplanes/kaeso/pull/467))

**Contributors:** @dvnold

**Full changes:** [v0.4.2...v0.4.3](https://github.com/logicplanes/kaeso/compare/v0.4.2...v0.4.3)

### Added

- Adding a custom provider takes only its base URL and key. Kaeso detects the API format, key style, embeddings, token counting and the models the server offers, and the review step lets you change any of it before saving. A trailing or doubled `/v1` and a pasted endpoint path are dropped from the base URL when it is saved. ([#449](https://github.com/logicplanes/kaeso/pull/449))
- Agents such as Claude Code, Codex, Cursor, Gemini CLI and OpenCode can connect to an instance's remote MCP server at `/mcp`. They sign in with OAuth 2.1 and PKCE through the console and act with rights the user grants, capped by the instance's and each organization's agent policies. Every tool call is kept for 90 days as the agent's activity, and the console lists each account's agents so they can be renamed or revoked. ([#450](https://github.com/logicplanes/kaeso/pull/450))
- The documentation site and the UI library's showcase end their sidebars with a legal menu linking to Kaeso's privacy policy and imprint, a theme menu and a language menu. The menus replace the language and theme selects, and the UI library's `ThemeMenu` can open upward with its `side` and `align` props. ([#474](https://github.com/logicplanes/kaeso/pull/474))
- Kaeso now has Claude Code and Cursor plugins that connect to an instance's `/mcp` address and include a skill on using its tools. The console's Agents page also offers one-click connection steps for Claude Code, Cursor and GitHub Copilot, filled in with the instance's own address. ([#475](https://github.com/logicplanes/kaeso/pull/475))

### Changed

- The Add models dialog shows models in a table with a checkbox on each row, and models already in the catalogue stay locked. Adding a provider key starts with provider tiles, with connected providers first, and a press on a tile continues to the key step. ([#487](https://github.com/logicplanes/kaeso/pull/487))

#### Gateway

- A fresh install starts with the model catalogue snapshot as of 25 October 2026, which holds 555 models and their prices for the providers Kaeso supports. The providers list adds `munito` and `munito-eu`. ([#489](https://github.com/logicplanes/kaeso/pull/489))

### Removed

#### UI library

- The design tokens package no longer exports `brandVersion`, a second version number in its brand file that could disagree with Kaeso's own. Nothing in the repository read it, and a test now keeps a version field out of the brand file. ([#467](https://github.com/logicplanes/kaeso/pull/467))

### Fixed

- Refusals to sign up or create an organization, shown on installs that are invitation-only or limit email domains, are now translated in German and Chinese in the console. The check that finds untranslated Platform API messages now covers more of the gateway's services. ([#435](https://github.com/logicplanes/kaeso/pull/435))
- Toasts, empty states and confirmations in the console are shorter, each says one thing, and they are checked for length in English, German and Chinese. Several German dialog texts are shortened to fit as well. ([#440](https://github.com/logicplanes/kaeso/pull/440))
- Two refusals about a model's tool choice and thinking settings fit on one line: `auto` with the tool named in the prompt, and `enabled` thinking with `budget_tokens`. Their German and Chinese wording follows the new text. ([#443](https://github.com/logicplanes/kaeso/pull/443))
- The development pages and agent guides now match the code. The update job runs every 60 seconds with a five-minute gap between checks, and the development stack's mock serves xAI and Together AI as well. ([#446](https://github.com/logicplanes/kaeso/pull/446))
- Buttons in the console start with a verb: the last step after creating a key reads Finish, and the updates page button reads See what's new. The new wording is in English, German and Chinese, and the docs follow it. ([#448](https://github.com/logicplanes/kaeso/pull/448))
- Overview pages end with a Next steps list. The docs site no longer keeps redirects, and documentation kept in the repository's READMEs, such as the model catalogue sync details, now lives on the docs site. Fixture conventions have their own reference page. ([#453](https://github.com/logicplanes/kaeso/pull/453))
- The console shows images people sent with a new Thumbnail component from the UI library, which crops them to a square and shows the text alternative when an image cannot load. Every UI library component now exports its variant lists, and the showcase checks that each page declares them. ([#455](https://github.com/logicplanes/kaeso/pull/455))
- Grok CLI over Chat Completions works with models served in other formats, because the `model_id` field it replays on assistant messages is left out when a request crosses to another format. Chat providers receive that field only in profiles that keep it, which is SpaceX's. ([#462](https://github.com/logicplanes/kaeso/pull/462))
- Mistral answers over Chat Completions now arrive as plain text, with its thinking in `reasoning_content`, so Grok CLI and other Chat apps no longer fail or print raw thinking JSON. Empty stream chunks no longer crash tool-call turns, and reasoning that Chat apps replay reaches the providers that accept it. ([#463](https://github.com/logicplanes/kaeso/pull/463))
- Codex works with Claude, Grok and Mistral models, and a thread that started on a GPT model can switch to any of them. System and developer messages reach Anthropic in the next user turn, and hosted web search goes to providers with a matching tool and is left out elsewhere. What a provider cannot take is named in the `kaeso-left-out` header of the reply. ([#464](https://github.com/logicplanes/kaeso/pull/464))
- Mistral accepts requests from apps that send `temperature: 0`, such as aider's default, because the Mistral profile sends `top_p: 1` with greedy sampling, which changes nothing at that temperature. Error bodies in Mistral's and vLLM's shapes now reach apps with their message, and context window errors are reported as context too long. ([#465](https://github.com/logicplanes/kaeso/pull/465))
- Claude models work in Chat Completions and Responses apps that replay history. Claude's signed thinking is restored only into the turn it came from, and for the models that check the binding, a block whose conversation changed is dropped instead of failing the request. Carried reasoning is stored under new keys, so a conversation in flight during the upgrade continues without its earlier thinking. ([#466](https://github.com/logicplanes/kaeso/pull/466))
- Claude Code's tool search works with GPT, Grok and Mistral models. Tools marked for deferred loading stay out of the upstream request until a tool search loads them, then they are sent as ordinary function tools, instead of the first request of each session being refused. ([#472](https://github.com/logicplanes/kaeso/pull/472))
- Claude Code's WebSearch works with GPT and Grok models, and Mistral answers it without searching instead of failing. Anthropic's web search tool becomes hosted web search on OpenAI Responses and xAI's search on SpaceX, with its domain filters, and search results return to Anthropic apps as search blocks. ([#473](https://github.com/logicplanes/kaeso/pull/473))

#### Console

- The invitation page drops an invitation's token from its cache as soon as the person leaves the page, including when the invitation has expired, been revoked or already been used. Before, the token could stay in the cache after the page was left. ([#437](https://github.com/logicplanes/kaeso/pull/437))
- The console no longer invents a name, key prefix or format list for a provider that the gateway does not list, such as one an old key still points at. It shows the provider's id as the gateway sent it, a plain ellipsis for the prefix, and only the format the model is sent as. ([#439](https://github.com/logicplanes/kaeso/pull/439), [#452](https://github.com/logicplanes/kaeso/pull/452))
- Confirmation dialogs in the console open fresh every time, so a removal in progress for one limit or key no longer shows as busy for another item. About twenty dialogs for sessions, organizations, users, providers, keys, members and roles now keep their requests in their own component. ([#457](https://github.com/logicplanes/kaeso/pull/457))

#### UI library

- The UI library's showcase examples and tests use Kaeso's placeholder key prefix and gateway address, so the code people copy from the showcase matches the repository's placeholders. ([#436](https://github.com/logicplanes/kaeso/pull/436))
- Calendar, ToastCapsule, DisplayOptions, ThemeMenu, SelectionBar and CommandPalette now forward a ref to their root element, where before the ref was dropped. ThemeMenu takes `data-force` for its states page, and SwapText and ThemeMenu gain showcase pages. ([#441](https://github.com/logicplanes/kaeso/pull/441))
- Every UI library component that renders an element forwards its ref, and the lint check now fails for an exported component that renders without one. Props and behavior are unchanged, and the few components with no root element are listed with their reasons. ([#451](https://github.com/logicplanes/kaeso/pull/451))

#### Documentation

- The self-hosting pages name the console's real pages and labels, add `KAESO_TRUSTED_PROXIES` to the `.env` table, and describe the notice sent when someone changes their email address. The requirements and the 529 response are corrected to match the code. ([#444](https://github.com/logicplanes/kaeso/pull/444))
- The Platform API, concept and console pages now match the code, including which operations need a recent sign-in, the `PUT` and `DELETE` conventions, the model inputs and outputs, and that Mistral receives developer messages as system messages. ([#447](https://github.com/logicplanes/kaeso/pull/447))

## [0.4.2] - 2026-10-25

Kaeso 0.4.2 lets organization admins add their own providers and rotate keys in one step. Hard budgets and token rates now refuse a single request that could cross them, so operators may see more refusals up front. Streams that finished are no longer logged as client disconnects, and request logs, the console and a Server-Timing header show the time Kaeso adds to each request. Codex and the ChatGPT desktop app can use Claude, Grok and Mistral models, and many documentation and console layouts were reworked for phones.

**Contributors:** @dvnold

**Full changes:** [v0.4.1...v0.4.2](https://github.com/logicplanes/kaeso/compare/v0.4.1...v0.4.2)

### Added

- Organization admins can add custom providers, such as a self-hosted vLLM, Ollama or an internal proxy, on any API format Kaeso speaks. Managing them needs the new `providers.custom.manage` right, and the `custom_providers.enabled` and `custom_providers.allowed_networks` settings control whether they may be used and which networks they may reach. ([#427](https://github.com/logicplanes/kaeso/pull/427))
- API keys and platform keys can be rotated in one step, from the console or the Platform API. The new key keeps the old key's name, owner, access, models and key limits, and the old key keeps working for a grace period of 0, 1, 24 or 168 hours that you choose. ([#422](https://github.com/logicplanes/kaeso/pull/422))
- Request logs, the console and a `Server-Timing` header show the time Kaeso itself spends on each model request, apart from the provider's time. The log gains `overhead_ms` and `stream_processing_ms` columns, and the `kaeso_gateway_overhead_seconds` metric records the same per provider. ([#423](https://github.com/logicplanes/kaeso/pull/423))
- Codex and the ChatGPT desktop app can use Claude, Grok and Mistral models. Namespaced tools become plain function tools, hosted web search maps to Claude's web search tool, and client metadata stays out of other formats. Claude always searches live, so a Codex setting of cached search is refused for Claude with a hint to set live search or turn search off, and the refusal for Mistral gives the same hint. ([#396](https://github.com/logicplanes/kaeso/pull/396))

#### Documentation

- The new `kaeso-bench` crate measures the time and memory Kaeso adds under thousands of concurrent streams, client disconnects and slow clients, end to end over real sockets. Its report gives the Kaeso version, commit, CPU and every parameter with each run. ([#425](https://github.com/logicplanes/kaeso/pull/425))

### Changed

- Requirements for the end-to-end suites are now installed in CI with hash checking, so every package is pinned with its hashes. Two advisory fixes update `undici` to 8.12.3 and `crypto-bigint` to 0.7.6. ([#384](https://github.com/logicplanes/kaeso/pull/384))
- Diagrams in the documentation sit centred in a framed well with a caption. Kaeso's own parts are drawn in ink, outside services are dashed, and edge labels no longer have lines running through them, with the frame provided by a new `Figure` component in the UI library. ([#388](https://github.com/logicplanes/kaeso/pull/388))
- On phones, long lines in code blocks wrap with a hanging indent instead of running off the edge, and Copy still copies the code as written. ([#402](https://github.com/logicplanes/kaeso/pull/402))
- The documentation search no longer shows a count beside each group, while the console's pickers and palette keep theirs. ([#403](https://github.com/logicplanes/kaeso/pull/403))
- Table headers can wrap onto several lines with the `wrap` option of `Table.HeaderCell`, and the documentation wraps headers longer than 24 characters, so the sign-in comparison fits its column. ([#405](https://github.com/logicplanes/kaeso/pull/405))
- The `density` option of `Table` set to compact puts 8px between columns, and the documentation uses it for tables of five or more columns. Wide Configuration and Chinese KPP tables now fit desktop widths, and long type names may break between words. ([#408](https://github.com/logicplanes/kaeso/pull/408))
- Development dependencies in the console, docs and UI library move `happy-dom` to 21.0.4, which fixes a code injection vulnerability in its ESM compiler. ([#414](https://github.com/logicplanes/kaeso/pull/414))
- On phones, a wide diagram in the documentation opens on the part it is about, the nodes marked in ink, instead of its middle. The `Figure` component takes a `focus` option for the share of its width to open on. ([#418](https://github.com/logicplanes/kaeso/pull/418))
- Wrapped code lines keep the indent of the line they continue, so a continued command option sits under its own option. The Platform API error example, the last code block that scrolled on desktop, now wraps at every width. ([#430](https://github.com/logicplanes/kaeso/pull/430))

#### Documentation

- The diagrams on the introduction, concept, self-hosting and development pages are redrawn to show the install's boundary, each check's statuses and the path of a request. The Upgrades page gains a diagram that links each update result to the step where it happens. ([#389](https://github.com/logicplanes/kaeso/pull/389), [#390](https://github.com/logicplanes/kaeso/pull/390), [#391](https://github.com/logicplanes/kaeso/pull/391))
- On phones, documentation tables of three or more columns show each row as a list with its labels instead of scrolling sideways. ([#394](https://github.com/logicplanes/kaeso/pull/394))
- Console, guide, API, self-hosting and contributor pages are split into shorter sentences and paragraphs, with lists where they listed things. No fact, setting or command changed. ([#395](https://github.com/logicplanes/kaeso/pull/395), [#397](https://github.com/logicplanes/kaeso/pull/397), [#398](https://github.com/logicplanes/kaeso/pull/398))
- The KPP reference pages read in shorter sentences, with lists for what each codec refuses and drops. The provider profiles and the Chat Completions providers move to a new page, and links to it are updated. ([#399](https://github.com/logicplanes/kaeso/pull/399))
- Documentation search shows pages whose title or description match first, then matching sections, then pages that only mention the words. ([#400](https://github.com/logicplanes/kaeso/pull/400))
- The changelog is no longer built into the documentation. The sidebar's Changelog link goes to the changelog on the Kaeso website, in the page's language, and old changelog addresses redirect there. ([#401](https://github.com/logicplanes/kaeso/pull/401))
- Long code in documentation tables breaks after separators such as slashes, dots and equals signs, so reference tables such as the Platform API routes fit the desktop column. ([#406](https://github.com/logicplanes/kaeso/pull/406))
- The two KPP allowlist tables on the cross-format page now fit the desktop width, with scope and origin dialect in one column. ([#407](https://github.com/logicplanes/kaeso/pull/407))
- Shell commands and code samples are reformatted to fit the page on desktop, with long commands continuing on the next line after a backslash. The upgrade script names the release address once and checks the hash in two steps. ([#409](https://github.com/logicplanes/kaeso/pull/409))
- Markdown, text and plain-text examples wrap like prose at every width instead of scrolling sideways. ([#410](https://github.com/logicplanes/kaeso/pull/410))
- On phones, a table column with mostly short values sits beside its labels even when one value is long. ([#411](https://github.com/logicplanes/kaeso/pull/411))
- The diagram on the benchmark page runs top to bottom and fits a phone without scrolling. The stream counts in its options table are written as 100, 1,000 and 5,000. ([#428](https://github.com/logicplanes/kaeso/pull/428))
- The key rotation example in the Platform API guide names the key's address once, so its command fits the desktop column. ([#429](https://github.com/logicplanes/kaeso/pull/429))
- The German and Chinese custom provider examples send the English prompt, so their code matches the other languages. ([#431](https://github.com/logicplanes/kaeso/pull/431))
- The Configuration settings tables show each variable and TOML key in one column, so most names stay on one line on desktop and phones, and a table cell can now hold a line break. ([#432](https://github.com/logicplanes/kaeso/pull/432))

### Fixed

- A non-streaming Responses request through a ChatGPT plan now returns the reasoning, web search call and message items with their types, as OpenAI does. Before, the items came back without a type. ([#416](https://github.com/logicplanes/kaeso/pull/416))
- A stream whose last event reached the app is logged as a success, even when the app closes the connection right after it. Before, such requests could be logged as client disconnects with status 499, and a provider that closed its stream after the last event could fail the request. ([#421](https://github.com/logicplanes/kaeso/pull/421))
- A stream that ends because the app stopped reading it is logged as `client_stalled` instead of `client_disconnected`, and the console and metrics show the new kind. Real disconnects stay `client_disconnected`. ([#424](https://github.com/logicplanes/kaeso/pull/424))

#### UI library

- On this page marks the section you are reading when you scroll back up, and marks the last section at the bottom of a page. ([#393](https://github.com/logicplanes/kaeso/pull/393))
- Tables no longer scroll sideways by two pixels when a link ends a text column at the table's edge. ([#404](https://github.com/logicplanes/kaeso/pull/404))

#### Documentation

- Long page titles in the documentation sidebar no longer end in an ellipsis, with short navigation labels in English, German and Chinese. ([#392](https://github.com/logicplanes/kaeso/pull/392))
- A diagram group's title moves along its top edge when an arrow enters the group at its top-left corner, so the arrow is no longer hidden behind the title. ([#415](https://github.com/logicplanes/kaeso/pull/415))

### Security

- Hard spend budgets and token rates refuse a request whose output bound could cross them, even when it is the only request in flight. Password reset is refused where the organization requires SSO, and the Azure wire server address is blocked as a metadata address. ([#385](https://github.com/logicplanes/kaeso/pull/385))

## [0.4.1] - 2026-10-23

Kaeso 0.4.1 is a patch release with no breaking changes, so existing provider keys, integrations and configuration need no change. It fixes the stop reason that Chat Completions and Messages clients receive when a ChatGPT account calls a tool, and it corrects many console and UI library layouts, most visibly on phones. Dependency updates for the gateway, console, docs and showcase are included.

**Contributors:** @dvnold

**Full changes:** [v0.4.0...v0.4.1](https://github.com/logicplanes/kaeso/compare/v0.4.0...v0.4.1)

### Changed

- The Rust workspace moves the `p256` crate from 0.13.2 to 0.14.0. Installs need no configuration change. ([#372](https://github.com/logicplanes/kaeso/pull/372))
- The pnpm workspace moves React and React DOM to 19.3.1 and `happy-dom` to 21.0.3 across the console, docs, showcase and UI library, with their type packages and `@tanstack/react-virtual` updated alongside. ([#376](https://github.com/logicplanes/kaeso/pull/376))

#### UI library

- The showcase app moves `@astrojs/react` to 7.1.0 and `@astrojs/mdx` to 8.1.0, which raise the minimum Node.js version to 22.22.0 for the showcase. ([#374](https://github.com/logicplanes/kaeso/pull/374), [#375](https://github.com/logicplanes/kaeso/pull/375))

### Fixed

- The QR code keeps its square size beside text, so the Grok sign-in code and the authenticator setup code on Security no longer sit small in a tall box. ([#377](https://github.com/logicplanes/kaeso/pull/377))
- Chat Completions and Messages clients now get `finish_reason` `tool_calls` and `stop_reason` `tool_use` when a ChatGPT account calls a tool, streamed or not, instead of `stop` or `end_turn`. The `openai_responses` mapping page now describes how the stop reason is derived. ([#379](https://github.com/logicplanes/kaeso/pull/379))
- Project limit meters, audit log filters, the instance organization stat captions, settings page headers and role badges now line up across the console, including on phones. The settings header wraps like the page header, and role badges use one neutral style everywhere. ([#380](https://github.com/logicplanes/kaeso/pull/380))

#### Console

- Console tables give room to the columns that hold the most text, so names, models, targets and costs stay readable on phones and desktop. The org overview breakdown is now a data table on phones, where it previously ran off screen. ([#383](https://github.com/logicplanes/kaeso/pull/383))

#### UI library

- Collapsible rows such as Customize and Every right are now plain disclosures without a box or padding, so their chevrons and hints line up with the section around them. ([#378](https://github.com/logicplanes/kaeso/pull/378))
- Settings rows stack their controls under their text on phones, and ghost buttons at a row's end align with outlined buttons. Chart table toggles get their own line on phones, code blocks fade out at the edge they can scroll to, and pinned table headers run straight to the edge. ([#381](https://github.com/logicplanes/kaeso/pull/381))
- Off switches read clearly in dark mode, and select and date triggers share the outline of text inputs. Side sheets fill the phone screen, copyable ids keep their copy icon inside the card edge, and picker option counts line up with the values beside them. ([#382](https://github.com/logicplanes/kaeso/pull/382))

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

[Unreleased]: https://github.com/logicplanes/kaeso/compare/v0.7.5-rc.2...HEAD
[0.7.5-rc.2]: https://github.com/logicplanes/kaeso/releases/tag/v0.7.5-rc.2
[0.7.5-rc.1]: https://github.com/logicplanes/kaeso/releases/tag/v0.7.5-rc.1
[0.7.4]: https://github.com/logicplanes/kaeso/releases/tag/v0.7.4
[0.7.3]: https://github.com/logicplanes/kaeso/releases/tag/v0.7.3
[0.7.2]: https://github.com/logicplanes/kaeso/releases/tag/v0.7.2
[0.7.1]: https://github.com/logicplanes/kaeso/releases/tag/v0.7.1
[0.7.0]: https://github.com/logicplanes/kaeso/releases/tag/v0.7.0
[0.6.0]: https://github.com/logicplanes/kaeso/releases/tag/v0.6.0
[0.5.6]: https://github.com/logicplanes/kaeso/releases/tag/v0.5.6
[0.5.5]: https://github.com/logicplanes/kaeso/releases/tag/v0.5.5
[0.5.4]: https://github.com/logicplanes/kaeso/releases/tag/v0.5.4
[0.5.3]: https://github.com/logicplanes/kaeso/releases/tag/v0.5.3
[0.5.2]: https://github.com/logicplanes/kaeso/releases/tag/v0.5.2
[0.5.1]: https://github.com/logicplanes/kaeso/releases/tag/v0.5.1
[0.5.0]: https://github.com/logicplanes/kaeso/releases/tag/v0.5.0
[0.4.8]: https://github.com/logicplanes/kaeso/releases/tag/v0.4.8
[0.4.7]: https://github.com/logicplanes/kaeso/releases/tag/v0.4.7
[0.4.6]: https://github.com/logicplanes/kaeso/releases/tag/v0.4.6
[0.4.5]: https://github.com/logicplanes/kaeso/releases/tag/v0.4.5
[0.4.4]: https://github.com/logicplanes/kaeso/releases/tag/v0.4.4
[0.4.3]: https://github.com/logicplanes/kaeso/releases/tag/v0.4.3
[0.4.2]: https://github.com/logicplanes/kaeso/releases/tag/v0.4.2
[0.4.1]: https://github.com/logicplanes/kaeso/releases/tag/v0.4.1
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
