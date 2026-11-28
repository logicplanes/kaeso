---
title: Gehostete Websuche über Formate hinweg
nav: Websuche
description: Wie die Codecs gehostete Websuche-Tools zwischen OpenAI Responses, SpaceX und Anthropic Messages übertragen.
order: 3
---

OpenAI Responses nennt sein gehostetes Suchtool `web_search`, früher auch `web_search_2025_08_26`, `web_search_preview` und `web_search_preview_2025_03_11`; Kaeso behandelt alle vier gleich. Für Anthropic wird das Tool zu Anthropics Server-Tool `web_search_20250305` mit dem Namen `web_search`:

| Responses | Anthropic |
| --- | --- |
| `filters.allowed_domains`, `filters.excluded_domains` (eines davon; eine leere Liste zählt als keine) | `allowed_domains`, `blocked_domains` |
| `enable_image_understanding`, `enable_image_search` (von xAI) | weggelassen, bei `true` in `kaeso-left-out` genannt |
| `user_location` (`approximate`, mit `city`, `region`, `country`, `timezone`) | `user_location` |
| `external_web_access: true`, `search_context_size: "medium"` (der Standard), `search_content_types: ["text"]` | nichts; sie beschreiben, was Anthropics Suche tut |
| `search_content_types` mit `image` | weggelassen und in `kaeso-left-out` genannt; Anthropics Suche liefert nur Textergebnisse |
| `external_web_access: false` | das Tool wird [weggelassen](cross-format.md#weggelassene-tools) |
| eine andere `search_context_size`, jedes andere Feld | `Unsupported` |

Für die Responses-API von SpaceX (`WebSearchTool::Xai`) behält das Tool die Felder, die xAI dafür dokumentiert ([Websuche](https://docs.x.ai/developers/tools/web-search), gelesen am 2026-10-25):

| Responses | SpaceX |
| --- | --- |
| `filters.allowed_domains`, `filters.excluded_domains` (je höchstens fünf, nicht beide) | wie gesendet |
| `enable_image_understanding`, `enable_image_search` | wie gesendet |
| `search_content_types` mit `image` | `enable_image_search: true` |
| `external_web_access: true`, `search_context_size: "medium"`, `search_content_types: ["text"]` | nichts; SpaceX lehnt `external_web_access` ab und sucht immer live |
| `external_web_access: false` | das Tool wird [weggelassen](cross-format.md#weggelassene-tools) |
| mehr als fünf Domains, beide Listen, `user_location`, eine andere `search_context_size`, jedes andere Feld | `Unsupported` |

`external_web_access: false` bittet OpenAI, nur seinen Cache von Seiten zu lesen. Das sendet Codex bei seinem Standard `web_search = "cached"`. Claude und SpaceX suchen immer live. Eine Live-Suche würde Seiten lesen, die Kaeso laut der App nicht abrufen soll, daher wird die Suche stattdessen weggelassen. Chat Completions hat keine gehostete Suche, daher wird das Tool auch dort weggelassen. OpenAI bekommt das Tool wie gesendet.

Anthropics Websuche-Tool geht den umgekehrten Weg: `web_search_20250305`, das `WebSearch` in Claude Code sendet, `web_search_20260209` und `web_search_20260318`. Ein Responses-Ziel bekommt das gehostete `web_search`-Tool, und die Grenze für Suchen wird zu einem Feld der Anfrage ([Websuche-Tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/web-search-tool)):

| Anthropic | OpenAI | SpaceX |
| --- | --- | --- |
| `allowed_domains` | `filters.allowed_domains` | `filters.allowed_domains` |
| `blocked_domains` | `filters.blocked_domains` | `filters.excluded_domains` |
| `user_location` (`approximate`, mit `city`, `region`, `country`, `timezone`) | `user_location` | `Unsupported` |
| `max_uses` | `max_tool_calls` | `max_turns` |
| `allowed_callers`, das `["direct"]` oder der Standard der Version ist | nichts; die gehostete Suche ist eine direkte Suche | nichts |
| `response_inclusion` (`web_search_20260318`) | nichts; es betrifft nur Suchen aus der Codeausführung | nichts |
| `cache_control` | nichts; Responses cacht selbst ([Cache-Hinweise](cross-format.md#cache-hinweise)) | nichts |
| ein Standort, der nicht `approximate` ist, jedes andere Feld | `Unsupported` | `Unsupported` |

- **Beide Domainlisten** in einem Tool sind `InvalidRequest`, mit Claudes eigener Meldung (`anthropic_messages/errors/web-search-both-domain-lists.json`).
- **Dynamisches Filtern.** Ab `web_search_20260209` filtert Claude Ergebnisse mit ausgeführtem Code, außer `allowed_callers` ist `["direct"]`. Ein Responses-Modell liest die Ergebnisse in beiden Fällen direkt.
- **Quellen.** Die Anfrage fordert `include: ["web_search_call.action.sources"]` an, weil OpenAI die gefundenen Seiten einer Suche nur auf Anfrage auflistet.
- **Die Grenze.** OpenAIs `max_tool_calls` zählt auch das Öffnen von Seiten und das Suchen in ihnen, ist also enger als `max_uses`. Die Route eines ChatGPT-Abos lehnt `max_tool_calls` ab, daher fällt die Grenze dort weg ([Nutzung eines ChatGPT-Abos](openai-responses.md#nutzung-eines-chatgpt-abos)).
- **SpaceX** ignoriert `max_tool_calls` und verwirft `blocked_domains` ohne Fehler, daher bekommt es eigene Namen. Sein `max_turns` begrenzt die Runden von Tool-Aufrufen, und eine Runde kann mehr als einmal suchen. Seine übrigen Grenzen gelten auch: höchstens fünf Domains in einer Liste.
- **Die Suche erzwingen.** Ein `tool_choice`, das `web_search` erzwingt, ist `Unsupported`; mit `auto` und `any` entscheidet das Modell.
- **Chat Completions** hat keine gehostete Suche, daher wird das Tool dort weggelassen.

Wie Suchen eine App in Antworten und Streams erreichen und was mit Suchen geschieht, die eine App zurückschickt, steht unter [weggelassene Tools](cross-format.md#weggelassene-tools).
