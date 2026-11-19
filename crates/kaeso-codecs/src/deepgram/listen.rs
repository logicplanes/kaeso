//! Pre-recorded transcription, `POST /v1/listen`
//! (<https://developers.deepgram.com/reference/speech-to-text/listen-pre-recorded>): the audio
//! as the body with its media type, or `{"url"}` as JSON, every option in the query string,
//! and the transcript under `results.channels[0].alternatives[0]`, timed words beside it and
//! timed `utterances` when asked for.

use kaeso_kpp::{
    AudioInput, Extras, KppError, RawJson, Reply, Request, Source, StreamMode,
    TimestampGranularity, TranscriptFormat, TranscriptSegment, TranscriptWord,
    TranscriptionOperation, TranscriptionRequest, TranscriptionResponse,
};
use serde_json::{Map, Value};

use super::{D, LISTEN_PATH, QUERY};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, JSON_MEDIA_TYPE};
use crate::video_jobs::{Fields, provider_object, put_extras, refused};

const PROVIDER: &str = "Deepgram";
const BODY: &str = "body";
const OCTET_STREAM: &str = "application/octet-stream";

fn no(feature: &str, param: &str, hint: &str) -> KppError {
    refused(D, PROVIDER, feature, param, hint)
}

fn bad(detail: &str) -> KppError {
    KppError::provider(format!(
        "Deepgram sent a transcript Kaeso cannot read: {detail}."
    ))
}

/// The query parameters Kaeso doesn't type, from a request's extras.
pub(super) fn query_extras(extras: &Extras) -> Vec<(String, String)> {
    if !extras.is_from(D) {
        return Vec::new();
    }
    let Some(Ok(Value::Array(pairs))) = extras.fields.get(QUERY).map(RawJson::to_value) else {
        return Vec::new();
    };
    pairs
        .iter()
        .filter_map(|p| match p.as_array()?.as_slice() {
            [Value::String(k), Value::String(v)] => Some((k.clone(), v.clone())),
            _ => None,
        })
        .collect()
}

/// Keeps the query parameters Kaeso doesn't type as one extras field.
pub(super) fn keep_query(extras: &mut Extras, rest: Vec<(String, String)>) {
    if rest.is_empty() {
        return;
    }
    let pairs: Vec<Value> = rest
        .into_iter()
        .map(|(k, v)| Value::Array(vec![k.into(), v.into()]))
        .collect();
    if let Ok(raw) = RawJson::from_serialize(&pairs) {
        extras.fields.insert(QUERY.to_owned(), raw);
    }
}

/// `path?name=value&…` with every name and value percent-encoded.
pub(super) fn with_query(path: &str, pairs: &[(String, String)]) -> String {
    let mut out = path.to_owned();
    for (i, (k, v)) in pairs.iter().enumerate() {
        out.push(if i == 0 { '?' } else { '&' });
        crate::models::percent_encode(&mut out, k);
        out.push('=');
        crate::models::percent_encode(&mut out, v);
    }
    out
}

pub(super) fn path_under(ctx: &EgressCtx, path: &str) -> String {
    match &ctx.base_path {
        Some(base) => format!("{base}{path}"),
        None => path.to_owned(),
    }
}

pub(super) fn decode_request(
    pairs: Vec<(String, String)>,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    let mut extras = Extras::new(kaeso_kpp::Origin::new(D));
    let mut model = None;
    let mut language = None;
    let mut rest = Vec::new();
    for (k, v) in pairs {
        match k.as_str() {
            "model" if model.is_none() => model = Some(v),
            "language" if language.is_none() => language = Some(v),
            _ => rest.push((k, v)),
        }
    }
    keep_query(&mut extras, rest);
    let content_type = headers.get("content-type").unwrap_or(OCTET_STREAM);
    let source = if crate::media::essence(content_type) == JSON_MEDIA_TYPE {
        let mut f = Fields::parse(body, D, "request")?;
        let url = f.str("url")?.ok_or_else(|| {
            KppError::invalid_request("Send the audio as the body, or JSON with a `url`.")
                .with_param("url")
        })?;
        f.rest_into(&mut extras, BODY);
        Source::Url(url)
    } else {
        Source::bytes(content_type, bytes::Bytes::copy_from_slice(body))
    };
    let mut r = TranscriptionRequest::new(
        model.unwrap_or_default(),
        TranscriptionOperation::Transcribe,
        AudioInput::new(source),
    );
    r.language = language;
    r.extras = extras;
    Ok(Request::Transcription(r))
}

