use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request};
use serde::Deserialize;
use serde::de::{DeserializeOwned, Deserializer, MapAccess, Visitor};
use serde_json::Value;
use serde_json::error::Category;
use serde_json::value::RawValue;
use serde_path_to_error::Segment;

use super::error::ApiError;

// serde can't combine `deny_unknown_fields` with `flatten`; a request that flattens shared
// fields ends with this, which takes whatever no other field took and refuses it.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoOtherFields;

impl<'de> Deserialize<'de> for NoOtherFields {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Refuse;
        impl<'de> Visitor<'de> for Refuse {
            type Value = NoOtherFields;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("no further fields")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<NoOtherFields, A::Error> {
                match map.next_key::<String>()? {
                    Some(field) => Err(serde::de::Error::unknown_field(&field, &[])),
                    None => Ok(NoOtherFields),
                }
            }
        }
        deserializer.deserialize_map(Refuse)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Body<T>(pub T);

impl<S, T> FromRequest<S> for Body<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<Box<RawValue>>::from_request(req, state).await {
            Ok(axum::Json(raw)) => read(raw.get()).map(Body),
            Err(JsonRejection::MissingJsonContentType(_)) => Err(ApiError::bad_request(
                "Send the body as JSON with `Content-Type: application/json`.",
            )),
            Err(JsonRejection::JsonDataError(_) | JsonRejection::JsonSyntaxError(_)) => {
                Err(ApiError::bad_request("The body is not valid JSON."))
            }
            Err(JsonRejection::BytesRejection(e))
                if e.status() == axum::http::StatusCode::PAYLOAD_TOO_LARGE =>
            {
                Err(ApiError::TooLarge(
                    "The request body is larger than this install allows.".into(),
                ))
            }
            Err(JsonRejection::BytesRejection(_)) => {
                Err(ApiError::bad_request("The body could not be read."))
            }
            Err(_) => Err(ApiError::bad_request("The body could not be read.")),
        }
    }
}

// `json` is valid JSON, so every error is about a field.
pub(crate) fn read<T: DeserializeOwned>(json: &str) -> Result<T, ApiError> {
    let at = |e: &PathError| {
        let path = e.path();
        (!path.iter().all(|s| matches!(s, Segment::Unknown))).then(|| path.to_string())
    };
    let e = match serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_str(json)) {
        Ok(v) => return Ok(v),
        Err(e) => e,
    };
    let path = at(&e);
    let detail = e.inner().to_string();
    let detail = detail
        .rsplit_once(" at line ")
        .map_or(detail.as_str(), |(t, _)| t);
    let quoted = |prefix: &str| {
        detail
            .strip_prefix(prefix)
            .and_then(|rest| rest.split('`').next())
    };
    if let Some(field) = quoted("missing field `") {
        let field = path.map_or_else(|| field.to_owned(), |p| format!("{p}.{field}"));
        return Err(ApiError::invalid(&field, format!("`{field}` is required.")));
    }
    if let Some(field) = quoted("unknown field `") {
        let field = path.as_deref().unwrap_or(field);
        return Err(ApiError::invalid(
            field,
            format!("`{field}` is not a field of this request."),
        ));
    }
    let Some(p) = path else {
        return Err(ApiError::bad_request(
            "The body does not have the shape this endpoint expects.",
        ));
    };
    // serde_json refuses a value of the wrong JSON type where an enum goes as a syntax error
    // that names nothing it takes; a string that is no variant makes it list the variants.
    // `Value` sorts the keys of an object, so the probe can stop at another wrong field first;
    // that says nothing about this one.
    let probe = |value: Value| -> Probe {
        let Ok(mut doc) = serde_json::from_str::<Value>(json) else {
            return Probe::Unknown;
        };
        let Some(field) = slot(&mut doc, e.path()) else {
            return Probe::Unknown;
        };
        *field = value;
        match serde_path_to_error::deserialize::<_, T>(&doc) {
            Ok(_) => Probe::Taken,
            Err(again) if at(&again).as_deref() == Some(p.as_str()) => Probe::Refused(again),
            // serde names a missing field once it has read every field there, this one too.
            Err(again)
                if again.inner().to_string().starts_with("missing field `")
                    && at(&again).is_none_or(|q| p.starts_with(&format!("{q}."))) =>
            {
                Probe::Taken
            }
            Err(_) => Probe::Unknown,
        }
    };
    let variants = |detail: &str| -> Option<Vec<String>> {
        let list = detail
            .strip_prefix("unknown variant `")?
            .split_once("`, expected ")?
            .1;
        let mut names: Vec<String> = list
            .split('`')
            .skip(1)
            .step_by(2)
            .map(|n| format!("`{n}`"))
            .collect();
        if matches!(probe(Value::Null), Probe::Taken) {
            names.push("`null`".to_owned());
        }
        Some(names)
    };
    let names = if e.inner().classify() == Category::Syntax {
        match probe(Value::String(String::new())) {
            Probe::Refused(again) => variants(&again.inner().to_string()),
            Probe::Taken | Probe::Unknown => None,
        }
    } else {
        variants(detail)
    };
    let message = match (names, detail.rsplit_once(", expected ").map(|(_, x)| x)) {
        (Some(names), _) => format!("`{p}` must be one of {}.", names.join(", ")),
        (None, Some("a boolean")) => format!("`{p}` must be `true` or `false`."),
        (None, Some("a string")) => format!("`{p}` must be a string."),
        (None, Some("a sequence")) => format!("`{p}` must be an array."),
        (None, Some(x)) if x == "a map" || x.starts_with("struct ") => {
            format!("`{p}` must be an object.")
        }
        (None, Some(x))
            if INTEGERS.contains(&x) && detail.starts_with("invalid value: integer ") =>
        {
            format!("`{p}` is out of range.")
        }
        (None, Some(x)) if INTEGERS.contains(&x) => format!("`{p}` must be a whole number."),
        _ => String::new(),
    };
    // The console shows a message in a toast, at most 120 characters; a long list of values
    // doesn't fit, and the API reference lists them.
    let message = if message.is_empty() || message.chars().count() > 120 {
        format!("`{p}` has a value of the wrong type or form.")
    } else {
        message
    };
    Err(ApiError::invalid(&p, message))
}

