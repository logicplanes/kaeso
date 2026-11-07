---
title: Emails
description: How the gateway builds the emails it sends, for changes to the kaeso-mail crate.
order: 4
---

Every email Kaeso sends comes from `crates/kaeso-mail`: one layout, one stylesheet built from Kaeso's tokens, and one function per email in English, German and Simplified Chinese. [Email](../../self-hosting/email.md) covers what operators set up and see; this page covers the code.

## What an email is

| Part | In the sign-in email | Style |
| --- | --- | --- |
| Wordmark or cover | Kaeso | The wordmark alone, top left; announcements open with a [cover](#covers) instead |
| Where, when a notification is about an organization or project | Acme / Checkout | `small`, tertiary, above the heading |
| Heading | Sign in to Kaeso | `title-lg` |
| One sentence | The link works once and expires in 15 minutes. | `body-lg`, secondary |
| Figures, when a report has them | 12,480 requests, 312 tool calls, $41.20 | A panel of up to three values, `title` and `caption` |
| Details, when there are facts | Organization: Acme, Provider: Anthropic, Key: Production (…8QAA) | Label and value between hairlines, `small` tertiary and `body-lg` |
| A table, when a report lists projects | Checkout, 9,120, $30.10 | Rows between hairlines, numbers right-aligned in tabular figures, grouped under each organization's name when there are several |
| A list, when there are changes | What changed, then up to four lines of the release notes; in the digest, the lines under Acme or Acme / Checkout | A short label and bulleted lines |
| One button | Sign in | The `xl` pill, ink on white, 48px tall |
| A text link, when there is more to read | Read the release notes | Underlined, ink |
| One quiet line | If you did not ask to sign in, you can ignore this email. | `small`, tertiary |
| Footer | Sent by Kaeso at kaeso.example.com. | `caption` |

- **No card.** A single left-aligned column, at most 560px wide, on the canvas: white in light mode, black in dark mode. Spacing follows the 8px grid, with more space between groups than inside them; on a phone the margins shrink to 16px and the heading to 24px.
- **Copy limits.** A subject of at most 60 characters without a period, a heading of at most 48, and sentences of at most 100. German and Chinese get 15% more: 69, 55 and 115. A test enforces the limits.
- **Details name the thing.** An email about a key, a budget or a model answers what, where and when on its own: a list of labels and values under the sentence names the organization, the project, the provider, the key and its last four characters, the amount used and when it resets, the provider's answer and the times. Each send site passes what it knows; the email never guesses.
- **No raw links.** The HTML never shows a URL; the button carries it. The plain-text part has the link in full on its own line, and it says the same words in the same order.
- **Footer.** "Sent by Kaeso at" the install's host, from `KAESO_PUBLIC_URL`. Notices also say why they arrived: security and organization notices are always sent, and alerts say how to turn them off. Notifications name the right that made the person get them, link to their settings and, unless they are security emails, to a one-click unsubscribe, which `List-Unsubscribe` and `List-Unsubscribe-Post` carry too.

The text and HTML of every email are snapshots in `crates/kaeso-mail/src/snapshots/`.

## How it is built

| File | Job |
| --- | --- |
| `templates.rs` | One function per email, which is the catalogue of [what Kaeso sends](../../self-hosting/email.md#what-kaeso-sends): subject, heading, sentence, button, quiet line and why it is sent, in each language |
| `notices.rs` | The notifications: one `Fact` per kind of event, rendered as `[Organization or project] fact` with the reason and links, and the daily digest |
| `locale.rs` | `Locale` (`en`, `de`, `zh-CN`), and dates and numbers as each language writes them |
| `layout.rs` | The shared layout, as HTML with maud and as plain text |
| `style.rs` | The values of `@logicplanes/kaeso-tokens` in its `token!` macro, each role's inline style, and the stylesheet. The layout never writes a colour or size of its own |
| `assets.rs`, `assets/` | The wordmark and cover images and their SVG sources |
| `mailer.rs` | A queue of up to 1,024 emails, three retries after 1, 5 and 30 seconds, the SMTP, log and in-memory transports, and the MIME structure |

- **Email-safe HTML.** Tables give the structure, and everything that can be an inline style is one. A `<style>` block holds only color schemes, resets and dark mode. Conditional comments give Outlook for Windows a fixed 560px column. No web fonts, external images, scripts or tracking pixels.
- **The button** is a padded link with a full radius. For Outlook for Windows, which can't pad or round a link, it is a VML `roundrect` whose width comes from the label, so it never wraps.
- **Escaping.** maud escapes the names people choose, such as organizations, keys and projects, in text and attributes. The only unescaped markup is fixed: the doctype, Outlook's conditional comments and the VML button, which escapes what it embeds. Tests send markup and quotes through every name.
- **MIME.** `multipart/alternative` with the plain text first, and an HTML part that carries the images it shows inline by `Content-ID`, the two wordmark PNGs or the cover, so mail apps don't list them as attachments.

## Notifications

`services::notifications` in `kaeso-server` decides who gets a notification and when.

1. Every audit event is collected for the request or job that wrote it. The dispatcher runs once the request answered with success or the job finished, so a change that rolled back notifies nobody.
2. `dispatch::kinds` maps each audit `Action` to the kinds it notifies about, in one `match` without a wildcard, so a new action needs a decision. Events without an action call `system` directly: a connection or provider account that signs out, a held tool, a provider key refused three times in a row, a finished export, a hard budget used up and a new release.
3. Recipients come from rights, never from role names: the rights of the kind in `kaeso-core`'s notification catalogue, held in the organization or project, the instance admins, or the person the event is about. The person who acted is left out, except for security emails.
4. Each person's setting chooses now, daily or off, and a muted organization stops all but security emails. `notifications_sent` keeps one email per subject and day, and past 20 immediate emails an hour the rest go to `notification_digest`.
5. The `notifications` job runs every minute. At 8:31 in the install's time zone it sends waiting digests, on the 1st the monthly cost report and on Mondays the weekly summary. The weekly summary names the week and each organization, and its button opens the usage of that week; a digest line keeps the organization and project it was about, so the digest groups its lines by them.

## Languages

`services::mail_lang` in `kaeso-server` picks the language. The person's choice in their account settings wins (`users.locale`; `null` means Automatic). Otherwise it takes the first language Kaeso writes from the `Accept-Language` of the request that caused the email.

If the database can't answer, the fallbacks decide, so the lookup never stops an email. [Email](../../self-hosting/email.md#languages) has the rules for each kind of recipient.

Each email's function has one `match` on the language, with whole sentences and values put in by name, never fragments joined together. German addresses people with "Sie". Chinese uses "您", full-width punctuation, and a space between Chinese and Latin words or digits, which `locale::zh` puts at the seams. Dates and numbers follow the language:

| | English | German | Chinese |
| --- | --- | --- | --- |
| Date | `14 Oct 2026` | `14. Okt. 2026` | `2026年10月14日` |
| Month | `October 2026` | `Oktober 2026` | `2026年10月` |
| Amount | `1,250` | `1.250` | `1,250` |
| Dollars | `$1,250.00` | `1.250,00 $` | `$1,250.00` |
| Percentage | `80%` | `80 %` | `80%` |

## The wordmark

Emails carry the wordmark as two inline PNGs, light and dark, about 6 KB together. A self-hosted install's address may be internal, and many mail apps block remote images, so linked images could fail.

Each PNG is three times its display size and rendered from `assets/wordmark-light.svg` and `assets/wordmark-dark.svg`. A test renders the SVGs again with resvg and fails when the committed PNGs differ. To write them again:

```bash
KAESO_UPDATE_MAIL_ASSETS=1 cargo test -p kaeso-mail assets
```

## Covers

Announcements open with a cover instead of the wordmark: invitations, sign-up and setup links, the test email, a new app, an accepted invitation, the weekly summary, the digest, the monthly cost report, and a new or installed version. Alerts, security and organization emails never do; they stay short and start with the wordmark.

Each cover is Kaeso's release art, drawn by `drawReleaseArt` in `packages/ui` in its wide shape, with the wordmark in the corner the art keeps clear. It travels inline like the wordmark: a 1120×480 PNG shown at 560×240, under 60 KB, with alt text in each language. It holds no text but the wordmark, so it needs no translation and looks the same in light and dark. `scripts/mail-covers.mjs` writes the SVGs, a test next to it checks them against the release art, and the assets test renders the PNGs from them. To draw them again after the release art changes:

```bash
node scripts/mail-covers.mjs
KAESO_UPDATE_MAIL_ASSETS=1 cargo test -p kaeso-mail assets
```

The email about a new version writes the version as text, not into the cover. It lists the first four changes of the release notes and how many more there are, says whether the release installs by itself in the update window, from the updates page or by hand, and links to the notes.

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

The test checks that each email arrives with both parts and exactly the images it shows inline, and that Mailpit's HTML check reports enough support across mail apps. It then deletes its messages. `KAESO_TEST_SMTP_URL` and `KAESO_TEST_MAIL_API` point it elsewhere.
