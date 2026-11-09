use kaeso_kpp::{
    ErrorKind, ImageSize, ListOrder, Reply, Request, RequestKind, Source, VideoImage,
    VideoImageRole, VideoJob, VideoList, VideoOperation, VideoRequest, VideoStatus, VideoVariant,
};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{assert_json_eq, fixtures, sidecar};

const DIR: &str = "openai_videos";

fn headers_for(name: &str, folder: &str) -> HeaderList {
    match sidecar(DIR, folder, name, "meta") {
        Some(meta) => HeaderList::new().with(
            "content-type",
            meta["content_type"].as_str().unwrap_or("application/json"),
        ),
        None => HeaderList::new(),
    }
}

fn decode(body: &[u8], headers: &HeaderList) -> VideoRequest {
    match OpenAiVideos::decode_request(RequestKind::Video, body, headers).unwrap() {
        Request::Video(r) => r,
        other => panic!("{other:?}"),
    }
}

fn encode(profile: VideosProfile, req: &VideoRequest) -> Result<EncodedRequest, KppError> {
    let r = Request::Video(req.clone());
    let mut ctx = EgressCtx::new(&r).with_videos_profile(profile);
    OpenAiVideos::encode_request(&r, &mut ctx)
}

fn ctx(profile: VideosProfile) -> EgressCtx {
    EgressCtx::for_kind(RequestKind::Video, kaeso_kpp::StreamMode::Off).with_videos_profile(profile)
}

fn own(folder: &str, ext: &str) -> Vec<(String, Vec<u8>)> {
    fixtures(DIR, folder, ext)
        .into_iter()
        .filter(|(n, _)| !n.starts_with("providers/"))
        .collect()
}

fn remix_of(body: &[u8]) -> VideoRequest {
    match decode_remix(body, &HeaderList::new()).unwrap() {
        Request::Video(r) => r,
        other => panic!("{other:?}"),
    }
}

#[test]
fn every_json_request_round_trips() {
    for (name, body) in own("requests", "json") {
        if name == "remix" {
            let mut r = remix_of(&body);
            assert_eq!(r.operation, VideoOperation::Remix);
            r.remix_of = Some("video_123".into());
            r.model = "sora-2".into();
            r.validate().unwrap();
            let enc = encode(VideosProfile::OPENAI, &r).unwrap();
            assert_eq!(enc.path, "/v1/videos/video_123/remix");
            assert_json_eq(&name, &body, &enc.body);
            continue;
        }
        let r = decode(&body, &HeaderList::new());
        r.validate().unwrap();
        let enc = encode(VideosProfile::OPENAI, &r).unwrap();
        assert_eq!(enc.path, "/v1/videos", "{name}");
        assert_eq!(enc.headers.get("content-type"), Some("application/json"));
        assert_json_eq(&name, &body, &enc.body);
    }
}

#[test]
fn an_upload_round_trips_part_for_part() {
    let (name, body) = own("requests", "multipart").remove(0);
    let headers = headers_for(&name, "requests");
    let r = decode(&body, &headers);
    assert_eq!(r.model, "sora-2");
    assert_eq!(r.seconds, Some(8));
    assert_eq!(r.size, Some(ImageSize::pixels(1280, 720)));
    let image = &r.images[0];
    assert_eq!(image.role, VideoImageRole::FirstFrame);
    assert_eq!(image.filename.as_deref(), Some("first.png"));
    let enc = encode(VideosProfile::OPENAI, &r).unwrap();
    let ct = enc.headers.get("content-type").unwrap().to_owned();
    let again = decode(&enc.body, &HeaderList::new().with("content-type", ct));
    assert_eq!(again, r);
}

#[test]
fn the_sdks_seconds_as_a_number_and_unknown_fields_survive() {
    let body = br#"{"model":"m","prompt":"p","seconds":8,"input_reference":{"image_url":"data:image/png;base64,AAAA","detail":"high"},"extra":true}"#;
    let r = decode(body, &HeaderList::new());
    assert_eq!(r.seconds, Some(8));
    assert!(matches!(&r.images[0].source, Source::Base64(b) if b.to_base64() == "AAAA"));
    assert!(r.extras.fields.contains_key("extra"));
    let enc = encode(VideosProfile::OPENAI, &r).unwrap();
    assert_json_eq("numbers", body, &enc.body);
}

