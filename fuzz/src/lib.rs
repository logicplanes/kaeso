use kaeso_codecs::gemini::GeminiAction;
use kaeso_codecs::openai_responses::ResponsesProfile;
use kaeso_codecs::sse::{SseError, SseItem, SseParser, write_item};
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, dispatch};
use kaeso_kpp::{
    CountTokensResponse, DialectId, EmbedResponse, ImageOperation, ImageResponse, ModelList, Reply,
    Request, RequestKind, Response, StreamEvent, StreamMode,
};
use kaeso_mcp::{Message, RequestHeaders, headers, names};

const KINDS: [RequestKind; 7] = [
    RequestKind::Generate,
    RequestKind::CountTokens,
    RequestKind::Embed,
    RequestKind::Image,
    RequestKind::Speech,
    RequestKind::Transcription,
    RequestKind::Video,
];

const ELEVENLABS_PATHS: [&str; 8] = [
    "/v1/text-to-speech/v1",
    "/v1/text-to-speech/v1/stream/with-timestamps",
    "/v1/text-to-dialogue/with-timestamps",
    "/v1/sound-generation",
    "/v1/music/stream",
    "/v1/voice-changer/v1",
    "/v1/speech-to-text",
    "/v1/text-to-speech/v1/with-timestamps",
];

const IMAGE_OPERATIONS: [ImageOperation; 3] = [
    ImageOperation::Generate,
    ImageOperation::Edit,
    ImageOperation::Variation,
];

const LIVE: StreamMode = StreamMode::on(true);

const SSE_LIMIT: usize = 1 << 16;

const NESTING_STEP: usize = 32;

const TOOL_RESULT_OPEN: &str = r#"{"type":"tool_result","tool_use_id":"toolu_fuzz","content":["#;

fn salt(data: &[u8]) -> u8 {
    data.iter().fold(0, |acc, byte| acc.wrapping_add(*byte))
}

fn chunk_size(data: &[u8]) -> usize {
    usize::from(salt(data)) + 1
}

pub fn decoders(dialect: DialectId, data: &[u8]) {
    let salt = salt(data);
    let headers = HeaderList::new().with("retry-after", "3");
    for action in GeminiAction::ALL
        .into_iter()
        .filter(|a| a.dialect() == dialect)
    {
        if let Ok(req) = dispatch::decode_gemini_request("models/m", action, data, &headers) {
            request(dialect, &req);
        }
    }
    if dialect == DialectId::OpenAiImages {
        let upload = headers
            .clone()
            .with("content-type", "multipart/form-data; boundary=b");
        for operation in IMAGE_OPERATIONS {
            for h in [&headers, &upload] {
                if let Ok(req) = dispatch::decode_image_request(operation, data, h) {
                    request(dialect, &req);
                }
            }
        }
    }
    let upload = headers
        .clone()
        .with("content-type", "multipart/form-data; boundary=b");
    if dialect == DialectId::OpenAiTranscriptions {
        for operation in [
            kaeso_kpp::TranscriptionOperation::Transcribe,
            kaeso_kpp::TranscriptionOperation::Translate,
        ] {
            if let Ok(req) = dispatch::decode_transcription_request(operation, data, &upload) {
                request(dialect, &req);
            }
        }
    }
    if dialect == DialectId::ElevenLabs {
        for path in ELEVENLABS_PATHS {
            let query = if salt.is_multiple_of(2) {
                "output_format=pcm_24000"
            } else {
                ""
            };
            let Some(call) = kaeso_codecs::elevenlabs::ElevenLabsCall::from_path(path, query)
            else {
                continue;
            };
            for h in [&headers, &upload] {
                if let Ok(req) = dispatch::decode_elevenlabs_request(&call, data, h) {
                    request(dialect, &req);
                }
            }
        }
        if let Ok(text) = std::str::from_utf8(data) {
            use kaeso_codecs::elevenlabs::realtime::{self, RealtimeApi};
            for api in [
                RealtimeApi::TextToSpeech,
                RealtimeApi::TextToSpeechContexts,
                RealtimeApi::TextToDialogue,
                RealtimeApi::TextToDialogueContexts,
                RealtimeApi::SpeechToText,
            ] {
                match realtime::read_client_message(api, text) {
                    Ok(read) => {
                        let again = realtime::read_client_message(api, &read.forwarded)
                            .unwrap_or_else(|e| panic!("a forwarded message reads again: {e:?}"));
                        assert_eq!(again.credential, None, "credentials never pass on");
                        assert_eq!(again.characters, read.characters);
                    }
                    Err(e) => {
                        let _ = realtime::error_message(api, &e);
                    }
                }
            }
            let _ = RealtimeApi::from_path(text);
            let _ = realtime::bytes_per_second(text);
            let _ = kaeso_codecs::elevenlabs::decode_generation_query(text);
        }
        if let Ok(g) = kaeso_codecs::elevenlabs::decode_generation(200, &headers, data) {
            let written = kaeso_codecs::elevenlabs::encode_generation(&g)
                .unwrap_or_else(|e| panic!("a generation writes: {e:?}"));
            let again = kaeso_codecs::elevenlabs::decode_generation(200, &headers, &written)
                .unwrap_or_else(|e| panic!("a written generation reads again: {e:?}"));
            assert_eq!(again.status, g.status);
        }
    }
    for kind in KINDS {
        if let Ok(req) = dispatch::decode_request(dialect, kind, data, &headers) {
            request(dialect, &req);
        }
        let ctx = EgressCtx::for_kind(kind, StreamMode::Off);
        if let Ok(reply) = dispatch::decode_response(dialect, 200, &headers, data, &ctx) {
            response(&reply);
        }
    }
    videos(dialect, data, &headers, &upload);
    let err = dispatch::decode_error(dialect, 400 + u16::from(salt), &headers, data);
    for target in DialectId::ALL {
        let _ = dispatch::encode_error(target, &err);
    }
    models(dialect, data, salt, &headers);
    stream(dialect, data);
    if dialect == DialectId::AnthropicMessages {
        nested(data, salt, &headers);
    }
}

