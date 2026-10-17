---
title: Documentation site
description: How the Markdown pages in apps/docs/content become the static site at docs.kaeso.app.
order: 3
---

The documentation is the `kaeso-docs` package in `apps/docs`: Markdown pages in three languages, built into a static site that looks and works like the console and is drawn only with `@logicplanes/kaeso-ui`. How to write a page is under [writing documentation](../documentation.md); this page is how the site works.

```text
content/<language>/**/*.md ─▶ scripts/content (at build time, in Vite) ─▶ virtual modules ─▶ src/ (React, @logicplanes/kaeso-ui) ─▶ dist/
                              frontmatter · marked's lexer · links,          site/<language>: sidebar, titles, previous/next, a loader per page
                              anchors and images checked · search words      page/<language>/<path>: one page's blocks, its own chunk
                              the same pages in every language               search/<language>: the index, loaded with the search
```

## Content model

| Piece | Where | Rule |
| --- | --- | --- |
| A language | `content/en`, `content/de`, `content/zh-CN` | English at `/`, the others under `/de` and `/zh-CN`; every language has the same files with the same `order` |
| A page | `content/<language>/<folders>/<name>.md` | Its address is its path: `content/en/console/api-keys.md` is `/console/api-keys`; `index.md` is its folder's page |
| The home page | `content/<language>/index.md` | Required |
| A sidebar group | A folder in a language | Named by its `index.md`'s `title`, or by the folder's name; placed by that `index.md`'s `order` |
| A nested page | A folder inside a group, with `index.md` | Its pages are indented under it; at most three folder levels |
| Order inside a folder | Frontmatter `order` | The folder's `index.md` first, then by `order`, then by label |
| A page's sidebar label | Frontmatter `nav` | Defaults to the title; `index.md` defaults to the language's "Overview" (the home page: "Introduction") |
| Images and files | Anywhere in `content/` | Published only when a page shows or links to them, with their size read at build time |
| Drafts and private notes | Names starting with `_` or `.` | Left out of the site |

Frontmatter has exactly four fields: `title` and `description` (one short sentence, at most 140 characters) are required, `order` and `nav` are optional. Any other field is an error, so a typo never silently does nothing.

## The build

`apps/docs/scripts/content` turns the pages into the site's modules:

| File | Job |
| --- | --- |
| `frontmatter.ts` | The four fields, with a message for each mistake; one sentence per description, in Latin and Chinese punctuation |
| `slug.ts` | GitHub's heading anchors (`## Rate limits` is `#rate-limits`, repeats get `-1`, `-2`) and addresses from file names |
| `compile.ts` | One page: marked's lexer with raw HTML turned off, then the page format of `src/markdown/ast.ts`; links checked; callouts; search words |
| `site.ts` | One language: addresses, the sidebar, previous and next, relative links resolved as GitHub resolves them, images and their sizes, search |
| `docs.ts` | Every language: a site each, with its prefix and labels, and the checks that they hold the same pages in the same order |
| `plugin.ts` | The Vite plugin: virtual modules, reload on a change in development, an HTML file per page and language with its head for search engines and link previews, `404.html`s and the sitemap |
| `check.ts` | `pnpm --filter kaeso-docs check:content`: the same checks without building the app |

Every problem names its file: a link to a page or heading that doesn't exist, an image without alt text or from outside `content/`, a link with a scheme other than http, https or mailto, a second `#` heading, two files with one address, a page missing in a language or with another `order` there. In development they are printed and the page still renders; the build and `check:content` fail on them.

### Safety

Pages are edited in a browser on GitHub, so the build treats them as untrusted text. marked only tokenizes, with its HTML tokenizers off: `<script>`, `<img onerror>` and every other tag stay literal text. Link references are off, and links keep only http, https and mailto, after whitespace and control characters are removed, so `java\nscript:` is caught. Images come only from `content/`. The browser receives plain objects, never HTML, and nothing sets `innerHTML`. `scripts/content/compile.test.ts` holds the cases.

### Links that work on GitHub and on the site

A link is written between files, as GitHub follows it: `[keys](../console/api-keys.md#create-a-key)`. The build resolves it against the file's folder, checks that the page and the heading exist, and writes the site's address in the page's language (`/de/console/api-keys#schlüssel-erstellen`). Headings use GitHub's anchors, so the same anchor works in both places; a translated page links to the translated heading's anchor.

## The app