pub(super) fn check(r: &TranscriptionRequest) -> Result<(), KppError> {
    crate::cross::check_extras(&r.extras, D, "", "request")?;
    if r.operation == TranscriptionOperation::Translate {
        return Err(no(
            "translations",
            "model",
            "transcribe the audio and translate the text instead.",
        ));
    }
    if !matches!(r.audio.source, Source::Base64(_) | Source::Url(_)) {
        return Err(no("file ids", "file", "upload the file or send a link."));
    }
    if let TranscriptFormat::Other(f) = r.format_or_default() {
        return Err(no(
            &format!("`response_format: {f}`"),
            "response_format",
            "use json, text, srt, vtt, verbose_json or diarized_json.",
        ));
    }
    // Deepgram's comparison with Whisper: no prompting and no temperature
    // (https://developers.deepgram.com/docs/migrating-from-openai-whisper-to-deepgram).
    let unsupported = [
        ("prompt", r.prompt.is_some()),
        ("temperature", r.temperature.is_some()),
        ("num_speakers", r.speakers.is_some()),
    ];
    if let Some((field, _)) = unsupported.iter().find(|(_, set)| *set) {
        return Err(no(&format!("`{field}`"), field, "remove it."));
    }
    Ok(())
}

fn query_of(r: &TranscriptionRequest) -> Vec<(String, String)> {
    let own = r.extras.is_from(D);
    let mut q = vec![("model".to_owned(), r.model.clone())];
    let on = |k: &str| (k.to_owned(), "true".to_owned());
    match &r.language {
        Some(l) => q.push(("language".to_owned(), l.clone())),
        // Deepgram transcribes English when no language is named; OpenAI detects it.
        None if !own => q.push(on("detect_language")),
        None => {}
    }
    if !own {
        let format = r.format_or_default();
        q.push(on("smart_format"));
        if format.needs_timing() || r.timestamps.contains(&TimestampGranularity::Segment) {
            q.push(on("utterances"));
        }
        let diarize = match r.diarize {
            Some(d) => d,
            None => format == TranscriptFormat::DiarizedJson,
        };
        if diarize {
            q.push(("diarize_model".to_owned(), "latest".to_owned()));
        }
    }
    q.extend(query_extras(&r.extras));
    q
}

pub(super) fn encode_request(
    r: &TranscriptionRequest,
    ctx: &mut EgressCtx,
) -> Result<EncodedRequest, KppError> {
    check(r)?;
    ctx.stream = StreamMode::Off;
    ctx.transcript_format = Some(r.format_or_default());
    let path = with_query(&path_under(ctx, LISTEN_PATH), &query_of(r));
    let (content_type, body) = match &r.audio.source {
        Source::Base64(b) => {
            let media = if b.media_type.trim().is_empty() {
                OCTET_STREAM.to_owned()
            } else {
                b.media_type.clone()
            };
            (media, b.data.to_vec())
        }
        Source::Url(u) => {
            let mut out = Map::new();
            out.insert("url".to_owned(), u.clone().into());
            put_extras(&mut out, &r.extras, D, BODY);
            (JSON_MEDIA_TYPE.to_owned(), crate::wire::to_body(&out)?)
        }
        _ => return Err(no("file ids", "file", "upload the file or send a link.")),
    };
    Ok(EncodedRequest::post_json(
        path,
        HeaderList::new().with("content-type", content_type),
        body,
    ))
}

