---
title: API keys
description: Create keys for apps and scripts, limit them, and turn them off.
order: 5
---

Apps and scripts call Kaeso with an **API key**. A key belongs to a member or a service account in one project and acts with their rights there; its own model access and limits can narrow it further.

A key is `sk-kso-` followed by 48 letters and digits. Kaeso keeps only a hash of it, so the console shows the whole key once, when it is created, and afterwards only its first and last characters.

## Create a key

On a project's **API keys** page, choose **Create key** and answer four steps:

1. **What is it for?** A name, such as `checkout-web`, which shows in request logs and alerts, and **For**: you, or a service account of the project.
2. **When does it expire?** In 1, 7, 30 or 90 days, **On a date**, or **Never**. The default is 90 days, or the longest choice allowed. A key with a date stops at the start of that day, UTC. A maximum key lifetime set by the organization or the project hides longer choices and **Never**.
3. **What can it call?**
   - **Inference**: the model API, for apps.
   - **Platform**: the Platform API, with its owner's rights in this project, for scripts.
   - **Inference and platform**: both.
4. **Models and limits**, optional: the models it may call and [limits](limits-and-budgets.md) of its own. Without them it may call what its owner may, within its owner's and the project's limits.

**Key created** then shows the key, with a sample request that uses it. Copy it before you close the dialog: the console forgets it on closing and can't show it again, and asks first if you haven't copied it.

| To create | Right |
| --- | --- |
| Your own keys | **Create own keys** |
| A service account's keys | **Create service account keys** |
| A key with platform access | Also **Give keys platform access**, and a recent sign-in |

> [!TIP]
> Give each app and each environment its own key. Its usage, logs and limits are then its own, and turning it off affects nothing else.

## The key list and a key's sheet

**Active** lists the keys that work; **All** adds disabled, expired and archived keys. **Whose keys**, above the list, shows only one member's or service account's keys.

Choose a key to open its sheet: whom it belongs to, its access, who created it and when, its expiry, its last use and its id. Its **Models** and **Limits** can be changed there at any time, with the right **Manage limits**.

## Disable, enable, archive

| Action | What it does |
| --- | --- |
| Disable | The key stops working at once; it can be enabled again |
| Enable | A disabled key works again |
| Archive | The key stops working for good; this can't be undone, and its history stays |

The actions are in the menu of a key's sheet. Changing your own keys needs the right **Manage own keys**, anyone's keys **Manage all keys**.

A key also stops working when it expires, when its project is archived, and when its member leaves the project or the organization. Kaeso emails 7 days and 1 day before a key expires: to its owner, or for a service account's key to the project's admins. Each person can turn these emails off under **Notifications** in their account.

**API keys** in your account lists your own keys across every project, to disable, enable or archive them; new keys are created in a project.

## Platform keys

**Platform keys**, in the organization's settings, are for scripts and infrastructure that manage the whole organization, such as Terraform. A platform key starts with `sk-kso-org-`, acts with the Admin role's rights in the organization and every project, and never calls models.

**Create platform key** asks what uses it and when it expires, with the same choices and the organization's maximum key lifetime. Creating one needs the right **Create platform keys** and a recent sign-in. Platform keys are disabled, enabled and archived like API keys, with the right **Disable platform keys**.

[Platform API](../api/platform-api.md) explains how scripts authenticate with keys.
