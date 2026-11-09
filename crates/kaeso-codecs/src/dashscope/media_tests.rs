use base64::Engine as _;
use kaeso_kpp::{
    AudioFormat, AudioInput, ErrorKind, ImageDelivery, ImageInput, ImageOperation, ImageRequest,
    ImageSize, Origin, RawJson, Reply, Request, RequestKind, Source, SpeechOperation,
    SpeechRequest, StreamEvent, StreamMode, TranscriptFormat, TranscriptionOperation,
    TranscriptionRequest, Voice,
};
use serde_json::{Value, json};

use super::images::{GENERATION_PATH, ImageFamily, TEXT2IMAGE_PATH, image_family};
use super::speech::{SYNTHESIZER_PATH, SpeechFamily, speech_family};
use super::transcription::transcribes;
use super::*;
use crate::test_support::{assert_json_eq, fixtures};

const DIR: &str = "dashscope";

fn round_trip_requests(kind: RequestKind, folder: &str) -> Vec<(String, Request, EncodedRequest)> {
    let mut out = Vec::new();
    for (name, body) in fixtures(DIR, &format!("requests/{folder}"), "json") {
        let req = DashScope::decode_request(kind, &body, &HeaderList::new())
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        req.validate().unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let mut ctx = EgressCtx::new(&req);
        let enc =
            DashScope::encode_request(&req, &mut ctx).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_json_eq(&name, &body, &enc.body);
        out.push((name, req, enc));
    }
    out
}

fn round_trip_replies(kind: RequestKind, folder: &str) -> Vec<(String, Reply)> {
    let mut out = Vec::new();
    for (name, body) in fixtures(DIR, &format!("responses/{folder}"), "json") {
        let ctx = EgressCtx::for_kind(kind, StreamMode::Off);
        let reply = DashScope::decode_response(200, &HeaderList::new(), &body, &ctx)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let back = DashScope::encode_response(&reply, &EncodeCtx::new(kind, StreamMode::Off))
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_json_eq(&name, &body, &back);
        out.push((name, reply));
    }
    out
}

fn image(model: &str) -> ImageRequest {
    ImageRequest::new(model, ImageOperation::Generate).with_prompt("A red fox")
}

fn edit(model: &str, n: usize) -> ImageRequest {
    let mut r = ImageRequest::new(model, ImageOperation::Edit).with_prompt("Add a hat");
    for i in 0..n {
        r.images.push(ImageInput::new(Source::Url(format!(
            "https://example.com/{i}.png"
        ))));
    }
    r
}

fn sent_image(r: &ImageRequest) -> Result<Value, KppError> {
    let req = Request::Image(r.clone());
    let mut ctx = EgressCtx::new(&req);
    let enc = DashScope::encode_request(&req, &mut ctx)?;
    assert_eq!(enc.path, GENERATION_PATH);
    assert!(ctx.stream.is_off());
    Ok(serde_json::from_slice(&enc.body).unwrap())
}

fn image_param(r: &ImageRequest) -> String {
    sent_image(r).unwrap_err().param.unwrap_or_default()
}

#[test]
fn image_fixtures_round_trip_and_read_their_images_and_sizes() {
    let requests = round_trip_requests(RequestKind::Image, "images");
    assert_eq!(requests.len(), 8);
    for (name, req, enc) in &requests {
        let task = name.ends_with("-task");
        let path = if task {
            TEXT2IMAGE_PATH
        } else {
            GENERATION_PATH
        };
        assert_eq!(enc.path, path, "{name}");
        assert_eq!(
            enc.headers.get("x-dashscope-async"),
            task.then_some("enable"),
            "{name}"
        );
        let Request::Image(r) = req else { panic!() };
        let edits = r.operation == ImageOperation::Edit;
        assert_eq!(edits, name.contains("edit"), "{name}");
    }
    let replies = round_trip_replies(RequestKind::Image, "images");
    let by = |n: &str| {
        let Reply::Image(r) = &replies.iter().find(|(name, _)| name == n).unwrap().1 else {
            panic!()
        };
        r.clone()
    };
    let edit = by("qwen-image-edit");
    assert_eq!(edit.images.len(), 2);
    assert_eq!(edit.settings.size, Some(ImageSize::pixels(1536, 1024)));
    let wan = by("wan27");
    assert_eq!(wan.settings.size, Some(ImageSize::pixels(2985, 1405)));
    assert_eq!(
        (wan.usage.input_tokens, wan.usage.output_tokens),
        (18_790, 2)
    );
    let z = by("z-image");
    assert_eq!(
        z.images[0].revised_prompt.as_deref(),
        Some("Photo of a stylish young woman...")
    );
    assert!(matches!(&z.images[0].source, Source::Url(u) if u.contains("dashscope-result-sgp")));
}

