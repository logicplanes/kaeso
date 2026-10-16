---
title: Performance
description: The bundle budgets of the console, the docs and the UI library, the codec benchmarks, and the fuzzing results.
order: 8
---

Performance is measured, not guessed: the console's and the docs' builds fail when a page grows past its budget, the UI library's build fails when a component does, and KPP's codecs have benchmarks and fuzz targets. A change that needs a larger budget raises the ceiling in the same pull request, updates this page, and says why.

## The console

`pnpm --filter kaeso-console build` ends with `apps/console/scripts/budget.mjs`, which reads the build's manifest (`dist/.vite/manifest.json`) and adds up, gzipped, the JavaScript a route needs before it paints:

- the entry chunk and everything it imports statically;
- the lazy chunks of the route and of every layout route above it (`__root`, `_app`, `orgs/$org`, `projects/$project` and so on), with everything they import statically;
- for the project overview, the console routes' decisions (`src/auth/route-gates.ts`), which the frame loads beside the session before the page paints.

Dynamic imports that load when used are not counted: tooltips' positioning, the date picker's calendar, the command palette, the recent sign-in dialog, the shortcuts list, "Create organization", the toaster, and the parts of a page that load beside its data. The script also fails the build when a module under `src/features` is in the entry or its static imports, since every page, sign-in included, loads the entry first.

A route fails the build when it is over its ceiling. The target is the size a route should reach; the ceiling is the measured size rounded up to the next kilobyte, so any growth fails the build.

| Route | File | Target | Ceiling | Measured, gzipped |
| --- | --- | --: | --: | --: |
| Sign-in | `src/routes/sign-in.tsx` | 120 kB | 130 kB | 129.4 kB |
| Project overview | `src/routes/_app/orgs/$org/projects/$project/index.tsx` | 180 kB | 238 kB | 237.7 kB |

1 kB is 1,024 bytes. Neither route meets its target yet. Of sign-in's 129.4 kB, the `framework` chunk (React and React DOM with the scheduler, TanStack Router and Query, openapi-fetch, and the console's API client, session and formatting) is 97.9 kB, the `ui` chunk (the library's `cn` with `tailwind-merge`, its button, text, field, input and logo, the page title and the public page's layout) 18.4 kB, the entry with the route tree and its preload lists 8.9 kB, and the sign-in page's own chunks the rest. The project overview adds the console's frame (sidebar and menus with their Base UI and Floating UI parts), the stat tiles and charts with their d3 modules, and the page.

### What moves the numbers

- **Routes.** The route tree is in the entry, so every route adds its path, its search validation and its `beforeLoad` to every page. A route file imports only types from its page: a value would bring the page's module and its imports into the entry. Its search check is a function in the route file that `validateSearch` and the component's `Route.useSearch({ select })` share, so the check is in the entry once and the split component imports it from there. Loaders, components, pending, error and not-found components are split into the route's own chunks.
- **Preload lists.** For every lazy chunk the route tree can load, the entry carries the list of chunks it needs, so the browser fetches them at once. Every chunk in a list costs its file name; a new shared chunk that many pages reach makes every page larger.
- **Shared chunks.** The bundler puts modules several lazy pages share into one chunk. When a page the overview doesn't need shares a module with one it does, the overview loads that chunk too, with whatever else the bundler put in it.
- **The library.** A `@logicplanes/kaeso-ui` component that a first paint draws, or that one it draws imports, is part of the budget; one that loads on interaction is not.
- **First paint versus later.** Anything a page shows only after its data arrives, after an interaction or after the first paint can load then: the overview's breakdown and aside (`src/features/project/overview-parts.tsx`), its bar charts (`src/kaeso/usage-bars.tsx`) and period picker (`src/kaeso/period-picker.tsx`), sign-in's steps after the address (`src/features/sign-in/sign-in-steps.tsx`), the settings sidebar, the sidebar's menus, the connection toast and the single-key shortcuts (`src/shell/listeners.tsx`, with keys pressed before they load kept in `src/shell/early-keys.ts`).

### What the build does to keep them small

`apps/console/vite.config.ts`:

- **`uiEntryImports`** rewrites `import { … } from "@logicplanes/kaeso-ui"` to the library's per-component entry points, and each `icons.name` to that one icon's import from `@hugeicons/core-free-icons`, so a route carries only the components and icons it draws.
- **`codeSplittingOptions`** in the router plugin splits each route's loader, component, error, not-found and pending component into chunks of their own (`autoCodeSplitting`).
- **`codeSplitting.groups`** puts the framework and the console's first-paint modules (`src/api`, `src/auth`, `src/format`) in one `framework` chunk, the library parts every page paints with (`cn`, button, logo, text, section, field, input group, the page title, the public page's layout and the three icons every page draws) in one `ui` chunk, and the route gates with what the frame imports anyway in a `gates` chunk. Split by which pages share them, these modules would compress worse and add file names to every preload list.
- **`routePreloads` and `resolvePreloads`** (`build.modulePreload.resolveDependencies`) leave out of the preload lists the chunks the entry already imports, under `_app` the chunks the console's frame brings, and the chunks only the instance settings pages and the playground need; those load with their page's own chunk.
- **`chunkFileNames`** names shared chunks by seven characters of hash alone; pages and the `framework` and `ui` groups keep their names. The manifest and the source maps say what each holds.
- **`build.modulePreload.polyfill`** is off: every browser the build targets preloads modules itself.
- **`resolve.alias`** sends `use-sync-external-store`'s shim to React's own `useSyncExternalStore`, so the router's store and Base UI don't carry a second implementation.
- **`themeInit`** serves the library's pre-paint theme script as `/theme-init.js`, and **`fontPreload`** preloads the interface font the stylesheet uses, so the first paint has its theme and its type without inline script.

