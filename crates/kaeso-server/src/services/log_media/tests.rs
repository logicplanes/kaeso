use serde_json::{Value, json};

use super::*;

const PNG: [u8; 33] = [
    0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 3,
    0x20, 0, 0, 2, 0x58, 8, 6, 0, 0, 0, 0, 0, 0, 0,
];

fn b64(data: &[u8]) -> String {
    STANDARD_PADDED.encode(data)
}

fn off(text: &str) -> Option<String> {
    without_media(text, &mut Keep::nothing())
}

fn stripped(text: &str) -> Value {
    serde_json::from_str(&off(text).unwrap()).unwrap()
}

fn png_left_out() -> Value {
    json!({ "kaeso_media": {
        "media_type": "image/png", "bytes": 33, "width": 800, "height": 600, "reason": "not_kept"
    }})
}

// Bytes no signature knows, long enough to count as a file beside a media type.
fn noise(len: usize) -> String {
    b64(&(0..len)
        .map(|i| (i * 31 % 251) as u8 | 1)
        .collect::<Vec<_>>())
}

#[test]
fn every_way_an_app_sends_media_inline_is_left_out_with_its_type_and_size() {
    let png = b64(&PNG);
    let wav = b64(b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0");
    let pdf = b64(b"%PDF-1.7 a short document");
    let mp3 = b64(b"ID3\x04\0\0\0\0\0\0 a short sound");
    let chat = format!(
        r#"{{"messages":[{{"role":"user","content":[{{"type":"text","text":"What is this?"}},{{"type":"image_url","image_url":{{"url":"data:image/png;base64,{png}"}}}},{{"type":"input_audio","input_audio":{{"data":"{wav}","format":"wav"}}}},{{"type":"file","file":{{"filename":"a.pdf","file_data":"{pdf}"}}}}]}}]}}"#
    );
    let out = stripped(&chat);
    let parts = &out["messages"][0]["content"];
    assert_eq!(parts[0]["text"], "What is this?");
    assert_eq!(parts[1]["image_url"]["url"], png_left_out());
    assert_eq!(
        parts[2]["input_audio"]["data"]["kaeso_media"]["media_type"],
        "audio/wav"
    );
    assert_eq!(
        parts[3]["file"]["file_data"]["kaeso_media"],
        json!({ "media_type": "application/pdf", "bytes": 25, "reason": "not_kept" })
    );
    assert_eq!(parts[3]["file"]["filename"], "a.pdf");

    let anthropic = format!(
        r#"{{"messages":[{{"role":"user","content":[{{"type":"image","source":{{"data":"{png}","type":"base64","media_type":"image/png"}}}}]}}]}}"#
    );
    assert_eq!(
        stripped(&anthropic)["messages"][0]["content"][0]["source"]["data"],
        png_left_out()
    );

    let gemini = format!(
        r#"{{"contents":[{{"parts":[{{"inlineData":{{"mimeType":"application/pdf","data":"{pdf}"}}}},{{"inline_data":{{"mime_type":"audio/mpeg","data":"{mp3}"}}}}]}}]}}"#
    );
    let parts = &stripped(&gemini)["contents"][0]["parts"];
    assert_eq!(
        parts[0]["inlineData"]["data"]["kaeso_media"]["media_type"],
        "application/pdf"
    );
    assert_eq!(parts[1]["inline_data"]["data"]["kaeso_media"]["bytes"], 24);

    let reply = format!(
        r#"{{"id":"r","blocks":[{{"image":{{"source":{{"base64":{{"media_type":"image/png","data":"{png}"}}}}}}}}]}}"#
    );
    assert_eq!(
        stripped(&reply)["blocks"][0]["image"]["source"]["base64"]["data"],
        png_left_out()
    );
}

#[test]
fn a_body_no_parser_accepts_is_still_searched() {
    let png = b64(&PNG);
    let url = format!("data:image/png;base64,{png}");
    let deep = "[".repeat(300);
    for broken in [
        format!(r#"{{"messages":[{{"content":[{{"image_url":{{"url":"{url}"}}}}]"#),
        format!(r#"{{"a":"{url}",}}"#),
        format!(r#"{{"note":"\ud83d","a":"{url}"}}"#),
        format!(r#"{{"n":1e999,"a":"{url}"}}"#),
        format!(r#"{deep}{{"a":"{url}"}}"#),
        format!(r#"{{"a":"{url}"#),
        format!(r#"{{"a":"{url}"#).trim_end_matches('=').to_owned(),
    ] {
        assert!(serde_json::from_str::<Value>(&broken).is_err());
        let out = off(&broken).unwrap();
        assert!(!out.contains(&png[..20]), "{out}");
        assert!(out.contains(r#""reason":"not_kept""#), "{out}");
    }
    let cut = format!(
        r#"{{"model":"m","image":{{"media_type":"image/png","data":"{}"#,
        noise(120)
    );
    let out = off(&cut).unwrap();
    assert!(out.ends_with(r#""reason":"not_kept"}}"#), "{out}");
}

#[test]
fn media_inside_a_string_is_found() {
    let png = b64(&PNG);
    let arguments = serde_json::to_string(&json!({
        "path": "receipt.png",
        "upload": { "mimeType": "image/png", "data": png },
        "preview": format!("data:image/png;base64,{png}"),
    }))
    .unwrap();
    let output = serde_json::to_string(&json!([
        { "type": "image", "data": png, "mimeType": "image/png" }
    ]))
    .unwrap();
    let body = json!({
        "messages": [
            { "role": "assistant", "tool_calls": [{ "function": { "name": "save", "arguments": arguments } }] },
            { "role": "tool", "content": output },
            { "role": "user", "content": format!("Look: ![receipt](data:image/png;base64,{png}) and tell me.") },
        ],
        "input": [{ "type": "function_call_output", "output": format!("saved data:IMAGE/PNG;name=a.png;BASE64,{png}") }],
    })
    .to_string();
    let out = stripped(&body);
    let arguments: Value = serde_json::from_str(
        out["messages"][0]["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(arguments["path"], "receipt.png");
    assert_eq!(arguments["upload"]["data"], png_left_out());
    assert_eq!(arguments["preview"], png_left_out());
    let output: Value =
        serde_json::from_str(out["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(output[0]["data"], png_left_out());
    assert_eq!(
        out["messages"][2]["content"],
        "Look: ![receipt]([kaeso_media: image/png, 33 bytes, not_kept]) and tell me."
    );
    assert_eq!(
        out["input"][0]["output"],
        "saved [kaeso_media: IMAGE/PNG, 33 bytes, not_kept]"
    );
    assert!(!out.to_string().contains(&png[..20]));
}

#[test]
fn a_data_url_is_read_as_the_codecs_read_it() {
    let png = b64(&PNG);
    let long = format!("data:image/png;name={};base64,{png}", "n".repeat(600));
    for url in [
        format!("DATA:image/png;base64,{png}"),
        format!("Data:image/png;BASE64,{png}"),
        format!("  \n data:image/png;base64,{png}"),
        long,
        format!("data:image/png;base64,{}", png.replace('/', "\\/")),
    ] {
        let text = format!(r#"{{"url":"{}"}}"#, url.replace('\n', "\\n"));
        assert_eq!(stripped(&text)["url"], png_left_out(), "{url}");
    }
    let svg = "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%3E%3C%2Fsvg%3E";
    assert_eq!(
        stripped(&format!(r#"{{"url":"{svg}"}}"#))["url"]["kaeso_media"],
        json!({ "media_type": "image/svg+xml", "bytes": svg.len() - "data:image/svg+xml,".len(), "reason": "not_kept" })
    );
    assert_eq!(off(r#"{"url":"data:,hi"}"#), None);
}

#[test]
fn a_file_is_known_by_the_type_beside_it_whatever_its_key_or_by_how_it_starts() {
    let unknown = noise(60);
    let resource = format!(
        r#"{{"content":[{{"type":"resource","resource":{{"uri":"file:///a.bin","mimeType":"application/x-thing","blob":"{unknown}"}}}}]}}"#
    );
    assert_eq!(
        stripped(&resource)["content"][0]["resource"]["blob"]["kaeso_media"],
        json!({ "media_type": "application/x-thing", "bytes": 60, "reason": "not_kept" })
    );
    for key in ["b64_json", "bytes", "blob", "file_data"] {
        let text = format!(r#"{{"{key}":"{unknown}"}}"#);
        assert_eq!(stripped(&text)[key]["kaeso_media"]["bytes"], 60, "{key}");
    }
    let odd = format!(
        r#"{{"payload":"{}","content_type":"audio/pcm"}}"#,
        noise(900)
    );
    assert_eq!(
        stripped(&odd)["payload"]["kaeso_media"]["media_type"],
        "audio/pcm"
    );
    let pcm = format!(r#"{{"mediaType":"audio/pcm","data":"{}"}}"#, noise(120));
    assert_eq!(stripped(&pcm)["data"]["kaeso_media"]["bytes"], 120);

    for (start, media_type) in [
        (&b"II*\0 a scan"[..], "image/tiff"),
        (b"PK\x03\x04 a document", "application/zip"),
        (
            b"<svg xmlns='http://www.w3.org/2000/svg'/>",
            "image/svg+xml",
        ),
        (b"  <!DOCTYPE html><p>hi", "text/html"),
        (b"GIF89a\x02\x00\x03\x00 short", "image/gif"),
        (b"RIFF\0\0\0\0AVI LIST", "video/x-msvideo"),
    ] {
        let text = format!(r#"{{"result":"{}"}}"#, b64(start));
        assert_eq!(
            stripped(&text)["result"]["kaeso_media"]["media_type"],
            media_type,
            "known by its first bytes, however short"
        );
    }
    for (start, media_type) in [
        (&b"\xff\xfa\x90\0 a sound"[..], "audio/mpeg"),
        (b"\xff\xe3\x18\0 a sound", "audio/mpeg"),
        (b"\xff\xf1\x50\x80 a sound", "audio/aac"),
        (b"BM a small bitmap", "image/bmp"),
    ] {
        assert_eq!(sniff(start), Some((media_type, false)));
        let typed = format!(
            r#"{{"media_type":"{media_type}","value":"{}"}}"#,
            b64(start)
        );
        assert_eq!(
            stripped(&typed)["value"]["kaeso_media"]["media_type"],
            media_type
        );
        assert_eq!(off(&format!(r#"{{"value":"{}"}}"#, b64(start))), None);
    }
}

#[test]
fn media_without_a_stated_type_is_known_by_how_its_file_starts() {
    let mut jpeg = vec![
        0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc0, 0, 17, 8, 1, 0x40, 2, 0x80,
    ];
    jpeg.resize(400, 0);
    let text = format!(r#"{{"blob":"{}"}}"#, b64(&jpeg));
    assert_eq!(
        stripped(&text)["blob"],
        json!({ "kaeso_media": {
            "media_type": "image/jpeg", "bytes": 400, "width": 640, "height": 320, "reason": "not_kept"
        }})
    );
    for (start, media_type) in [
        (&b"GIF89a\x02\x00\x03\x00"[..], "image/gif"),
        (
            b"RIFF\0\0\0\0WEBPVP8X\0\0\0\0\0\0\0\0\x0f\0\0\x07\0\0",
            "image/webp",
        ),
        (b"RIFF\0\0\0\0WAVEfmt ", "audio/wav"),
        (b"%PDF-1.7", "application/pdf"),
        (b"ID3\x04", "audio/mpeg"),
        (b"OggS", "audio/ogg"),
        (b"fLaC", "audio/flac"),
        (b"\0\0\0\x18ftypmp42", "video/mp4"),
        (b"\0\0\0\x18ftypM4A ", "audio/mp4"),
        (b"\x1a\x45\xdf\xa3", "video/webm"),
    ] {
        assert_eq!(sniff(start), Some((media_type, true)));
    }
    assert_eq!(dimensions(b"GIF89a\x02\x00\x03\x00"), Some((2, 3)));
    assert_eq!(
        dimensions(b"RIFF\0\0\0\0WEBPVP8X\0\0\0\0\0\0\0\0\x0f\0\0\x07\0\0"),
        Some((16, 8))
    );
}

#[test]
fn text_that_only_looks_like_media_stays() {
    let signature = "c2lnbmF0dXJl".repeat(40);
    let sound_like = b64(b"\xff\xfb\x90\0 an encrypted thought, not a sound, of some length here");
    let kept = [
        r#"{"messages":[{"role":"user","content":"data: the plan, base64 or not"}]}"#.to_owned(),
        format!(r#"{{"content":[{{"type":"thinking","thinking":"hm","signature":"{signature}"}}]}}"#),
        format!(r#"{{"reasoning":{{"opaque":[{{"kind":"anthropic_redacted_thinking","data":"{sound_like}"}}]}}}}"#),
        format!(r#"{{"type":"reasoning","encrypted_content":"{sound_like}","media_type":"x/y"}}"#),
        r#"{"source":{"type":"text","media_type":"text/plain","data":"SGVsbG8gd29ybGQgYWdhaW4="}}"#.to_owned(),
        r#"{"blocks":[{"document":{"source":{"text":{"media_type":"text/plain","data":"SGVsbG8gd29ybGQgYWdhaW4="}}}}]}"#.to_owned(),
        r#"{"image_url":{"url":"https://example.com/base64/cat.png"}}"#.to_owned(),
        r#"{"response_format":{"type":"json_schema","format":"wav","name":"configuration_name_1"}}"#.to_owned(),
        r#"{"media_type":"image/png","name":"a_reasonably_long_file_name","id":"msg_0123456789abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz"}"#.to_owned(),
        r#"{"arguments":"{\"city\":\"Paris\",\"note\":\"plain words\"}"}"#.to_owned(),
        format!(r#"{{"mimeType":"inode/directory","location":"/{}"}}"#, "projects/".repeat(9)),
        format!(r#"{{"content_type":"application/json","sha256":"{}"}}"#, "0123456789abcdef".repeat(4)),
        format!(r#"{{"content_type":"application/json","digest":"{}"}}"#, "0123456789abcdef".repeat(80)),
        r#"{"mimeType":"application/json","result":"completed_successfully"}"#.to_owned(),
        r#"{"image":"library/postgres-alpine","mimeType":"application/vnd.oci.image"}"#.to_owned(),
        format!(r#"{{"resource":{{"uri":"file:///a.txt","mimeType":"text/plain","text":"{}"}}}}"#, "token".repeat(300)),
        format!(r#"{{"mimeType":"text/plain","name":"{}","path":"{}"}}"#, "n".repeat(1100), "p".repeat(1100)),
        format!(r#"{{"content":"{}"}}"#, "please summarise the following report for the team ".repeat(100)),
        r#"{"data":"configuration_name_1","media_type":"application/json"}"#.to_owned(),
    ];
    for text in kept {
        assert_eq!(off(&text), None, "{text}");
    }
}

#[test]
fn only_the_media_itself_is_swapped_and_the_rest_stays_byte_for_byte() {
    let png = b64(&PNG);
    let text = format!(
        r#"{{"z":1.50,"a":"café \"x\"","big":12345678901234567890123,"{png}":"a key that reads like the file","same":"{png}x","url":"data:image\/png;base64,{}"}}"#,
        png.replace('/', "\\/")
    );
    let out = off(&text).unwrap();
    assert_eq!(
        out,
        format!(
            r#"{{"z":1.50,"a":"café \"x\"","big":12345678901234567890123,"{png}":"a key that reads like the file","same":"{png}x","url":{}}}"#,
            png_left_out()
        )
    );
}

#[test]
fn a_body_that_is_not_json_is_logged_as_its_type_and_size_never_as_its_text() {
    let mut upload = PNG.to_vec();
    upload.extend([0xff, 0xfe]);
    let logged: Value = serde_json::from_str(&body(&upload, &mut Keep::nothing())).unwrap();
    assert_eq!(logged["kaeso_media"]["media_type"], "image/png");
    assert_eq!(logged["kaeso_media"]["bytes"], 35);

    let form = b"--kb\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.svg\"\r\n\r\n<svg/>\r\n--kb--\r\n";
    let logged: Value = serde_json::from_str(&body(form, &mut Keep::nothing())).unwrap();
    assert_eq!(
        logged["kaeso_media"],
        json!({ "media_type": "multipart/form-data", "bytes": form.len(), "reason": "not_kept" })
    );
    let logged: Value = serde_json::from_str(&body(b"prompt=hello", &mut Keep::nothing())).unwrap();
    assert_eq!(logged["kaeso_media"]["media_type"], "text/plain");
    assert_eq!(body(b" {\"a\":1}", &mut Keep::nothing()), " {\"a\":1}");
    assert_eq!(body(b"", &mut Keep::nothing()), "");
}

#[test]
fn decoded_media_is_left_out_before_it_is_written() {
    let mut image = Source::bytes("image/png", PNG.to_vec());
    let mut link = Source::Url("https://example.com/a.png".into());
    let mut left_out = LeftOut::default();
    let mut keep = Keep::nothing();
    left_out.fit(&mut image, &mut keep);
    left_out.fit(&mut link, &mut keep);
    let written: Value =
        serde_json::from_str(&left_out.write(&vec![image, link]).unwrap()).unwrap();
    assert_eq!(written[0]["base64"]["data"], png_left_out());
    assert_eq!(written[1]["url"], "https://example.com/a.png");
    assert_eq!(LeftOut::default().write(&"plain").unwrap(), "\"plain\"");
}

fn within(limit: std::time::Duration, work: impl FnOnce() -> Option<String>) -> Option<String> {
    let started = std::time::Instant::now();
    let out = work();
    assert!(started.elapsed() < limit, "took {:?}", started.elapsed());
    out
}

#[test]
fn nesting_no_format_has_is_logged_as_one_file_at_once() {
    let limit = std::time::Duration::from_secs(5);
    let strings = "\"AAAAAAAAAAAAAAAA\",".repeat((1 << 20) / 38);
    let nested = format!("{}{strings}", "[".repeat(1 << 19));
    let brackets = "[".repeat(8 << 20);
    let keyed = format!(r#"{{"{}":{}"#, "k".repeat(200), "[".repeat(8 << 20));
    let objects = r#"{"a":"#.repeat(1 << 20);
    for body in [nested, brackets, keyed, objects] {
        let out = within(limit, || off(&body)).unwrap();
        let out: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            out["kaeso_media"],
            json!({ "media_type": "application/json", "bytes": body.len(), "reason": "not_kept" })
        );
    }
    let wide = format!(r#"{{"data":[{}"x"]}}"#, strings.repeat(8));
    assert_eq!(
        within(limit, || off(&wide)),
        None,
        "many strings in one array are read once"
    );
    let at_the_limit = format!("{}{}", "[".repeat(128), "]".repeat(128));
    assert_eq!(off(&at_the_limit), None);
}

#[test]
fn a_flood_of_schemes_is_read_once() {
    let flood = serde_json::to_string(&"data:".repeat(400_000)).unwrap();
    assert_eq!(
        within(std::time::Duration::from_secs(5), || off(&flood)),
        None
    );
    let headers = serde_json::to_string(&"data:image/png;".repeat(150_000)).unwrap();
    assert_eq!(
        within(std::time::Duration::from_secs(5), || off(&headers)),
        None
    );
}

fn wrapped(text: &str, every: usize, with: &str) -> String {
    text.as_bytes()
        .chunks(every)
        .map(|line| std::str::from_utf8(line).unwrap())
        .collect::<Vec<_>>()
        .join(with)
}

#[test]
fn base64_that_is_wrapped_or_not_quite_valid_is_still_a_file() {
    let mut file = PNG.to_vec();
    file.resize(600, 5);
    let clean = b64(&file);
    for (name, text) in [
        ("lines of 76", wrapped(&clean, 76, "\n")),
        (
            "lines of 60 with carriage returns",
            wrapped(&clean, 60, "\r\n"),
        ),
        (
            "a stray space",
            format!("{} {}", &clean[..300], &clean[300..]),
        ),
        ("one character too many", format!("{clean}A")),
    ] {
        let typed =
            json!({ "source": { "type": "base64", "media_type": "image/png", "data": text } })
                .to_string();
        let media = &stripped(&typed)["source"]["data"]["kaeso_media"];
        assert_eq!(media["media_type"], "image/png", "{name}");
        assert_eq!(media["width"], 800, "{name}");
        assert!(
            media["bytes"].as_u64().unwrap().abs_diff(600) <= 1,
            "{name}"
        );
        let keyed = json!({ "data": [{ "b64_json": text }] }).to_string();
        assert!(
            stripped(&keyed)["data"][0]["b64_json"]["kaeso_media"].is_object(),
            "{name}"
        );
        let url = json!({ "url": format!("data:image/png;base64,{text}") }).to_string();
        assert_eq!(
            stripped(&url)["url"]["kaeso_media"]["media_type"],
            "image/png",
            "{name}"
        );
        let inside =
            json!({ "content": format!("See data:image/png;base64,{text}\nand tell me.") })
                .to_string();
        let out = off(&inside).unwrap();
        assert!(
            !out.contains(&clean[80..120]) && !out.contains(&clean[700..740]),
            "{name}: {out}"
        );
    }
    let noisy = noise(6000).replace('A', "!");
    assert!(noisy.contains('!') && base64_of(&noisy).is_none());
    assert_eq!(
        stripped(&json!({ "anything": noisy }).to_string())["anything"]["kaeso_media"]["media_type"],
        "application/octet-stream",
        "long text that is nearly all base64 alphabet is a file"
    );
}

#[test]
fn a_file_inside_a_text_block_is_found_by_how_it_starts() {
    let png = b64(&PNG);
    let result = format!(
        r#"{{"type":"tool_result","content":[{{"type":"text","text":"{png}"}},{{"type":"text","text":"SGVsbG8gd29ybGQgYWdhaW4="}}]}}"#
    );
    let out = stripped(&result);
    assert_eq!(out["content"][0]["text"], png_left_out());
    assert_eq!(out["content"][1]["text"], "SGVsbG8gd29ybGQgYWdhaW4=");
}

#[test]
fn audio_is_found_where_nothing_states_a_type_the_search_knows() {
    let sound = noise(1200);
    let short = noise(150);
    for (text, at, media_type) in [
        (
            format!(r#"{{"input_audio":{{"data":"{short}","format":"aiff"}}}}"#),
            "/input_audio/data",
            "audio/aiff",
        ),
        (
            format!(r#"{{"audio":{{"id":"audio_abc","data":"{sound}"}}}}"#),
            "/audio/data",
            "application/octet-stream",
        ),
        (
            format!(r#"{{"input_audio":"{sound}"}}"#),
            "/input_audio",
            "application/octet-stream",
        ),
        (
            format!(r#"{{"type":"audio/pcm","data":"{short}"}}"#),
            "/data",
            "audio/pcm",
        ),
        (
            format!(
                r#"{{"media_type":"audio/{}","data":"{short}"}}"#,
                "x".repeat(200)
            ),
            "/data",
            "application/octet-stream",
        ),
    ] {
        let out = stripped(&text);
        assert_eq!(
            out.pointer(at).unwrap()["kaeso_media"]["media_type"],
            media_type,
            "{text:.80}"
        );
    }
}

#[test]
fn media_hidden_by_escapes_depth_or_quotes_is_found() {
    let png = b64(&PNG);
    let escaped = format!(r#"{{"url":"\u0064\u0061ta:image/png\u003bbase64\u002c{png}"}}"#);
    assert_eq!(stripped(&escaped)["url"], png_left_out());
    assert_eq!(
        unescaped(r"\u+041 \u-041"),
        r"u+041 u-041",
        "a signed number is no escape"
    );

    let mut deep = json!({ "media_type": "image/png", "data": noise(300) }).to_string();
    for _ in 0..7 {
        deep = json!({ "arguments": deep }).to_string();
    }
    let out = off(&deep).unwrap();
    assert!(!out.contains(&noise(300)[40..120]), "{out}");
    assert!(out.contains("kaeso_media"), "{out}");

    let svg = "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%3E%3C%2Fsvg%3E";
    let text = json!({ "content": format!("![a]({svg}) and ![b](data:image/png;name=\"a b.png\";base64,{png}) end") }).to_string();
    assert_eq!(
        stripped(&text)["content"],
        format!(
            "![a]([kaeso_media: image/svg+xml, {} bytes, not_kept]) and ![b]([kaeso_media: image/png, 33 bytes, not_kept]) end",
            svg.len() - "data:image/svg+xml,".len()
        )
        .as_str()
    );
}

#[test]
fn nothing_an_app_sends_can_pass_for_a_file_taken_out() {
    let mut image = Source::bytes("image/png", PNG.to_vec());
    let mut left_out = LeftOut::default();
    left_out.fit(&mut image, &mut Keep::nothing());
    let old_mark = b64(b"kaeso media left out 0");
    let prompt = format!("a quote \" then \"{old_mark}");
    let text = left_out
        .write(&json!({ "prompt": prompt, "old": old_mark, "image": image }))
        .unwrap();
    let written: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(written["prompt"], prompt.as_str());
    assert_eq!(written["old"], old_mark.as_str());
    assert_eq!(written["image"]["base64"]["data"], png_left_out());
    let mut again = Source::bytes("image/png", PNG.to_vec());
    let mut other = LeftOut::default();
    other.fit(&mut again, &mut Keep::nothing());
    assert_ne!(
        serde_json::to_string(&image).unwrap(),
        serde_json::to_string(&again).unwrap()
    );
}

#[test]
fn a_file_where_a_type_belongs_is_found_and_never_repeated_as_the_type() {
    let both = r#"{"media_type":"see data:audio/wav;base64,AAAAAAAAAAAAAAAA here","text":"data:image/png;base64,AAAAAAAAAAAAAAAA"}"#;
    let out = stripped(both);
    assert_eq!(
        out["media_type"],
        "see [kaeso_media: audio/wav, 12 bytes, not_kept] here"
    );
    assert_eq!(out["text"]["kaeso_media"]["media_type"], "image/png");

    let mut file = PNG.to_vec();
    file.resize(45, 0);
    let png = b64(&file);
    let out = off(&format!(r#"{{"media_type":"{png}","data":"{png}"}}"#)).unwrap();
    assert!(!out.contains(&png[..16]), "{out}");
    assert_eq!(
        serde_json::from_str::<Value>(&out).unwrap()["data"]["kaeso_media"]["media_type"],
        "image/png",
        "the type the bytes show, not the text that stood where a type belongs"
    );
}

mod property {
    use proptest::prelude::*;

    use super::*;

    fn media() -> impl Strategy<Value = String> {
        let bytes = prop::collection::vec(any::<u8>(), 12..200);
        prop_oneof![
            bytes.clone().prop_map(|b| b64(&b)),
            bytes
                .clone()
                .prop_map(|b| format!("data:image/png;base64,{}", b64(&b))),
            bytes.clone().prop_map(|mut b| {
                b.splice(0..0, PNG);
                b64(&b)
            }),
            bytes.prop_map(|b| format!("see data:audio/wav;base64,{} here", b64(&b))),
        ]
    }

    fn key() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("data".to_owned()),
            Just("media_type".to_owned()),
            Just("mimeType".to_owned()),
            Just("type".to_owned()),
            Just("format".to_owned()),
            Just("text".to_owned()),
            Just("b64_json".to_owned()),
            Just("audio".to_owned()),
            "[a-z]{1,6}",
        ]
    }

    fn leaf() -> impl Strategy<Value = Value> {
        prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::from),
            any::<i64>().prop_map(Value::from),
            any::<String>().prop_map(Value::from),
            "[ -~]{0,40}".prop_map(Value::from),
            Just(Value::from("image/png")),
            Just(Value::from("text")),
            Just(Value::from("wav")),
            media().prop_map(Value::from),
        ]
    }

    fn document() -> impl Strategy<Value = Value> {
        leaf().prop_recursive(5, 48, 6, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
                prop::collection::vec((key(), inner.clone()), 0..6)
                    .prop_map(|fields| Value::Object(fields.into_iter().collect())),
                inner.prop_map(|v| Value::from(v.to_string())),
            ]
        })
    }

    fn left_out(v: &Value) -> bool {
        v.as_object()
            .is_some_and(|o| o.len() == 1 && o.get("kaeso_media").is_some_and(Value::is_object))
    }

    // Whether a string could hold a file at all: only such a string may differ in the log.
    fn could_hold_media(text: &str) -> bool {
        let mut run = 0;
        text.bytes().any(|b| {
            run = if base64_byte(b) || b == b'=' {
                run + 1
            } else {
                0
            };
            run >= MEDIA_MIN_CHARS
        })
    }

    fn same_but_for_media(before: &Value, after: &Value) -> Result<(), TestCaseError> {
        match (before, after) {
            (Value::Array(a), Value::Array(b)) => {
                prop_assert_eq!(a.len(), b.len());
                a.iter()
                    .zip(b)
                    .try_for_each(|(a, b)| same_but_for_media(a, b))
            }
            (Value::Object(a), Value::Object(b)) => {
                prop_assert!(a.keys().eq(b.keys()));
                a.values()
                    .zip(b.values())
                    .try_for_each(|(a, b)| same_but_for_media(a, b))
            }
            (Value::String(a), after) if Some(a.as_str()) != after.as_str() => {
                prop_assert!(could_hold_media(a), "{a:?} became {after}");
                prop_assert!(left_out(after) || after.is_string(), "{a:?} became {after}");
                Ok(())
            }
            (a, b) => {
                prop_assert_eq!(a, b);
                Ok(())
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn valid_json_stays_valid_and_only_media_changes(doc in document(), pretty in any::<bool>()) {
            let text = if pretty {
                serde_json::to_string_pretty(&doc).unwrap()
            } else {
                doc.to_string()
            };
            match off(&text) {
                None => {}
                Some(out) => {
                    let after: Value = serde_json::from_str(&out)
                        .map_err(|e| TestCaseError::fail(format!("{e}: {out}")))?;
                    same_but_for_media(&doc, &after)?;
                    prop_assert_eq!(off(&out), None, "what is left holds no file: {}", out);
                }
            }
        }

        #[test]
        fn any_text_is_searched_without_failing(text in "\\PC{0,400}", cut in 0usize..400) {
            let _ = off(&text);
            let json = json!({ "media_type": "image/png", "data": text, "nested": [text] }).to_string();
            let end = json.char_indices().map(|(i, _)| i).nth(cut).unwrap_or(json.len());
            let _ = off(&json[..end]);
        }
    }
}
