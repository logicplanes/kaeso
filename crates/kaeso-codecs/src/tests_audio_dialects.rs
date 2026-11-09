use kaeso_kpp::{
    AudioFormat, DialectId, ErrorKind, ModelsQuery, Reply, Request, RequestKind, Source,
    SpeechOperation, SpeechRequest, SpeechResponse, StreamEvent, StreamMode, TranscriptionResponse,
};

use crate::codec::{EgressCtx, EncodeCtx, HeaderList};
use crate::dispatch;

const AUDIO: [DialectId; 3] = [
    DialectId::OpenAiSpeech,
    DialectId::OpenAiTranscriptions,
    DialectId::MinimaxSpeech,
];

fn chat_request() -> Request {
    dispatch::decode_request(
        DialectId::OpenAiChat,
        RequestKind::Generate,
        br#"{"model":"m","messages":[{"role":"user","content":"Hi"}]}"#,
        &HeaderList::new(),
    )
    .unwrap()
}

fn unsupported(err: &kaeso_kpp::KppError) -> bool {
    matches!(err.kind, ErrorKind::Unsupported { .. })
}

fn other_reply(d: DialectId) -> Reply {
    if d == DialectId::OpenAiTranscriptions {
        Reply::Speech(SpeechResponse::new(
            "m",
            Source::base64("audio/mpeg", "SUQz").unwrap(),
        ))
    } else {
        Reply::Transcription(TranscriptionResponse::new("m", "Hi"))
    }
}

#[test]
fn audio_dialects_refuse_generation_and_answer_model_calls_in_their_shape() {
    let chat = chat_request();
    let error = br#"{"error":{"message":"No such model.","type":"invalid_request_error"}}"#;
    for d in AUDIO {
        let mut ctx = EgressCtx::new(&chat);
        assert!(
            unsupported(&dispatch::check(d, &chat, &ctx).unwrap_err()),
            "{d}"
        );
        assert!(dispatch::encode_request(d, &chat, &mut ctx).is_err(), "{d}");
        let gen_ctx = EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off);
        assert!(dispatch::decode_response(d, 200, &HeaderList::new(), b"{}", &gen_ctx).is_err());
        let failed =
            dispatch::decode_response(d, 404, &HeaderList::new(), error, &gen_ctx).unwrap_err();
        assert_eq!(failed.status, Some(404), "{d}");
        assert!(
            dispatch::encode_response(
                d,
                &other_reply(d),
                &EncodeCtx::new(RequestKind::Speech, StreamMode::Off)
            )
            .is_err(),
            "{d}"
        );
        assert_eq!(
            dispatch::decode_models(d, 401, &HeaderList::new(), error)
                .unwrap_err()
                .status,
            Some(401)
        );
        assert_eq!(
            dispatch::decode_model(d, 404, &HeaderList::new(), error)
                .unwrap_err()
                .status,
            Some(404)
        );
        assert_eq!(
            dispatch::encode_model_request(d, "m").unwrap().path,
            "/v1/models/m"
        );
        assert_eq!(
            dispatch::encode_models_request(d, &ModelsQuery::default())
                .unwrap()
                .path,
            "/v1/models"
        );
        assert_eq!(
            dispatch::decode_error(d, 429, &HeaderList::new(), error).status,
            Some(429)
        );
        assert_eq!(
            dispatch::reads_reply_as_stream(d, &gen_ctx),
            d == DialectId::OpenAiSpeech,
            "audio streams through as it arrives"
        );
    }
}

