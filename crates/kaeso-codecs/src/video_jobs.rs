//! What the video dialects share: reading and writing JSON objects field by field, keeping
//! every field a dialect doesn't type in its extras, sizes and resolutions, and the codec
//! methods video dialects answer the same way.

use indexmap::IndexMap;
use kaeso_kpp::{
    DialectId, Extras, ImageSize, KppError, Origin, RawJson, Request, RequestKind, Source,
    StreamEvent, VideoImage, VideoImageRole, VideoJob, VideoRequest,
};
use serde_json::{Map, Value};

use crate::codec::{StreamDecoder, StreamEncoder};
use crate::cross::check_extras;

/// A JSON object read field by field: each `take_*` removes a field, and what is left at the
/// end becomes the dialect's extras.
pub(crate) struct Fields {
    map: IndexMap<String, Value>,
    dialect: DialectId,
    what: &'static str,
}

impl Fields {
    /// Reads `body` as one JSON object, refusing a repeated key.
    pub(crate) fn parse(
        body: &[u8],
        dialect: DialectId,
        what: &'static str,
    ) -> Result<Self, KppError> {
        let invalid = |detail: String| {
            KppError::invalid_request(format!(
                "The {what} is not a valid {} {what}: {detail}.",
                dialect.display_name()
            ))
        };
        let raw: &serde_json::value::RawValue =
            serde_json::from_slice(body).map_err(|e| invalid(crate::wire::without_position(&e)))?;
        let Some(entries) = crate::wire::entries_of(raw) else {
            return Err(invalid("it is not a JSON object".to_owned()));
        };
        let mut map = IndexMap::new();
        for (key, value) in entries {
            let value: Value = serde_json::from_str(value.get())
                .map_err(|e| invalid(crate::wire::without_position(&e)))?;
            map.insert(key.into_owned(), value);
        }
        Ok(Fields { map, dialect, what })
    }

    pub(crate) fn from_map(
        map: Map<String, Value>,
        dialect: DialectId,
        what: &'static str,
    ) -> Self {
        Fields {
            map: map.into_iter().collect(),
            dialect,
            what,
        }
    }

    fn wrong(&self, key: &str, expected: &str) -> KppError {
        KppError::invalid_request(format!(
            "`{key}` in the {} {} must be {expected}.",
            self.dialect.display_name(),
            self.what
        ))
        .with_param(key.to_owned())
    }

    pub(crate) fn take(&mut self, key: &str) -> Option<Value> {
        match self.map.shift_remove(key) {
            Some(Value::Null) | None => None,
            Some(v) => Some(v),
        }
    }

    pub(crate) fn peek(&self, key: &str) -> Option<&Value> {
        self.map.get(key)
    }

