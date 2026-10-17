---
title: Request logs
description: Read every request a project served, export it, and choose what is kept.
order: 7
---

Kaeso logs every model request: the model, the provider, the status, tokens, cost, timing, the key and who sent it. With content logging on, it also keeps what the app sent and what it got back. An entry appears a few seconds after its request ends. Reading the logs needs the right **View request logs**.

## The list

A project's **Request logs** page lists requests, newest first, from the last hour, 24 hours, 7 days or 30 days, or a range you choose.

- **Add filter** narrows the list by key, member, model, provider, status, error kind, streaming and source (the API or the Playground); **Failed only** keeps the errors.
- **Live** adds new requests as they arrive. Once you have scrolled down, a button says how many are new.
- The display options choose the columns and the density.
- **Open a request by id** finds the request whose `kaeso-request-id` your app received.

## A request

Choose a request to open it:

| Tab | What it shows |
| --- | --- |
| Overview | The outcome, the route, who sent it, the timing, the stream and the usage |
| Request | What the app sent |
| Reply | What it got back |
| Raw | The log entry as the Platform API returns it |

A failed request shows its error kind, such as **Rate limited**, **Context too long**, **Timeout** or **Client disconnected**, and the provider's message where there was one. A stream that broke off is logged as a failure, with how far it got.

The **Request** and **Reply** tabs need the right **View prompts and outputs**. They say so when it is missing, when content wasn't logged, or when it has passed its retention. Embedding vectors are never kept. A reply logged in a format this console can't read yet, for example by a newer gateway, is shown as stored.

## Export

**Export** downloads the requests that match the current filters:

| Format | Holds |
| --- | --- |
| CSV | Metadata, for spreadsheets |
| JSON Lines | Metadata, one request per line |
| JSON Lines with content | Also the stored requests and replies |

Exporting needs the right **Export logs**, and with content also **View prompts and outputs**. In CSV, a value that starts with `=`, `+`, `-`, `@`, a tab or a carriage return gets a leading `'`, so a spreadsheet shows it as text instead of running it as a formula.

## Retention

Each organization chooses what is kept, and for how long, under **Organization settings** › **Logging**:

| Setting | Default | What it does |
| --- | --- | --- |
| Log request content | On | Whether requests and replies are kept with their log entries |
| Metadata retention | 90 days | How long entries are kept |
| Content retention | 30 days | How long requests and replies are kept; never longer than the metadata |

The install sets the defaults for new organizations and the longest retention an organization may choose ([instance settings](instance-administration.md#settings)). Shortening a retention deletes older entries; lengthening it applies to new ones. Changing these settings needs the right **Manage log settings**.

A project can turn content logging off for itself under its **Settings**, but not on while the organization has it off.

## Storage and deletion

The **Storage** tab, under **Logging**, shows how many entries the organization keeps and the space they take, per project.

On the **Deletions** tab, **Delete logs** deletes the logs of whole UTC days, from every project or one, for good; usage and costs stay. You confirm by typing `delete`, and the list shows each deletion as **Queued** or **Done**. At most 10 deletions wait at a time. Deleting needs the right **Delete logs** and a recent sign-in.