#[test]
fn models_pick_their_family() {
    assert_eq!(
        image_family("qwen-image-2.0-pro-2026-06-22"),
        Some(ImageFamily::QwenImage)
    );
    assert_eq!(image_family("wan2.7-image-pro"), Some(ImageFamily::Wan27));
    assert_eq!(image_family("wan2.6-image"), Some(ImageFamily::Wan26));
    assert_eq!(image_family("z-image-turbo"), Some(ImageFamily::ZImage));
    assert_eq!(image_family("wan2.6-t2i"), Some(ImageFamily::WanT2i));
    for m in [
        "wan2.5-t2i-preview",
        "wan2.2-t2i-plus",
        "wan2.2-t2i-flash",
        "wan2.1-t2i-plus",
        "wan2.1-t2i-turbo",
        "wanx2.1-t2i-turbo",
    ] {
        assert_eq!(image_family(m), Some(ImageFamily::WanT2iTask), "{m}");
    }
    assert_eq!(image_family("wan2.7-t2v"), None);
    assert_eq!(
        speech_family("qwen-audio-3.1-tts-flash"),
        Some(SpeechFamily::QwenAudio)
    );
    assert_eq!(speech_family("qwen3-tts-flash"), Some(SpeechFamily::Qwen3));
    assert_eq!(speech_family("qwen3-tts-flash-realtime"), None);
    assert!(transcribes("qwen-audio-3.0-asr-flash"));
    assert!(!transcribes("qwen-audio-3.1-asr-flash-filetrans"));
    assert!(!transcribes("qwen3-asr-flash-realtime"));
}

#[test]
fn openai_image_requests_become_dashscope_messages() {
    let mut r = edit("qwen-image-2.1-turbo", 1);
    r.images
        .push(ImageInput::new(Source::base64("image/png", "AAAA")));
    r.size = Some(ImageSize::pixels(1024, 768));
    r.count = Some(2);
    r.seed = Some(7);
    r.user = Some("u1".into());
    r.extras = Extras::new(Origin::new(DialectId::OpenAiImages))
        .with_field("prompt_extend", RawJson::from_serialize(&false).unwrap());
    let req = Request::Image(r.clone());
    let mut ctx = EgressCtx::new(&req);
    let enc = DashScope::encode_request(&req, &mut ctx).unwrap();
    let sent: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        sent,
        json!({
            "model": "qwen-image-2.1-turbo",
            "input": {"messages": [{"role": "user", "content": [
                {"image": "https://example.com/0.png"},
                {"image": "data:image/png;base64,AAAA"},
                {"text": "Add a hat"},
            ]}]},
            "parameters": {"n": 2, "size": "1024*768", "seed": 7, "prompt_extend": false},
        })
    );
    assert_eq!(ctx.left_out, vec!["user".to_owned()]);

    let mut tier = image("wan2.7-image");
    tier.resolution = Some("2k".into());
    assert_eq!(sent_image(&tier).unwrap()["parameters"]["size"], "2K");
    assert_eq!(
        sent_image(&edit("wan2.6-image", 2)).unwrap()["parameters"]["n"],
        1
    );
    let z = sent_image(&image("z-image-turbo")).unwrap();
    assert!(z.get("parameters").is_none());
    let mut auto = image("qwen-image-2.0");
    auto.size = Some(ImageSize::Auto);
    assert!(sent_image(&auto).unwrap().get("parameters").is_none());
}