    pub(crate) fn str(&mut self, key: &str) -> Result<Option<String>, KppError> {
        match self.take(key) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s)),
            Some(_) => Err(self.wrong(key, "a string")),
        }
    }

    pub(crate) fn bool(&mut self, key: &str) -> Result<Option<bool>, KppError> {
        match self.take(key) {
            None => Ok(None),
            Some(Value::Bool(b)) => Ok(Some(b)),
            Some(_) => Err(self.wrong(key, "`true` or `false`")),
        }
    }

    pub(crate) fn i64(&mut self, key: &str) -> Result<Option<i64>, KppError> {
        match self.take(key) {
            None => Ok(None),
            Some(Value::Number(n)) => n
                .as_i64()
                .map(Some)
                .ok_or_else(|| self.wrong(key, "a whole number")),
            Some(_) => Err(self.wrong(key, "a whole number")),
        }
    }

    pub(crate) fn u32(&mut self, key: &str) -> Result<Option<u32>, KppError> {
        match self.take(key) {
            None => Ok(None),
            Some(Value::Number(n)) => n
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .map(Some)
                .ok_or_else(|| self.wrong(key, "a whole number")),
            Some(_) => Err(self.wrong(key, "a whole number")),
        }
    }

    /// A count of seconds written as a number with no fraction, or as a string of digits.
    pub(crate) fn seconds(&mut self, key: &str) -> Result<Option<(u32, bool)>, KppError> {
        match self.take(key) {
            None => Ok(None),
            Some(Value::Number(n)) => n
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .map(|n| Some((n, false)))
                .ok_or_else(|| self.wrong(key, "a whole number of seconds")),
            Some(Value::String(s)) => digits(&s)
                .map(|n| Some((n, true)))
                .ok_or_else(|| self.wrong(key, "a whole number of seconds")),
            Some(_) => Err(self.wrong(key, "a whole number of seconds")),
        }
    }

    pub(crate) fn object(&mut self, key: &str) -> Result<Option<Fields>, KppError> {
        match self.take(key) {
            None => Ok(None),
            Some(Value::Object(m)) => Ok(Some(Fields::from_map(m, self.dialect, self.what))),
            Some(_) => Err(self.wrong(key, "an object")),
        }
    }

    pub(crate) fn array(&mut self, key: &str) -> Result<Option<Vec<Value>>, KppError> {
        match self.take(key) {
            None => Ok(None),
            Some(Value::Array(a)) => Ok(Some(a)),
            Some(_) => Err(self.wrong(key, "a list")),
        }
    }

    /// The fields no `take_*` read, as extras of the dialect.
    pub(crate) fn rest(self, origin: &Origin) -> Extras {
        let mut extras = Extras::new(origin.clone());
        self.rest_into(&mut extras, "");
        extras
    }

    /// Adds the fields no `take_*` read to `extras`, under `prefix.` when it isn't empty.
    pub(crate) fn rest_into(self, extras: &mut Extras, prefix: &str) {
        for (key, value) in self.map {
            let key = if prefix.is_empty() {
                key
            } else {
                format!("{prefix}.{key}")
            };
            if let Ok(raw) = RawJson::from_serialize(&value) {
                extras.fields.insert(key, raw);
            }
        }
    }
}

