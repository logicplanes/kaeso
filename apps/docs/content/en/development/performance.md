---
title: Performance
description: The bundle budgets of the console, the docs and the UI library, the codec benchmarks, and the fuzzing results.
order: 9
---

Performance is measured, not guessed. The console's and the docs' builds fail when a page grows past its budget, the UI library's build fails when a component does, and KPP's codecs have benchmarks and fuzz targets. A change that needs a larger budget raises it in the same pull request, says why, and updates this page.

All sizes are gzipped; 1 kB is 1,024 bytes.

## The console

`pnpm --filter kaeso-console build` ends with `apps/console/scripts/budget.mjs`. It reads the build's manifest (`dist/.vite/manifest.json`) and adds up the JavaScript a route needs before it paints:

- the entry chunk and everything it imports statically;
- the lazy chunks of the route and of every layout route above it (`__root`, `_app`, `orgs/$org`, `projects/$project` and so on), with their static imports;
- for the project overview, the route gates (`src/auth/route-gates.ts`), which the frame loads beside the session before the page paints.

Code that loads when used is not counted: tooltip positioning, the date picker's calendar, the command palette, the recent sign-in dialog, the shortcuts list, **Create organization**, the toaster, and the parts of a page that load beside its data. The script also fails the build when a module under `src/features` ends up in the entry or its static imports, since every page, sign-in included, loads the entry first.

| Route | File | Target | Ceiling | Measured |
| --- | --- | --: | --: | --: |
| Sign-in | `src/routes/sign-in.tsx` | 120 kB | 126 kB | 123.1 kB |
| Project overview | `src/routes/_app/orgs/$org/projects/$project/index.tsx` | 180 kB | 190 kB | 181.6 kB |

A route over its ceiling fails the build. The target is the size a route should reach; neither route meets it yet. The ceiling is the measured size plus a few kilobytes, about 5% where a route has shrunk, so a small change fits and a regression fails. A ceiling is raised only with a reason, in the pull request that needs it, and lowered when a route gets smaller.

Of sign-in's 123.1 kB:

| Chunk | Holds | Size |
| --- | --- | --: |
| `framework` | React and React DOM with the scheduler, TanStack Router and Query, openapi-fetch, and the console's API client, session and formatting | 95.3 kB |
| `ui` | The library's `cn` with its own `tailwind-merge` setup, its button, text, field, input and wordmark, the page title and the public page's layout | 13.3 kB |
| The entry | The route tree and its preload lists | 9.7 kB |
| The sign-in page | Its own chunks | The rest |

The project overview adds the console's frame, the stat tiles and charts, and the page. The frame is the sidebar with its Base UI and Floating UI parts; its menus load once the page is idle or on the first hover, focus or press, and the navigation sheet, with Base UI's dialog, once the page is idle or on the first tap. The charts use d3 with the library's own linear scale, without d3-scale's colour, interpolation and number-format modules.

`src/main.tsx` requests the chunks of the routes the address matches (`loadPageChunks` in `src/router.tsx`) before it loads a language and renders, so a page's code downloads while the entry is still running. German and Chinese fetch the console's texts and the library's at the same time.

### What moves the numbers

- **Routes.** The route tree is in the entry, so every route's path, search validation and `beforeLoad` are part of every page. A route file imports only types from its page; a value import would pull the page's module and its imports into the entry. The search check is a function in the route file that `validateSearch` and the component's `Route.useSearch({ select })` share, so it is in the entry once. Loaders, components, and pending, error and not-found components are split into the route's own chunks.
- **Modules the entry imports** are in the first paint whole, with everything they import, including exports only later pages use. That is why the session's queries (`src/auth/session-queries.ts`) and the period's search check (`src/format/period-search.ts`) are modules of their own: TanStack Query's observers and the date labels stay with the pages that use them.
- **Preload lists.** For every lazy chunk the route tree can load, the entry carries the list of chunks it needs, so the browser fetches them at once. Every chunk in a list costs its file name, so a new shared chunk that many pages reach makes every page larger.
- **Shared chunks.** The bundler puts modules that several lazy pages share into one chunk. When a page the overview doesn't need shares a module with one it does, the overview loads that chunk too, with whatever else is in it.
- **The library.** A `@logicplanes/kaeso-ui` component counts when the first paint draws it or imports it through something it draws; one that loads on interaction doesn't.
- **First paint versus later.** Anything shown only after data arrives, after an interaction or after the first paint can load then. Examples: the overview's breakdown and aside (`src/features/project/overview-parts.tsx`), its bar charts (`src/kaeso/usage-bars.tsx`) and period picker (`src/kaeso/period-picker.tsx`), sign-in's steps after the address (`src/features/sign-in/sign-in-steps.tsx`), the settings sidebar, the sidebar's menus, the connection toast, and the single-key shortcuts (`src/shell/listeners.tsx`, with keys pressed before they load kept by `src/shell/early-keys.ts`).

