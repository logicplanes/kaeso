---
title: Account and language
description: Your profile, password, two-factor authentication, sessions, theme and language.
order: 9
---

Your account is under **Account**, in the menu at the foot of the sidebar:

| Page | What it holds |
| --- | --- |
| Profile | Picture, name, email address and account id; deleting the account |
| Security | Password, two-factor authentication and recovery codes |
| Sessions | The browsers you are signed in with |
| API keys | Your own keys in every project ([API keys](api-keys.md)) |
| Memberships | Your organizations and their projects, with your roles; leaving an organization |
| Notifications | Which emails you receive |
| Appearance | Theme and language |

## Email address

**Change email** sends a confirmation link to the new address and tells the old one. The change takes effect when you open the link. It needs a recent sign-in.

## Password

**Change password** signs out your other sessions. If you sign in only with links or single sign-on and have no password, **Set password** adds one.

## Two-factor authentication

Two-factor authentication asks for a code from an authenticator app at every password sign-in; sign-in links and single sign-on need none.

1. Choose **Set up** under **Security**.
2. Scan the QR code with the app, or enter the setup key.
3. Enter the six-digit code the app shows, and choose **Turn on**.
4. Store the 10 recovery codes the console shows once: copy them or **Download .txt**. Each signs you in once instead of a code.

**Regenerate** replaces the recovery codes, and **Turn off** removes two-factor authentication; both need a recent sign-in. If you lose both your authenticator and your recovery codes, an instance admin can reset two-factor authentication for you.

## Sessions

**Sessions** lists every browser where you are signed in, and how you signed in. **End session** signs one out, and **Sign out everywhere** signs out all but this one. A session ends after 7 days without use, and after 30 days at most.

## Leaving an organization and deleting the account

**Leave organization**, under **Memberships**, takes you out of the organization and its projects and archives your keys there. **Delete account**, under **Profile**, takes you out of every organization and archives all your keys; history shows you as a deleted user. You confirm by typing your email address, and it needs a recent sign-in.

Neither works while you are an organization's last owner: make someone else an owner first.

## Notifications

**Budget alerts** and **Expiring keys** turn those emails on or off. Security emails, such as one about a changed password, are always sent.

## Theme and language

Under **Appearance**:

- **Theme**: **System**, **Light** or **Dark**. It is kept in this browser.
- **Language**: **Automatic**, English, Deutsch or 简体中文. Automatic follows your browser's language, or English when the console doesn't have it. The choice is saved to your account, so it follows you to other browsers, and emails from Kaeso use it too.

Product names and model ids stay as they are in every language.
