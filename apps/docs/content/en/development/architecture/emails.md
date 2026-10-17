---
title: Emails
description: How the gateway builds the emails it sends, for contributors to its kaeso-mail crate.
order: 4
---

Every email Kaeso sends comes from the gateway's `crates/kaeso-mail`: one layout, one stylesheet built from Kaeso's tokens, and one function per email that says what that email says, in English, German and Simplified Chinese. [Email](../../self-hosting/email.md) covers what operators set up and see; this page covers the design and the reasons behind it.

## What an email is

```text
KAESO                                   the wordmark alone, top left, 28px in and 28px down

Sign in to Kaeso                        heading: title-lg, 28/34, semibold
The link works once and expires in      one sentence: body-lg, 15/22, secondary
15 minutes.

( Sign in )                             one button: the xl pill, ink on white

If you did not ask to sign in, you      one quiet line: small, 13/18, tertiary
can ignore this email.

Sent by Kaeso at kaeso.example.com.     footer: caption, 12/16; the install, and why
```

- **No card.** A single left-aligned column, at most 562px wide, on the canvas, as in the console: white in light mode, black in dark mode. No panel, no border, no grey page around it.
- **Copy.** A subject of at most 60 characters without a period, a heading of at most 48, then at most one sentence, one button and one quiet line, such as the expiry or what to do if it wasn't you. Sentences follow the gateway's copy rules: one sentence, a semicolon for a second thought, a period at the end, at most 100 characters. A test enforces the limits.
- **No raw links.** The HTML never shows a URL; the button carries it. The plain-text part has the link in full on its own line, which is what text-only clients, tests and the console's end-to-end suite read.
- **Footer.** "Sent by Kaeso at" the install's host, from `KAESO_PUBLIC_URL` and linked to it, and, for notices, why it arrived: security and organization notices say they are always sent, and alerts say how to turn them off.
- **Plain text** says the same words in the same order and ends with a `-- ` signature line.

The rendered text and HTML of every email are snapshots in `crates/kaeso-mail/src/snapshots/`.

## How it is built