### What the build does to keep them small

In `apps/console/vite.config.ts`:

| Setting | Effect |
| --- | --- |
| `uiEntryImports` | Rewrites `import { … } from "@logicplanes/kaeso-ui"` to the library's per-component entry points, and each `icons.name` to that one icon's import from `@hugeicons/core-free-icons`, so a route carries only the components and icons it draws |
| `codeSplittingOptions` | Makes the router plugin split each route's loader, component, and error, not-found and pending components into chunks of their own (`autoCodeSplitting`) |
| `codeSplitting.groups` | Puts the framework and the console's first-paint modules (`src/api`, `src/auth`, `src/format`) in one `framework` chunk; the library parts every page paints with (`cn`, button, logo, text, section, field, input group, the page title, the public page's layout and the three icons every page draws) in one `ui` chunk; and the route gates with what the frame imports anyway in a `gates` chunk. Split by which pages share them, these modules would compress worse and lengthen every preload list |
| `routePreloads`, `resolvePreloads` (`build.modulePreload.resolveDependencies`) | Leave out of the preload lists the chunks the entry already imports, under `_app` the chunks the frame brings, and the chunks only the instance settings pages and the playground need; those load with their page's own chunk |
| `chunkFileNames` | Names shared chunks by seven characters of hash alone; pages and the `framework` and `ui` groups keep their names. The manifest and the source maps say what each holds |
| `build.modulePreload.polyfill` | Off: every browser the build targets preloads modules itself |
| `resolve.alias` | Sends `use-sync-external-store`'s shim to React's own `useSyncExternalStore`, so the router's store and Base UI don't carry a second implementation |
| `themeInit`, `fontPreload` | Serve the library's pre-paint theme script as `/theme-init.js` and preload the interface font, so the first paint has its theme and its type without an inline script |

## The documentation site

`pnpm --filter kaeso-docs build` ends with `apps/docs/scripts/budget.ts`, which measures what a page needs before it can paint and fails the build past a ceiling.

| Budget | Counts | Measured | Ceiling |
| --- | --- | --: | --: |
| The frame, per language | The entry with its static imports and stylesheet, the language's site module and the library's messages | 201.9 kB (English), 207.4 kB (German), 207.9 kB (Chinese) | 216 kB |
| One page's content | Each page's own chunk: its blocks and table of contents | 17.8 kB at most (`zh-CN/development/kpp/anthropic-messages`) | 24 kB a page |

The frame grows a little with every page, because the language's site module carries the sidebar and every page's title and description, so the sidebar paints with the frame and never shifts. German and Chinese also load the library's messages, about 5 kB. The frame's ceiling is the largest frame with a little room. A page's content has a ceiling of its own, so a page that grows too long fails with its address, whoever edits it; the answer is to split it into two pages.

The search with its index, the toaster, the navigation sheet and the code highlighter's grammars load after the first paint and are not counted. The build prints the search's size, 6.7 kB without the index, Base UI's dialog included.

| Part of the frame | Why it is there |
| --- | --- |
| React and TanStack Router (`framework`) | Every page |
| `AppFrame`, `Sidebar` | The frame; `Sidebar` brings Base UI's menu, and the navigation sheet loads once the page is idle or on the first tap |
| `Select` | The language and theme selects at the foot of the sidebar |
| Tooltips, `IconButton` | The search and collapse controls |
| The page and its renderer | `Text`, `Link`, `Code`, `CodeBlock` (its highlighter loads only with a code block), `Banner`, `Table`, `ScrollArea`, `CheckboxBox`, `Separator`, `TableOfContents`, and `Badge` with the date formatter for the changelog |
| The language's `site` module | The sidebar and every page's title and description |

The site also keeps the first paint small and stable:

