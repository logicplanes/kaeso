---
title: Email
description: How Kaeso sends email, what it sends, in which language, and how to test it.
order: 5
---

Kaeso sends email for sign-in links, invitations, password resets, key expiry warnings, budget alerts and security notices. The gateway sends every email itself, over SMTP.

## Set up SMTP

Add your SMTP server to `.env` and apply it with `docker compose up -d --wait`:

```bash
KAESO_SMTP_HOST=smtp.example.com
KAESO_SMTP_PORT=587
KAESO_SMTP_TLS=starttls
KAESO_SMTP_USER=kaeso
KAESO_SMTP_PASSWORD=…
KAESO_SMTP_FROM=Kaeso <kaeso@example.com>
```

| Setting | What to know |
| --- | --- |
| `KAESO_SMTP_TLS` | `starttls` on port 587, `tls` for implicit TLS (usually port 465), `none` only for a local mail catcher |
| `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD` | Leave both out for a server that needs no sign-in; a password needs a user |
| `KAESO_SMTP_FROM` | The sender of every email, as an address or `Name <address>`. The Compose file sets `Kaeso <kaeso@` and your domain; use an address your SMTP server may send from |

Every setting is listed under [configuration](configuration.md#email). The gateway refuses to start without `KAESO_SMTP_HOST` unless email is only logged.

## Without a mail server

To try Kaeso without a mail server, set `KAESO_MAIL_TRANSPORT=log`. The gateway then only logs that an email would have been sent, with its kind, never its recipient or content. Signing in with a password works, and the setup link is in the gateway's log either way; sign-in links, invitations and alerts are not delivered.

## What Kaeso sends

| Email | Subject | Sent when |
| --- | --- | --- |
| Invitation | {inviter} invited you to {org} on Kaeso | Someone invites a person to an organization |
| Instance invitation | {inviter} invited you to Kaeso | An instance admin invites a person to the install |
| Sign-in link | Sign in to Kaeso | Someone asks for a sign-in link |
| Password reset | Reset your Kaeso password | Someone asks to reset their password |
| Email change | Confirm your new Kaeso email address | Someone changes their email address; sent to the new address |
| Sign-up | Finish creating your Kaeso account | Someone signs up where the instance allows it |
| Password changed | Your Kaeso password was changed | A password changes |
| Two-factor on | Two-factor authentication is on | Someone turns on two-factor authentication |
| Two-factor off | Two-factor authentication is off | Someone turns off two-factor authentication |
| Two-factor reset | Your two-factor authentication was reset | An instance admin, or `kaeso user reset-2fa` on the server, resets it |
| Email changed | Your Kaeso email address was changed | An email address changes |
| Signed out | You were signed out of Kaeso | Every session of a person was ended |
| Account deactivated | Your Kaeso account was deactivated | An instance admin deactivates an account |
| Organization deletion | {org} is scheduled for deletion | An organization is scheduled for deletion; sent to its members and to instance admins |
| Ownership | You are now an owner of {org} | Someone makes a person an owner |
| Key expiry | The API key {key} expires on {date} | One of a person's API keys expires soon |
| Budget alert | {scope} used {percent}% of its {budget} | A budget reaches one of its alert thresholds |
| Test | Kaeso test email | An instance admin sends a test email |

Security and organization notices are always sent. Alerts say in their footer how to turn them off; people choose which alerts they get under **Settings** › **Account** › **Notifications**.

## Languages

Every email exists in English, German and Simplified Chinese, with dates and numbers written as each language writes them.

| Recipient | Language |
| --- | --- |
| A person with an account | The language they chose under **Settings** › **Account**; on Automatic, the language of the browser that caused the email; else English |
| Someone without an account yet, for an invitation or a sign-up link | The language of the person who invited them or asked, else English; an invitee who already has an account gets their own choice |
| Emails sent without a browser: key expiry warnings, budget alerts, organization deletion notices | Each recipient's choice, else English |

## How emails look

Every email has the console's look: the Kaeso wordmark at the top left, one heading, one sentence, at most one button, and a footer that names the install. The HTML never shows a raw link, and a plain-text part says the same words with the link in full.

- **Light and dark.** Mail apps that follow the system's dark mode get Kaeso's dark colors; Outlook.com's own dark mode gets the same values; apps that invert colors by themselves get a light design built to survive the inversion.
- **Nothing from outside.** The wordmark images travel inside the email, so they always show and never load anything from your install. There are no web fonts, external images, scripts or tracking pixels.

## Delivery

Emails wait in a queue in the gateway's memory and are sent one after another. A delivery that fails is tried three more times, after 1, 5 and 30 seconds, then given up and logged. Emails still queued when the gateway stops are not sent.

The metric `kaeso_emails_total{kind, outcome}` counts emails by `outcome`: `sent`, `failed`, or `dropped` when the queue is full. See [health and metrics](monitoring.md).

## Test it

In the console, open **Settings** › **Instance** › **Settings**. The **Mail** section shows the transport, the SMTP host and the sender; choose **Send test email** to send one to yourself, or to the recipient you enter. If it fails or doesn't arrive, the gateway's log says what the SMTP server answered; [troubleshooting](troubleshooting.md#email-doesnt-arrive) has more.
