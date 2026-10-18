---
title: Email
description: Connect an SMTP server, test it, and see which emails Kaeso sends and in which language.
order: 6
---

The gateway sends every email itself, over SMTP: sign-in links, invitations, password resets, key expiry warnings, budget alerts and security notices. It doesn't start until it has an SMTP server, unless you tell it to only log emails.

## Set up SMTP

Add your SMTP server to `.env`:

```bash title=".env"
KAESO_SMTP_HOST=smtp.example.com
KAESO_SMTP_PORT=587
KAESO_SMTP_TLS=starttls
KAESO_SMTP_USER=kaeso
KAESO_SMTP_PASSWORD=…
KAESO_SMTP_FROM="Kaeso <kaeso@example.com>"
```

Apply it with `docker compose up -d --wait`. Without `KAESO_SMTP_FROM`, Kaeso sends from `kaeso@` at your domain, so set an address your SMTP server may send from. Leave out the user and password for a server that needs no sign-in. [Configuration](configuration.md#email) lists every email setting and its values.

## Send a test email

1. In the console, open **Settings** › **Instance** › **Settings**. The **Mail** section shows the transport, the SMTP host and the sender.
2. Choose **Send test email**. It goes to you, or to the address you enter.
3. If it fails or doesn't arrive, read the SMTP server's answer in the gateway's log with `docker compose logs gateway`.

[Troubleshooting](troubleshooting.md#email-doesnt-arrive) has the usual causes.

## Try Kaeso without a mail server

Set `KAESO_MAIL_TRANSPORT=log` in `.env`. The gateway then logs the kind of each email instead of sending it, never its recipient or content.

Password sign-in still works, and `kaeso setup-link` prints the setup link either way. Sign-in links, invitations and alerts never arrive.

## What Kaeso sends

| Email | Sent when |
| --- | --- |
| Invitation | Someone invites a person to an organization |
| Instance invitation | An instance admin invites a person to the install |
| Sign-in link | Someone asks for a sign-in link |
| Password reset | Someone asks to reset their password |
| Email change | Someone changes their email address; sent to the new address |
| Sign-up | Someone signs up where the instance allows it |
| Password changed | A password changes |
| Two-factor on | Someone turns on two-factor authentication |
| Two-factor off | Someone turns off two-factor authentication |
| Two-factor reset | An instance admin, or `kaeso user reset-2fa`, resets it |
| Email changed | An email address changes |
| Signed out | Every session of a person ends |
| Account deactivated | An instance admin deactivates an account |
| Organization deletion | An organization is scheduled for deletion; sent to its members and to instance admins |
| Ownership | Someone makes a person an owner |
| Key expiry | One of a person's API keys expires soon |
| Budget alert | A budget reaches one of its alert thresholds |
| Test | An instance admin sends a test email |

Security and organization notices are always sent. Each person chooses which alerts they get under **Settings** › **Account** › **Notifications**, and every alert says in its footer how to turn it off.

Every email has an HTML and a plain-text part, in light and dark. Its images travel inside the email, so it loads nothing from your install and carries no tracking pixels.

## Languages

Every email exists in English, German and Simplified Chinese. Kaeso picks the language for each recipient:

| Recipient | Language |
| --- | --- |
| A person with an account | Their choice under **Settings** › **Account** › **Appearance**. On Automatic, the language of the browser that caused the email, otherwise English |
| Someone without an account, for an invitation or a sign-up link | The language of the person who invited them or asked, otherwise English |
| Recipients of key expiry warnings, budget alerts and deletion notices | Each recipient's choice, otherwise English |

## Delivery

Emails wait in a queue in the gateway's memory, up to 1,024 of them, and up to 16 go out at a time:

- When the SMTP server fails for the moment, the gateway tries three more times, after 1, 5 and 30 seconds.
- When the server refuses an email for good, the gateway doesn't try again.
- When the queue is full, the gateway drops the email.
- When the gateway stops, it waits up to 10 seconds for queued emails, and any still unsent are lost.

The gateway logs every email it can't deliver. `kaeso_emails_total` counts emails by `outcome`, `sent`, `failed` or `dropped` ([health and metrics](monitoring.md)).

## Next steps

- [Sign-in and single sign-on](sign-in.md): Choose how people sign in.
- [Troubleshooting](troubleshooting.md#email-doesnt-arrive): Find out why an email doesn't arrive.