fn nested(data: &[u8], salt: u8, headers: &HeaderList) {
    let Ok(inner) = std::str::from_utf8(data) else {
        return;
    };
    if !salt.is_multiple_of(8)
        || kaeso_kpp::serde_json::from_str::<&kaeso_kpp::serde_json::value::RawValue>(inner)
            .is_err()
    {
        return;
    }
    let levels = usize::from(salt) * NESTING_STEP;
    let mut blocks = TOOL_RESULT_OPEN.repeat(levels);
    blocks.push_str(inner);
    blocks.push_str(&"]}".repeat(levels));
    let request = format!(
        r#"{{"model":"m","max_tokens":1,"messages":[{{"role":"user","content":[{blocks}]}}]}}"#
    );
    if let Ok(req) = dispatch::decode_request(
        DialectId::AnthropicMessages,
        RequestKind::Generate,
        request.as_bytes(),
        headers,
    ) {
        self::request(DialectId::AnthropicMessages, &req);
    }
    let reply = format!(
        r#"{{"id":"msg_fuzz","type":"message","role":"assistant","model":"m","content":[{blocks}],"stop_reason":"end_turn","stop_sequence":null,"usage":{{"input_tokens":1,"output_tokens":1}}}}"#
    );
    let ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
    if let Ok(reply) = dispatch::decode_response(
        DialectId::AnthropicMessages,
        200,
        headers,
        reply.as_bytes(),
        &ctx,
    ) {
        response(&reply);
    }
}

fn request(dialect: DialectId, req: &Request) {
    if req.validate().is_err() {
        return;
    }
    if let Request::Generate(generate) | Request::CountTokens(generate) = req {
        let _ = generate.required_capabilities();
    }
    for target in DialectId::ALL {
        let mut ctx = EgressCtx::new(req);
        if dispatch::check(target, req, &ctx).is_err() {
            continue;
        }
        let _ = dispatch::encode_request(target, req, &mut ctx);
    }
    // A compatible provider that lists the input item types it takes, whose profile rewrites
    // what OpenAI's leaves alone: agent messages, local shell items, a bare tool_choice.
    let mut ctx = EgressCtx::new(req)
        .with_responses_profile(ResponsesProfile::compatible("SpaceX").xai_input_items());
    if dispatch::check(DialectId::OpenAiResponses, req, &ctx).is_ok() {
        let _ = dispatch::encode_request(DialectId::OpenAiResponses, req, &mut ctx);
    }
    let _ = dispatch::encode_response(
        dialect,
        &empty_reply(req),
        &EncodeCtx::for_request(req.clone()),
    );
}

