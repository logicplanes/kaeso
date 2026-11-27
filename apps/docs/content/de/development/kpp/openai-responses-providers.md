---
title: Responses-Anbieter
description: Wie der Responses-Codec seine Anfragen an jeden Anbieter dieses Formats anpasst, und was jeder Anbieter ergänzt.
order: 7
nav: Responses-Anbieter
---

Andere Anbieter als OpenAI sprechen [OpenAI Responses](openai-responses.md) mit eigenen Abweichungen. Schlagen Sie hier nach, wie der Codec jede Anfrage an einen Anbieter anpasst und was jeder Anbieter ergänzt, bevor Sie einen Anbieter hinzufügen oder sein Profil ändern.

Andere Anbieter als OpenAI, die Responses bereitstellen, zeichnen in `fixtures/openai_responses/providers/<id>/` auf ([Fixtures](fixtures.md)). Ihre zusätzlichen Felder folgen den Regeln von [OpenAI Responses](openai-responses.md); diese Seite nennt, was jeder Anbieter hinzufügt.

Jeder Anbieter in `PROVIDERS` nennt in seinem Feld `responses` ein `kaeso_codecs::openai_responses::ResponsesProfile`, und das Gateway übergibt es mit `EgressCtx::with_responses_profile` dem Encoder. `ResponsesProfile::OPENAI`, der Standard, sendet Anfragen so, wie die App sie geschrieben hat. `ResponsesProfile::compatible(name)` unterscheidet sich in `tool_namespaces: false` ([Tool-Namespaces](cross-format.md#tool-namespaces)) und `stores_responses: false`, da nur von OpenAI bekannt ist, dass es Antworten für `previous_response_id` aufbewahrt; eigene Anbieter bekommen `compatible`, da nur OpenAI Namespaces dokumentiert.

| Einstellung | Was der Encoder tut |
|---|---|
| `xai_web_search()` | Sendet die gehostete Suche als `web_search`-Tool von xAI ([gehostete Websuche](cross-format.md#gehostete-websuche)). |
| `without_null_reasoning_content()` | Lässt ein ausdrückliches `content: null` eines Reasoning-Items weg, das die App zurücksendet. |
| `tool_schema_root(root)` | Führt eine Union an der Wurzel eines Funktions-Tools, die der Anbieter nicht annimmt, zu einem Objekt-Schema zusammen; `ToolSchemaRoot::ObjectUnions` behält eine Union eingebetteter Objekte ([Tool-Schemas mit Union-Wurzel](cross-format.md#tool-schemas-mit-union-wurzel)). |
| `without_openai_encrypted_reasoning()` | Lässt Reasoning-Items weg, die die App mit OpenAIs `encrypted_content` zurücksendet. Er ist ein Fernet-Token ([Fernet-Spezifikation](https://github.com/fernet/spec/blob/master/Spec.md)) und beginnt daher mit `gAAAAA`, wie in jeder Aufnahme von OpenAI in `fixtures/openai_responses/`. |

## SpaceX

Quellen: die [Responses-Referenz](https://docs.x.ai/developers/rest-api-reference/inference/responses) und der [Vergleich mit Chat Completions](https://docs.x.ai/developers/model-capabilities/text/comparison) von xAI, gelesen am 2026-10-19, und die Live-Aufzeichnungen in `fixtures/openai_responses/providers/xai/`. Responses ist der `generation`-Dialekt von SpaceX.

| SpaceX-Feld | KPP | Hinweise |
|---|---|---|
| `reasoning`-Items mit `summary` und `encrypted_content`, das `grok-4.3` immer sendet | wie bei OpenAI | Ein `encrypted_content`, das ein anderer Anbieter geschrieben hat, lehnt SpaceX mit einem 400 `invalid-argument` ab (`errors/recorded-reasoning-encrypted.json`, für den von OpenAI). Der Verlauf einer App sagt nicht, welcher Anbieter ein Item geschrieben hat, daher werden Items mit dem von OpenAI weggelassen (`without_openai_encrypted_reasoning()`), etwa wenn ein Codex-Thread von GPT zu Grok wechselt. |
| `web_search_call`-Items, `url_citation`-Annotationen | wie bei OpenAI | |
| `namespace`-Tools (Anfrage) | zu Funktions-Tools umbenannt | SpaceX lehnt sie mit 422 ab (`unknown variant namespace`); sein Profil ist `compatible` ([Tool-Namespaces](cross-format.md#tool-namespaces)). |
| `web_search`-Tool (Anfrage) | das Tool von xAI (`xai_web_search()`) | SpaceX lehnt `external_web_access` ab (`Argument not supported: external_web_access`) und sucht immer live ([gehostete Websuche](cross-format.md#gehostete-websuche)). |
| Tool-Parameter mit einer Union an der Wurzel, deren Zweige nicht alle eingebettete Objekte sind, oder mit `allOf` an der Wurzel (Anfrage) | ein Objekt-Schema (`tool_schema_root(ToolSchemaRoot::ObjectUnions)`) | SpaceX lehnt sie mit `invalid_client_tool_schema` ab ([Tool-Schemas mit Union-Wurzel](cross-format.md#tool-schemas-mit-union-wurzel)). |
| `content: null` an einem zurückgesendeten `reasoning`-Item (Anfrage) | weggelassen (`without_null_reasoning_content()`) | Codex sendet seine früheren Reasoning-Items so zurück. Mit dem `null` kann SpaceX den `encrypted_content` des Items nicht lesen (`Could not decode the compaction blob`). |
| `usage.num_sources_used`, `num_server_side_tools_used`, `server_side_tool_usage_details`, `cost_in_usd_ticks`, `context_details` | `raw` der Nutzung | Responses-Apps erhalten sie zurück; die Token-Zahlen sind wie bei OpenAI typisiert. |
| Typen von Input-Items (Anfrage) | nur `XAI_INPUT_ITEMS` (`xai_input_items()`) | SpaceX lehnt jeden anderen Typ mit einem 422 ab, der die angenommenen auflistet (`errors/recorded-codex-agent-message.json`), und nimmt auch `custom_tool_call_output` an, das diese Liste auslässt (`responses/recorded-codex-gpt-history.json`). Codex' `agent_message` wird zu einer Nachricht, `local_shell_call` und `local_shell_call_output` zu einem Funktionsaufruf und seiner Ausgabe; jeder andere Typ ist vor dem Senden `Unsupported`. |
| `tool_choice` in einer Anfrage ohne Tools (Anfrage) | weggelassen | SpaceX lehnt es ab (`A tool_choice was set on the request but no tools were specified`), wie es die Hilfsaufrufe von Codex senden ([weggelassene Tools](cross-format.md#weggelassene-tools)). |
| `presence_penalty` (Anfrage) | Extras | SpaceX ignoriert es und gibt `0.0` zurück. |
| `max_output_tokens` (Anfrage) | `sampling.max_output_tokens` | SpaceX wendet es nur auf die sichtbare Ausgabe an, nicht auf Reasoning-Token (Responses-Referenz), wie bei [Chat Completions](openai-chat-providers.md#spacex). |
| `{"error": "<message>"}` | `KppError` | Siehe die [Fehlertabelle](openai-responses.md#fehler-errorresponse). |

## Vercel

Quellen: Vercels Seiten zur [OpenAI Responses API](https://vercel.com/docs/ai-gateway/sdks-and-apis/responses), gelesen am 2026-10-29, und die Live-Aufzeichnungen in `fixtures/openai_responses/providers/vercel/`. Ein Vercel-Modell wird als Responses gesendet, wenn sein Wert **Sendet als** das sagt; Vercels `generation`-Dialekt ist Chat Completions. Sein Profil ist `compatible`.

| Vercel-Feld | KPP | Hinweise |
|---|---|---|
| Ereignisse `response.reasoning.delta` und `response.reasoning.done` | wie `response.reasoning_text.delta` und `response.reasoning_text.done` | Vercels Namen für OpenAIs Ereignisse zum Reasoning-Text, mit denselben Feldern; Responses-Apps erhalten sie so, wie Vercel sie geschickt hat, andere Dialekte erhalten das Reasoning (`streams/recorded-reasoning-stream`). |
| `reasoning`-Items mit `content` (`reasoning_text`) und ohne `encrypted_content` | wie bei OpenAI | Die Modelle von OpenAI fügen `encrypted_content` hinzu, wenn die Anfrage danach fragt (`responses/recorded-gpt-reasoning-encrypted`). |
| `provider_metadata` an der Antwort, `usage.input_tokens_details.cache_write_tokens` | Extras, `raw` der Nutzung | Wie bei Chat Completions. |
| `POST /v1/responses/input_tokens` | — | Vercel antwortet mit 404, und Kaeso schätzt die Token-Zählung für Vercel-Modelle selbst, ohne etwas zu senden. |
| Fehler | `KppError` | OpenAIs Hülle mit Vercels eigenem `type`, wie bei [Chat Completions](openai-chat-gateways.md#vercel). |
