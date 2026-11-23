---
title: Sources
description: Add a source to a project, connect its client and follow the requests and tool calls it reports.
order: 4
---

A source lets a client that calls its providers directly, such as Claude Code, Codex or a LiteLLM proxy, report those requests to a project. They then show in the project's request logs, usage, costs and limits, marked as observed ([sources](../concepts/sources.md) explains how Kaeso counts them). You need `sources.view` to see **Sources** and `sources.manage` to add and change sources.

## Add a source

1. Open the project's **Sources** and choose **Add source**.
2. Choose what reports to Kaeso, such as **Claude Code**, and choose **Continue**.
3. Name the source, such as `Claude Code, Ada's laptop`, and choose whom its requests count for: you or one of the project's service accounts.
4. Keep **Match members by email** on to count an event that names a member's email address for that member. Turn on **Keep prompts and replies** to keep the content the client sends.
5. Choose **Add source**.

Kaeso shows the source's key once, with the exact lines that connect its client: environment variables, a settings file, a hook or a gateway's config. Copy the key now, because Kaeso keeps only a hash of it. Put the lines where they go and use the client once. The dialog waits for the first event, then shows **Connected** and what arrived, or why Kaeso refused it.

## Follow a source

**Sources** lists each source with its client, when it last reported, any refusals in the last 14 days, its events a day and its status:

| Status | Meaning |
| --- | --- |
| **Waiting** | No event yet |
| **Connected** | It reported within the last day |
| **Idle** | It reported before that |
| **Disabled** | Kaeso refuses what it sends until it's turned on again |
| **Archived** | Its key no longer works; its requests stay |

Open a source to see its activity by day, with the duplicates Kaeso left out and what it refused. Its sheet also shows the reason for the last refusal and its setup lines, with a placeholder for the key. **See requests** opens the request log with only its requests.

In the request log, an observed request has the **Observed** tag and names its source where a key would be. The **Reported by** filter shows one source's requests, and the **Source** filter all observed ones. **Analytics** breaks usage down **By source**, the gateway's own requests apart from each source's, and the tool-call log shows the MCP calls a source reported.

## Change a source

In a source's sheet you can:

- Rename it, or change whom its requests count for.
- Turn **Match members by email** or **Keep prompts and replies** on or off.
- Turn off **Accept its events** to have Kaeso refuse what it sends, for example while you move a client elsewhere.
- Choose **Give it a new key**. The old key stops at once, so put the new one in the client's setup.
- Choose **Archive source**. Archiving is permanent: the key stops working and the source's requests stay in the logs.

## Next steps

- [Claude Code](claude-code.md): Count Claude Code's requests and MCP tool calls.
- [Codex](codex.md): Count the requests and MCP tool calls of the Codex CLI.
- [Gemini CLI](gemini-cli.md): Count Gemini CLI's requests and MCP tool calls.
- [Antigravity](antigravity.md): Count Antigravity's model calls with a hook script.
- [LiteLLM](litellm.md): Count the requests a LiteLLM proxy serves.
- [OpenRouter](openrouter.md): Count your OpenRouter requests with Broadcast.
- [OpenTelemetry](opentelemetry.md): Count the requests of any app that exports GenAI telemetry.
- [HTTP](http.md): Send requests and tool calls from your own scripts.
