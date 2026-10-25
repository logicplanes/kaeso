#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::App;
use common::copy::{MAX_WRITTEN, problem, sent_problem};
use kaeso_testkit::services::TestServices;

const SCANNED: &[&str] = &[
    "crates/kaeso-server/src/platform",
    "crates/kaeso-server/src/auth",
    "crates/kaeso-server/src/services/accounts.rs",
    "crates/kaeso-server/src/services/admin_sso.rs",
    "crates/kaeso-server/src/services/agents.rs",
    "crates/kaeso-server/src/services/limits.rs",
    "crates/kaeso-server/src/services/managed.rs",
    "crates/kaeso-server/src/services/orgs.rs",
    "crates/kaeso-server/src/services/users.rs",
    "crates/kaeso-core/src/catalogue/mod.rs",
    "crates/kaeso-core/src/email.rs",
    "crates/kaeso-core/src/keys.rs",
    "crates/kaeso-core/src/limits.rs",
    "crates/kaeso-core/src/rights/mod.rs",
    "crates/kaeso-core/src/rights/presets.rs",
    "crates/kaeso-crypto/src/password.rs",
];

const NOT_COPY: &[&str] = &["crates/kaeso-server/src/platform/mod.rs"];

const LOG_MACROS: &[&str] = &[
    "trace",
    "debug",
    "info",
    "warn",
    "error",
    "trace_span",
    "debug_span",
    "info_span",
    "warn_span",
    "error_span",
    "panic",
    "unreachable",
];

#[derive(Debug, PartialEq)]
enum Token {
    Str { text: String, line: usize },
    Ident(String),
    Punct(char),
}

