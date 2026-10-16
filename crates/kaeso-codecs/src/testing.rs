use serde_json::Value;

use crate::sse::{SseError, SseItem, parse_all};

pub fn json_diff(expected: &Value, actual: &Value) -> Option<String> {
    diff_at("$", expected, actual)
}

fn diff_at(path: &str, expected: &Value, actual: &Value) -> Option<String> {
    match (expected, actual) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, va) in a {
                match b.get(k) {
                    Some(vb) => {
                        if let Some(d) = diff_at(&format!("{path}.{k}"), va, vb) {
                            return Some(d);
                        }
                    }
                    None => return Some(format!("{path}.{k}: missing (expected {va})")),
                }
            }
            b.iter()
                .find(|(k, _)| !a.contains_key(*k))
                .map(|(k, v)| format!("{path}.{k}: unexpected {v}"))
        }
        (Value::Array(a), Value::Array(b)) => {
            for (i, (va, vb)) in a.iter().zip(b).enumerate() {
                if let Some(d) = diff_at(&format!("{path}[{i}]"), va, vb) {
                    return Some(d);
                }
            }
            (a.len() != b.len())
                .then(|| format!("{path}: expected {} elements, got {}", a.len(), b.len()))
        }
        (Value::Number(a), Value::Number(b)) => {
            let both_integers = !a.is_f64() && !b.is_f64();
            let equal = a == b
                || (!both_integers
                    && matches!((a.as_f64(), b.as_f64()), (Some(x), Some(y)) if x == y));
            (!equal).then(|| format!("{path}: expected {a}, got {b}"))
        }
        (a, b) => (a != b).then(|| format!("{path}: expected {a}, got {b}")),
    }
}

pub fn json_bytes_diff(expected: &[u8], actual: &[u8]) -> Option<String> {
    let a: Value = match serde_json::from_slice(expected) {
        Ok(v) => v,
        Err(e) => return Some(format!("expected side is not JSON: {e}")),
    };
    let b: Value = match serde_json::from_slice(actual) {
        Ok(v) => v,
        Err(e) => {
            return Some(format!(
                "actual side is not JSON: {e}: {}",
                String::from_utf8_lossy(actual)
            ));
        }
    };
    json_diff(&a, &b)
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ComparableItem {
    Event {
        event: Option<String>,
        data: Value,
        fields: Vec<(String, String)>,
    },
    Comment(String),
}

pub fn sse_items(bytes: &[u8]) -> Result<Vec<ComparableItem>, SseError> {
    Ok(parse_all(bytes)?
        .into_iter()
        .map(|item| match item {
            SseItem::Comment(c) => ComparableItem::Comment(c),
            SseItem::Event(e) => {
                let mut fields = Vec::new();
                if let Some(id) = e.id {
                    fields.push(("id".to_owned(), id));
                }
                if let Some(retry) = e.retry {
                    fields.push(("retry".to_owned(), retry.to_string()));
                }
                fields.extend(e.other);
                ComparableItem::Event {
                    event: e.event,
                    data: serde_json::from_str(&e.data).unwrap_or(Value::String(e.data)),
                    fields,
                }
            }
        })
        .collect())
}

pub fn sse_diff(expected: &[u8], actual: &[u8]) -> Option<String> {
    let a = match sse_items(expected) {
        Ok(a) => a,
        Err(e) => return Some(format!("expected stream does not parse: {e}")),
    };
    let b = match sse_items(actual) {
        Ok(b) => b,
        Err(e) => return Some(format!("actual stream does not parse: {e}")),
    };
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        match (x, y) {
            (
                ComparableItem::Event {
                    event: ea,
                    data: da,
                    fields: fa,
                },
                ComparableItem::Event {
                    event: eb,
                    data: db,
                    fields: fb,
                },
            ) => {
                if ea != eb {
                    return Some(format!("item {i}: expected event {ea:?}, got {eb:?}"));
                }
                if let Some(d) = json_diff(da, db) {
                    return Some(format!("item {i} ({}): {d}", ea.as_deref().unwrap_or("-")));
                }
                if fa != fb {
                    return Some(format!("item {i}: expected fields {fa:?}, got {fb:?}"));
                }
            }
            (x, y) if x != y => return Some(format!("item {i}: expected {x:?}, got {y:?}")),
            _ => {}
        }
    }
    (a.len() != b.len()).then(|| {
        format!(
            "expected {} items, got {}; first extra: {:?}",
            a.len(),
            b.len(),
            if a.len() > b.len() {
                a.get(b.len())
            } else {
                b.get(a.len())
            }
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_compare_by_value_and_paths_are_reported() {
        assert_eq!(json_diff(&json!({"t": 1}), &json!({"t": 1.0})), None);
        let big: Value = serde_json::from_str("9007199254740993").unwrap();
        let near: Value = serde_json::from_str("9007199254740992").unwrap();
        assert!(json_diff(&big, &near).is_some(), "integers compare exactly");
        assert_eq!(
            json_diff(
                &json!({"a": [1, {"b": "x"}]}),
                &json!({"a": [1, {"b": "y"}]})
            ),
            Some("$.a[1].b: expected \"x\", got \"y\"".into())
        );
        assert_eq!(
            json_diff(&json!({"a": null}), &json!({})),
            Some("$.a: missing (expected null)".into())
        );
        assert_eq!(
            json_diff(&json!({}), &json!({"z": 1})),
            Some("$.z: unexpected 1".into())
        );
        assert!(json_diff(&json!([1, 2]), &json!([1])).is_some());
    }

    #[test]
    fn sse_streams_compare_by_event_fields_and_json_data() {
        let a = b"event: x\ndata: {\"a\":1,\"b\":2}\n\ndata: [DONE]\n\n";
        let b = b"event: x\ndata: {\"b\":2, \"a\":1.0}\n\ndata: [DONE]\n\n";
        assert_eq!(sse_diff(a, b), None);
        let c = b"event: y\ndata: {\"a\":1,\"b\":2}\n\ndata: [DONE]\n\n";
        assert!(sse_diff(a, c).unwrap().contains("expected event"));
        assert!(sse_diff(a, b"event: x\ndata: {\"a\":1,\"b\":2}\n\n").is_some());
        assert!(sse_diff(b"id: 1\ndata: x\n\n", b"data: x\n\n").is_some());
    }
}
