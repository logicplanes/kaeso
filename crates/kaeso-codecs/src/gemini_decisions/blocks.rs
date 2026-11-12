//! Content blocks: the `text`, `image`, `audio`, `video` and `document` blocks of the input and
//! the text blocks of question instructions, as the Interactions API writes them.

use kaeso_kpp::{
    AudioBlock, Block, DialectId, DocumentBlock, Extras, ImageBlock, KppError, Origin, RawItem,
    RawJson, Source, TextBlock, VideoBlock,
};
use serde_json::value::RawValue;

use super::DIALECT;
use super::wire::{MediaWire, TextWire};
use crate::gemini::parts::{Media, file_source, mime_of_url};
use crate::wire::{
    B64, Ext, extra_for, extras_from, parse_data_url, to_body, type_tag, verbatim_from_form,
    without_position,
};

/// The MIME type a block given by `uri` was sent with.
const FORM_MIME: &str = "mime_type";
/// The image or video field that sets how many tokens the media takes.
pub(super) const RESOLUTION: &str = "resolution";
/// OpenAI Decisions' image field of the same purpose.
const DETAIL: &str = "detail";

fn invalid(message: impl Into<String>, param: impl Into<String>) -> KppError {
    KppError::invalid_request(message).with_param(param)
}

fn unsupported(feature: &str, message: impl Into<String>, param: impl Into<String>) -> KppError {
    KppError::unsupported(feature, message.into()).with_param(param)
}

pub(super) fn json(text: String) -> Result<RawJson, KppError> {
    RawJson::from_string(text).map_err(|e| {
        KppError::provider(format!(
            "Kaeso wrote a Gemini Decisions value that is not JSON ({e}). This is a bug in Kaeso; please report it."
        ))
    })
}

pub(super) fn ser<T: serde::Serialize>(value: &T) -> Result<RawJson, KppError> {
    json(
        String::from_utf8(to_body(value)?)
            .map_err(|_| KppError::provider("Kaeso wrote a value that is not UTF-8."))?,
    )
}

/// Where a block stands: the input takes every block type, instructions take text only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Place {
    Input,
    Instructions,
}

fn media_of(ty: &str) -> Option<Media> {
    Some(match ty {
        "image" => Media::Image,
        "audio" => Media::Audio,
        "video" => Media::Video,
        "document" => Media::Document,
        _ => return None,
    })
}

/// Reads one content block; a block of a type the format doesn't document is kept whole, bound
/// to this format.
pub(super) fn decode_block(
    raw: &RawValue,
    place: Place,
    origin: &Origin,
    param: &str,
) -> Result<Block, KppError> {
    if !raw.get().starts_with('{') {
        return Err(invalid("Every content block must be an object.", param));
    }
    let ty = type_tag(raw).ok().flatten();
    let bad = |what: &str, e: serde_json::Error| {
        invalid(
            format!("The `{what}` block is not valid: {}.", without_position(&e)),
            param,
        )
    };
    match ty.as_deref() {
        Some("text") => {
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<TextWire>::from_raw(raw).map_err(|e| bad("text", e))?;
            let mut b = TextBlock::new(w.text);
            b.extras = extras_from(origin, extra, verbatim);
            Ok(Block::Text(b))
        }
        Some(t @ ("image" | "audio" | "video" | "document")) if place == Place::Input => {
            let media = media_of(t).unwrap_or(Media::Document);
            let Ext {
                known: w,
                extra,
                verbatim,
            } = Ext::<MediaWire>::from_raw(raw).map_err(|e| bad(t, e))?;
            let mut extras = extras_from(origin, extra, verbatim);
            let source = match (w.data, w.uri) {
                (Some(data), None) => {
                    let Some(mime) = w.mime_type else {
                        return Err(invalid(
                            format!("An `{t}` block with `data` needs its `mime_type`."),
                            format!("{param}.mime_type"),
                        ));
                    };
                    data.into_source(mime, &format!("{param}.data"))?
                }
                (None, Some(uri)) => {
                    if let Some(mime) = w.mime_type {
                        extras.form.insert(FORM_MIME.to_owned(), mime.into());
                    }
                    file_source(origin, &uri)
                }
                (Some(_), Some(_)) => {
                    return Err(invalid(
                        format!("An `{t}` block takes `data` or `uri`, not both."),
                        param,
                    ));
                }
                (None, None) => {
                    return Err(invalid(
                        format!("An `{t}` block needs `data` or `uri`."),
                        param,
                    ));
                }
            };
            Ok(media_block(media, source, extras))
        }
        Some(t @ ("image" | "audio" | "video" | "document")) => Err(invalid(
            format!("Question instructions take text blocks only, not an `{t}` block."),
            param,
        )),
        _ => Ok(Block::Unknown(RawItem::new(
            origin.clone(),
            RawJson::from(raw),
        ))),
    }
}

fn media_block(media: Media, source: Source, extras: Extras) -> Block {
    match media {
        Media::Image => {
            let mut b = ImageBlock::new(source);
            b.extras = extras;
            Block::Image(b)
        }
        Media::Audio => {
            let mut b = AudioBlock::new(source);
            b.extras = extras;
            Block::Audio(b)
        }
        Media::Video => {
            let mut b = VideoBlock::new(source);
            b.extras = extras;
            Block::Video(b)
        }
        Media::Document => {
            let mut b = DocumentBlock::new(source);
            b.extras = extras;
            Block::Document(b)
        }
    }
}