#[test]
fn decoding_through_dispatch_checks_the_shape_first() {
    let speech = br#"{"model":"tts-1","input":"Hi","voice":"alloy"}"#;
    let checked = crate::limits::Checked::new(DialectId::OpenAiSpeech, speech.as_slice()).unwrap();
    let req = dispatch::decode_checked_request(RequestKind::Speech, &checked, &HeaderList::new())
        .unwrap();
    assert_eq!(req.kind(), RequestKind::Speech);
    let t2a = br#"{"model":"speech-2.8-hd","text":"Hi"}"#;
    let checked = crate::limits::Checked::new(DialectId::MinimaxSpeech, t2a.as_slice()).unwrap();
    assert!(
        dispatch::decode_checked_request(RequestKind::Speech, &checked, &HeaderList::new()).is_ok()
    );
    let boundary = "kaeso";
    let upload = format!(
        "--{boundary}\r\ncontent-disposition: form-data; name=\"model\"\r\n\r\nwhisper-1\r\n\
         --{boundary}\r\ncontent-disposition: form-data; name=\"file\"; filename=\"a.mp3\"\r\n\
         content-type: audio/mpeg\r\n\r\nID3\r\n--{boundary}--\r\n"
    );
    let headers = HeaderList::new().with(
        "content-type",
        format!("multipart/form-data; boundary={boundary}"),
    );
    let checked =
        crate::limits::Checked::new(DialectId::OpenAiTranscriptions, upload.as_bytes()).unwrap();
    for req in [
        dispatch::decode_checked_request(RequestKind::Transcription, &checked, &headers).unwrap(),
        dispatch::decode_checked_transcription_request(
            kaeso_kpp::TranscriptionOperation::Translate,
            &checked,
            &headers,
        )
        .unwrap(),
    ] {
        assert_eq!(req.kind(), RequestKind::Transcription);
    }
    let speak = SpeechRequest::new("tts-1", SpeechOperation::Speech).with_input("Hi");
    let ctx = EncodeCtx::for_request(Request::Speech(speak.clone()));
    let wav = Reply::Speech(SpeechResponse::new(
        "m",
        Source::base64("audio/wav", "UklG").unwrap(),
    ));
    assert_eq!(
        dispatch::stream_media_type(DialectId::OpenAiSpeech, &ctx),
        "audio/mpeg"
    );
    assert_eq!(
        dispatch::response_media_type(DialectId::OpenAiSpeech, &wav, &ctx),
        "audio/wav"
    );
    for (format, media_type) in [
        (AudioFormat::Opus, "audio/opus"),
        (AudioFormat::Aac, "audio/aac"),
        (AudioFormat::Flac, "audio/flac"),
        (AudioFormat::Wav, "audio/wav"),
        (AudioFormat::Pcm16, "audio/pcm"),
    ] {
        let mut asked = speak.clone();
        asked.format = Some(format);
        let ctx = EncodeCtx::for_request(Request::Speech(asked));
        assert_eq!(
            dispatch::response_media_type(DialectId::OpenAiSpeech, &wav, &ctx),
            media_type
        );
    }
}

#[test]
fn broken_audio_streams_end_with_one_error() {
    let speak = Request::Speech(
        SpeechRequest::new("gpt-4o-mini-tts", SpeechOperation::Speech).with_input("Hi"),
    );
    for d in [DialectId::OpenAiSpeech, DialectId::MinimaxSpeech] {
        let mut ctx = EgressCtx::new(&speak);
        ctx.stream = StreamMode::on(true);
        let mut dec = dispatch::stream_decoder(d, &ctx).unwrap();
        let mut events = dec.feed(b"data: {not json\n\n");
        events.extend(dec.finish());
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, StreamEvent::Error(_)))
                .count(),
            1,
            "{d}: {events:?}"
        );
    }
}

#[test]
fn uploads_in_a_shared_body_are_slices_of_it_and_other_bodies_are_copied() {
    let boundary = "kaeso";
    let form = |file: &str| {
        format!(
            "--{boundary}\r\ncontent-disposition: form-data; name=\"model\"\r\n\r\nm\r\n\
             --{boundary}\r\ncontent-disposition: form-data; name=\"{file}\"; filename=\"a.png\"\r\n\
             content-type: image/png\r\n\r\n\x7fPNG\r\n--{boundary}--\r\n"
        )
        .into_bytes()
    };
    let headers = HeaderList::new().with(
        "content-type",
        format!("multipart/form-data; boundary={boundary}"),
    );
    let inside = |data: &bytes::Bytes, body: &[u8]| {
        let start = body.as_ptr() as usize;
        (start..start + body.len()).contains(&(data.as_ptr() as usize))
    };
    let audio = |req: Request| match req {
        Request::Transcription(r) => match r.audio.source {
            Source::Base64(b) => b.data,
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    let image = |req: Request| match req {
        Request::Image(r) => match &r.images[0].source {
            Source::Base64(b) => b.data.clone(),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };

    let body = bytes::Bytes::from(form("file"));
    let checked =
        crate::limits::Checked::new(DialectId::OpenAiTranscriptions, body.clone()).unwrap();
    let shared = audio(
        dispatch::decode_checked_transcription_request(
            kaeso_kpp::TranscriptionOperation::Transcribe,
            &checked,
            &headers,
        )
        .unwrap(),
    );
    assert_eq!(&shared[..], b"\x7fPNG");
    assert!(inside(&shared, &body), "a slice of the request body");

    let owned = form("file");
    let checked =
        crate::limits::Checked::new(DialectId::OpenAiTranscriptions, owned.clone()).unwrap();
    let copied = audio(
        dispatch::decode_checked_request(RequestKind::Transcription, &checked, &headers).unwrap(),
    );
    assert_eq!(&copied[..], b"\x7fPNG");
    assert!(!inside(&copied, checked.body()));

    let body = bytes::Bytes::from(form("image"));
    let checked = crate::limits::Checked::new(DialectId::OpenAiImages, body.clone()).unwrap();
    for req in [
        dispatch::decode_checked_image_request(kaeso_kpp::ImageOperation::Edit, &checked, &headers)
            .unwrap(),
        dispatch::decode_checked_request(RequestKind::Image, &checked, &headers).unwrap(),
    ] {
        assert!(inside(&image(req), &body));
    }
}
