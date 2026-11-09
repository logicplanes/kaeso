use kaeso_kpp::{
    ErrorKind, GeneratedImage, ImageBackground, ImageEvent, ImageFormat, ImageOperation,
    ImageQuality, ImageSize, KppError, Origin, RawItem, Source, StreamEvent,
};
use serde_json::{Map, Value};

use super::DIALECT;
use super::response::{settings_of, usage_of, wire_usage};
use super::wire::ImageEventW;
use crate::codec::{EgressCtx, EncodeCtx, StreamDecoder, StreamEncoder};
use crate::sse::{SseEvent, SseItem, SseParser, frame_extra, write_event, write_frame_extra};
use crate::wire::{Ext, extra_for, extras_from};

const PARTIAL: &str = "partial_image";
const COMPLETED: &str = "completed";

pub(crate) struct Decoder {
    parser: SseParser,
    origin: Origin,
    failed: bool,
}

impl Decoder {
    pub(super) fn new(ctx: &EgressCtx) -> Decoder {
        Decoder {
            parser: SseParser::new(),
            origin: ctx.origin(DIALECT),
            failed: false,
        }
    }

    fn item(&mut self, item: SseItem, out: &mut Vec<StreamEvent>) {
        let SseItem::Event(ev) = item else {
            if let SseItem::Comment(text) = item
                && let Ok(raw) = crate::sse::comment_extra(&text)
            {
                out.push(StreamEvent::Extra(RawItem::new(self.origin.clone(), raw)));
            }
            return;
        };
        if ev.data.trim() == "[DONE]" {
            return;
        }
        match self.event(&ev) {
            Ok(Some(event)) => out.push(event),
            Ok(None) => match frame_extra(&ev) {
                Ok(raw) => out.push(StreamEvent::Extra(RawItem::new(self.origin.clone(), raw))),
                Err(e) => self.fail(out, &e.to_string()),
            },
            Err(err) => {
                self.failed = true;
                out.push(StreamEvent::Error(err.with_origin(self.origin.clone())));
            }
        }
    }

    fn fail(&mut self, out: &mut Vec<StreamEvent>, detail: &str) {
        self.failed = true;
        out.push(StreamEvent::Error(
            KppError::provider(format!(
                "The provider sent a streamed event that is not a valid {} event: {detail}.",
                DIALECT.display_name()
            ))
            .with_origin(self.origin.clone()),
        ));
    }

    fn event(&self, ev: &SseEvent) -> Result<Option<StreamEvent>, KppError> {
        let value: Value = match serde_json::from_str(&ev.data) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        let ty = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if ty == "error" {
            let error = value.get("error").cloned().unwrap_or(Value::Null);
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("The provider ended the stream with an error.")
                .to_owned();
            return Err(KppError::new(ErrorKind::ProviderError, message).with_raw(value));
        }
        let ty = ty.as_str();
        let stage = ty.rsplit('.').next().unwrap_or_default();
        let known = ty.starts_with("image_generation.") || ty.starts_with("image_edit.");
        if !known || !matches!(stage, PARTIAL | COMPLETED) {
            return Ok(None);
        }
        let Ok(w) = serde_json::from_value::<Ext<ImageEventW>>(value) else {
            return Ok(None);
        };
        let settings = settings_of(
            w.size.as_deref(),
            w.quality.as_deref(),
            w.output_format.as_deref(),
            w.background.as_deref(),
        );
        let media = MEDIA_KEYS
            .iter()
            .find_map(|k| w.extra.get(*k)?.parse::<String>().ok())
            .or_else(|| {
                settings
                    .format
                    .as_ref()
                    .and_then(ImageFormat::media_type)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "image/png".to_owned());
        let Some(data) = &w.b64_json else {
            return Ok(None);
        };
        let source = Source::base64(media, data).map_err(|_| {
            KppError::provider("The provider sent an image that is not valid base64.")
        })?;
        let image = GeneratedImage::new(source);
        let mut event = if stage == PARTIAL {
            ImageEvent::partial(w.partial_image_index.unwrap_or(0), image)
        } else {
            ImageEvent::finished(image, w.usage.as_ref().map(|u| usage_of(u, &self.origin)))
        };
        event.created = w.created_at;
        event.settings = settings;
        event.extras = extras_from(&self.origin, w.extra.clone(), w.verbatim.clone());
        Ok(Some(StreamEvent::Image(event)))
    }
}

const MEDIA_KEYS: [&str; 2] = ["media_type", "mime_type"];

impl StreamDecoder for Decoder {
    fn feed(&mut self, bytes: &[u8]) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        if self.failed {
            return out;
        }
        match self.parser.feed(bytes) {
            Ok(items) => {
                for item in items {
                    if self.failed {
                        break;
                    }
                    self.item(item, &mut out);
                }
            }
            Err(e) => {
                self.failed = true;
                out.push(StreamEvent::Error(
                    KppError::from(e).with_origin(self.origin.clone()),
                ));
            }
        }
        out
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        if self.failed {
            return out;
        }
        match self.parser.finish() {
            Ok(items) => {
                for item in items {
                    self.item(item, &mut out);
                }
            }
            Err(e) => out.push(StreamEvent::Error(
                KppError::from(e).with_origin(self.origin.clone()),
            )),
        }
        self.failed = true;
        out
    }
}

