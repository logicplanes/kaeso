---
title: Formatwechsel
description: Die Regeln, denen die Codecs folgen, wenn eine Anfrage oder Antwort das API-Format wechselt, und ihre Gründe.
order: 2
---

Eine App kann ein API-Format sprechen, während der Anbieter ein anderes spricht. Für diesen Fall gelten die folgenden Regeln der Codecs, jeweils mit ihrem Grund. Eine Anfrage, die in ihrem eigenen Format bleibt, wird exakt reproduziert, und keine dieser Regeln greift.

| Daten aus einem anderen Format | In einer Anfrage | In einer Antwort |
| --- | --- | --- |
| Extras-Felder | `Unsupported`, außer sie stehen auf der [Allowlist](#extras) | Weggelassen |
| Unbekannte Blöcke, Tools und Tool-Auswahlen | `Unsupported`, außer [Tool-Namespaces](#tool-namespaces), [gehosteter Websuche](#gehostete-websuche) und dem [Tool-Verlauf von Codex](#tool-verlauf-von-codex) | Unbekannte Blöcke weggelassen, außer Websuchen |
| Zitate | `Unsupported` | Weggelassen |
| Datei-IDs eines Anbieters | `Unsupported`, außer der Zielanbieter hat sie ausgegeben | — |
| Lesbares Reasoning | Weggelassen; der [Reasoning-Blob](reasoning.md#reasoning-blobs) stellt es beim Ursprung wieder her | In der Reasoning-Form der App gezeigt, wo sie eine hat |
| Opakes Reasoning | Nur an den Dialekt und Anbieter zurückgespielt, die es ausgegeben haben | In einen Reasoning-Blob verpackt |
| Cache-Hinweise | Weggelassen, wo das Ziel selbst cacht | — |
| Sampling-Einstellungen, die das Ziel nicht hat | `Unsupported` | — |
| Stoppgründe, die das Format der App nicht hat | — | Der nächstliegende, den es hat |

## Anfragen

`cross::check_foreign` durchläuft eine Anfrage und lehnt mit `Unsupported` ab, was das Ziel nicht verwenden kann:

- **Extras-Felder aus einem anderen Dialekt**, außer das Feld steht auf der [Allowlist](#extras) oder darf aufgrund seines Werts das Format wechseln (`cross::is_droppable_value`). Extras, die Felder, aber keinen Ursprung haben, werden stattdessen mit `InvalidRequest` abgelehnt, weil niemand sie zurückschreiben könnte.
- **Unbekannte Blöcke, Tools und Tool-Auswahlen aus einem anderen Dialekt.** Ein unbekannter Block ohne `type`, aber mit `role` enthält eine ganze Nachricht, etwa eine `function`-Nachricht von OpenAI Chat. Die Ablehnung nennt ihn `` `<role>` message `` und fügt den Hinweis „Remove the message, or send the request to a model served through its original format.“ hinzu.
- **Datei-IDs eines Anbieters**, die der Zielanbieter nicht auflösen kann ([Dateien](#dateien)).
- **Zitate aus einem anderen Dialekt:** Kein Ziel dokumentiert, was Zitate in früheren Runden bewirken, und die Formate lassen sich nicht aufeinander abbilden.

Nicht abgelehnt wird, was die Encoder stattdessen weglassen: Reasoning-Blöcke, die das Ziel nicht zurückspielen kann ([Reasoning](reasoning.md)), und Cache-Hinweise, wo das Ziel selbst cacht ([Cache-Hinweise](#cache-hinweise)).

`check()` lehnt auch ab, was ein Ziel überhaupt nicht ausdrücken kann. Anthropic braucht `max_tokens`, daher ist eine Generierungsanfrage ohne `sampling.max_output_tokens` dort `InvalidRequest` und nennt das Feld der App für das Limit, statt einen erfundenen Wert zu bekommen.

Eine Ablehnung nennt das Feld, das die App gesendet hat, nie das des Ziels. Sie nennt es in `param` und beim Ausgabelimit auch in der Meldung. `cross::app_field` liest den Ursprung der Anfrage und bei OpenAI Chat, welches von `max_completion_tokens` und `max_tokens` sie enthielt; hat die App beide gesendet, wählt es `max_completion_tokens`. Die Namen folgen dem Format der App:

| Abgelehnte Einstellung | Aus Anthropic | Aus Chat | Aus Responses |
| --- | --- | --- | --- |
| Ein Ausgabelimit unter 16, für OpenAI Responses | `max_tokens` | `max_completion_tokens` oder `max_tokens` | |
| Ein fehlendes Ausgabelimit, für Anthropic | | `max_completion_tokens` | `max_output_tokens` |
| Stoppsequenzen | `stop_sequences` | `stop` | |
| Das Ausgabeformat | `output_config.format` | `response_format` | `text.format` |
| Der Reasoning-Aufwand | `output_config.effort` | `reasoning_effort` | `reasoning.effort` |
| Andere Reasoning-Einstellungen | `thinking` | `reasoning_effort` | `reasoning` |

Eine im Code gebaute Anfrage ohne Ursprung bekommt die Namen des Ziels. Inhalte werden nach ihrer Stelle in KPP benannt (`messages[i].blocks[j]`, `system[i]`, `tools[i]`).

Das veraltete Function Calling von OpenAI Chat übersteht den Round Trip innerhalb von Chat und wechselt in keinen anderen Dialekt. Legacy-Aufrufe haben keine Aufruf-IDs, daher sind sie keine KPP-Tool-Aufrufe:

- `functions` und `function_call` auf oberster Ebene sind Extras der Anfrage. Eine Ablehnung lautet etwa: `` `functions` comes from an OpenAI Chat Completions request and has no equivalent in Anthropic Messages. Remove it, or send the request to a model served through its original format. ``;
- das `function_call` einer Assistentennachricht ist ein Extra der Nachricht und wird als `messages[i].function_call` abgelehnt;
- eine `function`-Nachricht wird zu einer Benutzernachricht mit einem unbekannten Block und als `` `function` message `` abgelehnt.

## Antworten

Antworten fließen von einem Anbieter zu einer App, und eine fertige, bereits bezahlte Antwort abzulehnen wäre schlimmer, als weniger davon zu zeigen. Daten eines anderen Dialekts werden daher in dieser Rangfolge behandelt (`cross::reply_policy`, `cross::reasoning_reply_policy`):

1. **Abgebildet**, wo der Dialekt der App sie darstellen kann. Reasoning eines anderen Dialekts wird zu einem Reasoning-Item von OpenAI Responses oder einem Anthropic-`thinking`-Block. Der Originalblock wird in einen [Reasoning-Blob](reasoning.md#reasoning-blobs) verpackt, den die App unverändert zurücksendet.
2. **Weggelassen**, wo er das nicht kann. In Streams treten [Keepalive](#keepalives)-Frames an ihre Stelle, damit die Verbindung nie verstummt. Das betrifft Reasoning für Apps von OpenAI Chat (das Gateway [bewahrt es](reasoning.md#vom-gateway-aufbewahrtes-reasoning) zwischen den Runden auf), unbekannte Blöcke und Stream-Extras aus einem anderen Dialekt, fremde Zitate und Extras.

Das Gateway protokolliert die KPP-Antwort vollständig, daher geht in Kaesos Aufzeichnungen nichts verloren. Die Anfrageseite lehnt fremde Server-Tools, Zitate und ähnliche Features ab. Unbekannte Blöcke, Zitate und Extras erreichen einen anderen Dialekt daher nur, wenn der Anbieter etwas sendet, worum die Anfrage nicht gebeten hat.

Stoppgründe, die das Format der App nicht kennt, werden auf den nächstliegenden abgebildet, den es kennt; die Mapping-Seite jedes Dialekts listet sie auf.

## Extras

Extras aus Dialekt A werden nur in Dialekt A zurückgeschrieben. `check()` eines anderen Dialekts lehnt sie mit `Unsupported` ab und nennt das Feld. Ein Feld auf der Allowlist unten wird stattdessen weggelassen. Die Allowlist enthält nur Felder, die bekanntermaßen nicht ändern, was das Modell schreibt, etwa Beta-Header und Hinweise zur Auflösung.

Ein Eintrag ist nur erlaubt, wenn bekannt ist, dass das Feld keinen Einfluss auf die Ausgabe des Modells hat. Jeder Eintrag braucht eine Begründung in einer Zeile und die Fixture, die zeigt, dass ein echter Client das Feld sendet.

Einträge haben einen Geltungsbereich (`cross::EXTRAS_ALLOWLIST`): Ein Schlüssel ist nur dort erlaubt, wo er steht. Die Geltungsbereiche sind `request`, `message`, `tool`, `output`, `reasoning` und die Blockarten `text`, `image`, `document`, `tool_call`, `tool_result` und `refusal`. Header sind Schlüssel `header:<name>` im Bereich `request`.

Die Fixture-Pfade in diesen Tabellen sind relativ zu `crates/kaeso-codecs/fixtures/`.

### Allowlist

| Schlüssel | Bereich und Ursprung | Begründung | Fixture / Client |
| --- | --- | --- | --- |
| `header:anthropic-beta` | request, aus `anthropic_messages` | Beta-Header schalten nur Funktionen des Anbieters frei. Eine Funktion, die eine Anfrage wirklich nutzt, erscheint als Feld oder Block im Body und wird dort für sich geprüft. | Claude Code sendet bei jeder Anfrage `claude-code-20250219,interleaved-thinking-…` und weitere Betas; `anthropic_messages/requests/claude-code-mid-conversation-system.headers.json`, `anthropic_messages/requests/thinking-interleaved-tools.headers.json` |
| `header:anthropic-version` | request, aus `anthropic_messages` | Versionierung des Anthropic-Wire-Formats; anderswo bedeutungslos. Ein Wert gleich der festgelegten Version wird gar nicht gespeichert ([die Anthropic-Version](#die-anthropic-version)). | Anthropic-SDKs |
| `header:openai-beta` | request, aus `openai_chat`, `openai_responses`, `openai_embeddings` | Wie `anthropic-beta`. | Codex sendet `OpenAI-Beta: responses=experimental` |
| `cache_control` | request, aus `anthropic_messages` | Automatisches Prompt-Caching auf oberster Ebene. Cache-Hinweise sind Hinweise ([Cache-Hinweise](#cache-hinweise)). | `anthropic_messages/requests/system-string-multi-turn.json`, `explicit-nulls.json` |
| `display` (`thinking.display`) | reasoning, aus `anthropic_messages` | Steuert nur, ob Thinking-Text zurückkommt (`summarized` oder `omitted`). OpenAI Responses bekommt stattdessen `reasoning.summary: "auto"`, außer der Wert ist `omitted` ([lesbares Reasoning und Zusammenfassungen](reasoning.md#lesbares-reasoning-und-zusammenfassungen)). | `anthropic_messages/requests/thinking-display-omitted.json`, `thinking-adaptive-effort.json` |
| `image_url.detail` | image, aus `openai_chat` | Hinweis zur Bildauflösung (`low`, `high`, `auto`): Er ändert die Token-Kosten des Bildes, nicht die Frage; Ziele ohne ihn wählen ihre Auflösung selbst. | `openai_chat/requests/image-base64-detail.json` |
| `file.filename` | document, aus `openai_chat` | Der Dateiname, den OpenAI neben eingebettetem `file_data` verlangt. Metadaten, keine Anweisung. | `openai_chat/requests/file-pdf-data.json` |
| `prompt_cache_key` | request, aus `openai_chat` | Leitet Anfragen zu einem Prompt-Cache. Ein Cache-Hinweis ([Cache-Hinweise](#cache-hinweise)). | `openai_chat/requests/all-params-sdk.json` |
| `prompt_cache_key` | request, aus `openai_responses` | Wie oben. | `openai_responses/requests/sampling-and-ids.json` |
| `client_metadata` | request, aus `openai_responses` | Metadaten der Anfrage: Turn- und Thread-IDs, W3C-Trace-Kontext und `x-codex-turn-metadata`. Sie erreichen das Modell nie, und ein protokollierter Anfrage-Body behält sie. | Codex sendet sie bei jedem Zug; `openai_responses/requests/codex-namespace-tools.json` |
| `detail` (an `input_image`; `auto` ist bereits eine Form-Markierung) | image, aus `openai_responses` | Hinweis zur Bildauflösung, wie `image_url.detail` bei Chat. `input_file.detail` (PDF-Darstellung) steht **nicht** auf der Allowlist. | `openai_responses/requests/image-base64-file-id.json` |
| `phase` (an Assistentennachrichten: `commentary`, `final_answer`) | message, aus `openai_responses` | Kennzeichnet eine Assistentennachricht als Vorrede oder als endgültige Antwort, damit OpenAI sie beim erneuten Abspielen des Zugs unterscheiden kann. Der Text ist in beiden Fällen derselbe, und andere Formate haben keine solche Kennzeichnung. | Codex sendet die Nachrichten von GPT-6 damit zurück; ein Thread, der zu einem anderen Modell wechselt, trägt sie mit; `openai_responses/requests/codex-model-switch.json` |

### Bedingte Einträge

Diese Felder wechseln das Format nur mit den Werten unten (`cross::is_droppable_value` und die Regel pro Nachricht in `cross::check_foreign`); jeder andere Wert bleibt `Unsupported`.

| Schlüssel | Bereich und Ursprung | Bedingung und Begründung | Fixture / Client |
| --- | --- | --- | --- |
| `context_management` | request, aus `anthropic_messages` | Enthält nur `edits` mit mindestens einem Edit, und jeder Edit ist `clear_thinking_*`. Das Löschen von Thinking-Blöcken entscheidet nur, welche früheren Anthropic-`thinking`-Blöcke das Modell sieht, und diese erreichen nie einen anderen Dialekt ([Reasoning](reasoning.md)). | Claude Code 2.1.296 sendet bei jeder Anfrage `{"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]}`; `anthropic_messages/requests/claude-code-mid-conversation-system.json` |
| `text.verbosity` | request, aus `openai_responses` | `low` oder `high` (`medium` ist eine Form-Markierung). Ein Hinweis zur Ausgabelänge ([Feldklassen](index.md#feldklassen)): Ein Chat-Completions-Ziel, dessen Profil `verbosity` annimmt, bekommt ihn dort ([Anbieterprofile](openai-chat-providers.md#anbieterprofile)); Anthropic und andere Chat-Completions-Ziele haben keine Entsprechung, daher wird er weggelassen und in `kaeso-left-out` genannt. | Codex sendet `low` für GPT-Modelle und behält es, wenn ein Thread das Modell wechselt; `openai_responses/requests/multi-turn-roles.json` |
| `output_config` | message (`role: "system"`), aus `anthropic_messages` | `{}` oder nur `effort`, gleich dem `output_config.effort` der Anfrage. Den Aufwand der Anfrage zu wiederholen ändert nichts. | Claude Code 2.1.296 wiederholt den Aufwand der Anfrage in seiner Systemnachricht mitten in der Unterhaltung; dieselbe Fixture |

## Tool-Namespaces

Ein `namespace`-Tool von OpenAI Responses fasst Funktions-Tools unter einem Namen zusammen. Codex gruppiert so seine eigenen Tools und die Tools jedes MCP-Servers und sendet einen Aufruf eines solchen Tools als `function_call` mit `namespace`. OpenAI bekommt Namespaces unverändert. Jedes andere Ziel bekommt einfache Funktions-Tools: Anthropic, Chat Completions und die Responses-API von SpaceX, die Namespaces mit 422 ablehnt.

- **Tools.** Jedes Tool eines Namespace wird zu einem Funktions-Tool namens `<namespace>__<name>`, etwa `mcp__docs__search`. Die Beschreibung des Namespace steht vor der eigenen Beschreibung des Tools, getrennt durch eine Leerzeile.
- **Aufrufe im Gespräch.** Ein `function_call` mit `namespace` bekommt denselben Namen, und sein `namespace` wird entfernt.
- **Antworten.** Eine Responses-Antwort oder ein Stream übersetzt einen Aufruf, der wie ein umbenanntes Tool heißt, zurück in `name` und `namespace`. Die Namen stammen aus der Anfrage der App (`EncodeCtx::request`), daher behält ein Aufruf ohne Namespace in der App seinen Namen.
- **Namensgrenzen.** Verletzt ein umbenanntes Tool die Regeln des Ziels für Tool-Namen, wird es danach wie jeder andere Tool-Name umgeschrieben und zurückübersetzt.
- **Abgelehnt.** Ein Namespace mit anderen Schlüsseln als `type`, `name`, `description` und `tools`, ein Tool darin, das keine Funktion ist, ein Feld an einem solchen Tool, das nur Responses kennt, etwa `defer_loading`, und ein umbenannter Name, den schon ein anderes Tool trägt.

`ResponsesProfile::tool_namespaces` sagt, ob ein Responses-Ziel Namespaces annimmt; nur `ResponsesProfile::OPENAI` tut das. `EgressCtx::with_tool_namespaces` überschreibt es für eine Anfrage.

## Gehostete Websuche

OpenAI Responses nennt sein gehostetes Suchtool `web_search`, früher auch `web_search_2025_08_26`, `web_search_preview` und `web_search_preview_2025_03_11`; Kaeso behandelt alle vier gleich. Für Anthropic wird das Tool zu Anthropics Server-Tool `web_search_20250305` mit dem Namen `web_search`:

| Responses | Anthropic |
| --- | --- |
| `filters.allowed_domains`, `filters.excluded_domains` (eines davon; eine leere Liste zählt als keine) | `allowed_domains`, `blocked_domains` |
| `enable_image_understanding`, `enable_image_search` (von xAI) | weggelassen, bei `true` in `kaeso-left-out` genannt |
| `user_location` (`approximate`, mit `city`, `region`, `country`, `timezone`) | `user_location` |
| `external_web_access: true`, `search_context_size: "medium"` (der Standard), `search_content_types: ["text"]` | nichts; sie beschreiben, was Anthropics Suche tut |
| `search_content_types` mit `image` | weggelassen und in `kaeso-left-out` genannt; Anthropics Suche liefert nur Textergebnisse |
| `external_web_access: false` | das Tool wird [weggelassen](#weggelassene-tools) |
| eine andere `search_context_size`, jedes andere Feld | `Unsupported` |

Für die Responses-API von SpaceX (`WebSearchTool::Xai`) behält das Tool die Felder, die xAI dafür dokumentiert ([Websuche](https://docs.x.ai/developers/tools/web-search), gelesen am 2026-10-25):

| Responses | SpaceX |
| --- | --- |
| `filters.allowed_domains`, `filters.excluded_domains` (je höchstens fünf, nicht beide) | wie gesendet |
| `enable_image_understanding`, `enable_image_search` | wie gesendet |
| `search_content_types` mit `image` | `enable_image_search: true` |
| `external_web_access: true`, `search_context_size: "medium"`, `search_content_types: ["text"]` | nichts; SpaceX lehnt `external_web_access` ab und sucht immer live |
| `external_web_access: false` | das Tool wird [weggelassen](#weggelassene-tools) |
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
| `cache_control` | nichts; Responses cacht selbst ([Cache-Hinweise](#cache-hinweise)) | nichts |
| ein Standort, der nicht `approximate` ist, jedes andere Feld | `Unsupported` | `Unsupported` |

- **Beide Domainlisten** in einem Tool sind `InvalidRequest`, mit Claudes eigener Meldung (`anthropic_messages/errors/web-search-both-domain-lists.json`).
- **Dynamisches Filtern.** Ab `web_search_20260209` filtert Claude Ergebnisse mit ausgeführtem Code, außer `allowed_callers` ist `["direct"]`. Ein Responses-Modell liest die Ergebnisse in beiden Fällen direkt.
- **Quellen.** Die Anfrage fordert `include: ["web_search_call.action.sources"]` an, weil OpenAI die gefundenen Seiten einer Suche nur auf Anfrage auflistet.
- **Die Grenze.** OpenAIs `max_tool_calls` zählt auch das Öffnen von Seiten und das Suchen in ihnen, ist also enger als `max_uses`. Die Route eines ChatGPT-Abos lehnt `max_tool_calls` ab, daher fällt die Grenze dort weg ([Nutzung eines ChatGPT-Abos](openai-responses.md#nutzung-eines-chatgpt-abos)).
- **SpaceX** ignoriert `max_tool_calls` und verwirft `blocked_domains` ohne Fehler, daher bekommt es eigene Namen. Sein `max_turns` begrenzt die Runden von Tool-Aufrufen, und eine Runde kann mehr als einmal suchen. Seine übrigen Grenzen gelten auch: höchstens fünf Domains in einer Liste.
- **Die Suche erzwingen.** Ein `tool_choice`, das `web_search` erzwingt, ist `Unsupported`; mit `auto` und `any` entscheidet das Modell.
- **Chat Completions** hat keine gehostete Suche, daher wird das Tool dort weggelassen.

## Weggelassene Tools

Diese Tools werden aus einer Anfrage weggelassen statt abgelehnt (`cross::TOOLS_LEFT_OUT`). Jedes bietet dem Modell nur etwas an, das es nutzen kann, und eine App wie Codex fügt es jeder Anfrage hinzu, ohne Schalter pro Modell: Eine Ablehnung würde jedes Modell ohne das Tool aus dieser App unbrauchbar machen. Das Modell antwortet, wie es ohne das Tool antworten würde. Jedes weggelassene Tool wird im Header `kaeso-left-out` der Antwort ([Antwort-Header](../../api/model-api.md#antwort-header)) und im Log des Gateways genannt, und ein protokollierter Anfrage-Body behält die Tools der App.

| Tool | Von | Nach | Wann |
| --- | --- | --- | --- |
| `web_search` | `openai_responses` | `openai_chat` | immer: Chat Completions hat keine gehostete Suche |
| `web_search` | `openai_responses` | `anthropic_messages` | `external_web_access: false`: Claude sucht immer live |
| `web_search` | `openai_responses` | `openai_responses` | `external_web_access: false`, für einen Anbieter, dessen Profil `WebSearchTool::Xai` hat: SpaceX sucht immer live |
| `web_search_20250305` | `anthropic_messages` | `openai_chat` | immer: Chat Completions hat keine gehostete Suche |
| `web_search_20260209` | `anthropic_messages` | `openai_chat` | immer: Chat Completions hat keine gehostete Suche |
| `web_search_20260318` | `anthropic_messages` | `openai_chat` | immer: Chat Completions hat keine gehostete Suche |

Eine `tool_choice`, die ein weggelassenes Tool verlangt, selbst oder in `allowed_tools`, ist `Unsupported`. Wird jedes Tool weggelassen, entfallen auch `tool_choice: "auto"` oder `"none"` und `parallel_tool_calls` und werden in `kaeso-left-out` genannt, weil Anbieter sie ohne Tools ablehnen. `tool_choice: "required"` oder ein benanntes Tool ist `Unsupported`, da kein Tool zum Aufrufen bliebe.

Claudes Suchen erreichen eine Responses-App als `web_search_call`-Items. Ein `server_tool_use`-Block namens `web_search` wird zu einem Item mit der ID `ws_kaeso_…` und `action: {type: search, query}`. Sein Status ist `completed`, sobald das passende `web_search_tool_result` eintrifft, `failed`, wenn dieses Ergebnis ein Fehler ist, und `in_progress`, wenn die Antwort vorher endet, wie bei `pause_turn`. Streams senden `response.output_item.added`, `response.web_search_call.in_progress`, `.searching`, `.completed` und `response.output_item.done`. Die Ergebnisse selbst und die Zitate der Antwort werden weggelassen, wie andere Daten, die das Format der App nicht zeigen kann.

`web_search_call`-Items, die eine App zurücksendet, werden für Anthropic und Chat Completions verworfen. Anthropic spielt eine Suche nur mit ihren verschlüsselten Ergebnissen erneut ab, die diese Items nicht tragen, und Chat Completions hat kein solches Item. Die Antwort, die sie nutzte, bleibt im Gespräch. Items mit Kaesos `ws_kaeso_`-IDs werden auch für jedes Responses-Ziel verworfen, weil kein Responses-Anbieter sie ausgegeben hat. Jedes verworfene Item wird in `kaeso-left-out` mit Typ und ID genannt, etwa `web_search_call ws_67c9…`.

Die Suchen eines Responses-Modells erreichen eine Anthropic-App wie die von Claude. Jedes `web_search_call`-Item, das gesucht hat und abgeschlossen oder fehlgeschlagen ist, wird zu einem `server_tool_use`-Block namens `web_search` mit der ID `srvtoolu_kaeso_…` und `input: {query}`, gefolgt von einem `web_search_tool_result`. Eine abgeschlossene Suche listet ihre Quellen als `web_search_result`-Einträge: `title` wiederholt die `url` und `encrypted_content` ist leer, weil Responses nur die Adresse meldet, und `page_age` ist `null`. Eine fehlgeschlagene Suche wird zum Fehler `unavailable`.

Streams senden eine Suche, sobald ihr Item fertig ist: den Start von `server_tool_use`, ein `input_json_delta` mit der Anfrage und das Ergebnis. Items, die beim Ende der Antwort noch laufen, Items, die eine Seite öffnen oder in ihr suchen (`open_page`, `find_in_page`), und die `url_citation`-Anmerkungen der Antwort werden weggelassen, wie andere Daten, die das Format der App nicht zeigen kann. `usage.server_tool_use.web_search_requests` zählt die abgeschlossenen Suchen, weil eine fehlgeschlagene Suche nicht berechnet wird.

`server_tool_use`-Blöcke namens `web_search` und ihre `web_search_tool_result`-Blöcke werden verworfen, wenn eine App sie an ein anderes Format sendet, zusammen mit den `web_search_result_location`-Zitaten, die auf sie verweisen, so wie `web_search_call`-Items für Anthropic. Die Antwort, die sie nutzte, bleibt. Claude bekommt seine eigenen Blöcke unverändert zurück; Blöcke mit Kaesos `srvtoolu_kaeso_`-IDs werden auch für Claude verworfen, weil Claude ein Ergebnis ohne seinen verschlüsselten Inhalt nicht erneut abspielen kann.

## Tool-Verlauf von Codex

Ein Codex-Thread, der auf einem GPT-Modell begann, enthält Items, die nur OpenAI Responses kennt. Läuft der Thread auf Anthropic oder Chat Completions weiter, werden sie zu gewöhnlichen Tool-Aufrufen und -Ergebnissen, sodass das Modell den Verlauf behält:

| Responses-Item | KPP |
| --- | --- |
| `custom_tool_call` (ein Freiform-Tool wie `apply_patch` von Codex) | `Block::ToolCall` mit dem Namen des Tools (`<namespace>__<name>` bei einem `namespace`), sein `input`-Text als Argument `input` |
| `custom_tool_call_output` | `Block::ToolResult`, gelesen wie ein `function_call_output` |
| `local_shell_call` | `Block::ToolCall` namens `local_shell`, sein `action`-Objekt als Argumente |
| `local_shell_call_output` | `Block::ToolResult` für den Aufruf, den seine `id` nennt; Codex sendet stattdessen ein `function_call_output` |

OpenAI und SpaceX bekommen die Items wie gesendet: SpaceX antwortet aus einem solchen Verlauf (`openai_responses/providers/xai/responses/recorded-codex-gpt-history.json`). Ein Item, das nicht die Form oben hat, bleibt ein unbekannter Block und wird wie einer abgelehnt.

## Nutzung

`Usage::input_tokens` schließt Cache-Lesevorgänge und Cache-Schreibvorgänge aus, die eigene Felder sind. Das entspricht den Preisspalten von models.dev, sodass sich die Kosten als Summe aus jeder Anzahl mal ihrem Preis ergeben. Das rohe Nutzungsobjekt des Anbieters bleibt immer erhalten.

## Systemnachrichten

Eine Systemnachricht nach dem Beginn einer Unterhaltung bleibt als `Role::System`-Nachricht erhalten.

- **An Anthropic, aus Anthropic Messages:** unverändert gesendet; die API beurteilt ihre Position und welche Modelle sie unterstützen. Claude Code sendet solche Nachrichten bei jeder Anfrage.
- **An Anthropic, aus anderen Dialekten:** Der Text wird Teil der nächsten Benutzerrunde, nach ihren Tool-Ergebnissen, oder der letzten Benutzerrunde, wenn keine folgt. Codex sendet solche Nachrichten, wenn ein Thread das Modell (`<model_switch>`) oder den Modus (`<collaboration_mode>`) wechselt. Anthropic nimmt eine Systemnachricht nur direkt nach einer Benutzerrunde und direkt vor einer Assistentenrunde an, und nicht bei jedem Modell ([Systemnachrichten mitten in der Unterhaltung](https://platform.claude.com/docs/en/build-with-claude/mid-conversation-system-messages)). OpenAI-Apps setzen sie dagegen an beliebige Stellen, meist vor die Runde, die sie einleiten. Die Felder der Nachricht durchlaufen dieselbe Prüfung wie die jeder anderen Nachricht. Führende, vor jeder Benutzer- oder Assistentennachricht, werden Teil des System-Prompts; die meisten machen schon die Decoder dazu. Vor einer letzten Assistentennachricht, die Anthropic als Prefill fortsetzt, wird der Text stattdessen Teil der letzten Benutzerrunde.
- **An OpenAI:** spätere `system`- oder `developer`-Nachrichten. Bei Chat Completions rückt eine solche Nachricht zwischen den Tool-Aufrufen einer Assistentennachricht und ihren Ergebnissen hinter die Ergebnisse, weil Mistral dort eine Systemnachricht ablehnt (`Unexpected role 'tool' after role 'system'`).

## Keepalives

Ein Anbieter kann minutenlang streamen, ohne etwas zu erzeugen, das das Format der App zeigen kann: Claudes Thinking für eine App von OpenAI Chat, `ping`-Events von Anthropic, unbekannte Blöcke eines anderen Dialekts oder ein Reasoning-Modell von OpenAI, das während des Reasonings nichts sendet. Proxys mit Leerlauf-Timeouts (nginx und AWS ALB nach 60 Sekunden, Cloudflare nach 100) würden die Verbindung trennen, und die Zeit bis zum ersten Byte entspräche der gesamten Denkzeit. Daher verstummt ein laufender Stream nie wegen der Übersetzung: Jeder Stream-Encoder hat einen Keepalive-Frame, der in seinem Dialekt gültig ist und von den offiziellen SDKs ignoriert wird.

| Dialekt | Keepalive-Frame | Warum die SDKs ihn ignorieren |
| --- | --- | --- |
| OpenAI Chat Completions und Responses | Der SSE-Kommentar `: keepalive` und eine Leerzeile | Zeilen, die mit einem Doppelpunkt beginnen, sind in der SSE-Spezifikation Kommentare, und die Parser der OpenAI-SDKs überspringen sie |
| Anthropic Messages | `event: ping` mit `data: {"type": "ping"}`, genau wie Anthropic es sendet | Die Anthropic-SDKs überspringen Ping-Events |

Frames gelangen auf zwei Wegen auf die Leitung:

- **Im Stream.** Ein Encoder hängt den Frame an das an, was `StreamEncoder::encode` zurückgibt, wenn ein Event aus einem anderen Dialekt keine Bytes erzeugt hat. Ein Taktgeber begrenzt das ohne Uhr. Ein Extra-Event eines anderen Dialekts wie `ping` löst immer einen Frame aus, weil der Anbieter es bereits taktet. Verborgener Text, etwa Thinking, das die App nicht sehen kann, oder gepufferte Tool-Argumente, löst einen Frame pro `kaeso_codecs::HIDDEN_TEXT_PER_KEEPALIVE` Bytes aus, also 1.024. Alles, was der Encoder schreibt, setzt die Zählung zurück. Events aus dem eigenen Dialekt der App lösen nie Frames aus, sodass eine Wiedergabe im selben Dialekt exakt bleibt.
- **Vom Gateway gesteuert.** `StreamEncoder::keepalive` liefert den Frame auf Abruf, für Stille beim Anbieter, die der Encoder nicht sehen kann. Das Gateway schreibt ihn, wenn es `KAESO_STREAMS_KEEPALIVE_INTERVAL` lang nichts an die App geschrieben hat, standardmäßig 15 Sekunden.

Der Encoder für OpenAI Chat sendet seinen Rollen-Chunk außerdem, sobald die Antwort beginnt, statt mit dem ersten Text. So hört die App vom Stream, während das Modell denkt.

## Strukturierte Ausgabe

Strukturierte Ausgabe an Anthropic nutzt die nativen Structured Outputs von Anthropic, nie eine Nachbildung über ein erzwungenes Tool.

- Das Ausgabeformat `json_object` hat bei Anthropic keine Entsprechung und ist `Unsupported`.
- Das Ausgabeformat von Anthropic ist nur ein Schema: Der `name` eines Schemas wird weggelassen, während eine `description` oder `strict: false` `Unsupported` ist.
- Schemas, die von Anthropic an OpenAI gehen, bekommen den Namen `response`, weil OpenAI einen verlangt.

## Sampling

Sampling-Einstellungen, die ein Ziel nicht einhalten kann, sind `Unsupported` und werden nie weggelassen oder umskaliert:

| Ziel | Abgelehnt |
| --- | --- |
| Anthropic | `seed`, Frequency- und Presence-Penalties und eine `temperature` außerhalb von 0 bis 1, dem Bereich bei Anthropic |
| OpenAI Chat Completions | `top_k` und mehr als vier Stoppsequenzen |
| OpenAI Responses | `top_k`, `seed`, die Penalties und Stoppsequenzen, die es bei Responses nicht gibt, und ein Ausgabelimit unter 16 Token, dem Minimum von `CreateResponse` |

Zahlen behalten ihre Schreibweise innerhalb eines Dialekts: `temperature: 1` kommt als `1` zurück und `1.0` als `1.0`, solange der Wert unverändert ist. Über Dialekte hinweg gilt die eigene Schreibweise des Ziels, daher erreicht `temperature: 1` aus Chat Anthropic als `1.0`.

KPP hat kein `n`: Eine Chat-Anfrage mit `n` über 1 wird schon beim Dekodieren abgelehnt, unabhängig vom Ziel. `logprobs`, `logit_bias` und `top_logprobs` von Chat sind Extras, die jeder andere Dialekt ablehnt ([Extras](#extras)).

## Zustandsbehaftete Responses-Funktionen

`previous_response_id`, `conversation`, `store: true` und `background: true` brauchen Zustand bei OpenAI, und der Codec lehnt sie mit `Unsupported` ab. Auflösen könnte sie nur das Gateway, vor KPP.

## IDs von Antworten

Eine Antwort behält die ID des Anbieters unverändert, in welchem Format die App auch spricht. Eine Claude-Antwort erreicht eine OpenAI-Chat-Completions- oder Responses-App mit ihrer Anthropic-ID (`msg_…`). Eine OpenAI-Antwort erreicht eine Anthropic-Messages-App mit der ID von OpenAI (`chatcmpl-…` oder `resp_…`). Streams tragen dieselbe ID.

Die Codecs erfinden nie eine ID im Muster der App. Die ID des Anbieters benennt die Antwort beim Anbieter, und Apps behandeln IDs von Antworten als opake Zeichenketten. Eine Ausnahme sind Ausgabe-Items von Responses, weil andere Formate sie nicht haben ([Speicherung und IDs bei Responses](#speicherung-und-ids-bei-responses)). Ebenso ein Responses-Stream ohne Start-Event, der die ID `resp_kaeso_<created_at>` erhält ([OpenAI Responses](openai-responses.md)).

## Speicherung und IDs bei Responses

- **`store: false`.** Anfragen aus anderen Dialekten, die für OpenAI Responses kodiert werden, tragen `"store": false`: Kaeso ist zustandslos, und ohne dieses Feld würde OpenAI jede übersetzte Unterhaltung speichern. Aus Responses dekodierte Anfragen behalten die Schreibweise der App, daher bedeutet ein fehlendes `store` weiterhin, dass OpenAI die Antwort speichert.
- **Item-IDs.** Ausgabe-Items von Responses brauchen IDs, die Antworten aus anderen Dialekten nicht haben. Kaeso gibt ihnen IDs, die mit `msg_kaeso_`, `fc_kaeso_` und `rs_kaeso_` beginnen. Sendet eine App sie zurück, entfernt der Anfrage-Decoder sie, sodass OpenAI nie eine ID erhält, die es nicht selbst vergeben hat.

## Cache-Hinweise

Cache-Hinweise sind Hinweise. OpenAI cacht selbst, daher werden Hinweise aus anderen Dialekten für OpenAI weggelassen. OpenAIs eigenes `prompt_cache_breakpoint` wird zu einem KPP-`CacheHint` dekodiert und nur in den Dialekt zurückgeschrieben, aus dem es kam.

## Tool-Fehler

`ToolResult.is_error` ist ein Hinweis für Ziele ohne Fehler-Flag, also OpenAI Chat und Responses: Es wird weggelassen, und der Inhalt des Ergebnisses trägt den Fehlertext. Würde es abgelehnt, ließe sich kein fehlgeschlagener Tool-Aufruf eines Clients im Anthropic-Format wie Claude Code mehr an OpenAI routen.

## Dateien

Eine Datei-ID eines Anbieters (`file-…`) ist nur im Dateispeicher dieses Anbieters nutzbar. Die beiden OpenAI-Dialekte für Generierung teilen sich einen solchen Speicher, daher nimmt `cross::check_foreign` den `Origin` des Ziels entgegen, also seinen Dialekt und Anbieter. Datei-IDs, die zu einem anderen Anbieter wechseln, sind `Unsupported`.

## Die Anthropic-Version

Der Anthropic-Codec legt eine API-Version fest, `2023-06-01`, exportiert als `kaeso_codecs::anthropic_messages::ANTHROPIC_VERSION`. Es ist die einzige Version, die die [Seite zur Versionierung](https://platform.claude.com/docs/en/api/versioning) von Anthropic auflistet, die Version, die die offiziellen SDKs senden, und die, von der jede Anthropic-Fixture ausgeht.

- **Egress.** Jede Anfrage, die der Anthropic-Codec kodiert, trägt `anthropic-version: 2023-06-01`, außer die eigene Anthropic-Anfrage der App hat ein `anthropic-version` weitergegeben, das eine neuere Version nennt: ein wohlgeformtes `YYYY-MM-DD`, das nach der festgelegten Version einsortiert wird. Ein fehlerhafter oder älterer Wert wird ignoriert. Eine Anfrage aus einem anderen Dialekt trägt den Header nie und bekommt daher immer die festgelegte Version.
- **Ingress.** Ein weitergegebenes `anthropic-version`, das der festgelegten Version entspricht, wird nicht in den Extras der Anfrage gespeichert. Der Codec sendet es ohnehin, und es aufzubewahren würde jede Anfrage eines Anthropic-SDKs an ihren Dialekt binden. Jeder andere Wert wird als Extra `header:anthropic-version` aufbewahrt und beim Egress wie oben verglichen.
- **Warum die neuere Version gewinnt.** Anthropic ergänzt eine Version nur, mit neuen optionalen Eingaben und neuen Ausgabewerten, und eine App, die eine neuere Version anfordert, verlässt sich auf deren Verhalten. Die Decoder tolerieren unbekannte Felder, Blöcke und Events, daher führt eine neuere Version zu Extras und unbekannten Items, statt zu scheitern.

Um die festgelegte Version zu ändern, ändern Sie die Konstante, zeichnen die Fixtures mit `cargo xtask record` neu auf ([Aufzeichnen](fixtures.md#live-antworten-aufzeichnen)) und aktualisieren diesen Abschnitt.
