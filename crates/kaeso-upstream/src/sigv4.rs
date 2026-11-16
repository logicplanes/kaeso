//! AWS Signature Version 4 for requests to AWS services, such as Amazon Bedrock
//! (https://docs.aws.amazon.com/IAM/latest/UserGuide/create-signed-request.html). A signer of its
//! own on `hmac` and `sha2`: the canonical request, the string to sign, the derived signing key
//! and the `Authorization` header, checked against AWS's signing test suite.

use hmac::{Hmac, KeyInit, Mac};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

type HmacSha256 = Hmac<Sha256>;

/// The credentials a request is signed with: an access key, and with temporary credentials the
/// session token.
#[derive(Clone)]
pub struct AwsCredentials {
    pub access_key_id: String,
    pub secret_access_key: SecretString,
    pub session_token: Option<SecretString>,
}

impl std::fmt::Debug for AwsCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsCredentials")
            .field("access_key_id", &self.access_key_id)
            .finish_non_exhaustive()
    }
}

/// How a call signs itself: the credentials, the region and service in the credential scope,
/// and the moment it is signed at.
#[derive(Clone, Debug)]
pub struct SigV4 {
    pub credentials: AwsCredentials,
    pub region: String,
    pub service: String,
    pub at: OffsetDateTime,
}

/// What a signed request carries: `x-amz-date`, `x-amz-security-token` with temporary
/// credentials, and `authorization`.
#[derive(Debug, PartialEq, Eq)]
pub struct Signed {
    pub headers: Vec<(&'static str, String)>,
}

/// One request to sign. `path` is the path as it is sent, percent-encoding and all, and `query`
/// what follows the `?`; `headers` are the ones the signature covers besides `host`, `x-amz-date`
/// and `x-amz-security-token`, such as `content-type`.
pub struct Request<'a> {
    pub method: &'a str,
    pub host: &'a str,
    pub path: &'a str,
    pub query: &'a str,
    pub headers: &'a [(&'a str, &'a str)],
    pub body: &'a [u8],
}

const ALGORITHM: &str = "AWS4-HMAC-SHA256";

impl SigV4 {
    /// The headers that sign `req`.
    pub fn sign(&self, req: &Request<'_>) -> Signed {
        let date_time = amz_date(self.at);
        let date = &date_time[..8];
        let mut headers: Vec<(String, String)> = req
            .headers
            .iter()
            .map(|(n, v)| (n.to_ascii_lowercase(), normalize(v)))
            .collect();
        headers.push(("host".into(), req.host.to_owned()));
        headers.push(("x-amz-date".into(), date_time.clone()));
        if let Some(token) = &self.credentials.session_token {
            headers.push((
                "x-amz-security-token".into(),
                token.expose_secret().to_owned(),
            ));
        }
        headers.sort();
        let canonical_headers: String = headers.iter().map(|(n, v)| format!("{n}:{v}\n")).collect();
        let signed_headers = headers
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>()
            .join(";");
        let canonical = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            req.method,
            canonical_path(req.path),
            canonical_query(req.query),
            canonical_headers,
            signed_headers,
            hex(&Sha256::digest(req.body)),
        );
        let scope = format!("{date}/{}/{}/aws4_request", self.region, self.service);
        let to_sign = format!(
            "{ALGORITHM}\n{date_time}\n{scope}\n{}",
            hex(&Sha256::digest(canonical.as_bytes()))
        );
        let key = [
            date.as_bytes(),
            self.region.as_bytes(),
            self.service.as_bytes(),
            b"aws4_request",
        ]
        .iter()
        .fold(
            format!("AWS4{}", self.credentials.secret_access_key.expose_secret()).into_bytes(),
            |key, part| hmac(&key, part),
        );
        let signature = hex(&hmac(&key, to_sign.as_bytes()));
        let mut out = vec![("x-amz-date", date_time)];
        if let Some(token) = &self.credentials.session_token {
            out.push(("x-amz-security-token", token.expose_secret().to_owned()));
        }
        out.push((
            "authorization",
            format!(
                "{ALGORITHM} Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
                self.credentials.access_key_id
            ),
        ));
        Signed { headers: out }
    }
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = <HmacSha256 as KeyInit>::new_from_slice(key).unwrap_or_else(|_| {
        // HMAC takes a key of any length.
        unreachable!("HMAC refused a key")
    });
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn amz_date(at: OffsetDateTime) -> String {
    let at = at.to_offset(time::UtcOffset::UTC);
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second()
    )
}

// Trimmed, with runs of spaces as one.
fn normalize(v: &str) -> String {
    v.split_whitespace().collect::<Vec<_>>().join(" ")
}

