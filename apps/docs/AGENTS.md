# AGENTS.md

Rules for the documentation site and its pages, `apps/docs`. The root [`AGENTS.md`](../../AGENTS.md) holds the definition of done and the rules for the whole repository; commands run from the repository root.

## What the site is

**Kaeso's documentation**: the Markdown pages in `content/` and the static site that shows them at [docs.kaeso.app](https://docs.kaeso.app), in English, German and Simplified Chinese. The site looks and works like the [console](../console) and is drawn only with [`@logicplanes/kaeso-ui`](../../packages/ui).

- **Is:** the only home of documentation for people who use, run or integrate Kaeso: getting started, self-hosting, the console, the APIs, concepts and development. The code keeps short `README.md` files that link here.
- **Is not:** a design system. Every visible element is a `@logicplanes/kaeso-ui` component; the site owns composition and behaviour. A missing component or variant is added to `packages/ui`, in the same pull request when the site needs it.
- **Is not:** a CMS, a server or a place for decisions. Nothing runs but static files.
- **Not a place for plans.** Roadmaps, to-do lists and open questions go into issues.

## Setup

```
corepack enable
pnpm install
pnpm --filter kaeso-docs exec playwright install chromium
pnpm build:packages
pnpm --filter kaeso-docs dev
```

Node.js 24 (the repository's `.node-version`) and pnpm 11 (pinned in the root `package.json`). `pnpm --filter kaeso-docs dev` serves the site at http://localhost:4480 and reloads as pages are saved; `KAESO_DOCS_CONTENT=e2e/fixtures/content pnpm --filter kaeso-docs dev` serves the fixture pages instead, and `KAESO_DOCS_PORT` chooses another port. The site reads the build of `packages/ui`: run `pnpm build:packages` again after changing the library.

## Checks

The root definition of done covers the site. To run its own checks alone, after `pnpm build:packages` when the libraries changed:

```
pnpm --filter kaeso-docs check:content
pnpm --filter kaeso-docs diagrams
pnpm --filter kaeso-docs lint
pnpm --filter kaeso-docs typecheck
pnpm --filter kaeso-docs test
pnpm --filter kaeso-docs build
pnpm --filter kaeso-docs licenses:check
pnpm --filter kaeso-docs e2e
```

`check:content` checks every page in every language without building; `diagrams` draws the Mermaid blocks of the pages with Chromium and removes drawings no page uses; `lint` runs ESLint and then `check:content`. `test` runs Vitest with coverage thresholds. `build` fails on a content problem or a bundle budget (`scripts/budget.ts`); `licenses:check` then checks that every package whose code is in the build is MIT, ISC, BSD, 0BSD or Apache-2.0 licensed. `e2e` runs Playwright, axe included, against a production build of the fixture pages in `e2e/fixtures/content`. Then look at every page you changed in light and dark, at desktop and phone widths, in English, German and Chinese, and fix what looks wrong.

## Rules

1. **Only `@logicplanes/kaeso-ui`** for anything visible, from the workspace (`workspace:*`). No other UI library, no CSS of the site's own. **`className` holds layout only** (margin, width, grid, flex, gap, position); no colours, radii, type sizes, padding, heights, shadows or inline styles. `pnpm lint` enforces it (`eslint/kaeso.js`).
2. **Pages are content, not code.** Nothing in `content/` is executed or rendered as HTML. Raw HTML stays text, links keep only http, https and mailto or point at files in `content/`, images come only from `content/`. Never weaken this to make a page work; change the page.
3. **Links work on GitHub and on the site.** Pages link to files (`../console/api-keys.md#create-a-key`), never to site addresses. Heading anchors stay GitHub's.
4. **A broken page fails the build** with its file: a link, an anchor, an image, a frontmatter field, a page missing in a language. Problems are never only warnings in a build.
5. **Every page in three languages.** `content/en`, `content/de` and `content/zh-CN` hold the same files with the same `order`; a change to an English page changes both translations in the same commit, following [the translation rules](content/en/development/documentation.md#translating). The site's own words live in `src/i18n/messages`, typed so that a missing translation fails the typecheck.
6. **Pages describe Kaeso as it is.** Every statement is checked against the code in this repository (the gateway, kpp and the console); never document a feature that doesn't exist. Examples use Kaeso's own placeholders (`kaeso.example.com`, `sk-kso-…`, `owner@example.com`), and the current version is always `{{version}}`, which the build replaces with the root `package.json`'s version.
7. **No real content in tests.** The unit tests and the e2e suite use `e2e/fixtures/content` and `e2e/fixtures/CHANGELOG.md`, which read as Kaeso's own data. `content/` holds only real pages.
8. **The console's look and house rules**, everywhere:
   - the wordmark alone at the top left, 28px in and 28px down, the sidebar's labels on the same 28px line; the mark only for favicons;
   - no decorative icons, icon circles or badge dots; no serif type; no breadcrumbs; no setup checklists;
   - no keyboard-shortcut hints (⌘K works, its hint is not shown); no "N results" counters;
   - descriptions are one short sentence; every word follows Kaeso's voice: sentence case, plain words, no exclamation points;
   - every error is a toast, never a banner or text in the page;
   - nothing changes size when its state changes, and nothing shifts after the first paint;
   - a focus ring shows for the keyboard only, never after a click.
9. **Code blocks in pages never contain comments**; explain a command in the sentence before it.
10. **Diagrams are Mermaid, drawn by the build.** Every diagram of a system, an architecture, a request or data flow, a sequence, a state or a lifecycle is a ` ```mermaid ` block with `accTitle` and, in a sentence or two, `accDescr`, its labels translated like the page; never ASCII or box-drawing art in a code block, never a screenshot or an image of a diagram. `check:content` fails on a code block that draws with box-drawing characters or arrows. The build draws them in strict mode into static images, so nothing of them runs in a reader's browser; `pnpm --filter kaeso-docs diagrams` draws new and changed diagrams into `diagrams/`, which is committed with the page; [writing documentation](content/en/development/documentation.md#diagrams) has the rules.
11. **Tests assert on what a person sees**, and the content build's tests on what an author writes. The coverage thresholds are in `vitest.config.ts`.

## Where things are

| Path                  | Holds                                                                                                                                          |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `content/<language>/` | The pages. Folders are sidebar groups; a folder's `index.md` names and places it                                                               |
| `scripts/content/`    | Frontmatter, slugs, compiling a page, one site per language, the changelog page, the translation checks, the Vite plugin, `pnpm check:content` |
| `diagrams/`           | The drawn diagrams, a light and a dark SVG per Mermaid block, named by a hash of its source; written by `pnpm --filter kaeso-docs diagrams`    |
| `scripts/budget.ts`   | The bundle budgets `pnpm build` ends with                                                                                                      |
| `src/markdown/`       | The page format (`ast.ts`) and its renderer (`render.tsx`), library components only                                                            |
| `src/page/`           | A page: title, description, content, previous and next, Edit on GitHub, On this page; not found; load error                                    |
| `src/shell/`          | `AppFrame` and the sidebar, the search, the language select, links through the router, toasts, the document title                              |
| `src/i18n/`           | The three languages, the preference and the site's own words                                                                                   |
| `src/router.tsx`      | One route tree per language; every address loads its page's chunk                                                                              |
| `eslint/`             | The layout-only and no-colour lint rules, with tests                                                                                           |
| `e2e/`                | Playwright suite and its fixture pages in `e2e/fixtures/content`                                                                               |
