use kaeso_kpp::GenerateRequest;

use super::DIALECT;
use super::overrides::SET;

/// What a Responses request asks of the provider's stored responses: `store` as the app sent it
/// and the stored response it continues.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoredState {
    pub store: Option<bool>,
    pub previous: Option<String>,
}

impl StoredState {
    /// Whether the request asks for stored state outright: `store: true` or a continuation.
    pub fn asked(&self) -> bool {
        self.store == Some(true) || self.previous.is_some()
    }

    /// The field that asks for it, for messages: `previous_response_id` before `store`.
    pub fn field(&self) -> &'static str {
        if self.previous.is_some() {
            "previous_response_id"
        } else {
            "store"
        }
    }
}

/// Reads `store` and `previous_response_id` from a request a Responses app sent, which keeps
/// them as extras fields; a request from any other format asks for nothing.
pub fn stored_state(req: &GenerateRequest) -> StoredState {
    let Some(fields) = req.extras.fields_for(DIALECT) else {
        return StoredState::default();
    };
    let value = |key: &str| {
        fields
            .get(key)
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw.get()).ok())
    };
    let store = value("store").and_then(|v| v.as_bool()).or_else(|| {
        req.extras
            .form_value(DIALECT, SET)
            .and_then(|set| set.get("store"))
            .and_then(serde_json::Value::as_bool)
    });
    StoredState {
        store,
        previous: value("previous_response_id").and_then(|v| v.as_str().map(str::to_owned)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_continuation_or_store_true_asks_and_names_its_field() {
        let none = StoredState::default();
        assert!(!none.asked());
        let off = StoredState {
            store: Some(false),
            previous: None,
        };
        assert!(!off.asked());
        let store = StoredState {
            store: Some(true),
            previous: None,
        };
        assert!(store.asked());
        assert_eq!(store.field(), "store");
        let continues = StoredState {
            store: None,
            previous: Some("resp_1".into()),
        };
        assert!(continues.asked());
        assert_eq!(continues.field(), "previous_response_id");
        let foreign = GenerateRequest::new("m");
        assert_eq!(stored_state(&foreign), StoredState::default());
    }
}