/// A text block.
pub(super) fn text_block(text: &str, extras: &Extras) -> Result<RawJson, KppError> {
    let w = TextWire {
        ty: "text".to_owned(),
        text: text.to_owned(),
    };
    ser(&Ext::with(
        w,
        extra_for(extras, DIALECT),
        verbatim_from_form(extras, DIALECT, ""),
    ))
}

/// A media block. Images go inline or by URI as they came; `dry` leaves the bytes out, for a
/// check that writes nothing.
pub(super) fn media_json(
    kind: Media,
    source: &Source,
    extras: &Extras,
    dry: bool,
    param: &str,
) -> Result<RawJson, KppError> {
    let own = extras.is_from(DIALECT);
    let name = kind_name(kind);
    let mut w = MediaWire {
        ty: name.to_owned(),
        ..MediaWire::default()
    };
    match source {
        Source::Base64(b) => {
            w.data = Some(if dry {
                B64::Text(String::new())
            } else {
                B64::Bytes(b.data.clone())
            });
            w.mime_type = Some(b.media_type.clone());
        }
        Source::Url(url) if crate::wire::is_data_url(url) => {
            let Some(d) = parse_data_url(url) else {
                return Err(unsupported(
                    &format!("{name} data URL that is not base64"),
                    format!("Gemini takes inline {name}s as base64; send the {name} as base64."),
                    param,
                ));
            };
            w.data = Some(B64::Text(if dry {
                String::new()
            } else {
                d.data.to_owned()
            }));
            w.mime_type = Some(d.media_type.to_owned());
        }
        Source::Url(url) => {
            w.mime_type = url_mime(url, kind, extras, own, param)?;
            w.uri = Some(url.clone());
        }
        Source::FileId(f) if f.origin.is(DIALECT) || f.origin.is(DialectId::Gemini) => {
            w.mime_type = own
                .then(|| extras.form_str(DIALECT, FORM_MIME).map(str::to_owned))
                .flatten();
            w.uri = Some(f.id.clone());
        }
        Source::Text(_) => {
            return Err(unsupported(
                "text document",
                "Gemini's Decisions API takes PDF documents, not text ones; put the text in a text block.",
                param,
            ));
        }
        _ => {
            return Err(unsupported(
                &format!("{name} source"),
                format!("Gemini takes {name}s as base64, an https URL or a Files API URI."),
                param,
            ));
        }
    }
    let mut extra = extra_for(extras, DIALECT);
    if let Some(resolution) = resolution_of(kind, extras) {
        extra.insert(RESOLUTION.to_owned(), resolution);
    }
    ser(&Ext::with(
        w,
        extra,
        verbatim_from_form(extras, DIALECT, ""),
    ))
}

fn kind_name(kind: Media) -> &'static str {
    match kind {
        Media::Image => "image",
        Media::Audio => "audio",
        Media::Video => "video",
        Media::Document => "document",
    }
}

// The Decisions API takes Files API URIs and https URLs, but neither YouTube URLs nor Cloud
// Storage URIs (Decisions guide, "Limitations").
fn url_mime(
    url: &str,
    kind: Media,
    extras: &Extras,
    own: bool,
    param: &str,
) -> Result<Option<String>, KppError> {
    let name = kind_name(kind);
    if crate::media::is_youtube_url(url) {
        return Err(unsupported(
            "YouTube URL",
            "Gemini's Decisions API takes no YouTube URLs; send the video as an https URL to its file, a Files API URI or base64.",
            param,
        ));
    }
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    if lower.starts_with("gs://") {
        return Err(unsupported(
            "Cloud Storage URI",
            "Gemini's Decisions API takes no `gs://` URIs; send the file as an https URL, a Files API URI or base64.",
            param,
        ));
    }
    if !lower.starts_with("https://") {
        return Err(unsupported(
            &format!("{name} URL"),
            format!(
                "Gemini's Decisions API takes https URLs only; send the {name} as base64 or from an https URL."
            ),
            param,
        ));
    }
    if own {
        return Ok(extras.form_str(DIALECT, FORM_MIME).map(str::to_owned));
    }
    match mime_of_url(url, kind) {
        Some(m) => Ok(Some(m.to_owned())),
        None => Err(unsupported(
            &format!("{name} URL without a known file extension"),
            format!(
                "Gemini needs the MIME type of a {name} URL, which Kaeso reads from its file extension; send the {name} as base64 instead."
            ),
            param,
        )),
    }
}

// OpenAI Decisions' `detail` of `low` or `high` becomes the `resolution` of the same name; `auto`
// and `original` have no counterpart and are left out with the other hints.
fn resolution_of(kind: Media, extras: &Extras) -> Option<RawJson> {
    if kind != Media::Image || extras.is_from(DIALECT) {
        return None;
    }
    let detail = extras.fields_for(DialectId::OpenAiDecisions)?.get(DETAIL)?;
    match detail.parse::<String>().ok()?.as_str() {
        "low" | "high" => Some(detail.clone()),
        _ => None,
    }
}

/// The `detail` an image of this format takes to OpenAI Decisions: its `resolution` of `low` or
/// `high`.
pub(crate) fn detail_of(extras: &Extras) -> Option<RawJson> {
    let resolution = extras.fields_for(DIALECT)?.get(RESOLUTION)?;
    match resolution.parse::<String>().ok()?.as_str() {
        "low" | "high" => Some(resolution.clone()),
        _ => None,
    }
}