fn tokens(src: &str) -> Vec<Token> {
    let s: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1;
    while i < s.len() {
        let c = s[i];
        let next = s.get(i + 1).copied();
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if c == '/' && next == Some('/') {
            while i < s.len() && s[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            i += 2;
            while i + 1 < s.len() && !(s[i] == '*' && s[i + 1] == '/') {
                line += usize::from(s[i] == '\n');
                i += 1;
            }
            i += 2;
        } else if c == '"' || (c == 'r' && raw_string_at(&s, i)) {
            let start_line = line;
            let mut hashes = 0;
            let raw = c == 'r';
            if raw {
                i += 1;
                while s[i] == '#' {
                    hashes += 1;
                    i += 1;
                }
            }
            i += 1;
            let mut text = String::new();
            loop {
                let ch = s[i];
                if !raw && ch == '\\' {
                    let esc = s[i + 1];
                    match esc {
                        '\n' => {
                            line += 1;
                            i += 2;
                            while s[i].is_whitespace() {
                                line += usize::from(s[i] == '\n');
                                i += 1;
                            }
                            continue;
                        }
                        'n' => text.push('\n'),
                        other => text.push(other),
                    }
                    i += 2;
                    continue;
                }
                if ch == '"' && s[i + 1..].iter().take(hashes).all(|h| *h == '#') {
                    i += 1 + hashes;
                    break;
                }
                line += usize::from(ch == '\n');
                text.push(ch);
                i += 1;
            }
            out.push(Token::Str {
                text,
                line: start_line,
            });
        } else if c == '\'' {
            if next == Some('\\') {
                i += 3;
                while s[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else if s.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                i += 1;
            }
        } else if c.is_alphanumeric() || c == '_' {
            let start = i;
            while i < s.len() && (s[i].is_alphanumeric() || s[i] == '_') {
                i += 1;
            }
            out.push(Token::Ident(s[start..i].iter().collect()));
        } else {
            out.push(Token::Punct(c));
            i += 1;
        }
    }
    out
}

fn raw_string_at(s: &[char], i: usize) -> bool {
    let hashes = s[i + 1..].iter().take_while(|c| **c == '#').count();
    s.get(i + 1 + hashes) == Some(&'"')
}

fn skip_group(t: &[Token], open: usize) -> usize {
    let mut depth = 0usize;
    let mut i = open;
    while i < t.len() {
        match t[i] {
            Token::Punct('(' | '[' | '{') => depth += 1,
            Token::Punct(')' | ']' | '}') => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    i
}

fn is_ident(t: Option<&Token>, name: &str) -> bool {
    matches!(t, Some(Token::Ident(n)) if n == name)
}

struct Copy {
    line: usize,
    text: String,
    in_error: bool,
}

fn api_error_args(t: &[Token], i: usize) -> Option<usize> {
    if !is_ident(t.get(i), "ApiError")
        || t.get(i + 1) != Some(&Token::Punct(':'))
        || t.get(i + 2) != Some(&Token::Punct(':'))
    {
        return None;
    }
    let Some(Token::Ident(variant)) = t.get(i + 3) else {
        return None;
    };
    let opens = matches!(t.get(i + 4), Some(Token::Punct('(' | '{')));
    let carries_message = !matches!(variant.as_str(), "internal" | "Internal" | "not_found");
    (opens && carries_message).then(|| skip_group(t, i + 4))
}

fn copy_in(src: &str) -> Vec<Copy> {
    let t = tokens(src);
    let mut out = Vec::new();
    let mut in_error_until = 0;
    let mut i = 0;
    while i < t.len() {
        if let Some(end) = api_error_args(&t, i) {
            in_error_until = in_error_until.max(end);
        }
        match &t[i] {
            Token::Punct('#') if matches!(t.get(i + 1), Some(Token::Punct('['))) => {
                let end = skip_group(&t, i + 1);
                let test_only = is_ident(t.get(i + 2), "cfg") && is_ident(t.get(i + 4), "test")
                    || is_ident(t.get(i + 2), "test");
                if test_only {
                    let mut j = end;
                    while j < t.len() && !matches!(t[j], Token::Punct('{' | ';')) {
                        j += 1;
                    }
                    i = if matches!(t.get(j), Some(Token::Punct('{'))) {
                        skip_group(&t, j)
                    } else {
                        j + 1
                    };
                } else if is_ident(t.get(i + 2), "error") {
                    i += 2;
                } else {
                    i = end;
                }
            }
            Token::Ident(name)
                if LOG_MACROS.contains(&name.as_str())
                    && matches!(t.get(i + 1), Some(Token::Punct('!'))) =>
            {
                i = skip_group(&t, i + 2);
            }
            Token::Ident(name)
                if name == "expect" && matches!(t.get(i + 1), Some(Token::Punct('('))) =>
            {
                i = skip_group(&t, i + 1);
            }
            Token::Str { text, line } => {
                let in_error = i < in_error_until;
                if reads_as_copy(text) || (in_error && text.contains(' ')) {
                    out.push(Copy {
                        line: *line,
                        text: text.clone(),
                        in_error,
                    });
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

fn reads_as_copy(text: &str) -> bool {
    let sql = ["SELECT ", "INSERT ", "UPDATE ", "DELETE ", "WITH "]
        .iter()
        .any(|k| text.starts_with(k));
    text.contains(' ')
        && !sql
        && text
            .chars()
            .next()
            .is_some_and(|c| c.is_uppercase() || matches!(c, '`' | '{'))
}

fn rust_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for entry in std::fs::read_dir(path).unwrap() {
            rust_files(&entry.unwrap().path(), out);
        }
    } else if path.extension().is_some_and(|e| e == "rs") {
        out.push(path.to_owned());
    }
}

#[test]
fn platform_copy_is_one_short_sentence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for p in SCANNED {
        rust_files(&root.join(p), &mut files);
    }
    let mut checked = 0;
    let mut bad = Vec::new();
    for file in files {
        let name = file
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        if NOT_COPY.contains(&name.as_str()) || name.ends_with("/tests.rs") {
            continue;
        }
        for Copy {
            line,
            text,
            in_error,
        } in copy_in(&std::fs::read_to_string(&file).unwrap())
        {
            checked += 1;
            let unfinished =
                in_error && text.chars().any(char::is_alphabetic) && !text.ends_with(['.', '}']);
            let problem = problem(&text, MAX_WRITTEN)
                .or_else(|| unfinished.then(|| "does not end with a period".to_owned()));
            if let Some(p) = problem {
                bad.push(format!("{name}:{line}: {p}: {text}"));
            }
        }
    }
    assert!(
        checked > 200,
        "only {checked} messages found; is the scan broken?"
    );
    assert!(
        bad.is_empty(),
        "Platform API copy must be one sentence of at most {MAX_WRITTEN} characters \
         (AGENTS.md, \"Error copy\"):\n{}",
        bad.join("\n")
    );
}

#[test]
fn the_scan_finds_copy_and_skips_the_rest() {
    let src = r##"
        #[derive(Debug, thiserror::Error)]
        enum E {
            #[error("The code is not valid.")]
            Bad,
        }
        #[utoipa::path(get, path = "/x", description = "Long prose. For the docs.")]
        fn f(c: char) -> ApiError {
            let _ = ['"', '\'', '\u{1F600}'];
            let r#type = 1;
            let _: &'static str = "lowercase cause: {e}";
            tracing::info!(
                error = %e,
                "Logged. Not shown."
            );
            sqlx::query!("SELECT a FROM b WHERE c = $1");
            ApiError::Conflict(format!(
                "First line. \
                 Second line {}.",
                r#"Raw "quoted" text."#
            ));
            ApiError::invalid("model_id", format!("models.dev lists `{}`.", m));
            ApiError::internal("lowercase cause, never sent")
        }
        #[cfg(test)]
        mod tests {
            const T: &str = "Test only. Never sent.";
        }
    "##;
    let found: Vec<String> = copy_in(src).into_iter().map(|c| c.text).collect();
    assert_eq!(
        found,
        [
            "The code is not valid.",
            "First line. Second line {}.",
            "Raw \"quoted\" text.",
            "models.dev lists `{}`.",
        ]
    );
}

const CONSOLE_ERRORS: &str = "apps/console/src/i18n/locales/en/errors.json";

const CONSOLE_RULES: &str = "apps/console/src/api/gateway-errors.ts";

const SHOWN_IN_THE_PLAYGROUND: &[&str] = &["crates/kaeso-core/src/catalogue/capability.rs"];

const SHOWN_IN_REQUEST_LOGS: &[&str] = &[
    "crates/kaeso-server/src/model_api",
    "crates/kaeso-core/src/catalogue/resolve.rs",
    "crates/kaeso-server/src/services/credentials.rs",
];

const SHOWN_BY_CODE: &[&str] = &["crates/kaeso-server/src/platform/projects/model_check.rs"];

#[derive(Debug, PartialEq)]
enum Part {
    Text(char),
    Value,
}

fn parts(template: &str) -> Vec<Part> {
    let s: Vec<char> = template.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        match (s[i], s.get(i + 1)) {
            ('{', Some('{')) | ('}', Some('}')) => {
                out.push(Part::Text(s[i]));
                i += 2;
            }
            ('{', _) => {
                let mut depth = 0usize;
                while i < s.len() {
                    match s[i] {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    i += 1;
                    if depth == 0 {
                        break;
                    }
                }
                out.push(Part::Value);
            }
            (c, _) => {
                out.push(Part::Text(c));
                i += 1;
            }
        }
    }
    out
}

fn filled(template: &str) -> String {
    parts(template)
        .iter()
        .map(|p| match p {
            Part::Text(c) => c.to_string(),
            Part::Value => "x".to_owned(),
        })
        .collect()
}

fn fits(pattern: &[Part], text: &[char]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((Part::Text(c), rest)) => text.first() == Some(c) && fits(rest, &text[1..]),
        Some((Part::Value, rest)) => (1..=text.len()).any(|n| fits(rest, &text[n..])),
    }
}

fn translates(console: &str, gateway: &str) -> bool {
    let sample = |t: &str| filled(t).chars().collect::<Vec<char>>();
    fits(&parts(console), &sample(gateway)) || fits(&parts(gateway), &sample(console))
}

fn rule_keys(source: &str) -> Vec<&str> {
    source
        .split("rule(")
        .skip(1)
        .filter_map(|rest| rest.trim_start().strip_prefix('"'))
        .filter_map(|rest| rest.split('"').next())
        .collect()
}

#[test]
fn the_console_translates_every_message() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let catalog: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(root.join(CONSOLE_ERRORS)).unwrap()).unwrap();
    let source = std::fs::read_to_string(root.join(CONSOLE_RULES)).unwrap();
    let rules = rule_keys(&source);
    let console: Vec<&str> = catalog
        .iter()
        .filter(|(key, _)| rules.contains(&key.as_str()))
        .filter_map(|(_, v)| v.as_str())
        .collect();
    assert!(
        console.len() > 300,
        "only {} rules found in {CONSOLE_RULES}",
        console.len()
    );
    let mut files = Vec::new();
    for p in SCANNED
        .iter()
        .chain(SHOWN_IN_THE_PLAYGROUND)
        .chain(SHOWN_IN_REQUEST_LOGS)
    {
        rust_files(&root.join(p), &mut files);
    }
    let mut checked = 0;
    let mut missing = Vec::new();
    for file in files {
        let name = file
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        if NOT_COPY.contains(&name.as_str())
            || SHOWN_BY_CODE.contains(&name.as_str())
            || name.ends_with("/tests.rs")
        {
            continue;
        }
        for Copy { line, text, .. } in copy_in(&std::fs::read_to_string(&file).unwrap()) {
            if !text.ends_with('.') {
                continue;
            }
            checked += 1;
            if !console.iter().any(|c| translates(c, &text)) {
                missing.push(format!("{name}:{line}: {text}"));
            }
        }
    }
    assert!(
        checked > 200,
        "only {checked} messages found; is the scan broken?"
    );
    assert!(
        missing.is_empty(),
        "the console has no rule for these messages; add one with its German and Chinese \
         to {CONSOLE_RULES}:\n{}",
        missing.join("\n")
    );
}

#[test]
fn a_translation_covers_its_values_and_the_values_the_gateway_fills_in() {
    assert!(translates(
        "No such project, or you cannot see it.",
        "No such {what}, or you cannot see it."
    ));
    assert!(translates(
        "Passwords need at least {min} characters.",
        "Passwords need at least {} characters."
    ));
    assert!(translates(
        "`{name}` must be `true` or `false`.",
        "`{{{param}}}` must be `true` or `false`."
    ));
    assert!(!translates(
        "No such project, or you cannot see it.",
        "No such project, or you can see it."
    ));
    assert!(!translates(
        "The picture is empty.",
        "The picture is {} now."
    ));
    assert_eq!(
        parts("{a}{{b}}"),
        [
            Part::Value,
            Part::Text('{'),
            Part::Text('b'),
            Part::Text('}')
        ]
    );
}

#[test]
fn the_rule_reads_sentences() {
    for ok in [
        "That code isn't valid or was already used.",
        "That code isn't valid; codes change every 30 seconds.",
        "The alias `gpt.Mini` exists in this project.",
        "`from` must be a time such as 2026-10-01T00:00:00Z.",
        "Something went wrong in the gateway; report request req_01k7a3f8m2xq4v9c5r6t8w0yz1 to your admin.",
    ] {
        assert_eq!(sent_problem(ok), None, "{ok}");
    }
    for bad in [
        "The code is not valid. Codes change every 30 seconds and each works once.",
        "Lower levels already allow more: {}. Lower those first.",
        "Rate limit reached. `minute` resets soon.",
        "No period at the end",
    ] {
        assert!(sent_problem(bad).is_some(), "{bad}");
    }
}

fn binds_axum_json(t: &[Token], i: usize) -> bool {
    if !is_ident(t.get(i), "Json") || t.get(i + 1) != Some(&Token::Punct('<')) {
        return false;
    }
    let mut j = i;
    while j >= 3
        && t[j - 1] == Token::Punct(':')
        && t[j - 2] == Token::Punct(':')
        && matches!(t[j - 3], Token::Ident(_))
    {
        j -= 3;
    }
    j >= 2
        && t[j - 1] == Token::Punct(':')
        && t[j - 2] != Token::Punct(':')
        && matches!(t[j - 2], Token::Ident(_) | Token::Punct(')'))
}

#[test]
fn platform_handlers_read_json_bodies_with_body() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    rust_files(&root.join("crates/kaeso-server/src/platform"), &mut files);
    let mut bad = Vec::new();
    for file in &files {
        let t = tokens(&std::fs::read_to_string(file).unwrap());
        if (0..t.len()).any(|i| binds_axum_json(&t, i)) {
            bad.push(file.strip_prefix(&root).unwrap().display().to_string());
        }
    }
    assert!(files.len() > 40, "only {} files found", files.len());
    assert!(
        bad.is_empty(),
        "read JSON bodies with `platform::json::Body`, not axum's `Json`: {bad:?}"
    );
    let src = "async fn f(Json(b): Json<Create>, c: axum::Json<X>) -> ApiResult<(StatusCode, Json<V>)> { Ok(Json(v)) }";
    let t = tokens(src);
    assert_eq!((0..t.len()).filter(|i| binds_axum_json(&t, *i)).count(), 2);
}

struct Rejection {
    req: Request<Body>,
    status: StatusCode,
    kind: &'static str,
    param: Option<&'static str>,
    message: &'static str,
}

fn get(path: String, cookie: &str) -> Request<Body> {
    Request::builder()
        .uri(format!("/platform/v1{path}"))
        .header(header::COOKIE, format!("__Host-kaeso_session={cookie}"))
        .body(Body::empty())
        .unwrap()
}

fn post(path: String, cookie: &str, content_type: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(format!("/platform/v1{path}"))
        .header(header::COOKIE, format!("__Host-kaeso_session={cookie}"))
        .header(header::ORIGIN, common::ORIGIN)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body.to_owned()))
        .unwrap()
}