#[test]
fn broken_requests_name_their_field() {
    let param = |body: &[u8]| {
        OpenAiVideos::decode_request(RequestKind::Video, body, &HeaderList::new())
            .unwrap_err()
            .param
    };
    assert_eq!(
        param(br#"{"prompt":"p","size":"big"}"#).as_deref(),
        Some("size")
    );
    assert_eq!(
        param(br#"{"prompt":"p","seconds":"8.5"}"#).as_deref(),
        Some("seconds")
    );
    assert_eq!(param(br#"{"prompt":7}"#).as_deref(), Some("prompt"));
    assert_eq!(
        param(br#"{"prompt":"p","input_reference":{}}"#).as_deref(),
        Some("input_reference")
    );
    assert_eq!(
        param(br#"{"prompt":"p","input_reference":{"image_url":"u","file_id":"f"}}"#).as_deref(),
        Some("input_reference")
    );
    let form = b"--b\r\nContent-Disposition: form-data; name=\"video\"; filename=\"a.mp4\"\r\n\r\nx\r\n--b--\r\n";
    let err = OpenAiVideos::decode_request(
        RequestKind::Video,
        form,
        &HeaderList::new().with("content-type", "multipart/form-data; boundary=b"),
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("video"));
    let twice = b"--b\r\nContent-Disposition: form-data; name=\"prompt\"\r\n\r\nx\r\n--b\r\nContent-Disposition: form-data; name=\"prompt\"\r\n\r\ny\r\n--b--\r\n";
    let err = OpenAiVideos::decode_request(
        RequestKind::Video,
        twice,
        &HeaderList::new().with("content-type", "multipart/form-data; boundary=b"),
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("prompt"));
}

#[test]
fn every_reply_round_trips_as_a_video_resource() {
    for (name, body) in own("responses", "json") {
        let reply = OpenAiVideos::decode_response(
            200,
            &HeaderList::new(),
            &body,
            &ctx(VideosProfile::OPENAI),
        )
        .unwrap();
        let out = OpenAiVideos::encode_response(
            &reply,
            &EncodeCtx::new(RequestKind::Video, kaeso_kpp::StreamMode::Off),
        )
        .unwrap();
        assert_json_eq(&name, &body, &out);
        let polled = OpenAiVideos::decode_video_poll(
            200,
            &HeaderList::new(),
            &body,
            &ctx(VideosProfile::OPENAI),
        )
        .unwrap();
        assert_eq!(Reply::Video(polled), reply, "{name}");
    }
}

#[test]
fn a_failed_job_keeps_its_error_and_a_remix_its_source() {
    let read = |name: &str| {
        let (_, body) = own("responses", "json")
            .into_iter()
            .find(|(n, _)| n == name)
            .unwrap();
        OpenAiVideos::decode_video_poll(200, &HeaderList::new(), &body, &ctx(VideosProfile::OPENAI))
            .unwrap()
    };
    let failed = read("failed");
    assert_eq!(failed.status, VideoStatus::Failed);
    assert_eq!(failed.error.unwrap().code, "moderation_blocked");
    let remix = read("remix-queued");
    assert_eq!(remix.remixed_from.as_deref(), Some("video_123"));
    assert_eq!(remix.status, VideoStatus::Queued);
    let done = read("completed");
    assert_eq!((done.progress, done.seconds), (Some(100), Some(8)));
    assert_eq!(done.expires_at, Some(1_712_784_240));
}

#[test]
fn lists_and_deletions_are_written_as_openai_writes_them() {
    let (_, page) = own("lists", "json")
        .into_iter()
        .find(|(n, _)| n == "page")
        .unwrap();
    let original: Value = serde_json::from_slice(&page).unwrap();
    let item = serde_json::to_vec(&original["data"][0]).unwrap();
    let job = OpenAiVideos::decode_video_poll(
        200,
        &HeaderList::new(),
        &item,
        &ctx(VideosProfile::OPENAI),
    )
    .unwrap();
    let out = encode_list(&VideoList::new(vec![job], false)).unwrap();
    assert_json_eq("page", &page, &out);
    let empty: Value =
        serde_json::from_slice(&encode_list(&VideoList::default()).unwrap()).unwrap();
    assert_eq!(empty["first_id"], Value::Null);
    let (_, deleted) = own("lists", "json")
        .into_iter()
        .find(|(n, _)| n == "deleted")
        .unwrap();
    assert_json_eq("deleted", &deleted, &encode_deleted("video_123").unwrap());
}

#[test]
fn a_job_without_a_size_names_one_from_its_tier_and_ratio() {
    let mut job = VideoJob::new("video_1", VideoStatus::InProgress);
    job.resolution = Some("720p".into());
    job.aspect_ratio = Some("9:16".into());
    let v: Value = serde_json::from_slice(
        &OpenAiVideos::encode_response(
            &Reply::Video(job.clone()),
            &EncodeCtx::new(RequestKind::Video, kaeso_kpp::StreamMode::Off),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(v["size"], "720x1280");
    assert_eq!(v["seconds"], "");
    assert_eq!(v["progress"], 0);
    job.resolution = None;
    let v: Value = serde_json::from_slice(
        &OpenAiVideos::encode_response(
            &Reply::Video(job),
            &EncodeCtx::new(RequestKind::Video, kaeso_kpp::StreamMode::Off),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(v["size"], "");
}

#[test]
fn queries_read_as_openai_documents_them() {
    let q = decode_list_query("after=video_9&limit=5&order=asc").unwrap();
    assert_eq!(
        (q.after.as_deref(), q.limit, q.order),
        (Some("video_9"), Some(5), ListOrder::Asc)
    );
    assert_eq!(decode_list_query("").unwrap().limit(), 20);
    assert_eq!(decode_list_query("after=").unwrap().after, None);
    assert_eq!(
        decode_list_query("limit=101").unwrap_err().param.as_deref(),
        Some("limit")
    );
    assert_eq!(
        decode_list_query("order=newest")
            .unwrap_err()
            .param
            .as_deref(),
        Some("order")
    );
    assert_eq!(decode_content_query("").unwrap(), VideoVariant::Video);
    assert_eq!(
        decode_content_query("variant=thumbnail&x=%41").unwrap(),
        VideoVariant::Thumbnail
    );
    assert_eq!(
        decode_content_query("variant=gif")
            .unwrap_err()
            .param
            .as_deref(),
        Some("variant")
    );
    assert_eq!(unescape("a%2Fb+c%zz"), "a/b c%zz");
}

#[test]
fn together_gets_its_own_body_and_paths() {
    let mut r = VideoRequest::new("Wan-AI/wan2.7-t2v", VideoOperation::Generate)
        .with_prompt("A cartoon of an astronaut riding a horse on the moon");
    r.size = Some(ImageSize::pixels(1280, 720));
    r.seconds = Some(5);
    r.seed = Some(42);
    r.negative_prompt = Some("blurry".into());
    r.audio = Some(true);
    let enc = encode(VideosProfile::TOGETHER, &r).unwrap();
    assert_eq!(enc.path, "/v2/videos");
    let golden = |name: &str| {
        fixtures(DIR, "requests", "json")
            .into_iter()
            .find(|(n, _)| n == &format!("providers/togetherai/{name}"))
            .unwrap()
            .1
    };
    assert_json_eq("together create", &golden("create"), &enc.body);
    let mut frames = VideoRequest::new("Wan-AI/wan2.7-i2v", VideoOperation::Generate)
        .with_prompt("The astronaut waves");
    frames.resolution = Some("720p".into());
    frames.aspect_ratio = Some("16:9".into());
    for (url, role) in [
        ("https://example.com/first.png", VideoImageRole::FirstFrame),
        ("https://example.com/last.png", VideoImageRole::LastFrame),
        ("https://example.com/style.png", VideoImageRole::Reference),
    ] {
        frames = frames.with_image(VideoImage::new(Source::Url(url.into()), role));
    }
    let enc = encode(VideosProfile::TOGETHER, &frames).unwrap();
    assert_json_eq("together frames", &golden("create-frames"), &enc.body);
    let c = ctx(VideosProfile::TOGETHER);
    assert_eq!(
        OpenAiVideos::encode_video_poll("a/b", "m", &c)
            .unwrap()
            .path,
        "/v2/videos/a%2Fb"
    );
    assert!(
        OpenAiVideos::encode_video_delete("j", "m", &c)
            .unwrap()
            .is_none()
    );
    let del = OpenAiVideos::encode_video_delete("j", "m", &ctx(VideosProfile::OPENAI))
        .unwrap()
        .unwrap();
    assert_eq!((del.method, del.path.as_str()), ("DELETE", "/v1/videos/j"));
}

#[test]
fn together_takes_the_apps_other_fields_and_refuses_what_it_cannot_carry() {
    let app = decode(
        br#"{"model":"Wan-AI/wan2.7-t2v","prompt":"A fox","steps":30,"guidance_scale":7}"#,
        &HeaderList::new(),
    );
    let enc = encode(VideosProfile::TOGETHER, &app).unwrap();
    let body: Value = serde_json::from_slice(&enc.body).unwrap();
    assert_eq!(
        (body["steps"].as_u64(), body["guidance_scale"].as_u64()),
        (Some(30), Some(7))
    );
    let app = decode(
        br#"{"model":"Wan-AI/wan2.7-i2v","prompt":"A fox","input_reference":{"image_url":"https://example.com/a.png","detail":"high"}}"#,
        &HeaderList::new(),
    );
    let err = encode(VideosProfile::TOGETHER, &app).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("input_reference"));
}

#[test]
fn together_replies_round_trip_and_name_the_video_link() {
    for (name, body) in fixtures(DIR, "responses", "json")
        .into_iter()
        .filter(|(n, _)| n.starts_with("providers/togetherai/"))
    {
        let job = OpenAiVideos::decode_video_poll(
            200,
            &HeaderList::new(),
            &body,
            &ctx(VideosProfile::TOGETHER),
        )
        .unwrap();
        let out = serde_json::to_vec(&reply::together_value(&job)).unwrap();
        assert_json_eq(&name, &body, &out);
        if name.ends_with("completed") {
            let video = job.output(VideoVariant::Video).unwrap();
            assert!(matches!(&video.source, Source::Url(u) if u.ends_with("example-video.mp4")));
        }
    }
    let queued =
        br#"{"id":"x","status":"queued","model":"m","created_at":1,"size":"","seconds":"5"}"#;
    assert!(
        OpenAiVideos::decode_video_poll(
            200,
            &HeaderList::new(),
            queued,
            &ctx(VideosProfile::TOGETHER)
        )
        .is_err(),
        "Together documents no queued status"
    );
}

#[test]
fn providers_refuse_what_they_lack_before_sending() {
    let base = VideoRequest::new("m", VideoOperation::Generate).with_prompt("p");
    let refusal = |p: VideosProfile, r: &VideoRequest| {
        let err = encode(p, r).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
        err.param.unwrap_or_default()
    };
    let mut seed = base.clone();
    seed.seed = Some(1);
    assert_eq!(refusal(VideosProfile::OPENAI, &seed), "seed");
    let upload = base.clone().with_image(
        VideoImage::new(
            Source::base64("image/png", "AAAA").unwrap(),
            VideoImageRole::FirstFrame,
        )
        .with_filename("a.png"),
    );
    assert_eq!(refusal(VideosProfile::TOGETHER, &upload), "input_reference");
    let reference = base.clone().with_image(VideoImage::new(
        Source::Url("u".into()),
        VideoImageRole::Reference,
    ));
    assert_eq!(
        refusal(VideosProfile::OPENAI, &reference),
        "input_reference"
    );
    let mut remix = VideoRequest::new("m", VideoOperation::Remix).with_prompt("p");
    remix.remix_of = Some("j".into());
    assert_eq!(refusal(VideosProfile::TOGETHER, &remix), "video_id");
    let other = Request::Image(kaeso_kpp::ImageRequest::new(
        "m",
        kaeso_kpp::ImageOperation::Generate,
    ));
    let mut c = EgressCtx::new(&other);
    assert!(OpenAiVideos::encode_request(&other, &mut c).is_err());
    let mut foreign = base;
    foreign.extras = kaeso_kpp::Extras::new(kaeso_kpp::DialectId::XaiVideos).with_field(
        "keyframes",
        kaeso_kpp::RawJson::from_serialize(&json!([])).unwrap(),
    );
    assert!(encode(VideosProfile::OPENAI, &foreign).is_err());
}

#[test]
fn errors_round_trip_in_openais_shape() {
    for (name, body) in own("errors", "json") {
        let status = sidecar(DIR, "errors", &name, "meta").unwrap()["status"]
            .as_u64()
            .unwrap() as u16;
        let err = OpenAiVideos::decode_error(status, &HeaderList::new(), &body);
        assert_eq!(err.param.as_deref(), Some("size"));
        let enc = OpenAiVideos::encode_error(&err);
        assert_eq!(enc.status, status);
        assert_json_eq(&name, &body, &enc.body);
    }
    let decoded = OpenAiVideos::decode_response(
        400,
        &HeaderList::new(),
        br#"{"error":{"message":"no"}}"#,
        &ctx(VideosProfile::OPENAI),
    )
    .unwrap_err();
    assert_eq!(decoded.message, "no");
    let mut s = OpenAiVideos::stream_decoder(&ctx(VideosProfile::OPENAI));
    assert_eq!(s.feed(b"x").len(), 1);
    assert!(s.finish().is_empty());
    let mut e = OpenAiVideos::stream_encoder(&EncodeCtx::new(
        RequestKind::Video,
        kaeso_kpp::StreamMode::Off,
    ));
    assert!(
        e.encode(&kaeso_kpp::StreamEvent::Error(KppError::provider("x")))
            .is_err()
    );
    assert!(e.finish().unwrap().is_empty());
}

#[test]
fn odd_replies_are_kept_or_refused_as_they_should_be() {
    let c = ctx(VideosProfile::OPENAI);
    let poll = |b: &[u8]| OpenAiVideos::decode_video_poll(200, &HeaderList::new(), b, &c);
    assert!(poll(br#"{"id":"v","object":"file","status":"queued"}"#).is_err());
    assert!(poll(br#"{"id":"v","object":"video","status":"thinking"}"#).is_err());
    assert!(poll(br#"{"id":"v","status":"queued","created_at":"yesterday"}"#).is_err());
    let job = poll(br#"{"id":"v","status":"failed","created_at":1712697600.7,"seconds":"eight","error":{"code":"c","message":"m","param":"size","type":"invalid_request_error","extra":1}}"#).unwrap();
    assert_eq!(job.created_at, Some(1_712_697_600));
    assert_eq!(job.seconds, None);
    let e = job.error.clone().unwrap();
    assert_eq!(
        (e.param.as_deref(), e.kind.as_deref()),
        (Some("size"), Some("invalid_request_error"))
    );
    let back: Value = serde_json::from_slice(
        &OpenAiVideos::encode_response(
            &Reply::Video(job),
            &EncodeCtx::new(RequestKind::Video, kaeso_kpp::StreamMode::Off),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(back["seconds"], "eight");
    assert_eq!(back["error"]["extra"], 1);
    let numbered = poll(br#"{"id":"v","status":"completed","seconds":8}"#).unwrap();
    let back: Value = serde_json::from_slice(
        &OpenAiVideos::encode_response(
            &Reply::Video(numbered),
            &EncodeCtx::new(RequestKind::Video, kaeso_kpp::StreamMode::Off),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        (back["seconds"].clone(), back["progress"].clone()),
        (json!(8), json!(100))
    );
}

#[test]
fn a_remix_reads_from_json_and_uploads_and_files_stay_out() {
    let form = b"--b\r\nContent-Disposition: form-data; name=\"prompt\"\r\n\r\nsnow\r\n--b--\r\n";
    let h = HeaderList::new().with("content-type", "multipart/form-data; boundary=b");
    let Request::Video(r) = decode_remix(form, &h).unwrap() else {
        panic!()
    };
    assert_eq!(
        (r.prompt.as_deref(), r.operation),
        (Some("snow"), VideoOperation::Remix)
    );
    let upload = b"--b\r\nContent-Disposition: form-data; name=\"input_reference\"; filename=\"a.png\"\r\n\r\nx\r\n--b--\r\n";
    assert!(decode_remix(upload, &h).is_err());
    let twice = b"--b\r\nContent-Disposition: form-data; name=\"input_reference\"; filename=\"a.png\"\r\n\r\nx\r\n--b\r\nContent-Disposition: form-data; name=\"input_reference\"; filename=\"b.png\"\r\n\r\ny\r\n--b--\r\n";
    assert!(OpenAiVideos::decode_request(RequestKind::Video, twice, &h).is_err());
    let both = b"--b\r\nContent-Disposition: form-data; name=\"input_reference[image_url]\"\r\n\r\nhttps://example.com/a.png\r\n--b\r\nContent-Disposition: form-data; name=\"input_reference\"; filename=\"b.png\"\r\n\r\ny\r\n--b--\r\n";
    assert!(OpenAiVideos::decode_request(RequestKind::Video, both, &h).is_err());
    let binary =
        b"--b\r\nContent-Disposition: form-data; name=\"prompt\"\r\n\r\n\xff\xfe\r\n--b--\r\n";
    assert!(OpenAiVideos::decode_request(RequestKind::Video, binary, &h).is_err());
    let big = HeaderList::new().with("content-type", "multipart/form-data");
    assert!(OpenAiVideos::decode_request(RequestKind::Video, form, &big).is_err());
    let mut many = Vec::new();
    for i in 0..40 {
        many.extend_from_slice(
            format!("--b\r\nContent-Disposition: form-data; name=\"f{i}\"\r\n\r\nx\r\n").as_bytes(),
        );
    }
    many.extend_from_slice(b"--b--\r\n");
    assert!(OpenAiVideos::decode_request(RequestKind::Video, &many, &h).is_err());
    let r = decode(
        br#"{"model":"m","prompt":"p","input_reference":{"file_id":"file-1"}}"#,
        &HeaderList::new(),
    );
    let enc = encode(VideosProfile::OPENAI, &r).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&enc.body).unwrap()["input_reference"]["file_id"],
        "file-1"
    );
    let two = r.clone().with_image(VideoImage::new(
        Source::Url("u".into()),
        VideoImageRole::FirstFrame,
    ));
    assert!(encode(VideosProfile::OPENAI, &two).is_err());
}
