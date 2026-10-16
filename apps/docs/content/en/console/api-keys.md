---
title: API keys
description: Create keys for apps and scripts, limit them, and turn them off.
order: 5
---

Apps and scripts call Kaeso with an **API key**. A key belongs to a member or a service account in one project, acts with their rights there, and can be narrowed further with its own model access and limits.

A key looks like `sk-kso-` followed by 48 letters and digits. Kaeso keeps only a hash of it: the console shows it once, when it's created, and afterwards only its first and last characters.

## Create a key

On a project's **API keys** page, choose **Create key**.

1. **What is it for?** A name, such as `checkout-web`, and whom it belongs to: you, or, with the right **Create service account keys**, a service account of the project.
2. **When does it expire?** In 1, 7, 30 or 90 days, on a date, or never. The default is 90 days, or the longest choice allowed. A key with a date stops at the start of that day, UTC. The organization or the project may set a longest lifetime, which hides longer choices and **Never**.
3. **What can it call?**
   - **Inference**: the model API, for apps.
   - **Platform**: the Platform API, with its owner's rights in this project, for scripts.
   - **Inference and platform**: both.
4. **Models and limits**, optional: the models it may call and [limits](limits-and-budgets.md) of its own.

Then copy the key. The dialog shows a sample request with it. Creating your own keys needs the right **Create own keys**; giving a key platform access also needs **Give keys platform access** and a recent sign-in.

> [!TIP]
> Give each app and each environment its own key. Its usage, logs and limits are then its own, and turning it off affects nothing else.

## Disable, enable, archive

| Action | What it does |
| --- | --- |
| Disable | The key stops working at once; enable it again at any time |
| Enable | A disabled key works again |
| Archive | The key stops working for good; this can't be undone |

Changing your own keys needs the right **Manage own keys**, and anyone's keys **Manage all keys**. Keys also stop when they expire, when their project is archived, and when their member leaves the project or the organization. The page groups keys as active, disabled and expired; archived keys are listed under **All**.

Expiring keys are announced by email 7 days and 1 day before: to their owner, or for a service account's key to the project's admins. Each person can turn these emails off under **Notifications** in their account.

## Your keys everywhere

**API keys** in your account lists your keys across every project, to disable, enable or archive them. New keys are created in a project.

## Platform keys

An organization's **Platform keys**, under its settings, are for scripts and infrastructure that manage the whole organization. A platform key starts with `sk-kso-org-`, acts with the Admin role's rights in the organization and every project, and never calls models. **Create platform key** asks for a name and an expiry, with the same choices and the organization's longest lifetime. Creating one needs the right **Create platform keys** and a recent sign-in.

See [Platform API](../api/platform-api.md#authentication) for how scripts use keys.
