---
title: MiniMax Images
description: Wie die Bildanfragen von KPP auf die Text-zu-Bild-API von MiniMax abgebildet werden.
order: 8
---

Schlagen Sie hier nach, wie eine Bildanfrage MiniMax erreicht: welches Feld wohin gehört, was abgelehnt wird und woher jede Abbildung stammt.

- **Codec:** `kaeso_codecs::minimax_images::MinimaxImages` (`crates/kaeso-codecs/src/minimax_images/`)
- **Pfad:** `POST /v1/image_generation`, ab der Wurzel des Hosts von MiniMax, mit `Authorization: Bearer`
- **Fixtures:** `crates/kaeso-codecs/fixtures/minimax_images/`, aus der Referenz von MiniMax gebaut, keine aufgezeichnet

**Quellen.** Die Referenz von MiniMax zu [Text zu Bild](https://platform.minimax.io/docs/api-reference/image-generation-t2i) und ihre [Fehlercodes](https://platform.minimax.io/docs/api-reference/errorcode), gelesen am 2026-11-08.

## Entscheidungen

- **Nur Generierungen.** Die Image-to-Image-API von MiniMax nimmt eine Figurenreferenz, kein Bild zum Ändern, deshalb wird eine Bearbeitung abgelehnt. Variationen werden wie überall abgelehnt ([OpenAI Images](openai-images.md#entscheidungen)).
- **Fehler können mit HTTP 200 kommen.** Ein fehlgeschlagenes `base_resp` ist unabhängig vom Status ein Fehler, nach seinem Code abgebildet.
- **Keine Streams.** MiniMax antwortet mit den fertigen Bildern. Kaeso fragt die ganze Antwort an, und das Gateway schreibt sie für eine streamende App als ein `image_generation.completed`-Event pro Bild.
- **Medientypen aus den Bytes.** MiniMax nennt das Format eines Base64-Bilds nicht, deshalb liest der Codec es aus den ersten Bytes.

## Anfrage

| KPP | MiniMax | Hinweise |
| --- | --- | --- |
| `model`, `prompt` | `model`, `prompt` | |
| `size` `WIDTHxHEIGHT` | `width`, `height` | `auto` lässt beide weg; MiniMax prüft den Bereich (512 bis 2.048, durch 8 teilbar). Ein MiniMax-Body mit `width` ohne `height` ist ungültig (`param` `width`). |
| `aspect_ratio` | `aspect_ratio` | |
| `count` | `n` | Höchstens 9. |
| `seed` | `seed` | |
| `delivery` `Url`, `Base64` | `response_format` `url`, `base64` | Jeder andere Wert ist ungültig, mit `param` `response_format`. |
| `user` | weggelassen, in `kaeso-left-out` genannt | MiniMax hat kein Feld für den Endnutzer. |
| `prompt_optimizer` und andere Schlüssel | Extras | Nur aus einer Anfrage im Format von MiniMax gesendet; andere Dialekte lehnen sie ab. |

## Abgelehnt

Jedes davon ist `Unsupported`, bevor etwas gesendet wird, mit dem Feld in `param`:

| KPP | `param` | Grund |
| --- | --- | --- |
| eine Bearbeitung (`ImageOperation::Edit`) | `image` | Die Image-to-Image-API nimmt eine Figurenreferenz, kein Bild zum Ändern. |
| eine Variation | `image` | Kein Anbieter bietet Variationen an. |
| eine `size`, die keine Pixelangabe und nicht `auto` ist | `size` | Senden Sie `WIDTHxHEIGHT` oder `aspect_ratio`. |
| `count` über 9 | `n` | Die Grenze von MiniMax. |
| `quality`, `format`, `compression`, `background`, `resolution`, `negative_prompt` | `quality`, `output_format`, `output_compression`, `background`, `resolution`, `negative_prompt` | Die Referenz von MiniMax hat keine solchen Felder. |
| `partial_images` über 0 in einem Stream | `partial_images` | MiniMax streamt nicht. |

## Antwort

| MiniMax | KPP |
| --- | --- |
| `data.image_urls[]` | Bilder als `Source::Url` |
| `data.image_base64[]` | Bilder als `Source::Base64`, der Medientyp aus den ersten Bytes gelesen, sonst `application/octet-stream` |
| `base_resp` mit `status_code` 0 | Extras `base_resp` |
| andere Schlüssel von `data` | Extras `data.<key>` |
| `metadata` und andere Schlüssel der obersten Ebene | Extras |

## Fehler

Ein `base_resp`, dessen `status_code` nicht 0 ist, ist ein Fehler, auch mit HTTP 200:

| `status_code` | KPP |
| --- | --- |
| `1002`, `1039` | `RateLimited` |
| `1004`, `2049` | `Authentication` |
| `1008` | `Billing` |
| `1026`, `1027` | `ContentFiltered` |
| `2013` | `InvalidRequest` |
| jeder andere | `ProviderError` |

Ein Body ohne `base_resp` wird als Fehler im Stil von OpenAI gelesen. Einer App im Format von MiniMax schreibt der Codec `base_resp` mit dem Code der Fehlerart: `1002` für ein Ratenlimit, `1004` für die Authentifizierung, `1008` für die Abrechnung, `1026` für einen Inhaltsfilter, `2013` für eine ungültige oder abgelehnte Anfrage und `1000` für den Rest. [MiniMax Speech](minimax-speech.md) liest Fehler genauso.

## Fixtures

| Datei | Deckt ab | Gebaut aus |
| --- | --- | --- |
| `requests/generate.json` | `aspect_ratio`, `n`, `prompt_optimizer` | Dem Anfragebeispiel der Referenz |
| `requests/generate-size-seed.json` | `width`, `height`, `seed`, `response_format: base64` | Dem Schema `ImageGenerationReq` |
| `responses/generate-urls.json` | Drei Links, `metadata`, `base_resp` | Dem Antwortbeispiel der Referenz, Links ergänzt |
| `responses/generate-base64.json` | Ein Base64-Bild | `data.image_base64` aus dem Schema |
| `errors/sensitive-prompt-200.json` | `1026` mit HTTP 200 | Der Referenz der Fehlercodes |
| `errors/invalid-key-200.json` | `2049` mit HTTP 200 | Der Referenz der Fehlercodes |

Es gab keinen MiniMax-Schlüssel mit Zugang zu Bildern, deshalb ist nichts aufgezeichnet. Die `README.md` des Ordners nennt die Herkunft, und `*.meta.json` den HTTP-Status.
