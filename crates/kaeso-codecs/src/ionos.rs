//! IONOS AI Model Hub's error body, `{"httpStatus": n, "messages": [{"errorCode", "message"}]}`,
//! one entry per problem, which its OpenAI-compatible API answers instead of OpenAI's `error`
//! object (<https://docs.ionos.com/cloud/ai/ai-model-hub/error-codes>).

use serde_json::Value;

/// The messages of an IONOS error body, joined, or `None` for any other body.
pub(crate) fn envelope_message(raw: &Value) -> Option<String> {
    raw.get("httpStatus")?.as_u64()?;
    let messages: Vec<&str> = raw
        .get("messages")?
        .as_array()?
        .iter()
        .filter_map(|e| e.get("message").and_then(Value::as_str))
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .collect();
    if messages.is_empty() {
        return None;
    }
    Some(messages.join("; "))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::envelope_message;

    #[test]
    fn reads_the_messages_of_an_ionos_error_and_nothing_else() {
        let refused = json!({"httpStatus": 401, "messages": [{"errorCode": "paas-auth-1", "message": "Unauthorized, wrong or no api key provided to process this request"}]});
        assert_eq!(
            envelope_message(&refused).as_deref(),
            Some("Unauthorized, wrong or no api key provided to process this request")
        );
        let two = json!({"httpStatus": 400, "messages": [{"errorCode": "VALIDATION_ERROR", "message": "First"}, {"message": " Second "}]});
        assert_eq!(envelope_message(&two).as_deref(), Some("First; Second"));
        for other in [
            json!({"messages": [{"message": "x"}]}),
            json!({"httpStatus": "400", "messages": [{"message": "x"}]}),
            json!({"httpStatus": 400, "messages": []}),
            json!({"httpStatus": 400, "messages": [{"errorCode": "X"}]}),
            json!({"error": {"message": "x"}}),
        ] {
            assert_eq!(envelope_message(&other), None, "{other}");
        }
    }
}