pub(crate) struct Encoder {
    prefix: &'static str,
    created: Option<u64>,
}

impl Encoder {
    pub(super) fn new(operation: ImageOperation, created: Option<u64>) -> Encoder {
        Encoder {
            prefix: match operation {
                ImageOperation::Edit => "image_edit",
                _ => "image_generation",
            },
            created,
        }
    }

    pub(super) fn for_ctx(ctx: &EncodeCtx) -> Encoder {
        let operation = match ctx.request.as_deref() {
            Some(kaeso_kpp::Request::Image(r)) => r.operation,
            _ => ImageOperation::Generate,
        };
        Encoder::new(operation, ctx.created_at)
    }

    // Every field but `usage` is required in `ImageGenPartialImageEvent` and
    // `ImageGenCompletedEvent`, so an event from another format that reports no setting says
    // `auto`; OpenAI's own events keep what they carried, as the spec's examples leave some out.
    pub(super) fn event_bytes(&mut self, e: &ImageEvent) -> Vec<u8> {
        let own = e.extras.is_from(DIALECT);
        let stage = if e.partial.is_some() {
            PARTIAL
        } else {
            COMPLETED
        };
        let ty = format!("{}.{stage}", self.prefix);
        let mut m = Map::new();
        m.insert("type".to_owned(), ty.clone().into());
        if let Source::Base64(b) = &e.image.source {
            m.insert("b64_json".to_owned(), b.to_base64().into());
        }
        if let Some(i) = e.partial {
            m.insert("partial_image_index".to_owned(), i.into());
        }
        let s = &e.settings;
        let format = s.format.clone().or_else(|| match &e.image.source {
            Source::Base64(b) if !own => ImageFormat::from_media_type(&b.media_type),
            _ => None,
        });
        let fields = [
            ("size", s.size.as_ref().map(ImageSize::as_wire)),
            ("quality", s.quality.as_ref().map(ImageQuality::as_wire)),
            (
                "background",
                s.background.as_ref().map(ImageBackground::as_wire),
            ),
            ("output_format", format.as_ref().map(ImageFormat::as_wire)),
        ];
        let created = if own {
            e.created
        } else {
            Some(e.created.or(self.created).unwrap_or(0))
        };
        if let Some(created) = created {
            m.insert("created_at".to_owned(), created.into());
        }
        for (key, value) in fields {
            let fallback = if key == "output_format" {
                "png"
            } else {
                "auto"
            };
            if let Some(value) = value.or((!own).then_some(fallback)) {
                m.insert(key.to_owned(), value.into());
            }
        }
        if e.partial.is_none()
            && let Some(usage) = e.usage.as_ref().and_then(wire_usage)
        {
            m.insert("usage".to_owned(), usage);
        }
        if own {
            for (k, v) in extra_for(&e.extras, DIALECT) {
                if let Ok(value) = v.to_value() {
                    m.entry(k).or_insert(value);
                }
            }
        }
        let data = Value::Object(m).to_string();
        let mut out = Vec::new();
        write_event(&mut out, Some(&ty), &data);
        out
    }
}

impl StreamEncoder for Encoder {
    fn encode(&mut self, event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        match event {
            StreamEvent::Image(e) => Ok(self.event_bytes(e)),
            StreamEvent::Error(err) => {
                let (_, ty, code) = crate::openai_responses::error::wire_for_kind(err);
                let body = serde_json::json!({
                    "type": "error",
                    "error": {"type": ty, "code": code, "message": err.message, "param": err.param},
                });
                let mut out = Vec::new();
                write_event(&mut out, Some("error"), &body.to_string());
                Ok(out)
            }
            StreamEvent::Extra(item) if item.origin.is(DIALECT) => {
                let mut out = Vec::new();
                write_frame_extra(&mut out, item.raw.as_raw()).map_err(|e| {
                    KppError::provider(format!("Kaeso could not write a streamed event ({e})."))
                })?;
                Ok(out)
            }
            _ => Ok(Vec::new()),
        }
    }

    fn finish(&mut self) -> Result<Vec<u8>, KppError> {
        Ok(Vec::new())
    }
}