- Imports from `@logicplanes/kaeso-ui` are rewritten to the library's per-component entries at build time (`uiEntryImports` in `apps/docs/vite.config.ts`).
- Pages are compiled at build time, so marked and the content build never reach the browser.
- Every page's HTML preloads the chunks of its language and of the page itself, so the first paint doesn't wait for a second round trip.
- The interface font is preloaded (`fontPreload`), so the first paint is in Inter and nothing reflows when it arrives.
- Images carry their width and height from the file, so the page doesn't shift as they load.

## The UI library

`pnpm build:packages` builds `@logicplanes/kaeso-ui` and ends with `packages/ui/scripts/budgets.mjs`, which measures the built package in `packages/ui/dist` and fails the build past a budget. An app pays only for the components it imports, so the budgets cover what one component costs on its own.

| Budget | Counts | Measured | Budget |
| --- | --- | --: | --: |
| The foundation | The shared chunks that at least 40% of the component entries import | 5.3 kB | 8 kB |
| `Button` | `components/button.js` and what it imports, without the foundation and other components' entries | 3.0 kB | 3 kB |
| `Menu` | `components/menu.js`, counted the same way | 4.8 kB | 9 kB |
| `DataTable` | `components/data-table.js`, counted the same way | 10.3 kB | 22 kB |
| `styles.css` | The whole stylesheet | 18.0 kB | 28 kB |

The same script fails the build when:

- a module in `dist` has a statement at module level that a bundler must keep, which would stop unused components from being dropped;
- a file guards `process.env.NODE_ENV` with `typeof process`, which keeps development checks in browser bundles;
- `formatMessage`, `useUiText` or `useLocale` would bring the formatters (`format.js`) with them. `formatMessage` alone is 0.7 kB.

## KPP

```bash
cargo bench -p kaeso-codecs --bench codecs
```

Each benchmark runs a full same-dialect cycle: decode the wire bytes into KPP, then encode KPP back into wire bytes. The inputs are built from the same KPP content for every dialect, so the numbers are comparable. Measured with criterion (`--warm-up-time 2 --measurement-time 5`) on an Apple M5 Pro, Rust 1.99.0, release profile:

| Benchmark | Anthropic Messages | OpenAI Chat | OpenAI Responses |
| --- | --: | --: | --: |
| 100-message conversation (a system prompt, a tool, 25 rounds of user text, assistant text with a tool call, a tool result and assistant text), decode and encode | 352 µs (78 MiB/s) | 242 µs (105 MiB/s) | 279 µs (90 MiB/s) |
| 2,000-event text stream fed in 512-byte chunks, decode and encode | 3.9 ms (about 2.0 µs an event, 61 MiB/s) | 10.6 ms (about 5.3 µs an event, 36 MiB/s) | 8.6 ms (about 4.3 µs an event, 52 MiB/s) |

For the gateway, a long conversation costs well under a millisecond in each direction and a streamed token a few microseconds, far below the tens of milliseconds between tokens from any model. The Chat and Responses stream codecs do more work per event than Anthropic's: every Chat chunk repeats the response envelope, and every Responses event carries a sequence number, some repeating the accumulated item. That is where optimisation would start.

CI checks that the benchmarks build (`cargo bench -p kaeso-codecs --bench codecs --no-run`). A change to performance-sensitive codec code compares a full run with the table above.

## Fuzzing

The [fuzz targets](testing.md#fuzzing) last ran for 10 minutes each, all five in parallel on the machine above, with `-rss_limit_mb=2048 -max_len=65536`:

| Target | Executions | Coverage (edges) | Peak memory | Result |
| --- | --: | --: | --: | --- |
| `sse` | 7,628,740 | 372 | 707 MB | No crash |
| `decode_anthropic_messages` | 3,599,755 | 11,047 | 588 MB | No crash |
| `decode_openai_chat` | 2,044,761 | 11,347 | 570 MB | No crash |
| `decode_openai_responses` | 1,166,478 | 10,263 | 571 MB | No crash |
| `decode_openai_embeddings` | 11,245,268 | 4,359 | 668 MB | No crash |

Peak memory is libFuzzer's whole process, including its in-memory corpus; it stayed flat after the first minutes, and no input reached the 2 GB limit. The library's memory per input is bounded by the input's size and the SSE frame limit (`sse::DEFAULT_MAX_FRAME_BYTES`, 16 MiB).
