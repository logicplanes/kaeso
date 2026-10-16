---
title: KPP-Interna
description: Wie die KPP-Typen und Codecs aufgebaut sind, für alle, die an kaeso-kpp und kaeso-codecs mitarbeiten.
order: 3
nav: KPP-Interna
---

KPP, das Kaeso Provider Protocol, ist das kanonische Format, das jede Modellanfrage durchläuft. Das Gateway dekodiert, was eine App sendet, in KPP und kodiert KPP für den Anbieter; die Antwort nimmt denselben Weg zurück. Was Aufrufer von Kaeso davon sehen, steht unter [Formatübersetzung](../../concepts/format-translation.md); diese Seiten sind für alle, die KPP ändern.

| Seite | Inhalt |
| --- | --- |
| [Formatwechsel](cross-format.md) | Die Regeln für Daten, die von einem API-Format in ein anderes wechseln, und ihre Gründe |
| [Feldklassen](field-classes.md) | Jedes KPP-Feld als semantisch oder als Hinweis |
| [Extras-Allowlist](extras-allowlist.md) | Die Extras, die beim Formatwechsel einer Anfrage weggelassen werden dürfen |
| [Anthropic Messages](anthropic-messages.md), [OpenAI Chat Completions](openai-chat.md), [OpenAI Responses](openai-responses.md), [OpenAI Embeddings](openai-embeddings.md) | Jedes Wire-Feld jedes Formats und wohin es in KPP gehört |
| [Fixtures und Aufzeichnungen](fixtures.md) | Die echten Payloads, gegen die die Codecs getestet werden, und wie neue aufgezeichnet werden |

## Crates und Module

| Crate | Enthält |
| --- | --- |
| `kaeso-kpp` | Nur die KPP-Typen: Anfragen, Antworten, Stream-Events, Fehler, Token-Zählungen, Embeddings und Modelllisten, mit `validate()` und `required_capabilities()`. Keine Codec-Logik, kein I/O. |
| `kaeso-codecs` | Einen Sans-IO-Codec pro API-Format, im Code Dialekt genannt (`DialectId`), und die Bausteine, die sie gemeinsam nutzen. Ein Codec macht aus Bytes KPP-Werte und aus KPP-Werten Bytes; Streams werden Chunk für Chunk eingespeist und erzeugen Events. |

