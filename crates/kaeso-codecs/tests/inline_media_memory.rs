#![cfg(test)]

// The only test in its binary: the allocator counts every thread of the process.

use base64::Engine as _;
use bytes::Bytes;
use kaeso_codecs::limits::Checked;
use kaeso_codecs::{EgressCtx, HeaderList, dispatch};
use kaeso_kpp::{DialectId, Request, RequestKind};
use peak_alloc::PeakAlloc;

#[global_allocator]
static ALLOC: PeakAlloc = PeakAlloc;

const IMAGE_BYTES: usize = 4 << 20;

fn image() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut x: u32 = 0x9e37_79b9;
    while bytes.len() < IMAGE_BYTES {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        bytes.push(x.to_le_bytes()[0]);
    }
    bytes
}

fn chat(b64: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": [
            {"type": "text", "text": "What is in this image?"},
            {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b64}")}},
        ]}],
    }))
    .unwrap()
}

fn messages(b64: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "model": "claude-sonnet-4-5",
        "max_tokens": 64,
        "messages": [{"role": "user", "content": [
            {"type": "text", "text": "What is in this image?"},
            {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": b64}},
        ]}],
    }))
    .unwrap()
}

// The most `work` held at once beyond what was held before it.
fn measure<T>(work: impl FnOnce() -> T) -> (T, usize) {
    let before = ALLOC.current_usage();
    ALLOC.reset_peak_usage();
    let out = work();
    (out, ALLOC.peak_usage().saturating_sub(before))
}

fn decode(from: DialectId, body: Bytes) -> Request {
    let checked = Checked::new(from, body).unwrap();
    let req = dispatch::decode_checked_request(RequestKind::Generate, &checked, &HeaderList::new())
        .unwrap();
    req.validate().unwrap();
    req
}

#[test]
fn an_inline_image_is_held_once_while_a_request_is_decoded_and_once_more_while_it_is_encoded() {
    let b64 = base64::engine::general_purpose::STANDARD.encode(image());
    let paths = [
        (
            "Chat to Chat",
            DialectId::OpenAiChat,
            DialectId::OpenAiChat,
            chat(&b64),
        ),
        (
            "Chat to Responses",
            DialectId::OpenAiChat,
            DialectId::OpenAiResponses,
            chat(&b64),
        ),
        (
            "Messages to Messages",
            DialectId::AnthropicMessages,
            DialectId::AnthropicMessages,
            messages(&b64),
        ),
        (
            "Messages to Responses",
            DialectId::AnthropicMessages,
            DialectId::OpenAiResponses,
            messages(&b64),
        ),
    ];
    drop(b64);
    for (path, from, to, body) in paths {
        let body = Bytes::from(body);
        let size = body.len();
        let (req, decoding) = measure(|| decode(from, body.clone()));
        let (sent, encoding) = measure(|| {
            let mut ctx = EgressCtx::new(&req);
            dispatch::encode_request(to, &req, &mut ctx).unwrap().body
        });
        // Decoding holds the decoded image and no copy of its base64: 0.75 of the body. It
        // used to read each message, part and URL as text first, 3.8 to 5 times the body.
        assert!(
            decoding <= size,
            "{path}: decoding took {decoding} bytes at its peak for a body of {size}"
        );
        // Encoding writes the base64 once, into a body of its final size, beside the decoded
        // image: 1 times the body. It used to build the data URL and every layer of JSON around
        // it, 3.8 to 5.8 times the body.
        assert!(
            encoding <= size + size / 8,
            "{path}: encoding took {encoding} bytes at its peak for a body of {size}"
        );
        assert_eq!(
            sent.capacity(),
            sent.len(),
            "{path}: the body is sized once"
        );
        assert!(
            sent.len() > IMAGE_BYTES * 4 / 3,
            "{path}: the image is sent"
        );
    }
}
