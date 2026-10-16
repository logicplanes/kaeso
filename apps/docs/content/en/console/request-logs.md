---
title: Request logs
description: Read every request a project served, export it, and choose what is kept.
order: 7
---

Every model request is logged: the model, the provider, the status, tokens, cost, timing, the key and who sent it. When content logging is on, the request and the reply are kept too. Entries appear a few seconds after a request ends.

## The list

A project's **Request logs** page lists requests, newest first, for the last hour, day, 7 days, 30 days or a range you choose.

- **Add filter** narrows the list by key, member, model, provider, status, error kind, whether the reply was streamed, and the source: the API or the Playground. **Failed only** shows only errors.
- **Live** adds new requests as they arrive; when you have scrolled down, a button shows how many are new.
- The display options choose the columns and the density.
- **Open a request by id** finds one by the `kaeso-request-id` your app received.

## A request

Choose a request to open it:

| Tab | What it shows |
| --- | --- |
| Overview | The outcome, the route, who sent it, timing, the stream's progress and usage |
| Request | What the app sent |
| Reply | What it got back |
| Raw | Both as JSON |

Prompts and replies need the right **View prompts and outputs**. Without it, or when content wasn't logged or has passed its retention, the tabs say so. Embedding vectors are never kept.

A failed request shows its error kind, such as **Rate limited**, **Context too long**, **Timeout** or **Client disconnected**, and the provider's message where there was one. A streamed reply that broke off is logged as a failure with how far it got.

## Export

**Export** downloads the requests that match the current filters as CSV, JSON Lines, or JSON Lines with content. Exporting needs the right **Export logs**, and content also **View prompts and outputs**.

## Retention

What is kept, and for how long, is set per organization under **Organization settings** › **Logging**:

| Setting | Default | What it does |
| --- | --- | --- |
| Log request content | On | Whether requests and replies are kept with their log entries |
| Metadata retention | 90 days | How long entries are kept |
| Content retention | 30 days | How long requests and replies are kept; never longer than the metadata |

The install sets the defaults for new organizations and the longest retention an organization may choose. Shortening retention deletes older entries; lengthening it applies to new ones. Changing these settings needs the right **Manage log settings**.

A project can turn content logging off for itself under its **Settings**, but can't turn it on when the organization has it off.

## Storage and deletion

The **Storage** tab, under **Logging**, shows how many entries the organization has and how much space they take, per project. On the **Deletions** tab, **Delete logs** deletes the logs of chosen whole UTC days, for every project or one, for good; usage and costs stay. You confirm by typing `delete`, and the list shows each deletion as queued or done. Deleting needs the right **Delete logs** and a recent sign-in.
