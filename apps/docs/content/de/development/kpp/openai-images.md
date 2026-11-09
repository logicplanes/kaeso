---
title: OpenAI Images
description: Wie jedes Feld der OpenAI Images API auf KPP abgebildet wird und wie andere Bildformate es übernehmen.
order: 8
---

Schlagen Sie hier jedes Feld der Images API von OpenAI nach: wohin es in KPP gehört, welche Anbieter es in welcher Form nehmen und woher jede Abbildung stammt. Prüfen Sie es, bevor Sie ändern, wie ein Bild-Codec ein Feld liest oder schreibt.

- **Codecs:** `kaeso_codecs::openai_images::OpenAiImages` (`crates/kaeso-codecs/src/openai_images/`) und Bildanfragen im Gemini-Codec (`crates/kaeso-codecs/src/gemini/images.rs`); das eigene Format von MiniMax hat [seine Seite](minimax-images.md)
- **Endpunkte:** `POST /v1/images/generations`, `POST /v1/images/edits` (JSON oder `multipart/form-data`) und `POST /v1/images/variations`
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_images/`, alle aus der Spezifikation und der Dokumentation der Anbieter gebaut, keine aufgezeichnet

**Quellen.** Die OpenAPI-Spezifikation von OpenAI im Commit `926a7b8133226630bac025350e84b0ca5c31d45a` (2026-11-07): `CreateImageRequest`, `CreateImageEditRequest`, `EditImageBodyJsonParam`, `ImageRefParam`, `CreateImageVariationRequest`, `ImagesResponse`, `Image`, `ImageGenUsage`, `ImagesUsage`, `ImageGenPartialImageEvent`, `ImageGenCompletedEvent` und ihre Gegenstücke `image_edit.*`. Die Bildschemata haben sich nach dem Commit geändert, den die anderen OpenAI-Codecs festhalten (`e95c0fe`), deshalb hält dieser Dialekt den neueren fest. RFC 7578 für `multipart/form-data`. Die Seiten der Anbieter stehen unter [Anbieter und ihre Formen](#anbieter-und-ihre-formen).

## Entscheidungen

Der Maintainer hat diese Entscheidungen an die Medienarbeit delegiert; jede ist hier festgehalten.

- **Eine KPP-Anfrage für alle drei Vorgänge.** `Request::Image(ImageRequest)` mit `ImageOperation` `Generate`, `Edit` oder `Variation`, und `Reply::Image(ImageResponse)`. Der Pfad nennt den Vorgang, deshalb werden Anfragen von Apps mit `dispatch::decode_image_request` dekodiert, so wie Gemini-Anfragen mit `decode_gemini_request`.
- **Ein Dialekt pro Wire-Format.** `openai_images` bedient OpenAI und jeden Anbieter, der eine Form seiner API spricht; jeden beschreibt ein `ImagesProfile`. `minimax_images` ist das eigene Format von MiniMax. Bildmodelle von Gemini nehmen Bildanfragen über den Dialekt `gemini` als Aufrufe von `generateContent`.
- **Variationen werden überall abgelehnt.** OpenAI hat seinen Endpunkt für Variationen eingestellt (410 in der Spezifikation), und kein anderer Anbieter dokumentiert einen. Die Anfrage wird dekodiert, damit die App einen klaren Fehler in ihrem eigenen Format bekommt: `Unsupported` (`image variations`, Parameter `image`), mit Verweis auf `/v1/images/edits`.
- **Keine Bildverarbeitung.** Kaeso wandelt Bilder nicht um, skaliert sie nicht und lädt sie nicht herunter. Eine Größe, die ein Anbieter nicht erzeugen kann, ein Link, wo Bytes nötig sind, oder Bytes, wo ein Anbieter nur mit Links antwortet, werden abgelehnt, bevor etwas gesendet wird.
- **Streams von Anbietern, die nicht streamen.** Mit `partial_images` 0 oder ohne den Wert sendet OpenAI das fertige Bild als ein Ereignis `*.completed`. Kaeso fragt solche Anbieter nach der ganzen Antwort und schreibt dasselbe Ereignis, eines pro Bild, mit der Nutzung am letzten. Teilbilder von diesen Anbietern zu verlangen ist `Unsupported` (Parameter `partial_images`).

## Lesart der Tabellen

- **typisiert**: ein typisiertes KPP-Feld.
- **Extras**: `Extras::fields`, gebunden an `openai_images`. Jeder Anbieter von `openai_images` bekommt sie, außer sein Profil lehnt das Feld ab; andere Dialekte lehnen sie ab ([Extras](cross-format.md#extras)).
- **Form**: eine codec-eigene Markierung in `Extras::form`.

## Anfrage

Die JSON-Anfrage zum Generieren, die JSON-Anfrage zum Bearbeiten und der Multipart-Upload teilen jedes Feld außer den Bildern.

| Wire-Feld | KPP | Hinweise |
|---|---|---|
| `model` | `ImageRequest.model` typisiert | Von `validate()` verlangt. |
| `prompt` | `prompt` typisiert | Zum Generieren und Bearbeiten verlangt. |
| `images[]` (JSON-Bearbeitung) `{image_url}` | `images[]` `Source::Url`, oder `Source::Base64` für eine kanonische Data-URL | Form `body: "json"`. |
| `images[]` `{file_id}` | `Source::FileId`, Ursprung `openai_images` | Nur an Anbieter gesendet, deren Profil Datei-IDs nimmt. |
| `image`, `image[]` (Upload) | `images[]` `Source::Base64` mit dem `Content-Type` des Teils, und `ImageInput.filename` | Form `body: "multipart"`; der erste Feldname in Form `image_field`. Ein Teil ohne Dateinamen ist `InvalidRequest`. |
| `mask` | `mask` typisiert | Ein Upload oder ein `ImageRefParam`. |
| `markup` | Extras `markup` als `{image_url}` | Ein Upload wird eine Data-URL und als Dateiteil zurückgeschrieben. |
| `n` | `count` typisiert | |
| `size` | `size` (`ImageSize`) typisiert | `auto` oder `WIDTHxHEIGHT`, andere Werte wie geschrieben. |
| `quality` | `quality` (`ImageQuality`) typisiert | `auto`, `low`, `medium`, `high`, `xhigh`, `max`, `standard`, `hd`; andere wie geschrieben. |
| `output_format` | `format` (`ImageFormat`) typisiert | `png`, `jpeg`, `webp`, `gif`, `svg`; andere wie geschrieben. |
| `output_compression` | `compression` typisiert | 0 bis 100, von `validate()` geprüft. |
| `background` | `background` typisiert | |
| `response_format` `url`, `b64_json` | `delivery` `Url`, `Base64` typisiert | Andere Werte: `InvalidRequest`. |
| `stream` `true` | `stream` an | `false`: Form `stream_off`. Im Upload nur `true`/`false`. |
| `partial_images` | `partial_images` typisiert | |
| `safety_identifier` | `user` typisiert | `user` daneben: Extras `user`. |
| `user` allein | `user` typisiert | Form `user: "user"`, damit es als `user` zurückgeschrieben wird. |
| `aspect_ratio`, `resolution`, `seed`, `negative_prompt` | typisiert | Keine Felder von OpenAI; SpaceX, OpenRouter, Together AI, QwenCloud, MiniMax und Gemini nehmen einige davon. Das Profil von OpenAI lehnt sie ab. |
| `moderation`, `style`, `input_fidelity`, `complexity`, `frame_count`, `frame_duration_ms`, `svg`, unbekannte Schlüssel | Extras | Upload-Felder mit Klammern werden verschachtelt: `svg[model]` wird `svg: {model}`. Uploads von `frame_count` und `frame_duration_ms` werden Zahlen. |
| Header `openai-beta` | Extras `header:openai-beta` | |

Der Multipart-Leser (`kaeso_codecs::multipart`) ist sans-IO und begrenzt: höchstens 64 Teile (`MAX_FORM_PARTS`), 50 MB pro Teil (`MAX_IMAGE_BYTES`, das Limit von OpenAI pro Bild) und 8 KiB Header pro Teil. Er liest nur CRLF-Rahmung, wie RFC 7578 es verlangt, und lehnt einen Teil ohne `Content-Disposition: form-data` oder ohne Feldnamen ab. Der Schreiber wählt eine Grenze, die in keinem Teil vorkommt, abgeleitet aus dem Inhalt, damit das Kodieren deterministisch bleibt.

`validate()` lehnt eine Generierung mit Bildern ab, eine Bearbeitung oder Variation ohne Bild, eine Variation mit mehr als einem Bild oder einer Maske, `n` 0 und `output_compression` über 100.

## Antwort

| Wire-Feld | KPP | Hinweise |
|---|---|---|
| `created` | `ImageResponse.created` | Als Zeitpunkt der Anfrage bei Kaeso geschrieben, wenn der Anbieter keinen sendet, weil `ImagesResponse` ihn verlangt. |
| `data[].b64_json` | `GeneratedImage.source` `Source::Base64` | Der Medientyp kommt aus `data[].mime_type` (SpaceX), `data[].media_type` (OpenRouter), dem `output_format` der Antwort oder `image/png`, dem Standard von OpenAI. |
| `data[].url` | `Source::Url` | `b64_json` und `url` zusammen: der Link in Extras `url`. |
| `data[].revised_prompt` | `revised_prompt` typisiert | |
| `background`, `output_format`, `size`, `quality` | `settings` typisiert | |
| `usage.input_tokens` (oder `prompt_tokens` von OpenRouter) | `usage.input_tokens`, ohne `input_tokens_details.cached_tokens` | Das Nutzungsobjekt bleibt in `usage.raw`. |
| `usage.output_tokens` (oder `completion_tokens`) | `usage.output_tokens` | |
| `usage.input_tokens_details.image_tokens` | `usage.input_image_tokens` | |
| `usage.output_tokens_details.image_tokens` (oder `completion_tokens_details`) | `usage.output_image_tokens` | |
| kein `usage` | Form `no_usage` | |
| `svg_usage`, `safety_identifier`, unbekannte Schlüssel | Extras | |

Eine Antwort aus einem anderen Format bekommt die Nutzungsgestalt von OpenAI: `input_tokens`, `output_tokens`, `total_tokens`, `input_tokens_details` und, wenn Bild-Token der Ausgabe bekannt sind, `output_tokens_details`.

## Streaming

`image_generation.partial_image` und `image_edit.partial_image` werden `StreamEvent::Image` mit `ImageEvent.partial` auf `partial_image_index`; Ereignisse `*.completed` werden fertige Bilder mit ihrer Nutzung. `created_at`, `size`, `quality`, `background` und `output_format` gehen nach `created` und `settings`. Die Namen der Ereignisse folgen dem Endpunkt der App: ein Bearbeitungsstream sagt `image_edit.*`.

Die Spezifikation verlangt in diesen Ereignissen jedes Feld außer `usage`. Ereignisse aus einem anderen Format bekommen `auto` für unbekannte Einstellungen, `png` für ein unbekanntes Format und `0` für eine unbekannte Zeit. Ein Stream-Fehler wird als `event: error` mit `{"type": "error", "error": {type, code, message, param}}` geschrieben, der Form, die OpenRouter dokumentiert, weil die Spezifikation von OpenAI kein Fehlerereignis definiert. Andere Ereignisse, etwa `image_generation.text_chunk` von OpenRouter, bleiben `StreamEvent::Extra`. Das abschließende `data: [DONE]` von OpenRouter wird nicht zurückgeschrieben.

## Fehler

Die Fehlergestalt von OpenAI, geteilt mit [OpenAI Responses](openai-responses.md#fehler-errorresponse); Fehler tragen den Ursprung `openai_images`. `moderation_blocked` und die anderen Codes von OpenAI behalten ihre Art, und rohe Fehler von OpenAI erreichen OpenAI-Apps unverändert.

## Anbieter und ihre Formen

Das `ImagesProfile` jedes Anbieters sagt, wo er Generierungen und Bearbeitungen bedient, wie er Größen, Bilder und die Auslieferung schreibt und welche Felder von OpenAI ihm fehlen. Ein fehlendes Feld ist `Unsupported`, bevor etwas gesendet wird, mit dem Feld in `param`.

| Profil | Generierungen | Bearbeitungen | Unterschiede | Quelle |
|---|---|---|---|---|
| `OPENAI` | `/v1/images/generations` | `/v1/images/edits`, Multipart für Uploads, JSON, wenn ein Bild ein Link oder eine Datei-ID ist | Streamt Teilbilder. | Spezifikation `926a7b8` |
| `VERCEL` | gleich | gleich, ohne Datei-IDs | Keine Streams; `user` statt `safety_identifier`; kein Limit für `n`. | [Vercel image generation](https://vercel.com/docs/ai-gateway/capabilities/image-generation/openai) |
| `XAI` (SpaceX) | gleich | `/v1/images/edits`, nur JSON: `image` oder `images[]` aus `{type: "image_url", url}`, höchstens 5, keine Maske | `aspect_ratio` und `resolution` statt `size`; `quality` `low`, `medium`, `auto`; kein Ausgabeformat, Hintergrund, keine Kompression oder Moderation. | [xAI images](https://docs.x.ai/developers/rest-api-reference/inference/images) |
| `TOGETHER` | gleich | auf Generierungen: ein Link in `image_url`; Uploads abgelehnt | `width` und `height`; `response_format: "base64"`; `n` bis 4; `jpeg` oder `png`; `aspect_ratio`, `seed`, `negative_prompt`. | [Together AI create image](https://docs.together.ai/reference/post-images-generations) |
| `QWENCLOUD` | gleich, unter `/compatible-mode` | auf Generierungen: `image`, eine Data-URL oder ein Link, oder eine Liste von bis zu 10 | Nur Links, 24 Stunden gültig: `b64_json` abgelehnt; `n` bis 6; `seed`, `negative_prompt`. Die Stufe in `usage.output_image_type`, etwa `qima_output_2k`, wird `settings.resolution` `2K`. | [QwenCloud OpenAI-compatible text-to-image](https://docs.qwencloud.com/api-reference/image-generation/qwen-text-to-image-openai) |
| `ZAI` | `/api/paas/v4/images/generations` | keine | Ein Bild pro Anfrage; `quality` `hd` oder `standard`; `user_id`; nur Links, 30 Tage gültig. | [Z.ai generate image](https://docs.z.ai/api-reference/image/generate-image) |
| `OPENROUTER` | `/v1/images` (unter `/api`) | auf Generierungen: `input_references` aus `{type: "image_url", image_url: {url}}`, bis zu 16 | Nur Bytes: `url` abgelehnt; `aspect_ratio`, `resolution`, `seed`; Streams nur mit `data:`, mit `[DONE]` am Ende. | [OpenRouter image generation](https://openrouter.ai/docs/guides/overview/multimodal/image-generation) |

`ImagesProfile::compatible(name)` ist die Form von OpenAI unter einem anderen Namen, für eigene Anbieter.

## Bildmodelle von Gemini

Der Gemini-Codec schreibt eine Bildanfrage als `generateContent` mit dem Prompt und den Bildern einer Bearbeitung als Teile `inlineData`, und `generationConfig.responseModalities: ["IMAGE"]`, wie es der [Leitfaden zur Bildgenerierung](https://aistudio.google.com/docs/image-generation) von Google tut. Quellen: dieser Leitfaden, Discovery-Revision `20261106` und `GoogleCloudAiplatformV1ImageConfig` der Agent Platform.

| KPP | Gemini | Hinweise |
|---|---|---|
| `size` | `imageConfig.aspectRatio` und `imageSize` | Nur die Größen aus den Tabellen des Leitfadens: eine Tabelle für die Bildmodelle von Gemini 3, eine für Gemini 2.5 Flash Image. `1024x1024` ist `1:1` bei `1K`; `1536x1024` wird nicht erzeugt und ist `Unsupported`. |
| `aspect_ratio`, `resolution` | `imageConfig.aspectRatio`, `imageSize` | Nicht zusammen mit `size`. |
| `seed` | `generationConfig.seed` | |
| `format`, `compression` | `imageConfig.imageOutputOptions` `mimeType`, `compressionQuality` | Nur Agent Platform (`GeminiProfile::image_output_options`); `png` oder `jpeg`. |
| `count` über 1, `quality`, `background`, `negative_prompt`, `mask`, Bilder als Link oder Datei-ID, `delivery` `Url` | `Unsupported` | Google warnt, dass das Modell der Zahl der Bilder nicht immer folgt; Gemini antwortet nur mit Bytes. |
| `user` | weggelassen (`kaeso-left-out: user`) | Gemini hat kein Feld für den Endnutzer. |

Die Bildteile der Antwort werden die Bilder, und `usageMetadata.candidatesTokensDetails` mit der Modalität `IMAGE` wird bei jeder Gemini-Antwort `usage.output_image_tokens`. Text, den das Modell neben das Bild schreibt, hat in einer Bildantwort keinen Platz und wird weggelassen. Eine Antwort ohne Bild ist ein Fehler: `ContentFiltered`, wenn die Filter von Google sie gestoppt haben, sonst `ProviderError`. Imagen wird nicht abgebildet: AI Studio hat es abgeschaltet, und die Agent Platform führt seine Endpunkte als eingestellt.

## MiniMax Images

`minimax_images` ist das eigene Format von MiniMax, `POST /v1/image_generation`. [MiniMax Images](minimax-images.md) bildet seine Felder, Ablehnungen und Fehler ab.
