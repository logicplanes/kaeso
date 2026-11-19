use std::time::Duration;

use kaeso_kpp::{
    ErrorKind, GeneratedImage, ImageOperation, ImageRequest, ImageResponse, ImageSize, KppError,
    Origin, Reply, Request, RequestKind, Source, StreamMode, VideoStatus,
};
use serde_json::{Value, json};

use super::*;

fn ctx(kind: RequestKind) -> EgressCtx {
    EgressCtx::for_kind(kind, StreamMode::Off)
}

fn decode(status: u16, body: &Value) -> KppError {
    Fal::decode_error(
        status,
        &HeaderList::new(),
        &serde_json::to_vec(body).unwrap(),
    )
}

fn body_of(enc: &EncodedError) -> Value {
    serde_json::from_slice(&enc.body).unwrap()
}

#[test]
fn every_error_type_reads_as_its_kind() {
    let cases = [
        ("content_policy_violation", ErrorKind::ContentFiltered),
        ("generation_timeout", ErrorKind::Timeout),
        ("request_timeout", ErrorKind::Timeout),
        ("startup_timeout", ErrorKind::Timeout),
        ("runner_scheduling_failure", ErrorKind::Overloaded),
        ("runner_connection_timeout", ErrorKind::Overloaded),
        ("runner_disconnected", ErrorKind::Overloaded),
        ("runner_connection_refused", ErrorKind::Overloaded),
        ("runner_connection_error", ErrorKind::Overloaded),
        ("downstream_service_unavailable", ErrorKind::Overloaded),
        ("internal_server_error", ErrorKind::ProviderError),
        ("internal_error", ErrorKind::ProviderError),
        ("downstream_service_error", ErrorKind::ProviderError),
        ("runner_incomplete_response", ErrorKind::ProviderError),
        ("runner_server_error", ErrorKind::ProviderError),
        ("no_media_generated", ErrorKind::ProviderError),
        ("concurrent_requests_limit", ErrorKind::RateLimited),
        ("bad_request", ErrorKind::InvalidRequest),
        ("client_disconnected", ErrorKind::InvalidRequest),
        ("client_cancelled", ErrorKind::InvalidRequest),
        ("a_type_kaeso_does_not_know", ErrorKind::PermissionDenied),
    ];
    for (ty, kind) in cases {
        let request = decode(403, &json!({"detail": "Refused.", "error_type": ty}));
        assert_eq!(
            (request.kind, request.message.as_str()),
            (kind.clone(), "Refused."),
            "{ty}"
        );
        let model = decode(
            403,
            &json!({"detail": [{"loc": ["body"], "msg": "m", "type": ty}]}),
        );
        assert_eq!(model.kind, kind, "{ty}");
        assert_eq!(model.param, None, "{ty}");
    }
    let by_type = decode(
        400,
        &json!({"detail": "Busy.", "type": "runner_disconnected"}),
    );
    assert_eq!(by_type.kind, ErrorKind::Overloaded);
}

#[test]
fn an_error_without_a_known_type_reads_its_kind_from_the_status() {
    let cases = [
        (400, ErrorKind::InvalidRequest),
        (405, ErrorKind::InvalidRequest),
        (409, ErrorKind::InvalidRequest),
        (422, ErrorKind::InvalidRequest),
        (401, ErrorKind::Authentication),
        (402, ErrorKind::Billing),
        (403, ErrorKind::PermissionDenied),
        (404, ErrorKind::NotFound),
        (413, ErrorKind::RequestTooLarge),
        (429, ErrorKind::RateLimited),
        (500, ErrorKind::ProviderError),
        (503, ErrorKind::Overloaded),
        (504, ErrorKind::Timeout),
    ];
    for (status, kind) in cases {
        let err = decode(status, &json!({"detail": "Something went wrong."}));
        assert_eq!(err.kind, kind, "{status}");
        assert_eq!(err.status, Some(status), "{status}");
        assert!(err.origin.as_ref().is_some_and(|o| o.is(D)), "{status}");
    }
}

