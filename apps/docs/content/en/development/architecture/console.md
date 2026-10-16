---
title: Console
description: How the web console reads data, draws its pages, loads its code and shows errors.
order: 2
---

The console is a static React app in `apps/console`, served on the same domain as the gateway. It signs people in through the gateway and keeps nothing but the HttpOnly session cookie the gateway sets. Lists, filters, periods and open details live in the address, so links can be shared and the back button works.

| Part | Where |
| --- | --- |
| Routes, loaders and pages | `src/routes`, `src/features` |
| The Platform API client, typed from `openapi.json`, and its queries | `src/api` |
| Session, rights and route guards | `src/auth` |
| The app frame, sidebar, command palette and shortcuts | `src/shell` |
| Console-only compositions of library parts | `src/kaeso` |
| The languages and their catalogs | `src/i18n` |
| Money, periods, labels and other values as the console writes them | `src/format` |
| The API types and route tree generators, the stacks, sample data, the translation check, budgets and licenses | `scripts` |
| The lint rules for layout-only classes and translated text | `eslint` |
| The end-to-end suite | `e2e` |
| The development stack's mock provider, identity provider and master key | `stack` |

## Rules

- **Only the Platform API**, and every call through the client in `src/api`. Nothing the API can't do is shown, and nothing is faked.
- **Rights come from the API, never guessed.** Organization rights from `auth/session`, project rights from `GET …/projects/{project}/my-rights`, and each right's name, description, category and needs from `GET orgs/{org}/rights`. A `403` is the final word: an area a person can't see is hidden, and a single action is disabled with the right's name and who can give it (`useNeedsRight`).
- **No secrets in the browser.** The session is a cookie the console never reads. Key secrets are shown once and never stored. `localStorage` holds only the last organization and project, recently opened items, the theme, the language, the sidebar's width and state, the request log's columns and the playground's unsent prompts; `sessionStorage` keeps an invitation's token until it is accepted and the address to return to after a recent sign-in.
- **Tokens in links travel in the fragment.** The console reads `location.hash` once, calls the API, and replaces the address without it.
- **Every visible element comes from `@logicplanes/kaeso-ui`**, and `className` holds layout only: margin, width, grid, flex, gap and position. `pnpm lint` enforces it with the rules in `apps/console/eslint/kaeso.js`.
- **Every word in three languages**, through `t()` ([translations](../translations.md#the-console)).
- **Descriptions are one sentence that fits on one line**, with limits per place: 60 characters for a settings row (50 beside an input), 80 for a section or page (70 beside actions), 100 for a dialog, 90 for an empty state, 60 for a toast. `src/copy.test.ts` checks the settings rows, sections and pages in every language, German and Chinese with 15% slack.

## Data

Every read is a TanStack Query query keyed by the request; every change is a mutation that invalidates what it touches and the audit log. The client in `src/api/client.ts` is `openapi-fetch` with the types `pnpm --filter kaeso-console generate` writes from the repository's `openapi.json` into `src/api/generated`; it sends the session cookie (same origin), and turns every failure into an `ApiError` with the API's `type`, `message`, `param` and the `kaeso-request-id` (`src/api/errors.ts`).

Lists use cursor paging (`src/api/paging.ts`); exports are streamed downloads with the cookie (`src/api/download.ts`).

Filters, periods and open sheets live in the address. Each route checks its search in one function that `validateSearch` and the page's `Route.useSearch({ select })` share, because the router hands a page its parents' raw values too; a value a page doesn't know falls back to its default.

Query defaults (`src/query.ts`): cached data shows at once and refetches in the background; lists are fresh for 10 seconds, the catalogue and settings for 60, and the session is always revalidated; client errors and changes are never retried, other failed reads twice.

## Styling

There is none of the console's own. `@logicplanes/kaeso-ui` provides every visible element and the stylesheet (`src/styles.css` imports the library's Tailwind entry). Console code uses Tailwind only for layout, from the same token scale. The library's pre-paint theme script is served as `/theme-init.js` by a Vite plugin, so the content security policy allows no inline script.

## Bundles

Routes are split by the router (`autoCodeSplitting`), so a page's own code loads with the page. Console code imports from `@logicplanes/kaeso-ui` as one package, and the build rewrites those imports to the library's per-component entry points and each `icons.name` to that one icon's import (`uiEntryImports` in `vite.config.ts`), so a page carries only the components and icons it draws. The library marks its module-level component factories pure, so a used component doesn't keep the rest of its file.

Parts that the first paint doesn't need load when used: the toaster, the command palette, the recent sign-in dialog, the shortcuts list, "Create organization", tooltips and the date picker's calendar. The overview's breakdown and aside, and sign-in's steps after the address, load alongside the data they wait for. `pnpm build` checks the result against the [budgets](../performance.md#the-console).

## Errors

Every error is a toast; nothing shows an error in the page. Changes go through `useApiMutation` (`src/auth/mutation.ts`), public forms through `useSubmit`, and failed loads through the query cache's handler (`src/query.ts`), which offers "Try again". Gateway failures carry the request id. A missing object (404) is the page's or sheet's own state, and an ended session goes to sign-in.