| Folder | Job |
| --- | --- |
| `src/router.tsx` | One route tree per language: a layout that loads the language's site module and the library's messages, and a route per address |
| `src/site/` | Loads a language's site module, its search index and the library's messages, each once |
| `src/page/` | The page (title, description, content, previous and next, Edit on GitHub, "On this page"), not found, and a page that can't load |
| `src/markdown/` | The page format (`ast.ts`) and its drawing with library components (`render.tsx`) |
| `src/shell/` | `AppFrame` with the sidebar, the search, the language and theme selects, links through the router, toasts |
| `src/i18n/` | The languages and their addresses, the remembered preference, and the site's own words in `messages/` |

The frame is the console's: `AppFrame` with its Skip to content link and `Sidebar`, the wordmark alone at the top left, 28px in and 28px down, the panel inset on the chrome, and the phone bar and navigation sheet below 1024px; the phone bar holds the search button beside the navigation toggle. A page is laid out like the console's settings pages: a `title-lg` title and a `body-lg` description in a 640px column; text is `body-lg`, headings `title` (`##`) and `title-sm` (`###`), code in `CodeBlock`, callouts in `Banner`, tables in `Table` inside a horizontal `ScrollArea`. From 1280px, "On this page" (`TableOfContents`) stands to the right; its column is kept on pages without it, so the text never moves between pages.

Hovering or focusing a link loads the page's chunk (`defaultPreload: "intent"`), so pages open at once. The search (`CommandPalette`) and its index load a moment after the first paint; the search button in the sidebar or the phone bar, ⌘K or Ctrl+K opens it, and what is typed before it has loaded becomes its query. It finds pages by title, description and words, headings by their own words, and offers the theme and the other languages.

The first visit to an English address follows the browser's languages and moves to `/de` or `/zh-CN` without a navigation; choosing a language in the menu or the search opens the same page in that language and is remembered as `kaeso-docs:locale`. The theme is the library's: system, light or dark, applied before the first paint by the theme script.

Every error is a toast: a page whose chunk can't load keeps the frame, shows an empty state with "Try again" and says what happened in a toast; a search that can't load closes with a toast.

## Design choices

- **Frontmatter, not a sidebar file.** A page's place is its folder and its `order`; a section's name and place are its `index.md`'s. Moving or adding a page on GitHub is one file per language, never a second file that must agree.
- **A leading `# Title`.** GitHub shows the frontmatter as a small table, so a page may start with `# ` and its title to read well there; the site drops that heading. Any other `#` heading is an error.
- **Callouts.** GitHub's five alerts become `Banner`s: Note neutral, Tip success, Important info, Warning and Caution warning. Caution doesn't take the danger tone, because the danger banner has `role="alert"`, which a screen reader announces as the page opens.
- **Tables** are at least 640px wide inside a horizontal `ScrollArea`: on a phone they scroll sideways instead of squeezing every column into a word per line.
- **No TanStack Query.** The site fetches nothing but its own chunks, which the router's loaders load and keep (`defaultStaleTime: Infinity`).
- **Not supported, on purpose:** raw HTML, MDX or components in pages, footnotes, images from other sites, link reference definitions.
- **Code** is highlighted for the library's languages (TypeScript, TSX, JavaScript, JSON, Bash, Python, YAML, diff, and their aliases); anything else is plain text in the same block.
- **One version.** The pages describe the latest release; there are no versioned copies.

## The build output

`pnpm --filter kaeso-docs build` writes `apps/docs/dist`: the app, a `<path>.html` for every page in every language (`/de/console/api-keys` is `de/console/api-keys.html`), a `404.html` per language marked `noindex`, `sitemap.xml` with every page in every language and its `hreflang` alternates, `robots.txt`, which allows everything and names the sitemap, `site.webmanifest`, the icons, `og.png` and `CNAME`. A page's head holds its `lang`, `<title>` and description, its canonical address, its `hreflang` alternates in the three languages and `x-default`, Open Graph and Twitter tags with `og.png`, its `og:locale` and the other two as alternates, and structured data: `WebSite` on a language's home page, `TechArticle` on every other page. The sitemap has no `lastmod`, since the build doesn't know when a page last changed. Any static host serves it: a request for `/console/api-keys` gets `console/api-keys.html`, and an unknown address gets `404.html`, which shows "not found" in the frame. The maintainer publishes it to docs.kaeso.app.

`vite.config.ts` does what the console's does: it rewrites imports from `@logicplanes/kaeso-ui` to the library's per-component entries (`uiEntryImports`) and serves the theme script as `/theme-init.js`; it also adds the `theme-color` meta tags and preloads the interface font. The build ends with `scripts/budget.ts`, which checks the frame and every page against their [bundle budgets](../performance.md#the-documentation-site), and with the site check (`scripts/check-site.mjs`, described under [the sites](../releasing.md#the-sites)); `pnpm --filter kaeso-docs licenses:check` (`scripts/licenses.ts`) then checks the license of every package in it.
