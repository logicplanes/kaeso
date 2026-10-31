---
title: Request logs
description: Find any request a project served, see what was sent and returned, export logs, and choose what is kept.
order: 8
---

Kaeso logs every model request: the model, the provider, the status, tokens, cost, timing, the key and who sent it. With content logging on, it also keeps what the app sent and what it got back. A request shows up a few seconds after it ends.

> [!NOTE]
> An install can run with request logs off ([choose how to keep request logs](../self-hosting/installation.md#choose-how-to-keep-request-logs)). Its projects have no **Request logs** page, and **Logging** says that logs are off. Usage and costs are counted the same.

## Find a request

Open a project's **Request logs** page. It lists requests newest first, from the last hour, 24 hours, 7 days or 30 days, or a range you choose.

- **Add filter** narrows the list by key, member, model, provider, status, error kind, streaming and source: the API or the Playground. **Failed only** keeps the errors.
- **App format** and **Provider format** narrow it to one route, such as OpenAI Responses to Anthropic Messages. Caching, overhead and errors often differ between routes.
- **Live** adds new requests as they arrive. Once you have scrolled down, a button says how many are new.
- The display options choose the columns and the density.
- **Open a request by id** finds the request whose `kaeso-request-id` header your app received.

Reading the logs needs the right **View request logs**.

## Read a request

Choose a request to open it:

| Tab | What it shows |
| --- | --- |
| Overview | The outcome, the route, who sent it, the timing, the stream and the usage |
| Request | What the app sent |
| Reply | What the app got back |
| Raw | The log entry as the Platform API returns it |

The timing keeps Kaeso's own work apart from the app's and the provider's. **Request upload** is the time the app took to send the request body, from when Kaeso began reading it until all of it had arrived; it grows with the size of the body and the app's connection, not with Kaeso's work. **Kaeso overhead** is Kaeso's own time before it called the provider, without the upload: checking the key, decoding, translating and checking limits. For a stream, **Stream processing** is the time Kaeso spent translating the provider's events for the app, without the waits for either side. The CSV export has all three, as `request_upload_ms`, `overhead_ms` and `stream_processing_ms`.

A failed request shows its error kind, such as **Rate limited**, **Context too long**, **Timeout**, **Client disconnected** or **Client stalled**. Where the provider sent a message, it shows that too. A stream that broke off is logged as a failure, with how far it got.

The **Request** and **Reply** tabs need the right **View prompts and outputs**. They tell you when that right is missing, when content wasn't logged, or when it has passed its retention. Kaeso never keeps embedding vectors. A reply logged in a format this console can't read yet, for example by a newer gateway, is shown as it was stored.

## Check prompt caching

The **Tokens in / out · cached** column ends with the share of each request's input read from the prompt cache, such as `· 0%`. A long prompt that is never cached costs the full input price every time.

Anthropic caches only the parts of a prompt that a request marks with `cache_control`, and Kaeso marks them itself for apps that use OpenAI's formats. A request sent in Anthropic's format without a mark, which read nothing from the cache and wrote nothing to it, shows its share in a warning color. Its **Usage** says **No cache breakpoint**, so a missing mark stands apart from a cache miss. Some Anthropic-compatible providers cache without marks; their requests show no warning once they read or write the cache.

A request's **Usage** also lists **Read from the cache** and, for Anthropic's format, how many **Cache breakpoints** Kaeso sent. The CSV export has that count as `cache_breakpoints`.

In a project with the [response cache](response-cache.md) on, **Usage** also says what the **Response cache** did: answered the request, sent it to the provider, or passed it by. The CSV export has it as `response_cache`.

## Export logs

**Export** downloads the requests that match the current filters:

| Format | Holds |
| --- | --- |
| CSV | Metadata, for spreadsheets |
| JSON Lines | Metadata, one request per line |
| JSON Lines with content | Metadata, and the stored requests and replies |

Exporting needs the right **Export logs**, and with content also **View prompts and outputs**.

In CSV, Kaeso puts every text value in double quotes. It adds a `'` in front of a value that starts with `=`, `+`, `-`, `@`, a tab or a carriage return. It does the same for a value that holds `;`, `|` or a tab, and after such a separator when one of those characters follows. A spreadsheet then shows the value as text instead of running it as a formula, whichever separator it splits on.

## Retention

Each organization chooses what it keeps, and for how long, under **Organization settings** › **Logging**:

| Setting | Default | What it does |
| --- | --- | --- |
| Log request content | On | Whether request and reply bodies are kept with their log entries |
| Metadata retention | 90 days | How long log entries are kept |
| Content retention | 30 days | How long request and reply bodies are kept. Never longer than the metadata |

Shortening a retention deletes older entries, and lengthening it applies to new ones. The install sets the defaults for new organizations and the longest retention an organization may choose ([instance settings](instance-administration.md#change-install-settings)). Changing these settings needs the right **Manage log settings**.

A project can turn content logging off for itself in its **Settings**, with the right **Switch content logging**. It can't turn it on while the organization has it off.

## Delete logs

The **Storage** tab, under **Logging**, shows how many entries the organization keeps and the space they take, per project. It and the **Deletions** tab need the right **View log storage**.

1. On the **Deletions** tab, choose **Delete logs**.
2. Choose whole UTC days, and every project or one.
3. Type `delete` to confirm.

Kaeso deletes the logs of those days for good, and keeps the usage and costs. The list shows each deletion as **Queued** or **Done**, and at most 10 deletions wait at a time. It also lists the deletions a shorter retention starts, as **Shorter metadata retention** or **Shorter content retention**. A deletion that failed and waits to run again shows "Retrying:" and the error. Deleting needs the right **Delete logs** and a recent sign-in.

## Next steps

- [Organizations and projects](organizations-and-projects.md): Turn content logging off for a single project.
- [Errors](../api/errors.md): Look up what a failed request returns to the app.
- [Requirements](../self-hosting/requirements.md): Plan the disk space request logs take.