/// A job id as one path segment: ids are opaque, so anything that would end the segment is
/// escaped.
pub(crate) fn path_segment(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for b in id.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub(crate) fn digits(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Writes a request's or a reply's extras of `dialect` back into `out`, top-level fields only
/// unless `prefix` names a nested object.
pub(crate) fn put_extras(
    out: &mut Map<String, Value>,
    extras: &Extras,
    dialect: DialectId,
    prefix: &str,
) {
    if !extras.is_from(dialect) {
        return;
    }
    for (key, raw) in extras.body_fields() {
        let name = if prefix.is_empty() {
            (!key.contains('.')).then_some(key)
        } else {
            key.strip_prefix(prefix)
                .and_then(|k| k.strip_prefix('.'))
                .filter(|k| !k.contains('.'))
        };
        if let Some(name) = name
            && let Ok(v) = raw.to_value()
        {
            out.insert(name.to_owned(), v);
        }
    }
}

pub(crate) fn put(out: &mut Map<String, Value>, key: &str, value: Option<impl Into<Value>>) {
    if let Some(v) = value {
        out.insert(key.to_owned(), v.into());
    }
}

pub(crate) fn body(out: &Map<String, Value>) -> Result<Vec<u8>, KppError> {
    crate::wire::to_body(out)
}

/// Refuses the extras of another dialect, before anything is sent.
pub(crate) fn check_foreign_extras(r: &VideoRequest, target: DialectId) -> Result<(), KppError> {
    check_extras(&r.extras, target, "", "request")?;
    for image in &r.images {
        check_extras(&image.extras, target, "input_reference", "request")?;
    }
    Ok(())
}

/// The request as a video request, or the error for any other kind.
pub(crate) fn video_request(req: &Request, dialect: DialectId) -> Result<&VideoRequest, KppError> {
    match req {
        Request::Video(r) => Ok(r),
        other => Err(not_videos(dialect, other.kind())),
    }
}

pub(crate) fn not_videos(dialect: DialectId, kind: RequestKind) -> KppError {
    KppError::unsupported_by(
        format!("{kind:?} request"),
        dialect,
        "It carries video jobs only.",
    )
}

/// The error a provider gets for a field it doesn't take, before anything is sent.
pub(crate) fn refused(
    dialect: DialectId,
    provider: &str,
    feature: &str,
    param: &str,
    hint: &str,
) -> KppError {
    KppError::unsupported_by(
        feature.to_owned(),
        dialect,
        format!("{provider} does not take {feature}; {hint}"),
    )
    .with_param(param.to_owned())
}

/// The provider's reply as a JSON object, or the error the reply's form calls for.
pub(crate) fn provider_object(body: &[u8], dialect: DialectId) -> Result<Fields, KppError> {
    Fields::parse(body, dialect, "reply").map_err(|e| {
        KppError::provider(format!(
            "The provider sent a reply Kaeso cannot read as {}: {}",
            dialect.display_name(),
            e.message
        ))
    })
}

/// An image a provider takes as a link: a URL, or a data URL made from inline bytes.
pub(crate) fn image_link(image: &VideoImage) -> Option<String> {
    match &image.source {
        Source::Url(u) => Some(u.clone()),
        Source::Base64(b) => Some(format!("data:{};base64,{}", b.media_type, b.data)),
        _ => None,
    }
}

/// A link a provider sent, kept as a URL, or as inline bytes when it is a data URL.
pub(crate) fn link_source(url: String) -> Source {
    match crate::wire::parse_data_url(&url) {
        Some(d) => Source::base64(d.media_type, d.data),
        None => Source::Url(url),
    }
}

/// Only first frames, last frames and references Kaeso knows: anything else in `images` is
/// refused with the role the provider lacks.
pub(crate) fn check_roles(
    r: &VideoRequest,
    dialect: DialectId,
    provider: &str,
    allowed: &[VideoImageRole],
) -> Result<(), KppError> {
    for image in &r.images {
        if !allowed.contains(&image.role) {
            let what = match image.role {
                VideoImageRole::FirstFrame => "a first frame",
                VideoImageRole::LastFrame => "a last frame",
                _ => "reference images",
            };
            return Err(refused(
                dialect,
                provider,
                what,
                "input_reference",
                "send the video request without it.",
            ));
        }
    }
    Ok(())
}

/// Refuses each typed field of `r` that the provider doesn't take, naming it in `param`.
pub(crate) fn refuse_unset(
    r: &VideoRequest,
    dialect: DialectId,
    provider: &str,
    takes: &[&str],
) -> Result<(), KppError> {
    let set: [(&str, &str, bool); 9] = [
        ("seconds", "seconds", r.seconds.is_some()),
        ("size", "size", r.size.is_some()),
        ("aspect_ratio", "aspect_ratio", r.aspect_ratio.is_some()),
        ("resolution", "resolution", r.resolution.is_some()),
        ("seed", "seed", r.seed.is_some()),
        (
            "negative_prompt",
            "negative_prompt",
            r.negative_prompt.is_some(),
        ),
        ("audio", "audio", r.audio.is_some()),
        ("n", "n", r.count.is_some_and(|n| n != 1)),
        ("user", "user", r.user.is_some()),
    ];
    for (name, param, present) in set {
        if present && !takes.contains(&name) {
            return Err(refused(
                dialect,
                provider,
                &format!("`{name}`"),
                param,
                "remove it.",
            ));
        }
    }
    Ok(())
}

/// The width and height of a resolution tier and an aspect ratio, where the tier names the
/// short side, as `720p` at `16:9` is 1280x720.
pub(crate) fn size_of(resolution: &str, aspect: &str) -> Option<ImageSize> {
    let short: u32 = resolution
        .strip_suffix(['p', 'P'])
        .and_then(|d| d.parse().ok())?;
    let (w, h) = aspect.split_once(':')?;
    let (w, h): (u32, u32) = (w.parse().ok()?, h.parse().ok()?);
    if w == 0 || h == 0 {
        return None;
    }
    let long = short.checked_mul(w.max(h))? / w.min(h);
    Some(if w >= h {
        ImageSize::pixels(long, short)
    } else {
        ImageSize::pixels(short, long)
    })
}

/// The resolution tier of a size, named by its short side, as 1280x720 is `720p`.
pub(crate) fn tier_of(size: &ImageSize) -> Option<String> {
    let (w, h) = size.dimensions()?;
    Some(format!("{}p", w.min(h)))
}

/// The aspect ratio of a size in lowest terms, as 1280x720 is `16:9`.
pub(crate) fn aspect_of(size: &ImageSize) -> Option<String> {
    let (w, h) = size.dimensions()?;
    let g = gcd(w, h);
    (g > 0).then(|| format!("{}:{}", w / g, h / g))
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Unix seconds of an RFC 3339 time such as `2026-05-29T02:17:56Z`, in UTC or with an offset.
pub(crate) fn unix_of_rfc3339(s: &str) -> Option<i64> {
    let (date, rest) = s.split_once('T')?;
    let mut d = date.split('-');
    let (y, m, day): (i64, i64, i64) = (
        d.next()?.parse().ok()?,
        d.next()?.parse().ok()?,
        d.next()?.parse().ok()?,
    );
    let i = rest.find(['Z', 'z', '+', '-'])?;
    let (clock, offset) = (rest.get(..i)?, rest.get(i..)?);
    let clock = clock.split('.').next()?;
    let mut c = clock.split(':');
    let (hh, mm, ss): (i64, i64, i64) = (
        c.next()?.parse().ok()?,
        c.next()?.parse().ok()?,
        c.next()?.parse().ok()?,
    );
    let offset_s = match offset {
        "Z" | "z" => 0,
        o => {
            let sign = if o.starts_with('-') { -1 } else { 1 };
            let (oh, om) = o.get(1..)?.split_once(':')?;
            sign * (oh.parse::<i64>().ok()? * 3600 + om.parse::<i64>().ok()? * 60)
        }
    };
    Some(days_from_civil(y, m, day) * 86_400 + hh * 3600 + mm * 60 + ss - offset_s)
}

// Howard Hinnant's days_from_civil (http://howardhinnant.github.io/date_algorithms.html).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The decoder and encoder of a video dialect's streams: video jobs never stream.
pub(crate) struct NoStream {
    pub(crate) origin: Origin,
    pub(crate) done: bool,
}

impl crate::codec::sealed::Sealed for NoStream {}

fn no_streaming(dialect: DialectId) -> KppError {
    KppError::unsupported_by(
        "streaming",
        dialect,
        "A video job answers with its status; ask for it again until it is done.",
    )
}

impl StreamDecoder for NoStream {
    fn feed(&mut self, _bytes: &[u8]) -> Vec<StreamEvent> {
        StreamDecoder::finish(self)
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        if std::mem::replace(&mut self.done, true) {
            return Vec::new();
        }
        vec![StreamEvent::Error(
            no_streaming(self.origin.dialect).with_origin(self.origin.clone()),
        )]
    }
}

impl StreamEncoder for NoStream {
    fn encode(&mut self, _event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        Err(no_streaming(self.origin.dialect))
    }

    fn finish(&mut self) -> Result<Vec<u8>, KppError> {
        Ok(Vec::new())
    }
}

pub(crate) fn no_stream(origin: Origin) -> Box<NoStream> {
    Box::new(NoStream {
        origin,
        done: false,
    })
}

/// The video job a reply carries, or the error for any other reply.
pub(crate) fn video_reply(
    reply: &kaeso_kpp::Reply,
    dialect: DialectId,
) -> Result<&VideoJob, KppError> {
    match reply {
        kaeso_kpp::Reply::Video(j) => Ok(j),
        other => Err(not_videos(dialect, other.kind())),
    }
}

pub(crate) fn raw(value: &Value) -> Option<RawJson> {
    RawJson::from_serialize(value).ok()
}

/// The model listing methods of a video dialect, which no app or provider lists models in:
/// the OpenAI form, as every video dialect's provider lists its models elsewhere.
macro_rules! openai_listing {
    ($d:expr) => {
        fn decode_models_query(query: &str) -> Result<kaeso_kpp::ModelsQuery, kaeso_kpp::KppError> {
            crate::openai_models::decode_query($d, query)
        }

        fn encode_models(list: &kaeso_kpp::ModelList) -> Result<Vec<u8>, kaeso_kpp::KppError> {
            crate::openai_models::encode_models(list)
        }

        fn encode_model(model: &kaeso_kpp::ModelInfo) -> Result<Vec<u8>, kaeso_kpp::KppError> {
            crate::openai_models::encode_model(model)
        }

        fn decode_models(
            status: u16,
            headers: &crate::codec::HeaderList,
            body: &[u8],
        ) -> Result<kaeso_kpp::ModelList, kaeso_kpp::KppError> {
            if !(200..300).contains(&status) {
                return Err(Self::decode_error(status, headers, body));
            }
            crate::openai_models::decode_models($d, body)
        }

        fn decode_model(
            status: u16,
            headers: &crate::codec::HeaderList,
            body: &[u8],
        ) -> Result<kaeso_kpp::ModelInfo, kaeso_kpp::KppError> {
            if !(200..300).contains(&status) {
                return Err(Self::decode_error(status, headers, body));
            }
            crate::openai_models::decode_model($d, body)
        }

        fn encode_model_request(model: &str) -> crate::codec::EncodedRequest {
            crate::openai_models::retrieve_request(model)
        }

        fn encode_models_request(
            query: &kaeso_kpp::ModelsQuery,
        ) -> Result<crate::codec::EncodedRequest, kaeso_kpp::KppError> {
            crate::openai_models::list_request($d, query)
        }
    };
}

pub(crate) use openai_listing;

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_kpp::RequestKind;

    const VIDEO_DIALECTS: [DialectId; 7] = [
        DialectId::OpenAiVideos,
        DialectId::XaiVideos,
        DialectId::MinimaxVideos,
        DialectId::ZaiVideos,
        DialectId::OpenRouterVideos,
        DialectId::Veo,
        DialectId::GeminiInteractions,
    ];

    #[test]
    fn video_dialects_list_models_in_the_openai_form_and_refuse_other_kinds() {
        use crate::codec::HeaderList;
        use crate::dispatch;
        use kaeso_kpp::{ModelInfo, ModelList, StreamMode};
        let list =
            br#"{"object":"list","data":[{"id":"m","object":"model","created":1,"owned_by":"o"}]}"#;
        let one = br#"{"id":"m","object":"model","created":1,"owned_by":"o"}"#;
        let h = HeaderList::new();
        for d in VIDEO_DIALECTS {
            let q = dispatch::decode_models_query(d, "").unwrap();
            let _ = dispatch::encode_models_request(d, &q);
            assert!(dispatch::encode_model_request(d, "m").is_ok(), "{d}");
            let models = dispatch::decode_models(d, 200, &h, list).unwrap();
            assert_eq!(models.models.len(), 1);
            assert!(dispatch::decode_models(d, 500, &h, b"{}").is_err());
            let model = dispatch::decode_model(d, 200, &h, one).unwrap();
            assert!(dispatch::decode_model(d, 404, &h, b"{}").is_err());
            assert!(dispatch::encode_models(d, &ModelList::new(vec![model.clone()])).is_ok());
            assert!(dispatch::encode_model(d, &ModelInfo::new("m")).is_ok());
            let ctx = round_trips::ctx();
            assert!(
                dispatch::decode_video_poll(d, 500, &h, br#"{"error":{"message":"no"}}"#, &ctx)
                    .is_err(),
                "{d}"
            );
            let err = dispatch::decode_error(d, 429, &h, br#"{"error":{"message":"slow down"}}"#);
            assert!(!dispatch::encode_error(d, &err).body.is_empty());
            let other = Request::Image(kaeso_kpp::ImageRequest::new(
                "m",
                kaeso_kpp::ImageOperation::Generate,
            ));
            let mut c = crate::codec::EgressCtx::new(&other);
            assert!(dispatch::check(d, &other, &c).is_err(), "{d}");
            assert!(dispatch::encode_request(d, &other, &mut c).is_err(), "{d}");
            assert!(
                dispatch::decode_request(d, RequestKind::Image, b"{}", &h).is_err(),
                "{d}"
            );
            let mut s = dispatch::stream_decoder(d, &ctx).unwrap();
            assert_eq!(s.feed(b"x").len(), 1);
            let mut e = dispatch::stream_encoder(
                d,
                &crate::codec::EncodeCtx::new(RequestKind::Video, StreamMode::Off),
            )
            .unwrap();
            assert!(e.finish().unwrap().is_empty());
            let image = kaeso_kpp::Reply::Image(kaeso_kpp::ImageResponse::new("m", Vec::new()));
            assert!(
                dispatch::encode_response(
                    d,
                    &image,
                    &crate::codec::EncodeCtx::new(RequestKind::Image, StreamMode::Off)
                )
                .is_err(),
                "{d}"
            );
        }
    }

    #[test]
    fn dialects_without_video_jobs_say_so() {
        use crate::codec::HeaderList;
        use crate::dispatch;
        let h = HeaderList::new();
        let ctx = round_trips::ctx();
        let d = DialectId::OpenAiChat;
        assert!(dispatch::encode_video_poll(d, "j", "m", &ctx).is_err());
        assert!(dispatch::decode_video_poll(d, 200, &h, b"{}", &ctx).is_err());
        assert!(dispatch::decode_video_poll(d, 500, &h, b"{}", &ctx).is_err());
        assert!(
            dispatch::encode_video_delete(d, "j", "m", &ctx)
                .unwrap()
                .is_none()
        );
        assert!(dispatch::encode_video_file(d, "f", &ctx).is_err());
        assert!(dispatch::decode_video_file(d, 200, &h, b"{}").is_err());
        assert!(dispatch::decode_video_file(d, 500, &h, b"{}").is_err());
        assert!(dispatch::encode_video_file(DialectId::XaiVideos, "f", &ctx).is_err());
        assert!(dispatch::decode_video_file(DialectId::XaiVideos, 200, &h, b"{}").is_err());
    }

    #[test]
    fn sizes_tiers_and_ratios_follow_each_other() {
        assert_eq!(size_of("720p", "16:9"), Some(ImageSize::pixels(1280, 720)));
        assert_eq!(
            size_of("1080P", "9:16"),
            Some(ImageSize::pixels(1080, 1920))
        );
        assert_eq!(size_of("2K", "16:9"), None);
        assert_eq!(size_of("720p", "0:9"), None);
        let s = ImageSize::pixels(1792, 1024);
        assert_eq!(tier_of(&s).as_deref(), Some("1024p"));
        assert_eq!(aspect_of(&s).as_deref(), Some("7:4"));
        assert_eq!(aspect_of(&ImageSize::Auto), None);
    }

    #[test]
    fn times_read_as_unix_seconds() {
        assert_eq!(unix_of_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix_of_rfc3339("2026-05-29T02:17:56Z"), Some(1_780_021_076));
        assert_eq!(
            unix_of_rfc3339("2026-05-29T04:17:56.123+02:00"),
            Some(1_780_021_076)
        );
        assert_eq!(unix_of_rfc3339("yesterday"), None);
    }

    #[test]
    fn fields_take_what_they_type_and_keep_the_rest() {
        let mut f = Fields::parse(
            br#"{"a":"x","n":3,"s":"8","b":true,"o":{"k":1},"l":[1],"rest":null,"z":2}"#,
            DialectId::OpenAiVideos,
            "request",
        )
        .unwrap();
        assert_eq!(f.str("a").unwrap().as_deref(), Some("x"));
        assert_eq!(f.u32("n").unwrap(), Some(3));
        assert_eq!(f.seconds("s").unwrap(), Some((8, true)));
        assert_eq!(f.bool("b").unwrap(), Some(true));
        assert!(f.object("o").unwrap().is_some());
        assert_eq!(f.array("l").unwrap().map(|l| l.len()), Some(1));
        assert_eq!(f.str("rest").unwrap(), None);
        assert_eq!(f.i64("z").unwrap(), Some(2));
        let extras = f.rest(&Origin::new(DialectId::OpenAiVideos));
        assert!(extras.fields.is_empty());
        let mut f =
            Fields::parse(br#"{"a":1,"x":[2]}"#, DialectId::OpenAiVideos, "request").unwrap();
        assert_eq!(f.str("a").unwrap_err().param.as_deref(), Some("a"));
        let extras = f.rest(&Origin::new(DialectId::OpenAiVideos));
        assert_eq!(extras.fields.len(), 1);
        assert!(Fields::parse(br#"{"a":1,"a":2}"#, DialectId::OpenAiVideos, "request").is_err());
        assert!(Fields::parse(b"[1]", DialectId::OpenAiVideos, "request").is_err());
    }
}

/// The round trips every video dialect's tests run over its fixtures.
#[cfg(test)]
pub(crate) mod round_trips {
    use kaeso_kpp::{Reply, Request, RequestKind, StreamMode, VideoJob};
    use serde_json::Value;

    use crate::codec::{Codec, EgressCtx, EncodeCtx, EncodedRequest, HeaderList};
    use crate::test_support::{assert_json_eq, fixtures};

    pub(crate) fn ctx() -> EgressCtx {
        EgressCtx::for_kind(RequestKind::Video, StreamMode::Off)
    }

    /// Every request fixture decodes, validates and encodes back to itself; returns what was
    /// sent, by fixture name.
    pub(crate) fn requests<C: Codec>(dir: &str) -> Vec<(String, EncodedRequest)> {
        requests_with::<C>(dir, &ctx())
    }

    pub(crate) fn requests_with<C: Codec>(
        dir: &str,
        base: &EgressCtx,
    ) -> Vec<(String, EncodedRequest)> {
        let mut out = Vec::new();
        for (name, body) in fixtures(dir, "requests", "json") {
            let req = C::decode_request(RequestKind::Video, &body, &HeaderList::new())
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            // Veo and Gemini name the model in the path, so their bodies carry none.
            if let Request::Video(r) = &req
                && r.operation == kaeso_kpp::VideoOperation::Generate
                && !r.model.is_empty()
            {
                req.validate().unwrap_or_else(|e| panic!("{name}: {e:?}"));
            }
            let mut ctx = base.clone();
            let enc = C::encode_request(&req, &mut ctx).unwrap_or_else(|e| panic!("{name}: {e:?}"));
            assert_eq!(enc.method, "POST", "{name}");
            assert_json_eq(&name, &body, &enc.body);
            out.push((name, enc));
        }
        out
    }

    /// Every reply that starts a job decodes and encodes back to itself.
    pub(crate) fn started<C: Codec>(dir: &str) -> Vec<(String, VideoJob)> {
        let mut out = Vec::new();
        for (name, body) in fixtures(dir, "responses", "json") {
            let reply = C::decode_response(200, &HeaderList::new(), &body, &ctx())
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            let back =
                C::encode_response(&reply, &EncodeCtx::new(RequestKind::Video, StreamMode::Off))
                    .unwrap();
            assert_json_eq(&name, &body, &back);
            let Reply::Video(job) = reply else {
                panic!("{name}")
            };
            out.push((name, job));
        }
        out
    }

    /// Every poll reply decodes, and `write` writes it back as the provider did.
    pub(crate) fn polls<C: Codec>(
        dir: &str,
        write: impl Fn(&VideoJob) -> Value,
    ) -> Vec<(String, VideoJob)> {
        polls_with::<C>(dir, &ctx(), write)
    }

    pub(crate) fn polls_with<C: Codec>(
        dir: &str,
        ctx: &EgressCtx,
        write: impl Fn(&VideoJob) -> Value,
    ) -> Vec<(String, VideoJob)> {
        let mut out = Vec::new();
        for (name, body) in fixtures(dir, "polls", "json") {
            let job = C::decode_video_poll(200, &HeaderList::new(), &body, ctx)
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            assert_json_eq(&name, &body, &serde_json::to_vec(&write(&job)).unwrap());
            out.push((name, job));
        }
        out
    }

    /// Every error fixture decodes to an error with the fixture's status. The error isn't
    /// written back: Kaeso never answers apps in a provider's video format.
    pub(crate) fn errors<C: Codec>(dir: &str) -> Vec<(String, kaeso_kpp::KppError)> {
        let mut out = Vec::new();
        for (name, body) in fixtures(dir, "errors", "json") {
            let status = crate::test_support::sidecar(dir, "errors", &name, "meta")
                .and_then(|m| m["status"].as_u64())
                .unwrap_or(400) as u16;
            let err = C::decode_error(status, &HeaderList::new(), &body);
            assert!(!err.message.is_empty(), "{name}");
            assert!(C::decode_response(status, &HeaderList::new(), &body, &ctx()).is_err());
            assert!(C::decode_video_poll(status, &HeaderList::new(), &body, &ctx()).is_err());
            out.push((name, err));
        }
        out
    }
}
