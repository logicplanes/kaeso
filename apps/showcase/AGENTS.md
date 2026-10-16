# AGENTS.md

Rules for the showcase at [ui.kaeso.app](https://ui.kaeso.app), `apps/showcase`: an Astro site with React islands that presents every component of `@logicplanes/kaeso-ui`. The root [`AGENTS.md`](../../AGENTS.md) holds the setup and the definition of done; the library's design rules are in [`packages/AGENTS.md`](../../packages/AGENTS.md).

## Commands

```sh
pnpm build:packages
pnpm --filter kaeso-showcase dev
pnpm --filter kaeso-showcase build
pnpm --filter kaeso-showcase e2e
```

`dev` serves the showcase at http://localhost:4321. `e2e` runs Playwright against the production build: accessibility (axe) in both themes, keyboard paths, type scale, motion, geometry, focus rings, phone width, resize loops, menu frames and even edges.

## Rules

The showcase is judged by how calm it looks: space and restraint over more content.

1. **The home page is the components.** `src/pages/index.astro` and `src/components/ComponentGrid.tsx`: a 52px bar (title and count at the left; package version, Get started and GitHub at the right), then a grid of live components. No hero, no marketing copy, no tiles or cards.
2. **The grid:** one component per cell, centred in at least 320px of height (240 on phones) with 24px padding; the cell's title (the demo's `meta.title`) at the top left links to its page. Hairlines between cells come from the grid's 1px gap on `bg-line`. Columns: 1, 2 from `md`, 3 from `xl`, 4 from `3xl` (1600px). **Keep the cell count a multiple of 12** so every row is full at every width. Content stays within 280px.
3. **Cells show Kaeso data** from `src/fixtures/kaeso.ts` (models, keys, members), in the quiet default state, nothing animated or opened on load. A component that needs a page of room (tables, charts with axes, app frames) belongs on its own page, not in the grid.
4. **The sidebar lists titles alone, no icons.** Two groups only: the uncaptioned start block (Overview, Installation, Theming, For agents, Changelog), then **Components**, every component in one alphabetical list.
5. **The showcase shows components, nothing else.** No foundations pages, example screens or review pages in the navigation. Design rules are Markdown for agents (rule 9).
6. **Component pages stay simple:** title, one sentence, Copy page and View source, the primary demo with Show code, its import line, then **Props**. A component's page is its demo module, `src/demos/<name>.tsx` (`meta`, `examples`, example functions, `States`); update it with every component change. `scripts/gen-pages.mjs` writes examples, keyboard, accessibility and do and don't to `packages/ui/ai/component-guides.md`. Every example renders on `/components/<slug>/examples` and every state on `/components/<slug>/states`: unlinked pages that the end-to-end tests use.
7. **Language and theme sit at the sidebar's foot, side by side.** The first visit follows the browser's languages; a choice there or `?lang=` is remembered. **Everything that isn't a name is translated**: component and prop descriptions, the chrome, the home grid, the guides. Chrome words live in `DOCS_TEXT` (`src/lib/locale.tsx`); descriptions and page text in `src/i18n/de.json` and `src/i18n/zh-CN.json`, keyed by the English with whitespace collapsed. `gen-pages` fails on every text missing from a dictionary. The demos inside component previews are code and stay English.
8. **Every page leaves room:** `max-w-docs` content, 40px from the panel's top, 56px between sections, prose at `max-w-prose`.
9. **Agent rules live in Markdown, shipped in the package.** `packages/ui/AGENTS.md` is the entry point (with `packages/ui/CLAUDE.md`, which loads it); `packages/ui/foundations/*.md` hold the design rules; `packages/ui/ai/` holds what the build generates. `AGENTS.md` also renders as `/for-agents` and inside `/llms.txt`. Change a rule in its file, never in a copy.
10. **The changelog page renders the repository's `CHANGELOG.md`** (`src/pages/changelog.astro`); never keep a second changelog.
