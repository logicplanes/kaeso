# AGENTS.md

Rules for the console, `apps/console`. The root [`AGENTS.md`](../../AGENTS.md) holds the definition of done and the rules for the whole repository; commands run from the repository root.

## What the console is

The **Kaeso console**: the web interface of a Kaeso install. People set Kaeso up, sign in, manage organizations, projects, members, API keys, provider keys, models and limits, compare models in the playground, and read usage and request logs. The contract is the repository's [`openapi.json`](../../openapi.json), which the gateway generates from its code, so the console and the gateway change in the same pull request; the look is [`@logicplanes/kaeso-ui`](../../packages/ui).

- **Is:** a single-page React application served on the same domain as the gateway. It talks only to the Platform API under `/platform/v1`.
- **Is not:** a design system. Every visible element is a `@logicplanes/kaeso-ui` component; the console owns composition, data and behavior. A missing component or variant is added to `packages/ui`, in the same pull request when the console needs it.
- **Is not:** a backend. No mock API, no demo mode, no server code. Development and tests run against a real gateway, built from this repository, in Docker (`pnpm stack:up`).
- **Not a place for plans.** Roadmaps, to-do lists and open questions go into issues.

## Setup

```
corepack enable
pnpm install
pnpm --filter kaeso-console exec playwright install chromium
pnpm stack:up
pnpm seed
pnpm dev
```

Node.js 24, pnpm 11 and a Docker engine with Compose. `pnpm stack:up` builds the gateway's image from `deploy/gateway.Dockerfile` (`KAESO_GATEWAY_IMAGE` names a prebuilt one instead) and starts it with PostgreSQL, ClickHouse, Valkey, Mailpit, a mock model provider, a mock identity provider, a mock ChatGPT and Grok sign-in and a mock release feed (the `console` profile of `deploy/compose.dev.yaml`, the repository's one development stack); `pnpm seed` creates an organization with projects, members, keys and weeks of usage; `pnpm dev` builds the libraries in `packages/`, starts the stack and serves the console at http://localhost:3000. Sign in as `owner@example.com` with the password `correct horse battery staple`.

## Where things are