#[test]
fn image_fields_dashscope_lacks_are_refused_before_sending() {
    let mut masked = edit("qwen-image-2.0", 1);
    masked.mask = Some(ImageInput::new(Source::Url(
        "https://example.com/m.png".into(),
    )));
    assert_eq!(image_param(&masked), "mask");
    let mut b64 = image("qwen-image-2.0");
    b64.delivery = Some(ImageDelivery::Base64);
    assert_eq!(image_param(&b64), "response_format");
    let mut quality = image("qwen-image-2.0");
    quality.quality = Some(kaeso_kpp::ImageQuality::High);
    assert_eq!(image_param(&quality), "quality");
    let mut ratio = image("wan2.7-image");
    ratio.aspect_ratio = Some("16:9".into());
    assert_eq!(image_param(&ratio), "aspect_ratio");
    let mut both = image("wan2.7-image");
    both.resolution = Some("2K".into());
    both.size = Some(ImageSize::pixels(1024, 1024));
    assert_eq!(image_param(&both), "resolution");
    let mut tier = image("wan2.7-image");
    tier.resolution = Some("8K".into());
    assert_eq!(image_param(&tier), "resolution");
    let mut qtier = image("qwen-image-2.0");
    qtier.resolution = Some("2K".into());
    assert_eq!(image_param(&qtier), "resolution");
    assert_eq!(image_param(&edit("qwen-image-2.0", 4)), "image");
    assert!(sent_image(&edit("qwen-image-2.1-turbo", 10)).is_ok());
    let mut neg = image("qwen-image-2.1-turbo");
    neg.negative_prompt = Some("blur".into());
    assert_eq!(image_param(&neg), "negative_prompt");
    let mut wneg = image("wan2.7-image");
    wneg.negative_prompt = Some("blur".into());
    assert_eq!(image_param(&wneg), "negative_prompt");
    assert_eq!(image_param(&edit("wan2.7-image", 10)), "image");
    assert_eq!(image_param(&image("wan2.6-image")), "image");
    assert_eq!(image_param(&edit("wan2.6-image", 5)), "image");
    assert_eq!(image_param(&edit("z-image-turbo", 1)), "image");
    let mut many = image("z-image-turbo");
    many.count = Some(2);
    assert_eq!(image_param(&many), "n");
    let mut seven = image("qwen-image-2.0");
    seven.count = Some(7);
    assert_eq!(image_param(&seven), "n");
    assert_eq!(image_param(&image("wan2.7-t2v")), "model");
    let mut partial = image("qwen-image-2.0");
    partial.stream = StreamMode::on(true);
    partial.partial_images = Some(2);
    assert_eq!(image_param(&partial), "partial_images");
    let mut file = edit("qwen-image-2.0", 0);
    file.images.push(ImageInput::new(Source::file_id(
        Origin::new(DialectId::OpenAiImages),
        "f",
    )));
    assert_eq!(image_param(&file), "image");
    let mut variation = edit("qwen-image-2.0", 1);
    variation.operation = ImageOperation::Variation;
    assert_eq!(image_param(&variation), "image");
    let mut foreign = image("qwen-image-2.0");
    foreign.extras = Extras::new(Origin::new(DialectId::OpenAiImages))
        .with_field("moderation", RawJson::from_serialize(&"low").unwrap());
    assert!(sent_image(&foreign).is_err());
}

