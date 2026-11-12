//! Cloudflare's API envelope, `{"errors": [{"code", "message"}], "success": false, "result",
//! "messages"}`. Cloudflare's REST API answers with it, whatever the request's format, when it
//! refuses a request itself: a refused token, an empty credit balance, a retired or unknown model.

use serde_json::Value;

/// The messages of a Cloudflare error envelope, joined, or `None` for any other body.
pub(crate) fn envelope_message(raw: &Value) -> Option<String> {
    if raw.get("success").and_then(Value::as_bool) != Some(false) {
        return None;
    }
    let messages: Vec<&str> = raw
        .get("errors")?
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
    fn reads_the_messages_of_a_cloudflare_envelope_and_nothing_else() {
        let refused = json!({"result": null, "success": false, "errors": [{"code": 10000, "message": "Authentication error"}], "messages": []});
        assert_eq!(
            envelope_message(&refused).as_deref(),
            Some("Authentication error")
        );
        let two = json!({"success": false, "errors": [{"message": "First"}, {"code": 1, "message": " Second "}]});
        assert_eq!(envelope_message(&two).as_deref(), Some("First; Second"));
        for other in [
            json!({"success": true, "errors": [{"message": "x"}]}),
            json!({"errors": [{"message": "x"}]}),
            json!({"success": false, "errors": []}),
            json!({"success": false, "errors": [{"code": 1}]}),
            json!({"error": {"message": "x"}}),
        ] {
            assert_eq!(envelope_message(&other), None, "{other}");
        }
    }
}
