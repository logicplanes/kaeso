use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request};
use serde::de::DeserializeOwned;

use super::error::ApiError;

#[derive(Debug, Clone, Copy, Default)]
pub struct Body<T>(pub T);

impl<S, T> FromRequest<S> for Body<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(v)) => Ok(Body(v)),
            Err(JsonRejection::MissingJsonContentType(_)) => Err(ApiError::bad_request(
                "Send the body as JSON with `Content-Type: application/json`.",
            )),
            Err(JsonRejection::JsonDataError(e)) => Err(data_error(&e.body_text())),
            Err(JsonRejection::JsonSyntaxError(_)) => {
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

fn data_error(text: &str) -> ApiError {
    let text = text
        .strip_prefix("Failed to deserialize the JSON body into the target type: ")
        .unwrap_or(text);
    let text = text.rsplit_once(" at line ").map_or(text, |(t, _)| t);
    let (path, detail) = match text.split_once(": ") {
        Some((p, d)) if !p.contains(' ') => (Some(p), d),
        _ => (None, text),
    };
    let quoted = |prefix: &str| {
        detail
            .strip_prefix(prefix)
            .and_then(|rest| rest.split('`').next())
    };
    if let Some(field) = quoted("missing field `") {
        let field = path.map_or_else(|| field.to_owned(), |p| format!("{p}.{field}"));
        return ApiError::invalid(&field, format!("`{field}` is required."));
    }
    if let Some(field) = quoted("unknown field `") {
        let field = path.unwrap_or(field);
        return ApiError::invalid(field, format!("`{field}` is not a field of this request."));
    }
    match path {
        Some(p) => ApiError::invalid(p, format!("`{p}` has a value of the wrong type or form.")),
        None => ApiError::bad_request("The body does not have the shape this endpoint expects."),
    }
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
    #[serde(deny_unknown_fields)]
    struct Create {
        name: String,
        #[serde(default)]
        rights: Vec<String>,
        #[serde(default)]
        limits: Vec<Limit>,
    }

    fn reject(body: &str) -> (String, Option<String>) {
        let Err(JsonRejection::JsonDataError(e)) =
            axum::Json::<Create>::from_bytes(body.as_bytes())
        else {
            panic!("{body} is not a data error");
        };
        match data_error(&e.body_text()) {
            ApiError::InvalidRequest { message, param } => (message, param),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_valid_body_is_read_whole() {
        let axum::Json(create) = axum::Json::<Create>::from_bytes(
            br#"{"name": "a", "rights": ["x"], "limits": [{"kind": "k"}]}"#,
        )
        .unwrap();
        assert_eq!(create.name, "a");
        assert_eq!(create.rights, ["x"]);
        assert_eq!(create.limits.len(), 1);
        assert_eq!(create.limits[0].kind, "k");
    }

    #[test]
    fn data_errors_name_the_field() {
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
                "`rights[1]` has a value of the wrong type or form.",
                Some("rights[1]"),
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
        }
    }
}