fn number(v: Option<Value>) -> Option<f64> {
    v.as_ref().and_then(Value::as_f64)
}

fn speaker(v: Option<Value>) -> Option<String> {
    match v? {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s),
        _ => None,
    }
}

fn wire_speaker(s: &str) -> Value {
    s.parse::<u64>().map_or_else(|_| s.into(), Value::from)
}

fn object_of(v: Value, what: &str) -> Result<Fields, KppError> {
    match v {
        Value::Object(m) => Ok(Fields::from_map(m, D, "reply")),
        _ => Err(bad(&format!("{what} is not an object"))),
    }
}

fn word_of(v: Value, origin: &kaeso_kpp::Origin) -> Result<TranscriptWord, KppError> {
    let mut f = object_of(v, "a word")?;
    let plain = f.str("word")?.unwrap_or_default();
    let punctuated = f.str("punctuated_word")?;
    let start = number(f.take("start")).unwrap_or(0.0);
    let end = number(f.take("end")).unwrap_or(start);
    let spoken_by = speaker(f.take("speaker"));
    let mut extras = Extras::new(origin.clone());
    let text = match punctuated {
        Some(p) => {
            if let Ok(raw) = RawJson::from_serialize(&plain) {
                extras.fields.insert("word".to_owned(), raw);
            }
            p
        }
        None => plain,
    };
    f.rest_into(&mut extras, "");
    let mut w = TranscriptWord::new(text, start, end);
    w.speaker = spoken_by;
    w.extras = extras;
    Ok(w)
}

fn segment_of(v: Value, origin: &kaeso_kpp::Origin) -> Result<TranscriptSegment, KppError> {
    let mut f = object_of(v, "an utterance")?;
    let start = number(f.take("start")).unwrap_or(0.0);
    let end = number(f.take("end")).unwrap_or(start);
    let text = f.str("transcript")?.unwrap_or_default();
    let mut s = TranscriptSegment::new(start, end, text);
    s.id = f.str("id")?;
    s.speaker = speaker(f.take("speaker"));
    s.extras = f.rest(origin);
    Ok(s)
}

fn only_one(list: Option<Vec<Value>>, what: &str) -> Result<Value, KppError> {
    match list {
        Some(mut items) if items.len() == 1 => Ok(items.remove(0)),
        Some(items) if items.len() > 1 => Err(bad(&format!(
            "it holds {} {what}s, and Kaeso asks for one",
            items.len()
        ))),
        _ => Err(bad(&format!("it has no {what}"))),
    }
}

pub(super) fn decode_response(body: &[u8], ctx: &EgressCtx) -> Result<Reply, KppError> {
    let origin = ctx.origin(D);
    let mut extras = Extras::new(origin.clone());
    let mut f = provider_object(body, D)?;
    let mut metadata = f.object("metadata")?;
    let duration = metadata.as_mut().and_then(|m| number(m.take("duration")));
    let channels = metadata
        .as_ref()
        .and_then(|m| m.peek("channels"))
        .and_then(Value::as_f64)
        .filter(|n| *n >= 1.0)
        .unwrap_or(1.0);
    if let Some(m) = metadata {
        m.rest_into(&mut extras, "metadata");
    }
    let mut results = f
        .object("results")?
        .ok_or_else(|| bad("it has no `results`"))?;
    let mut channel = object_of(
        only_one(results.array("channels")?, "channel")?,
        "a channel",
    )?;
    let mut alternative = object_of(
        only_one(channel.array("alternatives")?, "alternative")?,
        "an alternative",
    )?;
    let text = alternative
        .str("transcript")?
        .ok_or_else(|| bad("the transcript has no `transcript`"))?;
    let mut resp = TranscriptionResponse::new(String::new(), text);
    resp.format = Some(TranscriptFormat::Json);
    resp.words = alternative
        .array("words")?
        .unwrap_or_default()
        .into_iter()
        .map(|w| word_of(w, &origin))
        .collect::<Result<_, _>>()?;
    alternative.rest_into(&mut extras, "alternative");
    resp.language = channel.str("detected_language")?;
    channel.rest_into(&mut extras, "channel");
    resp.segments = results
        .array("utterances")?
        .unwrap_or_default()
        .into_iter()
        .map(|s| segment_of(s, &origin))
        .collect::<Result<_, _>>()?;
    results.rest_into(&mut extras, "results");
    f.rest_into(&mut extras, "");
    resp.duration = duration;
    // Deepgram bills each channel's length (https://deepgram.com/pricing, "multichannel").
    resp.usage_seconds = duration.map(|d| d * channels);
    resp.extras = extras;
    Ok(Reply::Transcription(resp))
}