fn empty_reply(req: &Request) -> Reply {
    match req.kind() {
        RequestKind::Embed => Reply::Embed(EmbedResponse::new(req.model(), Vec::new())),
        RequestKind::CountTokens => Reply::CountTokens(CountTokensResponse::new(0)),
        RequestKind::Image => Reply::Image(ImageResponse::new(req.model(), Vec::new())),
        _ => Reply::Generate(Response::new("resp_fuzz", req.model(), Vec::new())),
    }
}

// A video job read as a provider reports it, a file it names, and OpenAI's remix body and
// queries, then written back as OpenAI's video resource and listing.
fn videos(dialect: DialectId, data: &[u8], headers: &HeaderList, upload: &HeaderList) {
    let ctx = EgressCtx::for_kind(RequestKind::Video, StreamMode::Off)
        .with_videos_profile(kaeso_codecs::openai_videos::VideosProfile::TOGETHER);
    for c in [
        EgressCtx::for_kind(RequestKind::Video, StreamMode::Off),
        ctx,
    ] {
        if let Ok(job) = dispatch::decode_video_poll(dialect, 200, headers, data, &c) {
            response(&Reply::Video(job.clone()));
            let _ = kaeso_codecs::openai_videos::encode_list(&kaeso_kpp::VideoList::new(
                vec![job],
                true,
            ));
        }
    }
    let _ = dispatch::decode_video_file(dialect, 200, headers, data);
    if dialect == DialectId::OpenAiVideos {
        for h in [headers, upload] {
            if let Ok(req) = kaeso_codecs::openai_videos::decode_remix(data, h) {
                request(dialect, &req);
            }
            if let Ok(req) = dispatch::decode_request(dialect, RequestKind::Video, data, h) {
                request(dialect, &req);
            }
        }
        if let Ok(q) = std::str::from_utf8(data) {
            let _ = kaeso_codecs::openai_videos::decode_list_query(q);
            let _ = kaeso_codecs::openai_videos::decode_content_query(q);
        }
    }
}

fn response(reply: &Reply) {
    let ctx = EncodeCtx::new(reply.kind(), StreamMode::Off);
    for target in DialectId::ALL {
        let _ = dispatch::encode_response(target, reply, &ctx);
    }
}

fn other_family(dialect: DialectId) -> DialectId {
    if dialect == DialectId::AnthropicMessages {
        DialectId::OpenAiChat
    } else {
        DialectId::AnthropicMessages
    }
}

fn models(dialect: DialectId, data: &[u8], salt: u8, headers: &HeaderList) {
    let other = other_family(dialect);
    let status = if salt.is_multiple_of(2) {
        200
    } else {
        400 + u16::from(salt)
    };
    if let Ok(list) = dispatch::decode_models(dialect, status, headers, data) {
        let _ = dispatch::encode_models(dialect, &list);
        if let Ok(bytes) = dispatch::encode_models(other, &list)
            && let Ok(back) = dispatch::decode_models(other, 200, headers, &bytes)
        {
            let _ = dispatch::encode_models(dialect, &back);
        }
        let query = std::str::from_utf8(data)
            .ok()
            .and_then(|q| dispatch::decode_models_query(dialect, q).ok())
            .unwrap_or_default();
        let _ = ModelList::page(&list.models, &query, Some(u32::from(salt)));
    }
    if let Ok(model) = dispatch::decode_model(dialect, status, headers, data) {
        let _ = dispatch::encode_model(dialect, &model);
        if let Ok(bytes) = dispatch::encode_model(other, &model)
            && let Ok(back) = dispatch::decode_model(other, 200, headers, &bytes)
        {
            let _ = dispatch::encode_model(dialect, &back);
        }
    }
}

