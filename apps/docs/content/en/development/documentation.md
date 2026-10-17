---
title: Writing documentation
description: How the pages of this site are written, placed, linked, checked and translated.
order: 6
---

Every page of this site is a Markdown file in `apps/docs/content`, with one folder per language (`en`, `de`, `zh-CN`) holding the same file names. This is the only place documentation is kept; READMEs in the code stay short and link here. When people who use, run or integrate Kaeso will notice a change, its pull request updates these pages.

## Editing a page

**On GitHub:** choose **Edit on GitHub** at the foot of any page, or open the file in `apps/docs/content`, and propose the change against `dev`. GitHub renders the page much as the site does, and links between pages work there too.

**On your machine:**

```bash
pnpm build:packages
pnpm --filter kaeso-docs dev
```

The site runs at http://localhost:4480 and reloads as you save; the terminal names anything to fix. `KAESO_DOCS_PORT` chooses another port.

| Command | Does |
| --- | --- |
| `pnpm --filter kaeso-docs check:content` | Checks every page's frontmatter, links, anchors, images and addresses, and that each language has the same pages with the same `order`, without building |
| `pnpm --filter kaeso-docs build` | Builds the site into `apps/docs/dist`, then checks the [bundle budgets](performance.md#the-documentation-site) |
| `pnpm --filter kaeso-docs preview` | Serves the build as a static host would |

The site's own code is tested like any other package ([testing](testing.md#the-documentation-site)).

## A page

```markdown
---
title: API keys
description: Create keys for apps and scripts, and keep them under control.
order: 5
---

Every request to the model API carries a **key**.

## Create a key
```

| Field | Required | Meaning |
| --- | --- | --- |
| `title` | yes | The page's heading, its name in the sidebar and in search, and its browser tab |
| `description` | yes | One sentence under the title, at most 140 characters; also the description search engines and link previews show |
| `order` | no | A whole number; lower comes first within the folder. Pages without one, or with the same one, follow by file name |
| `nav` | no | A shorter label for the sidebar |

Any other field is an error. The title is the page's only `#` heading, so headings in the page start at `##`; a page may still begin with `# ` and its exact title to read well on GitHub, and the site shows it once. **On this page** lists the `##` and `###` headings.

## Where a page goes

| File | Address | In the sidebar |
| --- | --- | --- |
| `content/en/index.md` | `/` | "Introduction", first |
| `content/en/console/index.md` | `/console` | Names the group by its `title`, places it by its `order`, and is listed as "Overview" |
| `content/en/console/api-keys.md` | `/console/api-keys` | In that group |
| `content/de/console/api-keys.md` | `/de/console/api-keys` | The same page in German |
| `content/zh-CN/console/api-keys.md` | `/zh-CN/console/api-keys` | The same page in Simplified Chinese |

A file's name is its address: lowercase, words joined by hyphens (`provider-keys.md`). Renaming or moving a file changes its address in every language, and every link to it must follow; the build names the ones that don't. A folder inside a group needs an `index.md`, and folders nest at most three deep. Files and folders whose names start with `_` are not published.

## Links

Link to another page by its file, relative to the page you're writing, exactly as GitHub follows it:

```markdown
See [rate limits](limits-and-budgets.md#rates) and [the API](../api/index.md).
```

- An anchor is the heading as GitHub makes it: lowercase, punctuation removed, spaces as hyphens. A heading on the same page is `[budgets](#budgets)`.
- A link to a page or heading that doesn't exist fails the build, naming the file and the link.
- Reference-style links (`[text][ref]`) are not supported and show as text; write links inline.
- Other sites take `http`, `https` and `mailto` addresses only. Link a repository file that isn't a page on GitHub (`https://github.com/logicplanes/kaeso/blob/dev/<path>`), or name it as code: `crates/kaeso-mail`.

## Images

Put the image next to the page or in an `images/` folder beside it, and give it alt text that says what it shows:

```markdown
![The request flow: app, key, model, limits, provider](images/request-flow.svg)
```

PNG, JPEG, GIF, WebP, AVIF and SVG work. Images from other sites are not shown, and an image without alt text fails the build. Choose images that read on light and dark backgrounds; an SVG can switch its own colours with `@media (prefers-color-scheme: dark)`. Linking any other file in `content/` publishes that file.

## Callouts

GitHub's alerts, with the kind alone on the first line:

```markdown
> [!NOTE]
> A key's secret is shown once.
```

`NOTE`, `TIP`, `IMPORTANT`, `WARNING` and `CAUTION` each have their own look and title. Use them sparingly.

## Everything else

Pages are GitHub-flavoured Markdown: **bold**, _italic_, ~~strikethrough~~, `code`, lists, task lists, tables (a `--:` column is right-aligned, for numbers), block quotes, horizontal rules and fenced code. A fence names its language and may name its file:

````markdown
```ts title="support-bot/src/client.ts"
export const client = new OpenAI({ baseURL: "https://kaeso.example.com/v1" });
```
````

TypeScript, TSX, JavaScript, JSON, Bash (`sh`, `shell`, `console`), Python, YAML and diff are highlighted; other languages show as plain text. Raw HTML is not rendered and shows as text. Code blocks never contain comments: explain a command in the sentence before it.

Write the current version as `\{{version}}`. The build replaces it with the release's version in text, code and frontmatter, so no page falls behind the next release. A backslash in front keeps the characters as they are, as on this page.

## Writing

- **Describe Kaeso as its code does.** Check settings against the gateway's configuration, labels against the console's English catalog, and behaviour against the gateway, KPP and the console. Never document a feature that doesn't exist.
- **Describe the current state.** Plans, open questions and to-do lists go into issues.
- **Lead with what the reader needs**, one idea per paragraph. Use tables for reference data, numbered lists for steps, and prose where reasoning matters. Link to the page that covers a topic instead of repeating it.
- **Sentence case** for titles and headings, plain words, short sentences, no exclamation points.
- **Console labels** are bold and exactly as the console shows them: **API keys**, **Create key**, **Settings** › **Instance**.
- **Examples** use Kaeso's own placeholders (`kaeso.example.com`, `sk-kso-…`, `owner@example.com`), never real keys or personal data.

## Translating

A change to an English page is made in `content/de` and `content/zh-CN` in the same pull request.

- **Same structure:** the same file name, the same `order`, the same headings in the same order, and the same code blocks, byte for byte. Translate the title, the description, the text, tables and callouts.
- **Links** keep their file; an anchor becomes the translated page's own heading anchor.
- **Unchanged:** code, commands, settings (`KAESO_*`), paths, field names, rights (`keys.all.view`), product names and model ids.
- **Console labels** use the console's own translation, from `apps/console/src/i18n/locales/de` and `apps/console/src/i18n/locales/zh-CN`.
- **Write naturally**, not word for word, and keep every fact and number.

The house style and the glossary are under [translations](translations.md#house-style). Two rules matter most on pages:

| | German | Simplified Chinese |
| --- | --- | --- |
| Steps | Infinitive, verb last: „Schlüssel erstellen“ | A verb first: “创建密钥” |
| Descriptions | One sentence | One sentence, ending in “。” |

The site's own words, such as **Edit on GitHub** or the search, are in `apps/docs/src/i18n/messages/en.ts`, `de.ts` and `zh-CN.ts`, typed so that a missing translation fails the typecheck.
