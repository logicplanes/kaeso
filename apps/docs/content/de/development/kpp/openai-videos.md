---
title: OpenAI Videos
description: Wie die Videos API von OpenAI auf die Videojobs von KPP abgebildet wird und wie Together AI sie nimmt.
order: 8
---

Schlagen Sie hier jedes Feld der Videos API von OpenAI nach: wohin es in KPP gehört, wie später nach einem Videojob gefragt wird und woher jede Abbildung stammt. Prüfen Sie es, bevor Sie ändern, wie ein Video-Codec ein Feld liest oder schreibt.

- **Codec:** `kaeso_codecs::openai_videos::OpenAiVideos` (`crates/kaeso-codecs/src/openai_videos/`), mit den Helfern, die alle Video-Dialekte teilen, in `crates/kaeso-codecs/src/video_jobs.rs`
- **Endpunkte:** `POST /v1/videos` (JSON oder `multipart/form-data`), `POST /v1/videos/{video_id}/remix`, `GET /v1/videos`, `GET /v1/videos/{video_id}`, `GET /v1/videos/{video_id}/content` und `DELETE /v1/videos/{video_id}`
- **Fixtures:** `crates/kaeso-codecs/fixtures/openai_videos/`, aus der Spezifikation und der Referenz von Together AI gebaut, keine aufgezeichnet