fn stream(dialect: DialectId, data: &[u8]) {
    let Ok(mut decoder) =
        dispatch::stream_decoder(dialect, &EgressCtx::for_kind(RequestKind::Generate, LIVE))
    else {
        return;
    };
    let mut events = Vec::new();
    for chunk in data.chunks(chunk_size(data)) {
        events.extend(decoder.feed(chunk));
    }
    events.extend(decoder.finish());
    for target in DialectId::ALL {
        reencode(target, &events);
    }
}

fn reencode(target: DialectId, events: &[StreamEvent]) {
    let Ok(mut encoder) =
        dispatch::stream_encoder(target, &EncodeCtx::new(RequestKind::Generate, LIVE))
    else {
        return;
    };
    for event in events {
        let _ = encoder.encode(event);
        let _ = encoder.keepalive();
    }
    let _ = encoder.finish();
}

fn parse<'a>(
    limit: usize,
    chunks: impl IntoIterator<Item = &'a [u8]>,
) -> Result<Vec<SseItem>, SseError> {
    let mut parser = SseParser::with_limit(limit);
    let mut items = Vec::new();
    for chunk in chunks {
        parser.feed_into(chunk, &mut items)?;
    }
    items.extend(parser.finish()?);
    Ok(items)
}

pub fn sse(data: &[u8]) {
    let whole = parse(SSE_LIMIT, [data]);
    let chunked = parse(SSE_LIMIT, data.chunks(chunk_size(data)));
    assert_eq!(whole, chunked);
    let bytewise = parse(SSE_LIMIT, data.chunks(1));
    assert_eq!(whole, bytewise);
    if let Ok(items) = whole {
        let mut written = Vec::new();
        for item in &items {
            write_item(&mut written, item);
        }
        let limit = SSE_LIMIT.max(written.len());
        assert_eq!(parse(limit, [written.as_slice()]), Ok(items));
    }
}

pub fn mcp(data: &[u8]) {
    if let Ok(text) = std::str::from_utf8(data) {
        let encoded = headers::encode(text);
        assert_eq!(headers::decode(encoded.as_bytes()).as_deref(), Some(text));
        let name = names::exposed("fuzz", text);
        assert!(!name.is_empty() && name.len() <= names::MAX_NAME);
    }
    let Ok(message) = Message::parse(data) else {
        return;
    };
    let written = message.to_vec();
    let again = match Message::parse(&written) {
        Ok(m) => m,
        Err(e) => panic!("a written message doesn't parse again: {}", e.message),
    };
    assert_eq!(again.kind(), message.kind());
    assert_eq!(again.to_vec(), written);
    let request = RequestHeaders {
        protocol_version: Some(if salt(data).is_multiple_of(2) {
            b"2026-07-28"
        } else {
            b"2025-11-25"
        }),
        method: message.method().map(str::as_bytes),
        name: data.get(..usize::from(salt(data)).min(data.len())),
    };
    if let Ok(version) = headers::version_of(&request, &message) {
        let _ = headers::check(version, &request, &message);
    }
    let mut renamed = message.clone();
    renamed.set_name("fuzz__renamed");
    if renamed.name_field().is_some() && renamed.params().is_some() {
        let bytes = renamed.to_vec();
        let parsed = match Message::parse(&bytes) {
            Ok(m) => m,
            Err(e) => panic!("a renamed message doesn't parse again: {}", e.message),
        };
        assert_eq!(parsed.name().as_deref(), Some("fuzz__renamed"));
    }
}

pub fn audio_length(data: &[u8]) {
    if let Some(length) = kaeso_codecs::audio_length::audio_duration(data) {
        assert!(length.as_secs_f64() > 0.0 && length.as_secs_f64() <= 24.0 * 3600.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_event_just_under_the_limit_round_trips_although_writing_it_adds_a_colon_to_every_line() {
        let data = "{\n".repeat((SSE_LIMIT - 2) / 2);
        sse(data.as_bytes());
    }
}