fn wire_word(w: &TranscriptWord) -> Value {
    let mut m = Map::new();
    put_extras(&mut m, &w.extras, D, "");
    m.insert("start".to_owned(), w.start.into());
    m.insert("end".to_owned(), w.end.into());
    if m.contains_key("word") {
        m.insert("punctuated_word".to_owned(), w.text.clone().into());
    } else {
        m.insert("word".to_owned(), w.text.clone().into());
    }
    if let Some(s) = &w.speaker {
        m.insert("speaker".to_owned(), wire_speaker(s));
    }
    Value::Object(m)
}

fn wire_utterance(s: &TranscriptSegment) -> Value {
    let mut m = Map::new();
    put_extras(&mut m, &s.extras, D, "");
    m.insert("start".to_owned(), s.start.into());
    m.insert("end".to_owned(), s.end.into());
    m.insert("transcript".to_owned(), s.text.clone().into());
    if let Some(id) = &s.id {
        m.insert("id".to_owned(), id.clone().into());
    }
    if let Some(sp) = &s.speaker {
        m.insert("speaker".to_owned(), wire_speaker(sp));
    }
    Value::Object(m)
}

pub(super) fn encode_response(r: &TranscriptionResponse) -> Result<Vec<u8>, KppError> {
    let own = r.extras.is_from(D);
    let mut alternative = Map::new();
    put_extras(&mut alternative, &r.extras, D, "alternative");
    alternative.insert("transcript".to_owned(), r.text.clone().into());
    alternative.insert(
        "words".to_owned(),
        r.words.iter().map(wire_word).collect::<Vec<_>>().into(),
    );
    let mut channel = Map::new();
    put_extras(&mut channel, &r.extras, D, "channel");
    channel.insert(
        "alternatives".to_owned(),
        Value::Array(vec![Value::Object(alternative)]),
    );
    if let Some(l) = &r.language {
        channel.insert("detected_language".to_owned(), l.clone().into());
    }
    let mut results = Map::new();
    put_extras(&mut results, &r.extras, D, "results");
    results.insert(
        "channels".to_owned(),
        Value::Array(vec![Value::Object(channel)]),
    );
    if !r.segments.is_empty() {
        results.insert(
            "utterances".to_owned(),
            r.segments
                .iter()
                .map(wire_utterance)
                .collect::<Vec<_>>()
                .into(),
        );
    }
    let mut metadata = Map::new();
    put_extras(&mut metadata, &r.extras, D, "metadata");
    if let Some(d) = r.duration.or(r.usage_seconds) {
        metadata.insert("duration".to_owned(), d.into());
    }
    if !own && !metadata.is_empty() {
        metadata.insert("channels".to_owned(), 1.into());
    }
    let mut out = Map::new();
    put_extras(&mut out, &r.extras, D, "");
    if !metadata.is_empty() {
        out.insert("metadata".to_owned(), Value::Object(metadata));
    }
    out.insert("results".to_owned(), Value::Object(results));
    crate::wire::to_body(&out)
}