| File | Job |
| --- | --- |
| `templates.rs` | One function per email ([what Kaeso sends](../../self-hosting/email.md#what-kaeso-sends)), which is its catalogue: subject, heading, sentence, button, quiet line and why it is sent, in each language. `Email::render` renders it for an install. |
| `locale.rs` | `Locale` (`en`, `de`, `zh-CN`), how one is chosen, and dates and numbers as each language writes them. |
| `layout.rs` | The shared layout as HTML (maud: checked at compile time, every value escaped) and as plain text. |
| `style.rs` | The values of `@logicplanes/kaeso-tokens`, written once in its `token!` macro, each role's inline style composed from them at compile time, and the stylesheet. The layout never writes a color or size of its own. |
| `assets.rs`, `assets/` | The wordmark images and their SVG sources. |
| `mailer.rs` | The queue of up to 1,024 emails, three retries after 1, 5 and 30 seconds, the SMTP, log and in-memory transports, and the MIME structure. |

**Email-safe HTML.** Tables give the structure (`role="presentation"`), and everything that can be an inline style is one; a `<style>` block holds only what can't be: color schemes, resets and dark mode. Outlook for Windows ignores `max-width`, so conditional comments give it a fixed 562px column, Segoe UI as its font and 96 DPI images. No web fonts, no external CSS or images, no scripts, no tracking pixels. The font stack is the console's without its web fonts: Inter where it is installed, then each platform's interface font.

**The button** works everywhere: a padded link with a 9999px radius (an `xl` pill: 48px high, 22px in, 15px medium), and for Outlook for Windows, which can't pad or round a link, a VML `roundrect` with a 50% arc. VML needs a fixed width, taken from the label at 9px per character (16px per Chinese character) so it never wraps.

**Hidden preview text.** The sentence, followed by invisible characters, so inbox lists show it after the subject instead of the first words of the layout.

**Escaping.** maud escapes the names people choose, such as organizations, inviters, keys and projects, in text and attributes. The only unescaped markup is fixed: the doctype, Outlook's conditional comments and the VML button, which escapes the URL and label it embeds. Tests send markup and quotes through every name.

**MIME.** `multipart/alternative` with the plain text first and a `multipart/related` HTML part that carries both wordmark PNGs as inline parts by `Content-ID`, without a file name, so mail apps don't list them as attachments.

## Languages

Every email exists in English, German and Simplified Chinese. The design is the same in each; only the words change, and the HTML's `lang` says which. English is the source.

**Which language.** `services::mail_lang` in `kaeso-server` decides from the person's choice under account settings (`users.locale`, where `null` means Automatic) and the `Accept-Language` of the request that caused the email: its first language Kaeso writes, by preference, so `de-AT` is German, any Chinese is Simplified Chinese and any English is English. Looking up the choice never stops an email: if the database can't answer, the fallbacks decide. [Email](../../self-hosting/email.md#languages) has the rules for each kind of recipient.

**Writing.** Each email's function has one `match` on the language with whole sentences and values put in by name, never fragments joined together. The glossary and the house style are the [writing rules of `@logicplanes/kaeso-ui`](https://github.com/logicplanes/kaeso/blob/dev/packages/ui/foundations/writing.md). Emails address people formally: German uses "Sie" in short sentences; Chinese is concise and uses "您", full-width punctuation, and a space between Chinese and Latin words or digits, which `locale::zh` puts at the seams, never inside a name.

German and Chinese keep the copy rules with the console's 15% slack: subject 69, heading 55 and sentence 115 characters. Tests check these limits, the layout's tags against English, and every email's subject and heading in each language.

**Dates and numbers** follow the language:

| | English | German | Chinese |
| --- | --- | --- | --- |
| Date | `2026-10-14` | `14. Okt. 2026` | `2026年10月14日` |
| Amount | `1250` | `1.250` | `1,250` |
| Percentage | `80%` | `80 %` | `80%` |

## The wordmark

Interfaces show the wordmark alone, top left, at 60×12 (`Logo.Wordmark size="nav"`), and so do emails: the image is 64×16 with 2px of clear space, placed at 26px so the glyphs start at 28px, on the same line as the text below.

**Delivered inline, not linked.** A self-hosted install's public URL may be internal or unknown, so a linked image could fail to load, and many mail apps block remote images by default. Inline parts always show and need nothing from the install. The two PNGs together are about 6 KB.

**Sharp on every screen.** Each PNG is three times its display size (192×48), rendered from `assets/wordmark-light.svg` and `assets/wordmark-dark.svg`, whose paths are the ones in `@logicplanes/kaeso-ui`. A test renders the SVGs again with resvg and fails when the committed PNGs differ; `KAESO_UPDATE_MAIL_ASSETS=1 cargo test -p kaeso-mail assets` rewrites them. Both images have `alt="Kaeso"` and `width` and `height` attributes.

## Dark mode

Kaeso controls dark mode instead of leaving it to each mail app's automatic inversion. Mail apps fall into three kinds, and each gets its own layer:

1. **Apps that follow `prefers-color-scheme`**, such as Apple Mail on macOS and iOS, Outlook for Mac, iOS and Android, Thunderbird, Samsung Email and Hey. The document declares both schemes (`<meta name="color-scheme" content="light dark">`, `supported-color-schemes`, and `:root { color-scheme: light dark }`), which tells them not to invert it. The stylesheet's `@media (prefers-color-scheme: dark)` rules switch every role to its dark token with `!important`, since inline styles would otherwise win: the black canvas, `#fafafa` text, `#a3a3a3` and `#858585` secondary and tertiary text, the button to `#fafafa` with black text, and the light wordmark for the dark one.
2. **Outlook.com and the new Outlook**, which darken emails themselves and mark the elements they recolor with `data-ogsc` (color) and `data-ogsb` (background). The same rules, under `[data-ogsc]` and `[data-ogsb]`, replace their guesses with Kaeso's dark values, so the result matches the first layer.
3. **Apps that invert without asking**, such as the Gmail apps, Outlook for Windows in dark mode, and Gmail with accounts from other providers, which drops `<style>`. Nothing can be switched there, so the light design is built to survive inversion:
   - text is near-black `#0a0a0a`, never pure black, and the button's ink is `#0a0a0a` with white text, so inverted it becomes a light pill with dark text, the dark theme's own button;
   - the light wordmark has a 1.25px white halo, invisible on white; where the background is darkened behind it and the image is not, the halo outlines the glyphs and keeps the wordmark legible;
   - the dark wordmark is hidden with `display:none`, `max-height:0` and `mso-hide:all` inside a conditional comment, so it never appears on its own.

The stylesheet is generated from the same token macro as the inline styles, and tests check that every dark rule is `!important` and that every role has a rule in each layer.

## Previewing

The [development stack](../local-setup.md#3-start-the-development-stack) includes Mailpit, whose web interface at http://127.0.0.1:58025 has an HTML compatibility check and a dark preview. `cargo test -p kaeso-mail --test smtp` sends every email to it in every language and checks that:

- each arrives with both parts and both images inline;
- Mailpit's HTML check, based on caniemail data weighted by each mail app's share, reports at least 88% support.

The test then deletes its messages. `KAESO_TEST_SMTP_URL` and `KAESO_TEST_MAIL_API` point it elsewhere.

Every email scores about 91% supported, 8% partial and 1% unsupported. The partial and unsupported entries are expected: `@media`, `<style>` and `!important` for dark mode, which the third layer covers where they are missing; `border-radius`, which VML replaces in Outlook for Windows; and `<body>` styles, which the wrappers repeat.
