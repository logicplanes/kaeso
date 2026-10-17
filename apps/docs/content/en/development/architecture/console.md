---
title: Console
description: How the web console reads data, draws its pages, loads its code and shows errors.
order: 2
---

The console is a static React app in `apps/console`, served on the gateway's domain. It signs people in through the gateway and keeps nothing but the HttpOnly session cookie the gateway sets. Lists, filters, periods and open details live in the address, so links can be shared and the back button works.

| Part | Where |
| --- | --- |
| Routes, loaders and pages | `src/routes`, `src/features` |
| The Platform API client, typed from `openapi.json`, and its queries | `src/api` |
| Session, rights and route guards | `src/auth` |
| The app frame, sidebar, command palette and shortcuts | `src/shell` |
| Console-only compositions of library parts | `src/kaeso` |
| The languages and their catalogs | `src/i18n` |
| Money, periods, labels and other values as the console writes them | `src/format` |
| Generators for the API types and the route tree, the stacks, sample data, the translation check, budgets and licenses | `scripts` |
| The lint rules for layout-only classes and translated text | `eslint` |
| The end-to-end suite | `e2e` |
| The development stack's mock provider, identity provider and master key | `stack` |

## Rules

- **Only the Platform API**, and every call through the client in `src/api`. Nothing the API can't do is shown, and nothing is faked.
- **Rights come from the API, never guessed.** Organization rights come from `auth/session`, project rights from `GET …/projects/{project}/my-rights`, and each right's name, description, category and needs from `GET orgs/{org}/rights`. A `403` is final: an area a person can't see is hidden, and a single action is disabled with the right's name and who can give it (`useNeedsRight`).
- **No secrets in the browser.** The console never reads the session cookie. Key secrets are shown once and never stored. `localStorage` holds only the last organization and project, recently opened items, the theme, the language, the sidebar's width and state, the request log's columns and the playground's unsent prompts; `sessionStorage` holds an invitation's token until it is accepted.
- **Dialogs open fresh.** What a dialog, sheet or step dialog keeps (fields, step, result, a secret shown once) lives in a component that mounts anew on every open: inside a `DialogScope`, which also unmounts it once the dialog has finished closing, or rendered only while the dialog is open. Nothing comes back from the last open, and a sheet for one row never shows another row's data.
- **Tokens in links travel in the fragment.** The console reads `location.hash` once, calls the API, and replaces the address without it.
- **Every visible element comes from `@logicplanes/kaeso-ui`**, and `className` holds layout only: margin, width, grid, flex, gap and position. `pnpm lint` enforces it with the rules in `apps/console/eslint/kaeso.js`.
- **Every word in three languages**, through `t()` ([translations](../translations.md#the-console)).
- **Descriptions are one sentence that fits on one line.** The limits depend on the place: 60 characters for a settings row (50 beside an input), 80 for a section or page (70 beside actions), 100 for a dialog, 90 for an empty state and 60 for a toast. `src/copy.test.ts` checks settings rows, sections and pages in every language, German and Chinese with 15% slack.

## Data

Every read is a TanStack Query query keyed by the request; every change is a mutation that invalidates what it touches and the audit log.

- **The client** in `src/api/client.ts` is `openapi-fetch` with the types that `pnpm --filter kaeso-console generate` writes from the repository's `openapi.json` into `src/api/generated`. It sends the session cookie (same origin) and turns every failure into an `ApiError` with the API's `type`, `message`, `param` and the `kaeso-request-id` (`src/api/errors.ts`).
- **Lists** use cursor paging (`src/api/paging.ts`); **exports** are streamed downloads with the cookie (`src/api/download.ts`).
- **Search parameters.** Filters, periods and open sheets live in the address. Each route checks its search in one function shared by `validateSearch` and the page's `Route.useSearch({ select })`, because the router also hands a page its parents' raw values; a value a page doesn't know falls back to its default.
- **Freshness** (`src/query.ts`). Cached data shows at once and refetches in the background. Queries are fresh for 10 seconds; a few that rarely change stay fresh longer, such as a project and its rights (60 seconds) and the rights catalogue (30 minutes). The session is always revalidated. Client errors and changes are never retried; other failed reads are retried twice.

## Styling

The console has no styles of its own. `@logicplanes/kaeso-ui` provides every visible element and the stylesheet (`src/styles.css` imports the library's Tailwind entry), and console code uses Tailwind only for layout, from the same token scale. A Vite plugin serves the library's pre-paint theme script as `/theme-init.js`, so the content security policy allows no inline script.

## Bundles

The router splits routes (`autoCodeSplitting`), so a page's code loads with the page. Console code imports `@logicplanes/kaeso-ui` as one package; the build rewrites those imports to the library's per-component entry points and each `icons.name` to that icon's own import (`uiEntryImports` in `vite.config.ts`), so a page carries only the components and icons it draws. The library marks its module-level component factories pure, so a used component doesn't keep the rest of its file.

What the first paint doesn't need loads when it is used: the toaster, the command palette, the recent sign-in dialog, the shortcuts list, "Create organization", the sidebar's menus, tooltips and the date picker's calendar. The sidebar's menus and the calendar also load once the page is idle, so the first press opens them at once. The overview's breakdown and aside, and the sign-in steps after the email address, load alongside the data they wait for. `pnpm build` checks the result against the [budgets](../performance.md#the-console).

## Errors

Every error is a toast; nothing shows an error in the page.

- Changes go through `useApiMutation` (`src/auth/mutation.ts`), public forms through `useSubmit`, and failed loads through the query cache's handler (`src/query.ts`), which offers "Try again".
- Gateway failures carry the request id.
- Edits to catalogue models and limits send `If-Match` with the revision they were made on. A `412` reloads what changed, says so, and closes the edit's dialog.
- A missing object (404) is the page's or sheet's own state, and an ended session goes to sign-in.