**Quellen.** Die OpenAPI-Spezifikation von OpenAI im Commit `926a7b8133226630bac025350e84b0ca5c31d45a` (2026-11-07): `CreateVideoJsonBody`, `CreateVideoMultipartBody`, `ImageRefParam-2`, `CreateVideoRemixBody`, `VideoResource`, `VideoListResource`, `DeletedVideoResource`, `VideoContentVariant`, `Error-2` und die Feldnamen, die openai-python 3.41.0 schreibt. Together AI: [Video erstellen](https://docs.together.ai/reference/create-videos) und [Video-Metadaten abrufen](https://docs.together.ai/reference/get-videos-id), gelesen am 2026-11-09.

## Entscheidungen

Der Maintainer hat diese Entscheidungen an die Medienarbeit delegiert; jede ist hier festgehalten.

- **Ein Video ist ein Job.** `Request::Video(VideoRequest)` startet einen, mit `VideoOperation` `Generate` oder `Remix`; die Antwort ist `Reply::Video(VideoJob)`, der Job, wie der Anbieter ihn meldet. Spätere Fragen nach einem Job sind keine KPP-Anfrage: `encode_video_poll` und `decode_video_poll` des Traits `Codec` bauen und lesen diesen Aufruf, `encode_video_delete` löscht einen Job, wo der Anbieter das kann, und `encode_video_file` und `decode_video_file` machen aus einer Datei-ID einen Link.
- **Bilder sagen, wofür sie sind.** `VideoImage` trägt `VideoImageRole` `FirstFrame`, `LastFrame` oder `Reference`. `input_reference` von OpenAI war das erste Bild eines Sora-Videos und wird deshalb als `FirstFrame` dekodiert.
- **Ausgaben nennen, wo die Datei liegt.** `VideoJob.outputs` hält jede Datei als Link, als Bytes in der Antwort oder als Datei-ID, mit ihrer `VideoVariant` (`video`, `thumbnail`, `spritesheet`). Ein Link, der mit `/` beginnt, ist ein Pfad beim Anbieter.
- **Kein Anbieter dieser Form.** OpenAI hat seine Videomodelle am 2026-09-24 eingestellt. Apps sprechen die API weiter, deshalb antwortet Kaeso in dieser Form; `VideosProfile::OPENAI` bleibt für Round Trips, und nur Together AI bekommt Jobs in einer Form davon.
- **Größen gehen nur dorthin, wo der Anbieter sie listet.** Eine `size` wird nur dann zu Auflösung und Seitenverhältnis eines anderen Anbieters, wenn der Anbieter beides dokumentiert, so wie 1280x720 720p bei 16:9 ist. Kaeso skaliert und wandelt nichts um.

## Lesart der Tabellen

- **typisiert**: ein typisiertes KPP-Feld.
- **Extras**: `Extras::fields`, gebunden an `openai_videos`; andere Dialekte lehnen sie ab ([Extras](cross-format.md#extras)).
- **Form**: eine codec-eigene Markierung in `Extras::form`.

## Anfrage

| Wire-Feld | KPP | Klasse |
| --- | --- | --- |
| `model` | `VideoRequest.model` | typisiert |
| `prompt` | `prompt` | typisiert |
| Upload `input_reference` | `images[]` `FirstFrame`, Bytes mit dem Dateinamen | typisiert, Form `body: multipart` |
| `input_reference.image_url` | `images[]` `FirstFrame`, ein Link oder die Bytes einer Data-URL | typisiert |
| `input_reference.file_id` | `Source::FileId`, Ursprung `openai_videos` | typisiert |
| `seconds`, eine Ziffernfolge | `seconds` | typisiert |
| `seconds`, eine Zahl | `seconds`, mit der Markierung `seconds_number` | typisiert, Form |
| `size` | `size`, `WIDTHxHEIGHT` | typisiert |
| jedes andere Feld | das Feld | Extras |

Der Body eines Remix hält nur `prompt`; der Pfad nennt das Video. `openai_videos::decode_remix` dekodiert ihn mit leerem Modell, das der Aufrufer mit dem Modell des Videos und der Job-ID seines Anbieters (`remix_of`) füllt.

## Antwort

| `VideoResource` | KPP |
| --- | --- |
| `id` | `VideoJob.id` |
| `status` (`queued`, `in_progress`, `completed`, `failed`) | `status` |
| `progress` | `progress`; als 100 geschrieben für einen fertigen Job, der keinen nennt, sonst als 0 |
| `created_at`, `completed_at`, `expires_at` | dieselben, Unix-Sekunden |
| `prompt`, `model`, `remixed_from_video_id` | `prompt`, `model`, `remixed_from` |
| `seconds` | `seconds`, als String geschrieben |
| `size` | `size`, oder eine Größe aus `resolution` und `aspect_ratio`, oder `""`, weil das Feld Pflicht ist |
| `error` `{code, message, param, type}` | `error` |

`openai_videos::encode_list` schreibt `VideoListResource` mit `first_id`, `last_id` und `has_more`; `encode_deleted` schreibt `DeletedVideoResource`. `decode_list_query` liest `after`, `limit` (0 bis 100, standardmäßig 20) und `order`; `decode_content_query` liest `variant`. Fehler sind `ErrorResponse` von OpenAI, wie bei den anderen OpenAI-Dialekten.

## Together AI

`VideosProfile::TOGETHER` sendet `POST /v2/videos` und fragt `GET /v2/videos/{id}`; Together dokumentiert kein Löschen.

| KPP | Together AI |
| --- | --- |
| `size` | `width`, `height` |
| `resolution`, `aspect_ratio` | `resolution`, `ratio` |
| `seconds` | `seconds`, ein String |
| `seed`, `negative_prompt`, `audio` | `seed`, `negative_prompt`, `generate_audio` |
| Bilder `FirstFrame`, `LastFrame` | `media.frame_images[]` `{input_image, frame}` |
| Bilder `Reference` | `media.reference_images[]` |
| Antwort `status` `in_progress`, `completed`, `failed` | `status`; Together dokumentiert kein `queued` |
| Antwort `outputs.video_url` | ein Ausgabelink; `outputs.cost` bleibt in den Extras |

Together nimmt Bilder nur als Links (`input_image` ist „URL path to hosted image“), deshalb wird ein Upload abgelehnt (Parameter `input_reference`), ebenso ein Remix (Parameter `video_id`). Die übrigen Felder der App, als Extras behalten, erreichen Together unverändert, etwa `steps`; ein Feld von `input_reference` außer `image_url` wird abgelehnt.