Beide Crates werden auf crates.io veröffentlicht. Ihr Bibliothekscode darf nur `serde`, `serde_json`, `bytes`, `thiserror`, `indexmap`, `base64` und `mime` verwenden; `cargo deny` verbietet darin `tokio`, `reqwest`, `hyper` und `axum` ([Abhängigkeiten](../dependencies.md#kpp)).

| Modul | Dialekt |
| --- | --- |
| `anthropic_messages` | Anthropic Messages (`/v1/messages`, `/v1/messages/count_tokens`) |
| `openai_chat` | OpenAI Chat Completions (`/v1/chat/completions`) |
| `openai_responses` | OpenAI Responses (`/v1/responses`, `/v1/responses/input_tokens`) |
| `openai_embeddings` | OpenAI Embeddings (`/v1/embeddings`) |
| `openai_models` | Die OpenAI Models API, die sich die drei OpenAI-Codecs teilen |

Die gemeinsamen Module sind `codec` (der Vertrag), `dispatch` (die Codec-Funktionen, zur Laufzeit nach `DialectId` gewählt), `wire` (serde-Bausteine), `sse` (Parser und Writer für Event-Streams), `cross` (Regeln für Daten, die den Dialekt wechseln), `continuity` (Reasoning, das mit der Unterhaltung mitreist), `keepalive` (Frames, die laufende Streams nicht verstummen lassen), `models` (Hilfsfunktionen für Modelllisten) und `testing` (Round-Trip-Vergleich, auch für Crates, die die Codecs nutzen).

## Entwurfsprinzipien

- **Zustandslos.** Ein `GenerateRequest` trägt immer die ganze Unterhaltung.
- **Nachrichten aus geordneten Content-Blöcken.** Die Reihenfolge der Blöcke bleibt von Anfang bis Ende erhalten.
- **Roh, wo es auf Bytes ankommt.** Tool-Argumente, JSON-Schemas, unbekannte Blöcke und Zitate sind `RawJson`; opakes Reasoning wird so aufbewahrt, wie es ankam, und mit seinem `Origin` versehen.
- **Semantisch oder Hinweis.** Jedes Feld ist entweder semantisch (ein Ziel muss es einhalten oder mit `Unsupported` ablehnen) oder ein Hinweis (es darf ignoriert werden). [Feldklassen](field-classes.md) listet sie auf.
- **Gebundene Extras.** Was KPP nicht modelliert, wird in `Extras` aufbewahrt, mit dem Dialekt versehen, aus dem es kam, und nur in diesen Dialekt zurückgeschrieben. `fields` sind semantisch: Ein anderer Dialekt lehnt sie ab, außer sie stehen auf der [Allowlist](extras-allowlist.md). `form` ist Darstellung (ein String oder ein Array, `null` oder fehlend, wie Chunks gruppiert waren), und andere Dialekte ignorieren es. Unbekannte Schlüssel verschachtelter Objekte verwenden Schlüsselpfade mit Punkten, etwa `function.x`.
- **Ablehnen vor dem Senden.** `check()` gibt `Unsupported` zurück und nennt die Funktion, die ein Ziel nicht ausdrücken kann, bevor irgendetwas gesendet wird.
- **Nichts fällt stillschweigend weg.** Jedes Feld und jeder Block einer eingehenden Payload landet an genau einer Stelle: in einem typisierten KPP-Feld, einem dialektgebundenen `Extras`-Eintrag, einem `Unknown`-Item oder einem `KppError` der Art `Unsupported`. Ein Match-Arm, der Daten verwirft, ist ein Bug.
- **Additive öffentliche API.** Öffentliche Enums und erweiterbare Structs sind `#[non_exhaustive]`; Enum-Varianten umschließen `#[non_exhaustive]`-Structs mit Konstruktoren (`TextBlock::new`, `Block::text`, …), sodass Felder hinzukommen können, ohne Code zu brechen, der Werte baut oder matcht. Der `Codec`-Trait ist versiegelt.

KPP-Werte und Fehler werden als Wert zurückgegeben. `KppError` und einige Enum-Varianten sind groß, aber ein Wert wird einmal pro Anfrage gebaut, und Fehler sind der kalte Pfad; sie zu boxen würde jede Signatur umständlicher machen, daher erlaubt der Workspace `clippy::large_enum_variant` und `clippy::result_large_err`.

Jeder KPP-Typ implementiert `serde` für Logs und Snapshots. Diese JSON-Form ist über Patch-Releases hinweg stabil, aber kein Wire-Format.

## Typen

Die Typen sind davon geprägt, was die Codecs brauchen, um verlustfrei zu bleiben:

- **`Extras`** hat drei Felder: `origin`, `fields` (rohes JSON, Byte für Byte) und `form` (Darstellungsmarkierungen). Round Trips im selben Dialekt müssen Schreibweisen reproduzieren, die KPP wegnormalisiert, etwa `content` als String oder Array, `null` oder fehlend, und Chunk-Grenzen; diese Markierungen tragen keine Bedeutung und bringen einen anderen Dialekt daher nie dazu, eine Anfrage abzulehnen, wie es unbekannte `fields` tun. Zwei leere `Extras` sind gleich, unabhängig von ihrem Ursprung.
- **Weitergegebene Header** (`anthropic-beta`, `anthropic-version`, `openai-beta`) werden in `Extras::fields` unter Schlüsseln gespeichert, die mit `header:` beginnen; wiederholte Header werden mit `", "` verbunden. Kein Body-Feld enthält einen Doppelpunkt, daher kollidieren die Schlüssel nie, und Header sind wie jedes andere Extra an ihren Dialekt gebunden.
- **Rohe Items.** `Box<RawValue>` ist in `RawJson` verpackt, das Byte für Byte vergleicht. Unbekannte Blöcke, Tools, Tool-Auswahlen, Stream-Events und Deltas sind `RawItem { origin, raw }`; Zitate sind `Citation { origin, raw }`, weil Zitatformate über Dialekte hinweg fast nichts gemeinsam haben.
- **Antworten.** `Reply` ist `Generate`, `CountTokens` oder `Embed`, und `RequestKind` benennt dieselben drei Arten: Die Bodies von Anthropics `/v1/messages` und `/v1/messages/count_tokens` lassen sich ohne den Endpunkt nicht unterscheiden.
- **Tools.** `ToolDef` ist ein Funktions-Tool oder ein unbekanntes Tool, das vollständig aufbewahrt wird; `ToolChoice` ist `Auto`, `None`, `Required`, ein benanntes Tool oder unbekannt. Die `parameters` eines Funktions-Tools sind optional, wie OpenAI es erlaubt.
- **Einstellungen für Ausgabe und Reasoning.** `OutputSpec::JsonSchema` hat einen optionalen Namen (Anthropic-Schemas haben keinen) und eine Beschreibung. `ReasoningConfig` enthält Modus, Aufwand, Budget, Zusammenfassung und die Angabe, ob verschlüsseltes Reasoning mitgeliefert werden soll; seine Enums bewahren unbekannte Werte in `Other(String)` auf, weil Anbieter oft neue Aufwandsstufen einführen.
- **Reasoning-Blöcke** enthalten Ursprung, Zusammenfassungen, Textteile und opake Teile, weil ein OpenAI-Reasoning-Item sowohl eine ID als auch verschlüsselten Inhalt trägt und mehrere Textteile haben kann, und weil ein Block, der nur eine Zusammenfassung enthält, trotzdem seinen Ursprung braucht, um zu entscheiden, wohin er zurückgespielt werden darf.
- **Nutzung.** `Usage::input_tokens` schließt Cache-Lese- und -Schreibvorgänge aus, die eigene Felder sind ([Nutzung](cross-format.md#nutzung)); `Usage::raw` bewahrt das Nutzungsobjekt des Anbieters mit seinem Ursprung auf, sodass ein Encoder es nur für seinen eigenen Dialekt darüberlegt. Embedding-Vektoren sind `f64`, damit die Werte des Anbieters unverändert wieder ausgegeben werden.
- **Fehler.** `ErrorKind::Billing` gibt es für Anthropics `billing_error` (402) und OpenAIs `insufficient_quota`, die keine andere Art treffend beschreibt.

## Streams

Ein KPP-Stream ist `ResponseStart → (BlockStart → BlockDelta* → BlockStop)* → ResponseDelta* → ResponseStop`, wobei `Error` an jeder Stelle möglich ist und `Extra` Dialekt-Events trägt, die KPP nicht modelliert. Die Blockindizes gehören KPP selbst: 0, 1, 2, … in der Reihenfolge, in der Blöcke beginnen. Blöcke dürfen sich überlappen (OpenAI streamt parallele Tool-Aufrufe so), daher nennt jedes Event seinen Block. Jedes Lifecycle-Event trägt `Extras`, sodass eine Wiedergabe im selben Dialekt die ursprünglichen Events exakt reproduziert, einschließlich Feldern einzelner Events wie OpenAIs `obfuscation`.

Stream-Decoder melden Fehler im Stream selbst: erst die bis dahin dekodierten Events, dann ein abschließendes `StreamEvent::Error`, sodass nichts verloren geht, was zuvor im selben Chunk dekodiert wurde.

## Der Codec-Vertrag

`Codec` ist ein Trait aus assoziierten Funktionen, die zur Kompilierzeit gewählt werden: Anfragen, Antworten, Fehler, Stream-Decoder und -Encoder sowie Modelllisten, für beide Seiten des Gateways. Auf der Ingress-Seite spricht eine App das Format mit Kaeso; auf der Egress-Seite spricht Kaeso es mit einem Anbieter. Das Gateway erfährt beide Dialekte erst pro Anfrage, daher bietet `dispatch` dieselben Funktionen mit einer `DialectId` an; ein Dialekt ohne Codec ergibt `Unsupported`.

`decode_request` nimmt eine `RequestKind` entgegen. `EncodeCtx` trägt die dekodierte Anfrage der App, weil OpenAI Responses Anfragefelder in seiner Antwort wiederholt, und ein `created_at`, weil ein Sans-IO-Encoder eine deterministische Uhr braucht. `EgressCtx` trägt, was das Gateway über das Ziel weiß: die Art der Anfrage und den Stream-Modus, den Anbieter und ob das Modell Reasoning beherrscht oder adaptives Thinking hat; der Anfrage-Encoder ergänzt die Tool-IDs und -Namen, die er umgeschrieben hat, und die Antwort-Decoder übersetzen sie zurück.

Eine App hat eine Anfrage im Format Anthropic Messages gesendet, und das Modell wird über OpenAI Chat Completions bedient:

```rust
use kaeso_codecs::{EgressCtx, EncodeCtx, dispatch};
use kaeso_kpp::{DialectId, RequestKind};

let req = dispatch::decode_request(DialectId::AnthropicMessages, RequestKind::Generate, &body, &headers)?;
req.validate()?;
let mut egress = EgressCtx::new(&req).with_provider("openai");
let outgoing = dispatch::encode_request(DialectId::OpenAiChat, &req, &mut egress)?;

let reply = dispatch::decode_response(DialectId::OpenAiChat, status, &reply_headers, &reply_body, &egress)?;
let answer = dispatch::encode_response(DialectId::AnthropicMessages, &reply, &EncodeCtx::for_request(&req))?;
```

`encode_request` führt zuerst `check()` des Ziels aus. Streams funktionieren genauso mit `dispatch::stream_decoder` und `dispatch::stream_encoder`, Fehler mit `dispatch::decode_error` und `dispatch::encode_error`.

## Wire-Structs

Wire-Structs müssen jedes Feld behalten, auch solche, die das Struct nicht deklariert, und rohe Payloads (`RawValue`) Byte für Byte bewahren. `#[serde(flatten)]` kann nicht beides: Flattening puffert die Eingabe, und `RawValue` lässt sich nicht aus einem Puffer lesen, sodass Tool-Argumente und Schemas ihre Bytes verlören. `wire::Ext<T>` erledigt dieselbe Aufgabe ohne Puffern:

- Jeder Schlüssel des JSON-Objekts, den `T` nicht deklariert, wird roh in `Ext::extra` aufbewahrt und wird zu KPP-`Extras::fields`.
- Jeder deklarierte Schlüssel, dessen empfangene Schreibweise `T` nicht reproduzieren kann (`"x": null` für eine `Option`, die bei `None` übersprungen wird, `"flag": false` für einen `bool`, der bei false übersprungen wird, `"list": []` für einen `Vec`, der übersprungen wird, wenn er leer ist), wird roh in `Ext::verbatim` aufbewahrt und wird zu KPP-`Extras::form`.

Beim Serialisieren schreibt ein `Ext<T>` zuerst `T`, dann die verbatim-Schlüssel, die `T` nicht geschrieben hat, dann die zusätzlichen Schlüssel. Ein Round Trip aus Dekodieren und Kodieren ist daher exakt, und ein Codec, der ein typisiertes Feld ändert, etwa beim Übersetzen aus einem anderen Dialekt, setzt sich immer gegen eine veraltete verbatim-Schreibweise durch. `wire::Maybe<T>` unterscheidet fehlend von `null`, wo der Unterschied für den Codec sichtbar sein muss.

Wire-Structs, die in `Ext` verwendet werden, folgen diesen Regeln: kein `#[serde(flatten)]`, kein `alias`, kein `deny_unknown_fields`, keine untagged oder intern getaggten Enums; getaggte Unions werden mit `wire::type_tag` nach `"type"` unterschieden; JSON-Objekte mit doppelten Schlüsseln werden abgelehnt. Felder werden nur deklariert, wenn der Codec sie auf ein typisiertes KPP-Feld abbildet; alles andere bleibt absichtlich undeklariert und reist als Extras mit.

Der Codec für OpenAI Responses liest typisierte Werte mit einfachen serde-Structs und schreibt mit einem geordneten JSON-Baum (`openai_responses::node::Node`), der rohes JSON Byte für Byte einbetten kann; `serde_json::Value` sortiert Objektschlüssel und würde rohe Payloads umformatieren. [Wie genaue Schreibweisen erhalten bleiben](openai-responses.md#wie-genaue-schreibweisen-erhalten-bleiben) beschreibt, wie er sie reproduziert.

## Server-sent Events

`sse::SseParser` folgt dem [WHATWG-Algorithmus für Event-Streams](https://html.spec.whatwg.org/multipage/server-sent-events.html#event-stream-interpretation), mit bewussten Abweichungen, damit nichts verloren geht:

- Ein Frame mit Feldern, aber ohne `data:`-Zeile wird trotzdem gemeldet, mit leerem `data`.
- Kommentare und unbekannte Felder werden gemeldet statt ignoriert.
- Ein Stream, der mitten in einer Zeile endet, wird als `SseError::Truncated` gemeldet statt verworfen.

Bytes können in beliebigen Chunks eingespeist werden: Zeilen dürfen an jeder Stelle getrennt sein, auch zwischen `\r` und `\n` eines CRLF-Zeilenendes. Ein Frame, der größer ist als das Limit des Parsers (`sse::DEFAULT_MAX_FRAME_BYTES`, 16 MiB), ist ein Fehler; das begrenzt den Speicher, den jeder Stream belegen kann.

## Modelllisten

Modelllisten (`GET /v1/models`, `GET /v1/models/{id}`) gehören zu KPP, mit `ModelInfo`, `ModelLifecycle`, `ModelList` und `ModelsQuery`, sodass das Gateway die Wire-Formen desselben Pfads nie selbst kennen muss. Jeder Dialekt nutzt denselben Pfad, daher lässt sich der Ingress-Dialekt nicht aus ihm ablesen: `dispatch::models_dialect` wählt ihn anhand der Header (`anthropic-version` bedeutet Anthropic, alles andere OpenAI). `ModelList::page` wendet `before_id`, `after_id`, `limit` und den Lifecycle-Filter für jeden Dialekt gleich an.

- **Query-Strings** werden an `&` und `=` getrennt und als `application/x-www-form-urlencoded` percent-dekodiert (`+` ist ein Leerzeichen). Unbekannte Parameter werden zu Extras unter `query.<name>`.
- **Datumsangaben.** KPP speichert Unix-Sekunden. RFC-3339-Zeitangaben (Anthropic) und reine Datumsangaben (OpenAI `shutdown_date`) werden mit dem proleptischen gregorianischen Kalender umgerechnet, ohne Datumsbibliothek. Sekundenbruchteile und Zeitzonenversätze überstehen die Umrechnung nicht; der genau empfangene Text wird in der Form-Markierung `spelling` aufbewahrt, wann immer erneutes Kodieren ihn nicht reproduzieren würde. Ein Datum nach dem Jahr 9999 lässt sich nicht in RFC 3339 schreiben: Anthropics `created_at` bekommt dann den dokumentierten unbekannten Wert (die Epoche), andere Datumsangaben `null`.
- **Fehlende oder `null`-Felder.** Ein Encoder füllt ein Feld, für das KPP keinen Wert hat, mit dem dokumentierten Standardwert; hat der empfangene Body dieses Feld anders geschrieben, vermerkt das die Form-Markierung `none`.

## Stream-Encoder

**Anthropic Messages.** Anthropic streamt einen Content-Block nach dem anderen, während sich in KPP-Streams aus anderen Dialekten Blöcke überlappen können: OpenAI streamt parallele Tool-Aufrufe so und schließt jeden Block erst am Ende. Der Encoder ist ein expliziter Zustandsautomat. Der erste Block, der beginnt, ist aktiv und wird geschrieben, sobald er ankommt; Blöcke, die beginnen, während ein anderer aktiv ist, warten und werden in Startreihenfolge gepuffert; stoppt der aktive Block, wird der nächste wartende geschrieben, mit seinem Start, seinen gepufferten Deltas und seinem Stopp, falls dieser schon kam. Ein `ResponseDelta` oder `ResponseStop` schließt zuerst jeden offenen Block, und mehrere `ResponseDelta`s werden zu einem `message_delta` zusammengeführt, weil die SDKs `stop_reason` mit jedem einzelnen überschreiben. Anthropic-Indizes werden in der Reihenfolge vergeben, in der Blöcke geschrieben werden. Ein Stream, der nicht mit `ResponseStart` beginnt, bekommt ein `message_start` mit leerer ID.

**OpenAI Chat Completions.** Aus Chat dekodierte Events werden mithilfe der Chunk-Form-Markierungen wieder zu ihren ursprünglichen Chunks gruppiert, sodass eine Wiedergabe im selben Dialekt jeden Frame reproduziert. Events aus anderen Dialekten werden zu je einem Chunk. Der Stream beginnt mit einem Rollen-Chunk, sobald die Antwort startet (`delta: {"role": "assistant", "content": "", "refusal": null}`, OpenAIs eigener erster Chunk), sodass die App vom Stream hört, während das Modell denkt; ein Tool-Aufruf beginnt mit einem Header-Chunk, gefolgt von Argument-Chunks; die Nutzung wird nur gesendet, wenn die App danach gefragt hat (`stream_options.include_usage`), als letzter Chunk mit `choices: []`. `ResponseStop` schreibt `data: [DONE]`; ein `Error` schreibt `data: {"error": …}` und beendet den Stream ohne `[DONE]`.

**OpenAI Responses.** Die Stream-Engine erzeugt für jedes KPP-Event die SSE-Events in der Reihenfolge, in der OpenAI sie sendet, einschließlich aller redundanten (`.done`-Events, Part-Events, Item-Events, das abschließende Response-Objekt mit allen Ausgabe-Items), und vergibt `sequence_number`s. Der Decoder lässt dieselbe Engine als Schatten mitlaufen: Jedes SSE-Event, das die Engine an dieser Stelle erzeugen würde, wird dem KPP-Event zugeordnet, und Abweichungen werden als Overrides mit Geltungsbereich festgehalten; zugeordnete Events, die das Original nicht hatte, werden in der Form-Markierung `skip` vermerkt; Events, die dort ankommen, wo die Engine keine erzeugen würde, werden als `Extra` weitergereicht. Ein Stream im selben Dialekt wird exakt reproduziert, und ein Stream aus einem anderen Dialekt wird zu einem vollständigen, gültigen Responses-Stream.

Jeder Stream-Encoder kann außerdem einen Keepalive-Frame schreiben; wann, steht unter [Keepalives](cross-format.md#keepalives).

## Tests

| Suite | Was sie belegt |
| --- | --- |
| `crates/kaeso-kpp/tests/types.rs` | Die KPP-Typen allein: stabile Namen der Fehlerarten und Blöcke, `validate()` und `required_capabilities()`. |
| Fixture-Round-Trips (die `tests`-Module jedes Dialekts) | `decode → KPP → encode` reproduziert jede [Fixture](fixtures.md) eines Dialekts als gleichen JSON-Wert (bei Streams als gleiche Event-Folge), bei Streams unter vielen Chunk-Aufteilungen. Fixtures, die abgelehnt werden müssen, scheitern mit der genannten `Unsupported`-Funktion. |
| `crates/kaeso-codecs/tests/cross_matrix.rs` | Für jede Generierungs-Fixture von Dialekt A und jeden anderen Dialekt B: Anfragen durchlaufen `decode_A → check_B → encode_B` und müssen sich mit dem Decoder von B wieder dekodieren lassen, ohne dass sich die Unterhaltung ändert; Antworten und Streams durchlaufen `decode_A → encode_B → decode_B` und müssen Text, Verweigerungen und Tool-Aufrufe behalten, auch zurück in A. Anfragen und Antworten zur Token-Zählung wechseln zwischen Anthropic und Responses. Die Ergebnisse sind `insta`-Snapshots. |
| `crates/kaeso-codecs/tests/dispatch.rs` | `dispatch` dekodiert Anbieterfehler und kodiert Fehler mit einem Status in jedem Dialekt, und der Embeddings-Codec lehnt Generierungsantworten ab. |
| `crates/kaeso-codecs/tests/kpp_proptest.rs` | Zufällige gültige KPP-Anfragen und -Antworten überstehen Kodieren und Dekodieren für jeden Generierungsdialekt, auf der Teilmenge, die jeder Dialekt unterstützt. |
| `crates/kaeso-codecs/tests/keepalive.rs` | Alles, was eine App nicht sehen kann, bringt trotzdem regelmäßig Keepalive-Frames auf die Leitung, und für den Parser der App sind die Frames unsichtbar. |
| `crates/kaeso-codecs/tests/reasoning_carry.rs` | Reasoning geht an eine App eines anderen Dialekts und kommt in der nächsten Runde Byte für Byte zurück, über Apps für Responses, Anthropic und Chat. |
| `crates/kaeso-codecs/tests/responses_on_anthropic.rs` | Apps für OpenAI Responses auf Anthropic-Modellen und Anthropic-Apps auf OpenAI-Modellen: Reasoning-Anfragen, Thinking-Streams in der Event-Reihenfolge von OpenAI und Tool-Schleifen. |
| `e2e/conformance`, bedient von `crates/kpp-conformance` | Die offiziellen Python-SDKs von OpenAI und Anthropic parsen alles, was die Encoder erzeugen ([Tests](../testing.md#sdk-konformität)). |
| `fuzz/` | Jeder Decoder nimmt beliebige Bytes an, ohne einen Panic auszulösen, und der SSE-Parser liefert bei jeder Chunk-Aufteilung dieselben Items ([Fuzzing](../testing.md#fuzzing)). |

`kaeso_codecs::testing` vergleicht JSON strukturell: Die Reihenfolge der Schlüssel spielt nie eine Rolle, Ganzzahlen werden exakt verglichen, und eine Zahl mit Nachkommastellen oder Exponent wird mit jeder anderen Zahl nach ihrem Wert verglichen, sodass `1` gleich `1.0` ist. KPP speichert `temperature`, `top_p` und die Penalties als `f64`, und die Anbieter behandeln beide Schreibweisen gleich.

Ändert sich durch eine Änderung die Ausgabe über Dialekte hinweg, ist der Snapshot-Diff zu prüfen: `INSTA_UPDATE=always cargo test -p kaeso-codecs --test cross_matrix` ausführen und die geänderten `.snap`-Dateien lesen, bevor sie committet werden.

## Einen Codec ändern

1. **Zuerst die Fixture.** Die echte Payload in `crates/kaeso-codecs/fixtures/<dialect>/` ablegen, mit einer Herkunftszeile in der `README.md` dieses Ordners ([Fixtures](fixtures.md)).
2. **Die Quelle nennen.** Jede Abbildung nennt ihre Quelle auf der Mapping-Seite des Dialekts: die Dokumentation des Anbieters oder das OpenAPI-Schema beim festgelegten Spezifikations-Commit. Die Quellen sind die [OpenAPI-Spezifikation von OpenAI](https://github.com/openai/openai-openapi), die auf der Seite [Anthropic Messages](anthropic-messages.md) genannte OpenAPI-Spezifikation von Anthropic, die Typen der offiziellen SDKs ([openai-python](https://github.com/openai/openai-python), [anthropic-sdk-python](https://github.com/anthropics/anthropic-sdk-python)) und die Dokumentation der Anbieter. Wo die Quellen mehrdeutig sind, das vorsichtige Verhalten wählen, `Unsupported`, und ein Issue eröffnen.
3. **Erst ein fehlschlagender Test, dann Code.** Jedes Wire-Feld wird auf ein typisiertes KPP-Feld, einen an den Ursprung gebundenen `Extras`-Eintrag, ein `Unknown`-Item oder einen `Unsupported`-Fehler abgebildet.
4. **Die Mapping-Seite des Dialekts aktualisieren**, in allen drei Sprachen, im selben Pull Request.

Fehlermeldungen richten sich an die Person, die Kaeso aufgerufen hat: Sie nennen das Feld, das das Ziel nicht unterstützt, und was zu tun ist.
