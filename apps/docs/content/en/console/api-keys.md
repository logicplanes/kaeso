---
title: API keys
description: Create keys for apps and scripts, limit what they can do, and turn them off.
order: 5
---

Apps and scripts call Kaeso with an API key. A key belongs to a member or a service account in one project, and acts with their rights there. Its own model access and limits can narrow it further.

## Create a key

1. On a project's **API keys** page, choose **Create key**.
2. **What is it for?** Give the key a name, such as `checkout-web`. Request logs and alerts show it. Under **For**, choose yourself or a service account of the project.
3. **When does it expire?** Choose 1, 7, 30 or 90 days, **On a date**, or **Never**. A key with a date stops at the start of that day, UTC.
4. **What can it call?** Choose **Inference** for apps that call models, **Platform** for scripts that manage the project through the Platform API, or **Inference and platform** for both.
5. **Models and limits**, optional: choose the models the key may call and set [limits](limits-and-budgets.md) of its own. Without them, the key may call what its owner may, within its owner's and the project's limits.

**Key created** then shows the key and a sample request that uses it. A key is `sk-kso-` followed by 48 letters and digits, and afterwards the console shows only its first and last characters.

> [!IMPORTANT]
> Copy the key before you close the dialog. Kaeso keeps only a hash of it and can't show it again. If you haven't copied it, the console asks before it closes.

The default expiry is 90 days, or the longest choice allowed. When the organization or the project sets a maximum key lifetime, longer choices and **Never** are hidden.

Creating a key needs these rights:

| To create | Right |
| --- | --- |
| Your own keys | **Create own keys** |
| A service account's keys | **Create service account keys** |
| A key with platform access | Also **Give keys platform access**, and a recent sign-in |

Give each app and each environment its own key. Its usage, logs and limits are then its own, and turning it off affects nothing else.

## Find and change a key

**Active** lists the keys that work. **All** adds disabled, expired and archived keys. **Whose keys**, above the list, shows one member's or service account's keys.

Choose a key to open its sheet. It shows whom the key belongs to, its access, who created it and when, its expiry, its last use and its id. With the right **Manage limits**, you can change its **Models** and **Limits** there at any time.

## Turn a key off

The menu of a key's sheet has three actions:

| Action | What it does |
| --- | --- |
| Disable | The key stops working at once. You can enable it again |
| Enable | A disabled key works again |
| Archive | The key stops working for good. Its history stays |

Changing your own keys needs the right **Manage own keys**, and changing anyone's keys **Manage all keys**.

A key also stops working when it expires, when its project is archived, and when its member leaves the project or the organization. Kaeso emails 7 days and 1 day before a key expires. The email goes to the key's owner, or for a service account's key to the project's admins. Anyone can turn these emails off under **Expiring keys** in their account's **Notifications**.

**API keys** in your [account](account.md) lists your own keys across every project, where you can disable, enable or archive them. You create new keys in a project.

## Platform keys

Platform keys are for scripts and infrastructure that manage the whole organization, such as Terraform. A platform key starts with `sk-kso-org-`. It acts with the Admin role's rights in the organization and every project, and never calls models.

1. In **Organization settings**, open **Platform keys** and choose **Create platform key**.
2. Say what uses the key, and when it expires. The choices and the organization's maximum key lifetime are the same as for API keys.

Creating a platform key needs the right **Create platform keys** and a recent sign-in. You disable, enable and archive platform keys like API keys, with the right **Disable platform keys**.

## Next steps

- [Calling models](../get-started/calling-models.md): Send your first request with the new key.
- [Limits and budgets](limits-and-budgets.md): Cap what a key may request and spend.
- [Platform API](../api/platform-api.md): Manage projects, keys and models from scripts.