#[test]
fn image_replies_need_an_image_and_read_errors() {
    let ctx = EgressCtx::for_kind(RequestKind::Image, StreamMode::Off);
    let none = br#"{"output":{"choices":[]},"request_id":"r"}"#;
    assert!(DashScope::decode_response(200, &HeaderList::new(), none, &ctx).is_err());
    assert!(DashScope::decode_response(200, &HeaderList::new(), b"{}", &ctx).is_err());
    let (_, body) = fixtures(DIR, "errors", "json")
        .into_iter()
        .find(|(n, _)| n == "invalid-parameter-400")
        .unwrap();
    let err = DashScope::decode_response(400, &HeaderList::new(), &body, &ctx).unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert_eq!(err.message, "num_images_per_prompt must be 1");
    let mut reply = kaeso_kpp::ImageResponse::new(
        "m",
        vec![kaeso_kpp::GeneratedImage::new(Source::base64(
            "image/png",
            "AAAA",
        ))],
    );
    reply.images[0].revised_prompt = Some("p".into());
    let written: Value = serde_json::from_slice(
        &DashScope::encode_response(
            &Reply::Image(reply),
            &EncodeCtx::new(RequestKind::Image, StreamMode::Off),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        written["output"]["choices"][0]["message"]["content"],
        json!([{"image": "data:image/png;base64,AAAA"}, {"text": "p"}])
    );
}

#[test]
fn native_image_requests_keep_their_order_and_refuse_what_they_cannot_map() {
    let text_first = br#"{"model":"wan2.6-image","input":{"messages":[{"role":"user","content":[{"text":"t"},{"image":"https://example.com/a.png"}]}]}}"#;
    let req =
        DashScope::decode_request(RequestKind::Image, text_first, &HeaderList::new()).unwrap();
    let mut ctx = EgressCtx::new(&req);
    let enc = DashScope::encode_request(&req, &mut ctx).unwrap();
    assert_json_eq("text first", text_first, &enc.body);
    for bad in [
        &br#"{"model":"m","input":{"messages":[]}}"#[..],
        br#"{"model":"m","input":{"messages":[{"role":"assistant","content":[]}]}}"#,
        br#"{"model":"m","input":{"messages":[{"role":"user","content":[{"video":"v"}]}]}}"#,
        br#"{"model":"m","input":{"messages":[{"role":"user","content":[{"text":"a"},{"text":"b"}]}]}}"#,
        br#"{"model":"m","parameters":{"size":"big"}}"#,
    ] {
        assert!(
            DashScope::decode_request(RequestKind::Image, bad, &HeaderList::new()).is_err(),
            "{}",
            String::from_utf8_lossy(bad)
        );
    }
    assert!(DashScope::decode_request(RequestKind::Embed, b"{}", &HeaderList::new()).is_err());
}

fn speech(model: &str, voice: &str) -> SpeechRequest {
    SpeechRequest::new(model, SpeechOperation::Speech)
        .with_input("Hello")
        .with_voice(Voice::Named(voice.into()))
}

fn sent_speech(r: &SpeechRequest) -> Result<(EncodedRequest, EgressCtx), KppError> {
    let req = Request::Speech(r.clone());
    let mut ctx = EgressCtx::new(&req);
    let enc = DashScope::encode_request(&req, &mut ctx)?;
    Ok((enc, ctx))
}

fn speech_param(r: &SpeechRequest) -> String {
    sent_speech(r).unwrap_err().param.unwrap_or_default()
}

fn decode_stream(name: &str, ctx: &EgressCtx) -> Vec<StreamEvent> {
    let (_, bytes) = fixtures(DIR, "streams/speech", "sse")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap();
    let mut decoder = DashScope::stream_decoder(ctx);
    let mut events = Vec::new();
    for chunk in bytes.chunks(7) {
        events.extend(decoder.feed(chunk));
    }
    events.extend(decoder.finish());
    events
}

#[test]
fn speech_fixtures_round_trip_and_stream_their_audio() {
    let requests = round_trip_requests(RequestKind::Speech, "speech");
    let by = |n: &str| requests.iter().find(|(name, ..)| name == n).unwrap();
    let audio = &by("qwen-audio-tts").2;
    assert_eq!(audio.path, SYNTHESIZER_PATH);
    assert_eq!(audio.headers.get("x-dashscope-sse"), Some("enable"));
    let qwen3 = &by("qwen3-tts").2;
    assert_eq!(qwen3.path, GENERATION_PATH);
    assert_eq!(qwen3.headers.get("x-dashscope-sse"), None);

    let replies = round_trip_replies(RequestKind::Speech, "speech");
    for (name, reply) in &replies {
        let Reply::Speech(r) = reply else { panic!() };
        assert!(
            matches!(&r.audio, Source::Url(u) if u.contains(".wav")),
            "{name}"
        );
    }
    let Reply::Speech(link) = &replies
        .iter()
        .find(|(n, _)| n == "qwen3-tts-link")
        .unwrap()
        .1
    else {
        panic!()
    };
    assert_eq!(link.characters, Some(195));

    let mut ctx = EgressCtx::for_kind(RequestKind::Speech, StreamMode::on(false));
    ctx.audio_format = Some(AudioFormat::Wav);
    let events = decode_stream("qwen-audio-tts", &ctx);
    let pieces: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::Speech(s) if s.audio.is_some() => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(pieces.len(), 2);
    assert_eq!(pieces[0].media_type.as_deref(), Some("audio/wav"));
    assert_eq!(pieces[1].media_type, None);
    let bytes: Vec<u8> = pieces
        .iter()
        .flat_map(|p| {
            base64::engine::general_purpose::STANDARD
                .decode(p.audio.as_deref().unwrap())
                .unwrap()
        })
        .collect();
    assert!(bytes.starts_with(b"RIFF"));
    let Some(StreamEvent::Speech(end)) = events.last() else {
        panic!("{events:?}")
    };
    assert!(end.done);
    assert_eq!(end.characters, Some(22));
    assert_eq!(end.usage.as_ref().unwrap().output_tokens, 30);

    ctx.audio_format = Some(AudioFormat::Pcm16);
    let events = decode_stream("qwen3-tts-pcm", &ctx);
    let first = events
        .iter()
        .find_map(|e| match e {
            StreamEvent::Speech(s) if s.audio.is_some() => Some(s),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        first.media_type.as_deref(),
        Some(kaeso_kpp::PCM16_MEDIA_TYPE)
    );
    assert!(
        matches!(events.last(), Some(StreamEvent::Speech(s)) if s.done && s.characters == Some(57))
    );

    let events = decode_stream("invalid-voice", &ctx);
    let [StreamEvent::Error(err)] = events.as_slice() else {
        panic!("{events:?}")
    };
    assert_eq!(err.status, Some(400));
    assert!(err.message.contains("Engine error"));
}

#[test]
fn a_stream_that_stops_early_or_breaks_is_an_error() {
    let ctx = EgressCtx::for_kind(RequestKind::Speech, StreamMode::on(false));
    let mut d = DashScope::stream_decoder(&ctx);
    let mut events = d.feed(b"data:{\"output\":{\"audio\":{\"data\":\"AAAA\"}}}\n\n");
    events.extend(d.finish());
    assert!(matches!(events.last(), Some(StreamEvent::Error(e)) if e.status == Some(502)));
    let mut d = DashScope::stream_decoder(&ctx);
    let events = d.feed(b"data:{not json\n\n");
    assert!(matches!(events.as_slice(), [StreamEvent::Error(_)]));
    assert!(d.finish().is_empty());
    let video = EgressCtx::for_kind(RequestKind::Video, StreamMode::Off);
    let mut d = DashScope::stream_decoder(&video);
    assert!(matches!(d.finish().as_slice(), [StreamEvent::Error(_)]));
}

#[test]
fn openai_speech_requests_pick_the_streamed_or_whole_form() {
    let mut r = speech("qwen-audio-3.0-tts-flash", "longanhuan_v3.6");
    r.format = Some(AudioFormat::Pcm16);
    r.speed = Some(1.5);
    r.instructions = Some("Speak warmly.".into());
    r.language = Some("en".into());
    r.seed = Some(3);
    let (enc, ctx) = sent_speech(&r).unwrap();
    assert_eq!(enc.path, SYNTHESIZER_PATH);
    assert!(!ctx.stream.is_off());
    assert_eq!(ctx.audio_format, Some(AudioFormat::Pcm16));
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        body["input"],
        json!({
            "text": "Hello",
            "voice": "longanhuan_v3.6",
            "format": "pcm",
            "sample_rate": 24000,
            "rate": 1.5,
            "instruction": "Speak warmly.",
            "seed": 3,
            "language_hints": ["en"],
        })
    );
    let (_, ctx) = sent_speech(&speech("qwen-audio-3.1-tts-plus", "longanhuan_v3.1")).unwrap();
    assert_eq!(ctx.audio_format, Some(AudioFormat::Mp3));
    let base = speech("qwen-audio-3.0-tts-plus", "qwen-audio-3.0-tts-plus-bella");
    assert!(sent_speech(&base).is_ok());
    let mut id = speech("qwen-audio-3.1-tts-flash", "x");
    id.voice = Some(Voice::Id("cosyvoice-v3-prefix-1234".into()));
    assert!(sent_speech(&id).is_ok());

    let mut whole = speech("qwen3-tts-flash", "Cherry");
    whole.language = Some("de".into());
    let (enc, ctx) = sent_speech(&whole).unwrap();
    assert!(ctx.stream.is_off());
    assert_eq!(ctx.audio_format, Some(AudioFormat::Wav));
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(body["input"]["language_type"], "German");
    let mut pcm = speech("qwen3-tts-flash", "Cherry");
    pcm.format = Some(AudioFormat::Pcm16);
    let (enc, ctx) = sent_speech(&pcm).unwrap();
    assert!(!ctx.stream.is_off());
    assert_eq!(enc.headers.get("x-dashscope-sse"), Some("enable"));
}

#[test]
fn speech_dashscope_cannot_make_is_refused_before_sending() {
    assert_eq!(speech_param(&speech("qwen3-tts-flash", "alloy")), "voice");
    assert_eq!(
        speech_param(&speech("qwen-audio-3.1-tts-plus", "Cherry")),
        "voice"
    );
    let mut none = speech("qwen3-tts-flash", "Cherry");
    none.voice = None;
    assert_eq!(speech_param(&none), "voice");
    let mut mp3 = speech("qwen3-tts-flash", "Cherry");
    mp3.format = Some(AudioFormat::Mp3);
    assert_eq!(speech_param(&mp3), "response_format");
    let mut flac = speech("qwen-audio-3.1-tts-flash", "longanhuan_v3.1");
    flac.format = Some(AudioFormat::Flac);
    assert_eq!(speech_param(&flac), "response_format");
    let mut fast = speech("qwen-audio-3.1-tts-flash", "longanhuan_v3.1");
    fast.speed = Some(4.0);
    assert_eq!(speech_param(&fast), "speed");
    let mut bitrate = speech("qwen-audio-3.1-tts-flash", "longanhuan_v3.1");
    bitrate.bitrate = Some(32);
    assert_eq!(speech_param(&bitrate), "bitrate");
    let mut lang = speech("qwen3-tts-flash", "Cherry");
    lang.language = Some("nl".into());
    assert_eq!(speech_param(&lang), "language");
    let mut instr = speech("qwen3-tts-flash", "Cherry");
    instr.instructions = Some("x".into());
    assert_eq!(speech_param(&instr), "instructions");
    let mut stamps = speech("qwen3-tts-flash", "Cherry");
    stamps.timestamps = true;
    assert_eq!(speech_param(&stamps), "timestamps");
    assert_eq!(
        speech_param(&speech("qwen3-tts-flash-realtime", "Cherry")),
        "model"
    );
    let mut music = speech("qwen3-tts-flash", "Cherry");
    music.operation = SpeechOperation::Music;
    assert_eq!(speech_param(&music), "model");
}

fn transcription(model: &str, source: Source) -> TranscriptionRequest {
    TranscriptionRequest::new(
        model,
        TranscriptionOperation::Transcribe,
        AudioInput::new(source),
    )
}

#[test]
fn transcription_fixtures_round_trip_and_read_the_billed_seconds() {
    let requests = round_trip_requests(RequestKind::Transcription, "transcriptions");
    let (_, Request::Transcription(r), _) =
        requests.iter().find(|(n, ..)| n == "asr-context").unwrap()
    else {
        panic!()
    };
    assert_eq!(r.prompt.as_deref(), Some("Alibaba Speech Lab"));
    assert_eq!(r.language.as_deref(), Some("en"));
    let replies = round_trip_replies(RequestKind::Transcription, "transcriptions");
    let Reply::Transcription(t) = &replies[0].1 else {
        panic!()
    };
    assert_eq!(t.text, "Hello World, this is Alibaba Speech Lab.");
    assert_eq!(t.audio_seconds(), Some(4.0));
}

#[test]
fn openai_transcriptions_name_their_format_and_refuse_timing() {
    let mut r = transcription(
        "qwen-audio-3.1-asr-flash",
        Source::base64("audio/mpeg", "AAAA"),
    );
    r.prompt = Some("Kaeso".into());
    r.language = Some("de".into());
    let req = Request::Transcription(r);
    let mut ctx = EgressCtx::new(&req);
    let enc = DashScope::encode_request(&req, &mut ctx).unwrap();
    assert_eq!(enc.path, GENERATION_PATH);
    assert_eq!(ctx.transcript_format, Some(TranscriptFormat::Json));
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        body["parameters"],
        json!({"format": "mp3", "language_hints": ["de"]})
    );
    assert_eq!(
        body["input"]["messages"][1]["content"][0]["input_audio"]["data"],
        "data:audio/mpeg;base64,AAAA"
    );
    let mut named = transcription(
        "qwen-audio-3.0-asr-flash",
        Source::base64("application/octet-stream", "AAAA"),
    );
    named.audio.filename = Some("clip.WAV".into());
    let req = Request::Transcription(named);
    let enc = DashScope::encode_request(&req, &mut EgressCtx::new(&req)).unwrap();
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(body["parameters"]["format"], "wav");

    let param = |r: TranscriptionRequest| {
        let req = Request::Transcription(r);
        DashScope::encode_request(&req, &mut EgressCtx::new(&req))
            .unwrap_err()
            .param
            .unwrap_or_default()
    };
    let link = || {
        transcription(
            "qwen-audio-3.1-asr-flash",
            Source::Url("https://example.com/a.ogg".into()),
        )
    };
    let mut srt = link();
    srt.format = Some(TranscriptFormat::Srt);
    assert_eq!(param(srt), "response_format");
    let mut diarize = link();
    diarize.diarize = Some(true);
    assert_eq!(param(diarize), "diarize");
    let mut translate = link();
    translate.operation = TranscriptionOperation::Translate;
    assert_eq!(param(translate), "model");
    assert_eq!(
        param(transcription(
            "qwen-audio-3.1-asr-flash",
            Source::Url("https://example.com/a.flac".into())
        )),
        "file"
    );
    assert_eq!(
        param(transcription(
            "qwen-audio-3.1-asr-flash-filetrans",
            Source::Url("https://example.com/a.wav".into())
        )),
        "model"
    );
    let ctx = EgressCtx::for_kind(RequestKind::Transcription, StreamMode::Off);
    assert!(
        DashScope::decode_response(200, &HeaderList::new(), br#"{"output":{}}"#, &ctx).is_err()
    );
    for bad in [
        &br#"{"model":"m","input":{"messages":[]}}"#[..],
        br#"{"model":"m","input":{"messages":[{"role":"assistant","content":[{"type":"text","text":"t"}]}]}}"#,
        br#"{"model":"m","input":{"messages":[{"role":"user","name":"x","content":[{"type":"input_text","text":"t"}]}]}}"#,
    ] {
        assert!(
            DashScope::decode_request(RequestKind::Transcription, bad, &HeaderList::new())
                .is_err()
        );
    }
}

#[test]
fn native_speech_requests_and_replies_keep_every_field() {
    let body = br#"{"model":"qwen-audio-3.0-tts-plus","input":{"text":"Hi","voice":"longanlingxin","format":"opus","bit_rate":64,"rate":1,"pitch":1.2,"language_hints":["en"],"seed":9,"instruction":"Calm."},"extra":true}"#;
    let req = DashScope::decode_request(RequestKind::Speech, body, &HeaderList::new()).unwrap();
    let Request::Speech(r) = &req else { panic!() };
    assert_eq!(
        (r.speed, r.language.as_deref(), r.bitrate),
        (Some(1.0), Some("en"), Some(64))
    );
    let mut ctx = EgressCtx::new(&req);
    let enc = DashScope::encode_request(&req, &mut ctx).unwrap();
    assert_json_eq("native speech", body, &enc.body);
    assert_eq!(ctx.audio_format, Some(AudioFormat::Opus));
    let auto = br#"{"model":"qwen3-tts-flash","input":{"text":"Hi","voice":"Cherry","language_type":"Auto"}}"#;
    let req = DashScope::decode_request(RequestKind::Speech, auto, &HeaderList::new()).unwrap();
    let enc = DashScope::encode_request(&req, &mut EgressCtx::new(&req)).unwrap();
    assert_json_eq("auto language", auto, &enc.body);

    let mut ctx = EgressCtx::for_kind(RequestKind::Speech, StreamMode::Off);
    ctx.audio_format = Some(AudioFormat::Opus);
    let whole =
        br#"{"output":{"audio":{"data":"AAAA","id":"a"},"finish_reason":"stop"},"request_id":"r"}"#;
    let reply = DashScope::decode_response(200, &HeaderList::new(), whole, &ctx).unwrap();
    let Reply::Speech(s) = &reply else { panic!() };
    assert!(matches!(&s.audio, Source::Base64(b) if b.media_type == "audio/ogg"));
    let back = DashScope::encode_response(
        &reply,
        &EncodeCtx::new(RequestKind::Speech, StreamMode::Off),
    )
    .unwrap();
    assert_json_eq("whole speech", whole, &back);
    for bad in [
        &br#"{"output":{"audio":{"data":""}}}"#[..],
        br#"{"output":{}}"#,
        br#"{}"#,
    ] {
        assert!(DashScope::decode_response(200, &HeaderList::new(), bad, &ctx).is_err());
    }
    let text = kaeso_kpp::SpeechResponse::new("m", Source::text("text/plain", "x"));
    assert!(
        DashScope::encode_response(
            &Reply::Speech(text),
            &EncodeCtx::new(RequestKind::Speech, StreamMode::Off)
        )
        .is_err()
    );
    let mut rate = speech("qwen3-tts-flash", "Cherry");
    rate.sample_rate = Some(16_000);
    assert_eq!(speech_param(&rate), "sample_rate");
    let mut opus = speech("qwen3-tts-flash", "Cherry");
    opus.format = Some(AudioFormat::Opus);
    assert_eq!(speech_param(&opus), "response_format");
    let mut seed = speech("qwen3-tts-flash", "Cherry");
    seed.seed = Some(1);
    assert_eq!(speech_param(&seed), "seed");
    let mut fast = speech("qwen3-tts-flash", "Cherry");
    fast.speed = Some(1.2);
    assert_eq!(speech_param(&fast), "speed");
    let mut long = speech("qwen3-tts-flash", "Cherry");
    long.duration = Some(3.0);
    assert_eq!(speech_param(&long), "duration");
    assert_eq!(
        speech_param(&speech("cosyvoice-v3-plus", "longxiaochun")),
        "model"
    );
    let mut ctx = EgressCtx::for_kind(RequestKind::Speech, StreamMode::on(false));
    ctx.audio_format = Some(AudioFormat::Mp3);
    let mut d = DashScope::stream_decoder(&ctx);
    let events = d.feed(
        b":HTTP_STATUS/429\ndata:{\"code\":\"Throttling.RateQuota\",\"message\":\"slow down\"}\n\n",
    );
    let [StreamEvent::Error(err)] = events.as_slice() else {
        panic!("{events:?}")
    };
    assert_eq!(
        (err.kind.clone(), err.status),
        (ErrorKind::RateLimited, Some(429))
    );
}

fn image_task(name: &str) -> Vec<u8> {
    fixtures(DIR, "polls/images", "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1
}

#[test]
fn older_wan_text_to_image_runs_as_a_task_kaeso_waits_for() {
    let mut r = image("wan2.2-t2i-flash");
    r.size = Some(ImageSize::pixels(1024, 1024));
    r.negative_prompt = Some("people".into());
    let req = Request::Image(r);
    let mut ctx = EgressCtx::new(&req);
    let enc = DashScope::encode_request(&req, &mut ctx).unwrap();
    assert_eq!(enc.path, TEXT2IMAGE_PATH);
    assert_eq!(enc.headers.get("x-dashscope-async"), Some("enable"));
    assert_eq!(
        serde_json::from_slice::<Value>(&enc.body).unwrap(),
        json!({"model": "wan2.2-t2i-flash", "input": {"prompt": "A red fox", "negative_prompt": "people"}, "parameters": {"n": 1, "size": "1024*1024"}})
    );
    for (name, pending) in [
        ("started", true),
        ("running", true),
        ("succeeded", false),
        ("failed", false),
    ] {
        assert_eq!(
            crate::dispatch::pending_task(
                DialectId::DashScope,
                RequestKind::Image,
                &image_task(name)
            )
            .is_some(),
            pending,
            "{name}"
        );
    }
    assert_eq!(
        crate::dispatch::pending_task(
            DialectId::DashScope,
            RequestKind::Video,
            &image_task("started")
        ),
        None
    );
    let poll = crate::dispatch::encode_task_poll(
        DialectId::DashScope,
        "7d4ac4b2-2e1a-4c84-8c6d-xxxxxx",
        &ctx,
    )
    .unwrap();
    assert_eq!(poll.path, "/api/v1/tasks/7d4ac4b2-2e1a-4c84-8c6d-xxxxxx");
    assert!(crate::dispatch::encode_task_poll(DialectId::DashScope, "..", &ctx).is_err());
    assert!(crate::dispatch::encode_task_poll(DialectId::OpenAiImages, "t", &ctx).is_err());

    let ictx = EgressCtx::for_kind(RequestKind::Image, StreamMode::Off);
    let body = image_task("succeeded");
    let Reply::Image(done) =
        DashScope::decode_response(200, &HeaderList::new(), &body, &ictx).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        done.images.len(),
        1,
        "the image that failed alone makes none"
    );
    assert!(
        done.images[0]
            .revised_prompt
            .as_deref()
            .unwrap()
            .contains("carved windows")
    );
    let back = DashScope::encode_response(
        &Reply::Image(done),
        &EncodeCtx::new(RequestKind::Image, StreamMode::Off),
    )
    .unwrap();
    assert_json_eq("succeeded", &body, &back);
    let err = DashScope::decode_response(200, &HeaderList::new(), &image_task("failed"), &ictx)
        .unwrap_err();
    assert!(err.message.contains("size is not match"), "{err:?}");
    let err = DashScope::decode_response(200, &HeaderList::new(), &image_task("running"), &ictx)
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::ProviderError);
}

#[test]
fn wan_text_to_image_takes_a_prompt_and_refuses_edits_tiers_and_more_than_four() {
    let sent = sent_image(&image("wan2.6-t2i")).unwrap();
    assert_eq!(sent["parameters"]["n"], 1);
    assert_eq!(
        sent["input"]["messages"][0]["content"],
        json!([{"text": "A red fox"}])
    );
    assert_eq!(image_param(&edit("wan2.6-t2i", 1)), "image");
    let mut many = image("wan2.2-t2i-plus");
    many.count = Some(5);
    let req = Request::Image(many);
    let err = DashScope::check(&req, &EgressCtx::new(&req)).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("n"));
    let mut tier = image("wan2.6-t2i");
    tier.resolution = Some("2K".into());
    assert_eq!(image_param(&tier), "resolution");
}
