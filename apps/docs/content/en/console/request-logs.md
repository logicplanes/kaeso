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

## Find a tool call

**Tools** at the top of the page lists the calls MCP clients made through the project's MCP portal instead: the tool, whether it was called, failed or was refused, the caller, the client and how long the server took. A refused call says why, such as a tool that is off, waits for review or is beyond the caller's access, or a tool call rate. With content logging on, a call also keeps its arguments and its answer, shown only with the right **View prompts and outputs**.

The tool calls have the same toolbar: **Add filter** narrows them by key, member, MCP server, tool, status and error kind, **Live** adds new calls as they arrive, and **Open a tool call by id** finds one call. Switching between **Models** and **Tools** keeps the time range, the filters of both and **Live**.

## Read a request

Choose a request to open it:

| Tab | What it shows |
| --- | --- |
| Overview | The outcome, the route, who sent it, the timing, the stream and the usage |
| Request | What the app sent |
| Reply | What the app got back |
| Raw | The log entry as the Platform API returns it |

The timing keeps Kaeso's own work apart from the app's and the provider's. **Request upload** is the time the app took to send the request body, from when Kaeso began reading it until all of it had arrived; it grows with the size of the body and the app's connection, not with Kaeso's work. **Kaeso overhead** is Kaeso's own time before it called the provider, without the upload: checking the key, decoding, translating and checking limits. For a stream, **Stream processing** is the time Kaeso spent translating the provider's events for the app, without the waits for either side. A time under a millisecond shows in microseconds, such as 180 µs. The CSV export has all three in microseconds, as `request_upload_us`, `overhead_us` and `stream_processing_us`.

A failed request shows its error kind, such as **Rate limited**, **Context too long**, **Timeout**, **Client disconnected** or **Client stalled**. Where the provider sent a message, it shows that too. A stream that broke off is logged as a failure, with how far it got.

A media request shows what it sent and made, such as the images, the characters spoken or the length of the audio or video ([media in requests and replies](#media-in-requests-and-replies)). When the model has no price for what a request made, such as a video model without a price per clip, its cost shows as **Unknown**, not $0; the export's `cost_known` column says the same. A model whose prices are set to 0, such as a free model, costs a known $0. When a provider bills audio by its length but reports none, Kaeso reads the length from the file's header; the cost then says so, and the export's `cost_basis` is `measured` ([audio](../api/audio.md#costs-and-logs)). A request Kaeso refused before sending it, or that its provider refused or failed, costs $0 and adds nothing to usage, while its log still shows what it asked for. An app that disconnects pays for what its provider was already sent, and a realtime session for what it spoke or heard before it ended.

A request to an alias shows the **Alias**. When its route fell back, **Tried first** lists the steps that failed before the model that answered, each with its status and time, and the ones skipped because they had failed moments ago ([alias routes](../concepts/alias-routes.md)). The CSV export has them as `alias` and `fallbacks`. An image request shows **Images**: how many the app sent and the provider made, with the size and quality; the CSV export has them as `media_unit`, `media_input`, `media_output` and `media_detail`. A speech request shows **Speech**, the characters to speak, and a transcription **Audio**, the length of the transcribed audio, each with its format; the CSV's `media_unit` is `character` or `millisecond`. A video is logged twice: its start as `videos` or `videos/remix`, and its end as `videos/completed`, which carries the cost. Both show **Video**, the clips and their length, with the resolution; the CSV's `media_unit` is `video`, and `media_output` holds the milliseconds made.

The **Request** and **Reply** tabs need the right **View prompts and outputs**. They tell you when that right is missing, when content wasn't logged, or when it has passed its retention. Kaeso never keeps embedding vectors. A reply logged in a format this console can't read yet, for example by a newer gateway, is shown as it was stored.

## Media in requests and replies

By default, request logs keep no media. An image, an audio clip, a video or a file inside a request or a reply is logged as its media type, its size and, for an image, its dimensions. The tabs show it as a line such as **Image (PNG, 1,024 × 768, 1.2 MB), not kept**. Media in a tool call's arguments or answer is logged the same way. Inside longer text, such as Markdown or a tool's answer, a `data:` URL gives way to a note like `[kaeso_media: image/png, 1258291 bytes, not_kept]`. A request body that isn't JSON, or is nested more than 128 levels deep, is logged as one file. Base64 counts as a file even when it is wrapped into lines or not quite valid.

Turn on **Keep media** to keep these files with the content: under **Organization settings** › **Logging** for the organization, and in a project's **Settings** for the project. Both must be on, and content logging too. Kaeso then keeps what apps send inline or upload, and the images and speech that models return.

A request keeps at most 10 MiB of media, counted in the order the files appear across the request and its reply. A file that no longer fits shows as **too large to keep**. A streamed image or speech reply is logged the same way as one that arrives whole. Kaeso never keeps the video a video model makes, which the app fetches in a later request, or the audio of a realtime session.

The **Request** and **Reply** tabs show a kept image as a thumbnail that opens larger, play kept audio and video, and offer every file as a download. The console draws or plays only a file whose bytes are a PNG, JPEG, GIF, WebP or AVIF image, or MP3, WAV, Ogg, FLAC, MP4 or WebM audio or video. Anything else, such as an SVG, an HTML page or a PDF, is a download only. Media on the web is shown as a link, and the console never fetches it.

With content logging on, Kaeso logs a transcript's text and the reply of an image or speech request, with its media kept or left out as above.

Content logged before Kaeso left media out keeps the media it was stored with until its content retention ends. To remove it sooner, shorten the **Content** retention or [delete the logs](#delete-logs) of those days.

Kept media counts toward the space under **Storage**, is deleted with the content when its retention ends, and is part of an export with content.

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

On **Tools**, **Export** downloads the tool calls that match the filters there, in the same three formats; with content, each call carries its stored arguments and result. The file is named `kaeso-tool-calls-…`.

In CSV, Kaeso puts every text value in double quotes. It adds a `'` in front of a value that starts with `=`, `+`, `-`, `@`, a tab or a carriage return. It does the same for a value that holds `;`, `|` or a tab, and after such a separator when one of those characters follows. A spreadsheet then shows the value as text instead of running it as a formula, whichever separator it splits on.

## Retention

Each organization chooses what it keeps, and for how long, under **Organization settings** › **Logging**:

| Setting | Default | What it does |
| --- | --- | --- |
| Log request content | On | Whether request and reply bodies are kept with their log entries |
| Keep media | Off | Whether logged content keeps its images, audio and files, up to 10 MiB a request |
| Metadata retention | 90 days | How long log entries are kept |
| Content retention | 30 days | How long request and reply bodies are kept. Never longer than the metadata |

Shortening a retention deletes older entries at once, after you confirm it, and lengthening it applies to new ones. The install sets the defaults for new organizations and the longest retention an organization may choose ([instance settings](instance-administration.md#change-install-settings)). Changing these settings, or content logging or kept media for the organization, needs the right **Manage log settings** and a recent sign-in.

A project can turn content logging off for itself in its **Settings**, with the right **Switch content logging**. It can't turn it on while the organization has it off. The same right switches **Keep media** for the project, which can't be on while the organization keeps none.

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
