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
            .or_else(|| {
                fields
                    .get("")
                    .and_then(|whole| {
                        serde_json::from_str::<&serde_json::value::RawValue>(whole.get()).ok()
                    })
                    .and_then(|raw| crate::wire::field_of(raw, key))
                    .and_then(|v| serde_json::from_str::<serde_json::Value>(v.get()).ok())
            })
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

fn raw_is_item_reference(text: &str) -> bool {
    struct AnyItemRef;
    impl<'de> serde::de::Visitor<'de> for AnyItemRef {
        type Value = bool;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a JSON object")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> Result<Self::Value, A::Error> {
            let mut found = false;
            while let Some((k, v)) =
                map.next_entry::<std::borrow::Cow<'de, str>, &'de serde_json::value::RawValue>()?
            {
                if k == "type"
                    && serde_json::from_str::<String>(v.get()).ok().as_deref()
                        == Some("item_reference")
                {
                    found = true;
                }
            }
            Ok(found)
        }
    }
    if !text.trim_start().starts_with('{') {
        return false;
    }
    let mut de = serde_json::Deserializer::from_str(text);
    serde::Deserializer::deserialize_map(&mut de, AnyItemRef).unwrap_or(false)
}

/// Whether a Responses-format request carries an `item_reference` item in `input`.
pub fn has_item_reference(req: &GenerateRequest) -> bool {
    if !req.extras.is_from(DIALECT) {
        return false;
    }
    req.all_blocks().any(|block| match block {
        kaeso_kpp::Block::Unknown(item) if item.origin.is(DIALECT) => {
            raw_is_item_reference(item.raw.get())
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Codec, EgressCtx, HeaderList};
    use kaeso_kpp::{Request, RequestKind};

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
        assert!(!has_item_reference(&foreign));
    }

    #[test]
    fn item_references_and_without_store_are_recognized_and_written() {
        for raw in [
            br#"{"model":"gpt-4o","input":[{"type":"item_reference","id":"msg_1"}]}"#.as_slice(),
            br#"{"model":"gpt-4o","input":[{"type":"item_\u0072eference","id":"msg_1"}]}"#
                .as_slice(),
            br#"{"model":"gpt-4o","input":[{"type":"message","type":"item_reference","id":"msg_1"}]}"#
                .as_slice(),
        ] {
            let Request::Generate(g) = super::super::OpenAiResponses::decode_request(
                RequestKind::Generate,
                raw,
                &HeaderList::new(),
            )
            .unwrap() else {
                panic!()
            };
            assert!(has_item_reference(&g));
        }

        for raw in [
            br#"{"model":"gpt-4o","input":"Hi"}"#.as_slice(),
            br#"{"model":"gpt-4o","input":"Hi","store":null}"#.as_slice(),
            br#"{"model":"gpt-4o","input":"Hi","store":false}"#.as_slice(),
        ] {
            let Request::Generate(mut g) = super::super::OpenAiResponses::decode_request(
                RequestKind::Generate,
                raw,
                &HeaderList::new(),
            )
            .unwrap() else {
                panic!()
            };
            assert!(!has_item_reference(&g));
            super::super::without_store(&mut g);
            let req = Request::Generate(g);
            let mut ctx = EgressCtx::new(&req);
            let enc = super::super::OpenAiResponses::encode_request(&req, &mut ctx).unwrap();
            let v: serde_json::Value = serde_json::from_slice(&enc.body).unwrap();
            assert_eq!(v["store"], false);
        }

        let Request::Generate(dotted) = super::super::OpenAiResponses::decode_request(
            RequestKind::Generate,
            br#"{"model":"gpt-4o","input":"Hi","a.b":1,"store":true,"previous_response_id":"resp_9"}"#,
            &HeaderList::new(),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(
            stored_state(&dotted),
            StoredState {
                store: Some(true),
                previous: Some("resp_9".into()),
            }
        );
    }
}