#[test]
fn odd_error_bodies_still_say_what_happened() {
    let empty_list = decode(422, &json!({"detail": []}));
    assert_eq!(
        (
            empty_list.kind,
            empty_list.message.as_str(),
            empty_list.param
        ),
        (
            ErrorKind::InvalidRequest,
            "fal refused the request's input.",
            None
        )
    );
    let no_message = decode(
        422,
        &json!({"detail": [{"loc": ["query", "seed"], "type": "x"}]}),
    );
    assert_eq!(no_message.message, "fal refused the request's input.");
    assert_eq!(no_message.param.as_deref(), Some("query"));
    let unnamed = decode(422, &json!({"detail": [{"loc": [], "msg": "m"}]}));
    assert_eq!(unnamed.param, None);
    let platform = decode(401, &json!({"error": {"type": "authorization_error"}}));
    assert_eq!(platform.message, "fal returned HTTP 401.");
    let neither = decode(500, &json!({"detail": 5}));
    assert!(
        neither
            .message
            .starts_with("fal returned HTTP 500 with a body Kaeso could not read"),
        "{}",
        neither.message
    );
    assert_eq!(neither.raw, Some(json!({"detail": 5})));
}

#[test]
fn retry_after_is_read_and_written_back() {
    let err = Fal::decode_error(
        429,
        &HeaderList::new().with("retry-after", "12"),
        br#"{"detail": "Too many requests.", "error_type": "concurrent_requests_limit"}"#,
    );
    assert_eq!(err.retry_after, Some(Duration::from_secs(12)));
    let enc = Fal::encode_error(&err);
    assert_eq!(
        (enc.status, enc.headers.get("retry-after")),
        (429, Some("12"))
    );
    assert_eq!(body_of(&enc)["error_type"], "concurrent_requests_limit");
    let ours =
        KppError::new(ErrorKind::Overloaded, "Busy.").with_retry_after(Duration::from_millis(500));
    assert_eq!(
        Fal::encode_error(&ours).headers.get("retry-after"),
        Some("1")
    );
}

#[test]
fn errors_from_other_formats_get_fals_status_and_type() {
    let cases = [
        (ErrorKind::Authentication, 401, "value_error"),
        (ErrorKind::PermissionDenied, 403, "value_error"),
        (ErrorKind::NotFound, 404, "value_error"),
        (ErrorKind::RequestTooLarge, 413, "value_error"),
        (ErrorKind::RateLimited, 429, "value_error"),
        (ErrorKind::ProviderError, 500, "internal_server_error"),
        (ErrorKind::Overloaded, 503, "internal_server_error"),
        (ErrorKind::Timeout, 504, "generation_timeout"),
        (ErrorKind::ContentFiltered, 422, "content_policy_violation"),
        (ErrorKind::Billing, 422, "value_error"),
    ];
    for (kind, status, ty) in cases {
        let enc = Fal::encode_error(&KppError::new(kind.clone(), "m"));
        assert_eq!(enc.status, status, "{kind:?}");
        assert_eq!(enc.headers.get("content-type"), Some("application/json"));
        assert_eq!(
            body_of(&enc),
            json!({"detail": [{"loc": ["body"], "msg": "m", "type": ty}]}),
            "{kind:?}"
        );
    }
    let with_status = Fal::encode_error(&KppError::provider("m").with_status(502));
    assert_eq!(with_status.status, 502);
    let own_without_detail = KppError::new(ErrorKind::InvalidRequest, "Kaeso's words.")
        .with_origin(Origin::new(D))
        .with_raw(json!({"error": {"message": "x"}}));
    assert_eq!(
        body_of(&Fal::encode_error(&own_without_detail))["detail"][0]["msg"],
        "Kaeso's words."
    );
    let own = KppError::new(ErrorKind::InvalidRequest, "m")
        .with_origin(Origin::new(D))
        .with_raw(json!({"detail": "As fal sent it."}));
    let enc = Fal::encode_error(&own);
    assert_eq!(
        (enc.status, body_of(&enc)),
        (400, json!({"detail": "As fal sent it."}))
    );
}