type PathError = serde_path_to_error::Error<serde_json::Error>;

enum Probe {
    Taken,
    Refused(PathError),
    Unknown,
}

const INTEGERS: [&str; 10] = [
    "u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize",
];

fn slot<'a>(doc: &'a mut Value, path: &serde_path_to_error::Path) -> Option<&'a mut Value> {
    path.iter().try_fold(doc, |v, s| match s {
        Segment::Seq { index } => v.get_mut(*index),
        Segment::Map { key } => v.get_mut(key.as_str()),
        Segment::Enum { .. } | Segment::Unknown => None,
    })
}

#[cfg(test)]
mod tests {

    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Limit {
        kind: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Auth {
        Bearer,
        XApiKey,
        None,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Format {
        OpenaiEmbeddings,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Server {
        Vllm,
        Sglang,
        Ollama,
        LmStudio,
        LlamaCpp,
        Localai,
        Tgi,
        Lmdeploy,
        Nim,
        DockerModelRunner,
        Koboldcpp,
        Tabbyapi,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Create {
        name: String,
        #[serde(default)]
        rights: Vec<String>,
        #[serde(default)]
        limits: Vec<Limit>,
        #[serde(default)]
        auth: Option<Auth>,
        #[serde(default)]
        embeddings: Option<Format>,
        #[serde(default)]
        modes: Vec<Auth>,
        #[serde(default)]
        on: Option<bool>,
        #[serde(default)]
        max: Option<u32>,
        #[serde(default)]
        server: Option<Server>,
    }

    fn reject(body: &str) -> (String, Option<String>) {
        match read::<Create>(body) {
            Err(ApiError::InvalidRequest { message, param }) => (message, param),
            other => panic!("{body}: {other:?}"),
        }
    }

    #[derive(Debug, Default, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Shared {
        code: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct Flattening {
        name: String,
        #[serde(flatten)]
        shared: Shared,
        #[serde(flatten)]
        _other: NoOtherFields,
    }

    #[test]
    fn a_body_that_flattens_shared_fields_refuses_any_other() {
        let read_back: Flattening = read(r#"{"name": "a", "code": "123456"}"#).unwrap();
        assert_eq!(
            (read_back.name.as_str(), read_back.shared.code.as_deref()),
            ("a", Some("123456"))
        );
        match read::<Flattening>(r#"{"name": "a", "cdoe": "123456"}"#) {
            Err(ApiError::InvalidRequest { message, param }) => {
                assert_eq!(message, "`cdoe` is not a field of this request.");
                assert_eq!(param.as_deref(), Some("cdoe"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_valid_body_is_read_whole() {
        let create: Create = read(
            r#"{"name": "a", "rights": ["x"], "limits": [{"kind": "k"}], "auth": "x_api_key"}"#,
        )
        .unwrap();
        assert_eq!(create.name, "a");
        assert_eq!(create.rights, ["x"]);
        assert_eq!(create.limits.len(), 1);
        assert_eq!(create.limits[0].kind, "k");
        assert!(matches!(create.auth, Some(Auth::XApiKey)));
        let more: Create = read(
            r#"{"name": "a", "embeddings": "openai_embeddings", "modes": ["bearer", "none"], "on": true, "max": 3}"#,
        )
        .unwrap();
        assert!(matches!(more.embeddings, Some(Format::OpenaiEmbeddings)));
        assert!(matches!(more.modes[..], [Auth::Bearer, Auth::None]));
        assert_eq!((more.on, more.max), (Some(true), Some(3)));
        assert!(more.server.is_none());
    }

    #[test]
    fn data_errors_name_the_field_and_what_it_takes() {
        for (body, message, param) in [
            (r#"{}"#, "`name` is required.", Some("name")),
            (
                r#"{"name": "a", "limits": [{}]}"#,
                "`limits[0].kind` is required.",
                Some("limits[0].kind"),
            ),
            (
                r#"{"name": "a", "nmae": "b"}"#,
                "`nmae` is not a field of this request.",
                Some("nmae"),
            ),
            (
                r#"{"name": "a", "rights": ["x", 3]}"#,
                "`rights[1]` must be a string.",
                Some("rights[1]"),
            ),
            (
                r#"{"name": "a", "embeddings": true}"#,
                "`embeddings` must be one of `openai_embeddings`, `null`.",
                Some("embeddings"),
            ),
            (
                r#"{"name": "a", "embeddings": "openai"}"#,
                "`embeddings` must be one of `openai_embeddings`, `null`.",
                Some("embeddings"),
            ),
            (
                r#"{"name": "a", "auth": 5}"#,
                "`auth` must be one of `bearer`, `x_api_key`, `none`, `null`.",
                Some("auth"),
            ),
            (
                r#"{"name": "a", "modes": ["bearer", ["x"]]}"#,
                "`modes[1]` must be one of `bearer`, `x_api_key`, `none`.",
                Some("modes[1]"),
            ),
            (
                r#"{"name": "a", "modes": "bearer"}"#,
                "`modes` must be an array.",
                Some("modes"),
            ),
            (
                r#"{"name": "a", "limits": ["k"]}"#,
                "`limits[0]` must be an object.",
                Some("limits[0]"),
            ),
            (
                r#"{"name": "a", "on": "yes"}"#,
                "`on` must be `true` or `false`.",
                Some("on"),
            ),
            (
                r#"{"name": "a", "max": -1}"#,
                "`max` is out of range.",
                Some("max"),
            ),
            (
                r#"{"name": "a", "max": 5000000000}"#,
                "`max` is out of range.",
                Some("max"),
            ),
            (
                r#"{"embeddings": true}"#,
                "`embeddings` must be one of `openai_embeddings`, `null`.",
                Some("embeddings"),
            ),
            (
                r#"{"name": "a", "auth": 5, "embeddings": true}"#,
                "`auth` must be one of `bearer`, `x_api_key`, `none`.",
                Some("auth"),
            ),
            (
                r#"{"name": "a", "max": 1.5}"#,
                "`max` must be a whole number.",
                Some("max"),
            ),
            (
                r#"{"name": "a", "server": "vllm2"}"#,
                "`server` has a value of the wrong type or form.",
                Some("server"),
            ),
            (
                r#""x""#,
                "The body does not have the shape this endpoint expects.",
                None,
            ),
        ] {
            let (m, p) = reject(body);
            assert_eq!(m, message, "{body}");
            assert_eq!(p.as_deref(), param, "{body}");
            assert!(m.chars().count() <= 120, "{m}");
        }
    }
}
