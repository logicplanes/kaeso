---
title: Account and language
description: Keep your account secure, sign out other browsers, and choose your theme and language.
order: 9
---

Your account is under **Account**, in the menu at the foot of the sidebar. It has these pages:

| Page | What it holds |
| --- | --- |
| Profile | Picture, name, email address and account id, and deleting the account |
| Security | Password, two-factor authentication and recovery codes |
| Sessions | The browsers you are signed in with |
| API keys | Your own keys in every project ([API keys](api-keys.md)) |
| Memberships | Your organizations and their projects, with your roles, and leaving an organization |
| Notifications | Which emails you receive |
| Appearance | Theme and language |

## Change your email address

Under **Profile**, choose **Change email** and enter the new address. Kaeso sends a confirmation link to the new address and tells the old one. The change takes effect when you open the link.

## Change your password

Under **Security**, **Change password** sets a new password and signs out your other sessions. If you sign in only with links or single sign-on, you have no password, and **Set password** adds one.

## Turn on two-factor authentication

Two-factor authentication asks for a code from an authenticator app at every password sign-in. Sign-in links and single sign-on don't ask for one.

1. Under **Security**, choose **Set up**.
2. Scan the QR code with your authenticator app, or enter the setup key.
3. Enter the six-digit code the app shows, and choose **Turn on**.
4. Store the 10 recovery codes the console shows once: copy them or choose **Download .txt**. Each code signs you in once instead of an app code.

**Regenerate** replaces the recovery codes, and **Turn off** removes two-factor authentication. If you lose both your authenticator and your recovery codes, an instance admin can reset two-factor authentication for you.

## Sign out other browsers

**Sessions** lists every browser where you are signed in, and how you signed in. **End session** signs one out, and **Sign out everywhere** signs out all but this one.

A session ends after 7 days without use, and after 30 days at most.

## Confirm a sensitive action

Some actions need a sign-in from the last 15 minutes. If yours is older, the console asks you to sign in again before it carries them out.

- In your account: changing your email address, setting a password when you have none, turning off two-factor authentication, regenerating recovery codes and deleting the account.
- In an organization: the actions whose right needs a recent sign-in in [rights](../concepts/rights.md), such as adding provider keys, creating platform keys or deleting logs.
- As an instance admin: making someone an instance admin or removing it, deleting a user, resetting a user's two-factor authentication and appointing an organization's owner.

## Choose your emails

Under **Notifications**, **Budget alerts** and **Expiring keys** turn those emails on or off. Kaeso always sends security emails, such as one about a changed password.

## Choose a theme and language

Under **Appearance**:

- **Theme** is **System**, **Light** or **Dark**. This browser keeps it.
- **Language** is automatic, English, Deutsch or 简体中文. Automatic follows your browser's language, or English when the console doesn't have it. Kaeso saves the choice to your account, so it follows you to other browsers, and emails from Kaeso use it too.

Product names and model ids stay as they are in every language.

## Leave an organization or delete your account

**Leave organization**, under **Memberships**, takes you out of the organization and its projects and archives your keys there.

**Delete account**, under **Profile**, takes you out of every organization and archives all your keys. History shows you as a deleted user. Type your email address to confirm.

Neither works while you are an organization's last owner. Make someone else an owner first.

## Next steps

- [Organizations and projects](organizations-and-projects.md): Require two-factor authentication or single sign-on for an organization.
- [API keys](api-keys.md): Create keys for your apps and scripts.
- [Sign-in and single sign-on](../self-hosting/sign-in.md): See how the sign-in page works, and how to get back in.
