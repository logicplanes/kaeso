---
title: Emails
description: How the gateway builds the emails it sends, for changes to the kaeso-mail crate.
order: 4
---

Every email Kaeso sends comes from `crates/kaeso-mail`: one layout, one stylesheet built from Kaeso's tokens, and one function per email in English, German and Simplified Chinese. [Email](../../self-hosting/email.md) covers what operators set up and see; this page covers the code.

## What an email is

| Part | In the sign-in email | Style |
| --- | --- | --- |
| Wordmark | Kaeso | Alone, top left, 28px in and 28px down |
| Heading | Sign in to Kaeso | `title-lg` |
| One sentence | The link works once and expires in 15 minutes. | `body-lg`, secondary |
| One button | Sign in | The `xl` pill, ink on white |
| One quiet line | If you did not ask to sign in, you can ignore this email. | `small`, tertiary |
| Footer | Sent by Kaeso at kaeso.example.com. | `caption` |

- **No card.** A single left-aligned column, at most 562px wide, on the canvas: white in light mode, black in dark mode.
- **Copy limits.** A subject of at most 60 characters without a period, a heading of at most 48, and sentences of at most 100. German and Chinese get 15% more: 69, 55 and 115. A test enforces the limits.
- **No raw links.** The HTML never shows a URL; the button carries it. The plain-text part has the link in full on its own line, and it says the same words in the same order.
- **Footer.** "Sent by Kaeso at" the install's host, from `KAESO_PUBLIC_URL`. Notices also say why they arrived: security and organization notices are always sent, and alerts say how to turn them off.

The text and HTML of every email are snapshots in `crates/kaeso-mail/src/snapshots/`.

## How it is built

| File | Job |
| --- | --- |
| `templates.rs` | One function per email, which is the catalogue of [what Kaeso sends](../../self-hosting/email.md#what-kaeso-sends): subject, heading, sentence, button, quiet line and why it is sent, in each language |
| `locale.rs` | `Locale` (`en`, `de`, `zh-CN`), and dates and numbers as each language writes them |
| `layout.rs` | The shared layout, as HTML with maud and as plain text |
| `style.rs` | The values of `@logicplanes/kaeso-tokens` in its `token!` macro, each role's inline style, and the stylesheet. The layout never writes a colour or size of its own |
| `assets.rs`, `assets/` | The wordmark images and their SVG sources |
| `mailer.rs` | A queue of up to 1,024 emails, three retries after 1, 5 and 30 seconds, the SMTP, log and in-memory transports, and the MIME structure |

- **Email-safe HTML.** Tables give the structure, and everything that can be an inline style is one. A `<style>` block holds only color schemes, resets and dark mode. Conditional comments give Outlook for Windows a fixed 562px column. No web fonts, external images, scripts or tracking pixels.
- **The button** is a padded link with a full radius. For Outlook for Windows, which can't pad or round a link, it is a VML `roundrect` whose width comes from the label, so it never wraps.
- **Escaping.** maud escapes the names people choose, such as organizations, keys and projects, in text and attributes. The only unescaped markup is fixed: the doctype, Outlook's conditional comments and the VML button, which escapes what it embeds. Tests send markup and quotes through every name.
- **MIME.** `multipart/alternative` with the plain text first, and an HTML part that carries both wordmark PNGs inline by `Content-ID`, so mail apps don't list them as attachments.

## Languages

`services::mail_lang` in `kaeso-server` picks the language. The person's choice in their account settings wins (`users.locale`; `null` means Automatic). Otherwise it takes the first language Kaeso writes from the `Accept-Language` of the request that caused the email.

If the database can't answer, the fallbacks decide, so the lookup never stops an email. [Email](../../self-hosting/email.md#languages) has the rules for each kind of recipient.

Each email's function has one `match` on the language, with whole sentences and values put in by name, never fragments joined together. German addresses people with "Sie". Chinese uses "您", full-width punctuation, and a space between Chinese and Latin words or digits, which `locale::zh` puts at the seams. Dates and numbers follow the language:

| | English | German | Chinese |
| --- | --- | --- | --- |
| Date | `2026-10-14` | `14. Okt. 2026` | `2026年10月14日` |
| Amount | `1250` | `1.250` | `1,250` |
| Percentage | `80%` | `80 %` | `80%` |

## The wordmark

Emails carry the wordmark as two inline PNGs, light and dark, about 6 KB together. A self-hosted install's address may be internal, and many mail apps block remote images, so linked images could fail.

Each PNG is three times its display size and rendered from `assets/wordmark-light.svg` and `assets/wordmark-dark.svg`. A test renders the SVGs again with resvg and fails when the committed PNGs differ. To write them again:

```bash
KAESO_UPDATE_MAIL_ASSETS=1 cargo test -p kaeso-mail assets
```

## Dark mode

Kaeso sets dark mode itself instead of leaving it to each mail app's inversion. Mail apps fall into three kinds, and each gets its own layer:

1. **Apps that follow `prefers-color-scheme`**, such as Apple Mail and Thunderbird. The document declares both color schemes, so they don't invert it, and `@media (prefers-color-scheme: dark)` switches every role to its dark token with `!important`.
2. **Outlook.com and the new Outlook**, which darken emails themselves and mark what they recolor with `data-ogsc` and `data-ogsb`. The same rules under those attributes replace their guesses with Kaeso's dark values.
3. **Apps that invert without asking**, such as the Gmail apps. The light design is built to survive inversion. Its text is near-black, and its button becomes the dark theme's light pill. A white halo surrounds the light wordmark, and the dark wordmark stays hidden.

The stylesheet comes from the same token macro as the inline styles. Tests check that every dark rule is `!important` and that every role has a rule in each layer.

## Preview emails

The [development stack](../stack.md) includes Mailpit, whose web interface at http://127.0.0.1:58025 has an HTML compatibility check and a dark preview. Send every email in every language to it:

```bash
cargo test -p kaeso-mail --test smtp
```

The test checks that each email arrives with both parts and both images inline, and that Mailpit's HTML check reports enough support across mail apps. It then deletes its messages. `KAESO_TEST_SMTP_URL` and `KAESO_TEST_MAIL_API` point it elsewhere.