How the console reads data, checks rights, loads its code and shows errors is in [console architecture](https://docs.kaeso.app/development/architecture/console).

| Folder          | Job                                                                                                                                                                         |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/routes/`   | File-based routes: public pages, app mode (`/orgs/{org}/projects/{project}/…`) and settings mode (`/settings/…`), each with a loader, a pending skeleton and an error state |
| `src/api/`      | The client (its types generated from the repository's `openapi.json`), errors, paging, streamed downloads, and one query hook per resource                                  |
| `src/auth/`     | The session, rights helpers, guards (two-factor, single sign-on, scheduled deletion) and the recent sign-in dialog                                                          |
| `src/shell/`    | `AppFrame` in both modes, the sidebar, switchers, the command palette and shortcuts                                                                                         |
| `src/features/` | One folder per area: sign-in, public pages, account, the organization's home (`org-home/`: overview, projects, providers) and settings (`org/`), project, instance          |
| `src/kaeso/`    | Console-only compositions of library parts: the rights and limit editors, the create-key flow, usage panels                                                                 |
| `src/format/`   | Money, periods, labels and other values as the console writes them                                                                                                          |
| `src/i18n/`     | The message function, the catalogs in English, German and Simplified Chinese, and the language at start                                                                     |
| `src/test/`     | The unit-test harness: rendering the app, the network stub, the fixed clock and the fixtures                                                                                |
| `src/`          | At its top: the entry point, the router, the query client, search-parameter patches and the storage helpers                                                                 |
| `eslint/`       | The layout-only, no-color and no-literal-text lint rules, with tests                                                                                                        |
| `scripts/`      | Client and route generation, the stack, the seed, the README's recording, bundle budgets, the licence check and the catalog check                                           |
| `stack/`        | The mock provider, the mock identity provider, the mock ChatGPT and Grok sign-in, the mock release feed and updater, and the master key the local stack uses                |
| `public/`       | The favicons and the web app manifest                                                                                                                                       |
| `e2e/`          | Playwright suites against a stack of their own                                                                                                                              |

## Checks

The root definition of done covers the console. To run the console's own checks alone, after `pnpm build:packages` when the libraries changed:

```
pnpm --filter kaeso-console lint
pnpm --filter kaeso-console typecheck
pnpm --filter kaeso-console test
pnpm --filter kaeso-console i18n
pnpm --filter kaeso-console build
pnpm --filter kaeso-console licenses:check
pnpm --filter kaeso-console e2e
```

`test` runs Vitest with coverage thresholds. `build` fails when a route's JavaScript exceeds its budget (`scripts/budget.mjs`); `licenses:check` then checks the licence of every package in the bundle. `e2e` starts the console's own stack on other ports (console 3100, gateway 28080), so it never touches the development data; `KAESO_GATEWAY_IMAGE` runs it against a prebuilt gateway image. The container image is `deploy/console.Dockerfile` with `deploy/console/Caddyfile`; `docker build -f deploy/console.Dockerfile .` builds it. Then look at every page you changed in light and dark, at desktop and phone widths, in English, German and Chinese.

## Rules

1. **Only the Platform API**, and every call through the client in `src/api/`. Nothing the API cannot do is shown; nothing is faked.
2. **Rights come from the API, never guessed.** Org rights from `auth/session`; project rights from `GET …/projects/{project}/my-rights`; each right's name, description, category and needs from the catalogue (`GET orgs/{org}/rights`), never from code. A `403` is the final word: hide a whole area the person cannot see, disable a single action with the right's name and who can give it ("You need Export logs; ask Ada Okafor for it.", `useNeedsRight`). Providers too: each provider's name, key page, key prefix and formats come from `GET orgs/{org}/providers` (`useProviders`), never from code.
3. **No secrets client-side.** The session is an HttpOnly cookie the console never reads. Key secrets are shown once and never stored. `localStorage` holds only the last org and project, recently opened items, theme, language, the sidebar's width and state, the request log's columns and the playground's unsent prompts; `sessionStorage` holds an invitation's token only while its invitee signs in to accept it, and an agent's sign-in request only while its user signs in to answer it. Tokens from links leave memory and query keys once used, and mutations keep no result once their component is gone.
4. **Every word follows the voice** of [`@logicplanes/kaeso-ui`'s writing rules](../../packages/ui/foundations/writing.md): sentence case, a verb first on buttons, no exclamation points, errors that say what to do next. Show the API's `message` (translated where the console knows it) and attach it to the fields its `param` names.
5. **Every visible element from `@logicplanes/kaeso-ui`; `className` for layout only** (margin, width, grid, flex, gap, position). No colors, radii, type sizes, padding, heights, shadows or inline styles. `pnpm lint` enforces it (`eslint/kaeso.js`).
6. **No page-enter animation, no decorative cards, no decorative icons.** Content sits on the panel; motion comes from the library.
7. **The wordmark alone, top left:** the sidebar header shows `Logo.Wordmark`; the mark never appears in the interface.
8. **Tokens in URLs travel in the fragment.** Read `location.hash` once, call the API, replace the URL without it.
9. **No setup checklists.** No numbered step lists, onboarding blocks or banners in their place. A new project shows its normal pages with the library's empty states. A task in several steps is a `StepDialog`.
10. **Every word in English, German and Simplified Chinese** (the catalogs in `src/i18n/locales`, checked by `scripts/i18n.mjs`): text goes through `t("area.key", "English")`, whole sentences with placeholders, never fragments. `pnpm lint` refuses literal text; `pnpm i18n` checks the catalogs.
11. **Tests assert on what a person sees.** The coverage thresholds are in `vitest.config.ts`.
12. **Dialogs open fresh.** A dialog's fields, step, result and any secret it shows live in a component that mounts anew on every open, never beside the dialog in the page: inside a `DialogScope` with the dialog's `open`, which also keeps the close animation and forgets a secret once the dialog has closed. A sheet for a row keys its body by the row's id.

## Descriptions

A description is one short sentence that fits on one line. Drop detail before letting it wrap; detail a person needs goes where they need it: the dialog the row opens, the field's placeholder, or the toast when something is refused. No second sentence: join two clauses with a semicolon or cut one.

| Where                                                                         | Limit                                                |
| ----------------------------------------------------------------------------- | ---------------------------------------------------- |
| `SettingsRow` description                                                     | 60 characters; 50 when the control holds an `Input`  |
| `SettingsSection`, `SettingsPage`, `OrgPage`, `AppFrame.Settings` description | 80 characters; 70 when the header has `actions`      |
| `Dialog`/`AlertDialog` and dialog step descriptions                           | 100 characters, one sentence where possible          |
| `Empty` description                                                           | 90 characters, one sentence                          |
| Toast                                                                         | 60 characters, one sentence; a period only on errors |

Measured, not guessed: descriptions are Inter at 13px (`text-small`) in rows and sections and 15px (`text-body-lg`) under a page title, about 6.1px a character on average and 6.9px at most (7.3px at 15px). The settings column is 760px less 24px padding; a row's text gets 672px less its 20px padding, 16px gap and control. 60 characters (415px at most) fit beside any control up to 240px; an input with its Save button takes 305px, leaving room for 50. A header description has the column's 712px, less an action. Dialog and empty-state descriptions may take two lines of their narrower column; toasts stay on one. Each `${…}` in a template and each `{placeholder}` in a message counts as 12 characters.

`src/copy.test.ts` checks the row, section, page, dialog, step and empty-state limits, and the one sentence wherever the table asks for it, in every `description=` string, template literal or message it can read on those components and in a `StepDialog`'s steps, in all three languages; `src/toast-copy.test.ts` checks the length, the one sentence and the final period of every toast whose message it can read. A description or toast built in an expression they can't read is yours to keep within the limit.

## Style

- TypeScript strict; small modules with one job; explicit types at module boundaries.
- No dead code and no commented-out code.
- `src/api/generated/` and `src/routeTree.gen.ts` are generated by `pnpm --filter kaeso-console generate` and never committed.