#[test]
fn a_request_the_queue_completed_with_an_error_fails_with_its_kind() {
    let cases = [
        (
            json!({"status": "COMPLETED", "error": "Unsafe.", "error_type": "content_policy_violation"}),
            ErrorKind::ContentFiltered,
            400,
            "Unsafe.",
        ),
        (
            json!({"status": "COMPLETED", "error_type": "client_cancelled"}),
            ErrorKind::InvalidRequest,
            400,
            "The request was cancelled at fal.",
        ),
        (
            json!({"status": "COMPLETED", "error_type": "generation_timeout"}),
            ErrorKind::Timeout,
            504,
            "fal could not finish the request.",
        ),
        (
            json!({"status": "COMPLETED", "error_type": "runner_disconnected"}),
            ErrorKind::Overloaded,
            503,
            "fal could not finish the request.",
        ),
        (
            json!({"status": "COMPLETED", "error": "Broke."}),
            ErrorKind::ProviderError,
            502,
            "Broke.",
        ),
    ];
    for (body, kind, status, message) in cases {
        let err = error::failed(&body);
        assert_eq!(
            (err.kind, err.status, err.message.as_str()),
            (kind, Some(status), message),
            "{body}"
        );
        assert_eq!(err.raw, Some(body.clone()));
        let file =
            Fal::decode_video_file(200, &HeaderList::new(), &serde_json::to_vec(&body).unwrap())
                .unwrap_err();
        assert_eq!(file.status, Some(status), "{body}");
    }
    let job = Fal::decode_video_poll(
        200,
        &HeaderList::new(),
        br#"{"status": "COMPLETED", "error": "Broke.", "response_url": "https://queue.fal.run/fal-ai/veo3/requests/r1"}"#,
        &ctx(RequestKind::Video),
    )
    .unwrap();
    assert_eq!(job.status, VideoStatus::Failed);
    let failure = job.error.unwrap();
    assert_eq!(
        (failure.code.as_str(), failure.message.as_str()),
        ("failed", "Broke.")
    );
}

#[test]
fn queue_answers_that_are_not_json_are_provider_errors() {
    for err in [
        Fal::decode_video_poll(200, &HeaderList::new(), b"<html>", &ctx(RequestKind::Video))
            .unwrap_err(),
        Fal::decode_video_poll(
            200,
            &HeaderList::new(),
            br#"{"status": "SLEEPING"}"#,
            &ctx(RequestKind::Video),
        )
        .unwrap_err(),
        Fal::decode_video_file(200, &HeaderList::new(), b"<html>").unwrap_err(),
    ] {
        assert_eq!(err.kind, ErrorKind::ProviderError, "{err:?}");
    }
    let failed = Fal::decode_video_file(
        404,
        &HeaderList::new(),
        br#"{"detail": "Request not found."}"#,
    )
    .unwrap_err();
    assert_eq!(failed.kind, ErrorKind::NotFound);
}

#[test]
fn an_image_size_that_is_not_pixels_is_refused_and_a_file_id_cannot_be_written() {
    let mut r = ImageRequest::new("fal-ai/flux/dev", ImageOperation::Generate).with_prompt("a fox");
    r.size = Some(ImageSize::from_wire("portrait"));
    let err = Fal::check(&Request::Image(r), &ctx(RequestKind::Image)).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("size"), "{err:?}");

    let reply = Reply::Image(ImageResponse::new(
        "fal-ai/flux/dev",
        vec![GeneratedImage::new(Source::file_id(
            Origin::new(D),
            "file_1",
        ))],
    ));
    let err = Fal::encode_response(&reply, &EncodeCtx::new(RequestKind::Image, StreamMode::Off))
        .unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Unsupported { .. }), "{err:?}");
}