// Every byte but the unreserved characters is encoded, `%XX` in upper case.
fn uri_encode(s: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(b, b'-' | b'.' | b'_' | b'~')
            || (keep_slash && b == b'/')
        {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// Services other than S3 sign the path encoded once more, as sent: a model id's `:` goes out as
// `%3A` and is signed as `%253A`, the way AWS's SDKs do it.
fn canonical_path(path: &str) -> String {
    if path.is_empty() {
        return "/".into();
    }
    uri_encode(path, true)
}

fn canonical_query(query: &str) -> String {
    if query.is_empty() {
        return String::new();
    }
    let decode = |s: &str| percent_decode(s);
    let mut pairs: Vec<(String, String)> = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (uri_encode(&decode(k), false), uri_encode(&decode(v), false))
        })
        .collect();
    pairs.sort();
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&byte) = bytes.get(i) {
        match (byte, bytes.get(i + 1..i + 3)) {
            (b'%', Some(h)) => match std::str::from_utf8(h)
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                Some(b) => {
                    out.push(b);
                    i += 3;
                }
                None => {
                    out.push(b'%');
                    i += 1;
                }
            },
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // AWS's signing test suite (github.com/awslabs/aws-c-auth, tests/aws-signing-test-suite/v4).
    fn suite(token: Option<&str>) -> SigV4 {
        SigV4 {
            credentials: AwsCredentials {
                access_key_id: "AKIDEXAMPLE".into(),
                secret_access_key: SecretString::from("wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY"),
                session_token: token.map(SecretString::from),
            },
            region: "us-east-1".into(),
            service: "service".into(),
            at: time::macros::datetime!(2015-08-30 12:36:00 UTC),
        }
    }

    fn authorization(s: &Signed) -> &str {
        &s.headers
            .iter()
            .find(|(n, _)| *n == "authorization")
            .unwrap()
            .1
    }

    #[test]
    fn get_vanilla() {
        let s = suite(None).sign(&Request {
            method: "GET",
            host: "example.amazonaws.com",
            path: "/",
            query: "",
            headers: &[],
            body: b"",
        });
        assert_eq!(
            authorization(&s),
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, SignedHeaders=host;x-amz-date, Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31"
        );
        assert_eq!(s.headers[0], ("x-amz-date", "20150830T123600Z".into()));
    }

    #[test]
    fn get_vanilla_with_session_token() {
        let token = "6e86291e8372ff2a2260956d9b8aae1d763fbf315fa00fa31553b73ebf194267";
        let s = suite(Some(token)).sign(&Request {
            method: "GET",
            host: "example.amazonaws.com",
            path: "/",
            query: "",
            headers: &[],
            body: b"",
        });
        assert_eq!(
            authorization(&s),
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, SignedHeaders=host;x-amz-date;x-amz-security-token, Signature=07ec1639c89043aa0e3e2de82b96708f198cceab042d4a97044c66dd9f74e7f8"
        );
        assert!(s.headers.contains(&("x-amz-security-token", token.into())));
    }

    #[test]
    fn post_x_www_form_urlencoded_with_signed_body() {
        let body = b"Param1=value1";
        let s = suite(None).sign(&Request {
            method: "POST",
            host: "example.amazonaws.com",
            path: "/",
            query: "",
            headers: &[
                ("Content-Type", "application/x-www-form-urlencoded"),
                ("Content-Length", "13"),
                (
                    "x-amz-content-sha256",
                    "9095672bbd1f56dfc5b65f3e153adc8731a4a654192329106275f4c7b24d0b6e",
                ),
            ],
            body,
        });
        assert_eq!(
            authorization(&s),
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, SignedHeaders=content-length;content-type;host;x-amz-content-sha256;x-amz-date, Signature=d3875051da38690788ef43de4db0d8f280229d82040bfac253562e56c3f20e0b"
        );
    }

    #[test]
    fn a_bedrock_model_id_in_the_path_is_signed_encoded_twice_and_the_query_sorted() {
        assert_eq!(
            canonical_path("/model/amazon.nova-pro-v1%3A0/converse"),
            "/model/amazon.nova-pro-v1%253A0/converse"
        );
        assert_eq!(canonical_path(""), "/");
        assert_eq!(canonical_query("b=2&a=1&c=x%20y&d"), "a=1&b=2&c=x%20y&d=");
        assert_eq!(percent_decode("%zz%41"), "%zzA");
        assert_eq!(normalize("  a   b "), "a b");
    }

    #[test]
    fn the_secret_is_not_in_debug_output() {
        let s = suite(Some("token"));
        let shown = format!("{s:?}");
        assert!(
            !shown.contains("wJalr") && !shown.contains("token\""),
            "{shown}"
        );
    }
}