#[tokio::test]
async fn rejections_are_json_errors_that_name_the_parameter() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let (org, c) = (owner.org_id.clone(), owner.cookie.as_str());
    let project = app.get(&format!("/orgs/{org}/projects"), c).await.body["data"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let invalid = StatusCode::BAD_REQUEST;
    let cases = [
        Rejection {
            req: get(format!("/orgs/{org}/projects?limit=abc"), c),
            status: invalid,
            kind: "invalid_request",
            param: Some("limit"),
            message: "`limit` must be a whole number.",
        },
        Rejection {
            req: get(format!("/orgs/{org}/projects?limit=99999999999"), c),
            status: invalid,
            kind: "invalid_request",
            param: Some("limit"),
            message: "`limit` is out of range.",
        },
        Rejection {
            req: get(format!("/orgs/{org}/projects?include_archived=yes"), c),
            status: invalid,
            kind: "invalid_request",
            param: Some("include_archived"),
            message: "`include_archived` must be `true` or `false`.",
        },
        Rejection {
            req: get(format!("/orgs/{org}/audit-log?from=yesterday"), c),
            status: invalid,
            kind: "invalid_request",
            param: Some("from"),
            message: "`from` must be a time such as 2026-10-01T00:00:00Z.",
        },
        Rejection {
            req: get(format!("/orgs/{org}/audit-log?actor=bob"), c),
            status: invalid,
            kind: "invalid_request",
            param: Some("actor"),
            message: "`actor` must be an id starting with `usr_`.",
        },
        Rejection {
            req: get(format!("/orgs/{org}/audit-log?actor=usr_x"), c),
            status: invalid,
            kind: "invalid_request",
            param: Some("actor"),
            message: "`actor` is not a valid id.",
        },
        Rejection {
            req: get(format!("/orgs/{org}/members?limit=1&limit=2"), c),
            status: invalid,
            kind: "invalid_request",
            param: Some("limit"),
            message: "`limit` is given more than once.",
        },
        Rejection {
            req: get(
                format!("/orgs/{org}/projects/{project}/models/preview?model=gpt-4o"),
                c,
            ),
            status: invalid,
            kind: "invalid_request",
            param: Some("provider"),
            message: "`provider` is required.",
        },
        Rejection {
            req: get(format!("/orgs/{org}/projects/{project}/logs?status=ok"), c),
            status: invalid,
            kind: "invalid_request",
            param: Some("status"),
            message: "`status` must be a whole number.",
        },
        Rejection {
            req: get("/orgs/%FF/projects".to_owned(), c),
            status: invalid,
            kind: "invalid_request",
            param: Some("org"),
            message: "`org` in the path is not valid UTF-8.",
        },
        Rejection {
            req: post(format!("/orgs/{org}/projects"), c, "text/plain", "{}"),
            status: invalid,
            kind: "invalid_request",
            param: None,
            message: "Send the body as JSON with `Content-Type: application/json`.",
        },
        Rejection {
            req: post(format!("/orgs/{org}/projects"), c, "application/json", "{"),
            status: invalid,
            kind: "invalid_request",
            param: None,
            message: "The body is not valid JSON.",
        },
        Rejection {
            req: Request::builder()
                .uri(format!("/platform/v1/orgs/{org}/projects"))
                .header(header::AUTHORIZATION, "Basic b3duZXI6c2VjcmV0")
                .body(Body::empty())
                .unwrap(),
            status: StatusCode::UNAUTHORIZED,
            kind: "authentication",
            param: None,
            message: "Send the key as `Authorization: Bearer <key>`.",
        },
        Rejection {
            req: get("/no-such-thing".to_owned(), c),
            status: StatusCode::NOT_FOUND,
            kind: "not_found",
            param: None,
            message: "No such endpoint in the Platform API.",
        },
        Rejection {
            req: get("/auth/setup".to_owned(), c),
            status: StatusCode::METHOD_NOT_ALLOWED,
            kind: "invalid_request",
            param: None,
            message: "This endpoint does not accept `GET` requests.",
        },
    ];
    for case in cases {
        let what = format!("{} {}", case.req.method(), case.req.uri());
        let r = app.raw(case.req).await;
        assert_eq!(r.status, case.status, "{what}: {:?}", r.body);
        let content_type = r
            .headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(content_type, "application/json", "{what}");
        assert_eq!(r.error_type(), case.kind, "{what}: {:?}", r.body);
        assert_eq!(r.body["error"]["message"], case.message, "{what}");
        assert_eq!(
            r.body["error"]["param"].as_str(),
            case.param,
            "{what}: {:?}",
            r.body
        );
        if case.status == StatusCode::METHOD_NOT_ALLOWED {
            assert_eq!(r.headers.get(header::ALLOW).unwrap(), "POST", "{what}");
        }
    }
}