## The documentation site

`pnpm --filter kaeso-docs build` ends with `apps/docs/scripts/budget.ts`, which measures what a page needs before it can paint, gzipped, and fails the build past a ceiling.

| Budget | What is counted | Measured | Ceiling |
| --- | --- | --: | --: |
| The frame in each language | The entry with its static imports and stylesheet, the language's site module and the library's messages | 203.9 kB (English), 209.4 kB (German), 209.8 kB (Chinese) | 216 kB |
| One page's content | Each page's own chunk: its blocks and table of contents | 15.7 kB at most (`zh-CN/development/kpp/anthropic-messages`) | 24 kB a page |

The frame changes with the code and grows a little with every page, because the site module carries the sidebar and every page's title and description, so the sidebar paints with the frame and never shifts. German and Chinese also load the library's messages for their language, about 5 kB. The ceiling is the measured size of the largest frame with a little room. A page's content has a budget of its own, so a page that grows too long fails with its address, whoever edits it; the answer is two pages.

The search with its index, the toaster and the code highlighter's grammars load after the first paint and are not counted. The build prints the search's size, 3.3 kB without the index.

| Part of the frame | Why it is there |
| --- | --- |
| React and TanStack Router (`framework`) | Every page |
| `AppFrame`, `Sidebar`, the navigation sheet | The frame; `Sidebar` brings Base UI's menu with it, which the theme and language menus use too |
| Tooltips, `IconButton` | The search, collapse and theme controls |
| The page and its renderer | `Text`, `Link`, `Code`, `CodeBlock` (its highlighter loads only with a code block), `Banner`, `Table`, `ScrollArea`, `Checkbox`, `Separator`, `TableOfContents` |
| The sidebar and every page's title and description | The language's `site` module, so the sidebar paints with the frame and never shifts |

- Imports from `@logicplanes/kaeso-ui` are rewritten to the library's per-component entries at build time (`uiEntryImports` in `apps/docs/vite.config.ts`), so the frame holds only the components the site uses.
- Pages are compiled at build time: marked and the content build never reach the browser.
- Every page's HTML preloads the chunks its language and the page itself need, so the first paint doesn't wait for a second round trip.
- The interface font is preloaded (`fontPreload`), so the first paint is in Inter and nothing reflows when it arrives.
- Images carry their width and height from the file, so the page doesn't shift as they load.

## The UI library

`pnpm build:packages` builds `@logicplanes/kaeso-ui` and ends with `packages/ui/scripts/budgets.mjs`, which measures the built package in `packages/ui/dist`, gzipped, and fails the build past a budget. An app pays only for the components it imports, so the budgets cover what one component costs on its own.

| Budget | What is counted | Measured | Budget |
| --- | --- | --: | --: |
| The foundation | The shared chunks that at least 40% of the component entries import | 3.3 kB | 8 kB |
| `Button` | `components/button.js` and what it imports, without the foundation and other components' entries | 3.0 kB | 3 kB |
| `Menu` | `components/menu.js`, counted the same way | 4.8 kB | 9 kB |
| `DataTable` | `components/data-table.js`, counted the same way | 9.6 kB | 22 kB |
| `styles.css` | The whole stylesheet | 18.1 kB | 28 kB |

The same script also fails the build when:

- a module of `dist` has a statement at module level that a bundler must keep, so unused components could not be dropped;
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

For the gateway, a long conversation costs well under a millisecond in each direction, and a streamed token a few microseconds, far below the tens of milliseconds between tokens from any model. The Chat and Responses stream codecs do more per event than Anthropic's, since every Chat chunk repeats the response envelope and every Responses event carries a sequence number and some repeat the accumulated item; that is where optimisation would start.

Every change checks that the benchmarks build (`cargo bench -p kaeso-codecs --bench codecs --no-run`); a change to performance-sensitive codec code compares a full run with the table above.

## Fuzzing

The [fuzz targets](testing.md#fuzzing) were last run for 10 minutes each, all five in parallel on the machine above, with `-rss_limit_mb=2048 -max_len=65536`:

| Target | Executions | Coverage (edges) | Peak memory | Result |
| --- | --: | --: | --: | --- |
| `sse` | 7,628,740 | 372 | 707 MB | No crash |
| `decode_anthropic_messages` | 3,599,755 | 11,047 | 588 MB | No crash |
| `decode_openai_chat` | 2,044,761 | 11,347 | 570 MB | No crash |
| `decode_openai_responses` | 1,166,478 | 10,263 | 571 MB | No crash |
| `decode_openai_embeddings` | 11,245,268 | 4,359 | 668 MB | No crash |

Peak memory is libFuzzer's process including its in-memory corpus; it stayed flat after the first minutes, and no input reached the 2 GB limit. The library's memory per input is bounded by the input's size and the SSE frame limit (`sse::DEFAULT_MAX_FRAME_BYTES`, 16 MiB).
