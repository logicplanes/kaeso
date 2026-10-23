use std::collections::HashSet;

use kaeso_kpp::{DialectId, Extras, HEADER_PREFIX, RawJson};
use serde_json::Value;
use serde_json::value::RawValue;

use super::node::{Node, entries_of};
use crate::limits::MAX_DEPTH;

pub(super) const SET: &str = "set";
pub(super) const UNSET: &str = "unset";
pub(super) const CANON: &str = "canon";

#[derive(Clone, Debug, Default)]
pub(crate) struct Overrides {
    set: Vec<(String, RawJson, Option<Value>)>,
    unset: Vec<(String, Value)>,
}

pub(crate) fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            x == y
                || ((x.is_f64() || y.is_f64())
                    && matches!((x.as_f64(), y.as_f64()), (Some(p), Some(q)) if p == q))
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| values_equal(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| values_equal(v, w)))
        }
        _ => a == b,
    }
}

impl Overrides {
    pub(crate) fn diff(canon: &Node, actual: &RawValue, managed: &[&str]) -> Overrides {
        Overrides::diff_with(canon, actual, managed, &[])
    }

    pub(crate) fn diff_with(
        canon: &Node,
        actual: &RawValue,
        managed: &[&str],
        recurse: &[&str],
    ) -> Overrides {
        let mut out = Overrides::default();
        let mut path = Vec::new();
        let keys = Keys { managed, recurse };
        diff_at(Some(canon), actual, &mut path, &keys, &mut out);
        out
    }

    pub(crate) fn record(self, extras: &mut Extras, scope: &str) {
        let full = |p: &str| -> String {
            match (scope.is_empty(), p.is_empty()) {
                (true, _) => p.to_owned(),
                (false, true) => scope.to_owned(),
                (false, false) => format!("{scope}.{p}"),
            }
        };
        for (path, raw, canon) in self.set {
            let key = full(&path);
            match raw.to_value() {
                Ok(value)
                    if canon.as_ref().is_some_and(|c| values_equal(c, &value))
                        || is_form(&path, &value, canon.is_some()) =>
                {
                    form_set_insert(extras, key.clone(), value);
                }
                _ => {
                    extras.fields.insert(key.clone(), raw);
                }
            }
            if let Some(c) = canon {
                form_map_insert(extras, CANON, key, c);
            }
        }
        for (path, canon) in self.unset {
            let key = full(&path);
            if let Some(Value::Array(list)) = extras.form.get_mut(UNSET) {
                list.push(Value::String(key.clone()));
            } else {
                extras.form.insert(
                    UNSET.to_owned(),
                    Value::Array(vec![Value::String(key.clone())]),
                );
            }
            form_map_insert(extras, CANON, key, canon);
        }
    }

    pub(crate) fn split_off(&mut self, prefix: &str) -> Overrides {
        let strip = |p: &str| -> Option<String> {
            p.strip_prefix(prefix)
                .and_then(|r| r.strip_prefix('.'))
                .map(str::to_owned)
        };
        let mut out = Overrides::default();
        let mut keep = Vec::new();
        for (p, v, c) in std::mem::take(&mut self.set) {
            match strip(&p) {
                Some(rest) => out.set.push((rest, v, c)),
                None => keep.push((p, v, c)),
            }
        }
        self.set = keep;
        let mut keep = Vec::new();
        for (p, c) in std::mem::take(&mut self.unset) {
            match strip(&p) {
                Some(rest) => out.unset.push((rest, c)),
                None => keep.push((p, c)),
            }
        }
        self.unset = keep;
        out
    }
}

pub(crate) fn form_set_insert(extras: &mut Extras, key: String, value: Value) {
    form_map_insert(extras, SET, key, value);
}

fn form_map_insert(extras: &mut Extras, map: &str, key: String, value: Value) {
    match extras.form.get_mut(map) {
        Some(Value::Object(m)) => {
            m.insert(key, value);
        }
        _ => {
            let mut m = serde_json::Map::new();
            m.insert(key, value);
            extras.form.insert(map.to_owned(), Value::Object(m));
        }
    }
}

struct Keys<'a> {
    managed: &'a [&'a str],
    recurse: &'a [&'a str],
}

fn diff_at(
    canon: Option<&Node>,
    actual: &RawValue,
    path: &mut Vec<String>,
    keys: &Keys<'_>,
    out: &mut Overrides,
) {
    let managed = keys.managed;
    let text = actual.get().trim_start();
    if path.len() == 1
        && path.first().is_some_and(|k| managed.contains(&k.as_str()))
        && text.starts_with('[')
        && matches!(canon, Some(Node::Arr(_)))
    {
        return;
    }
    if path.len() < MAX_DEPTH {
        diff_children(canon, actual, path, keys, out);
        return;
    }
    diff_whole(canon, actual, path, out);
}

fn diff_children(
    canon: Option<&Node>,
    actual: &RawValue,
    path: &mut Vec<String>,
    keys: &Keys<'_>,
    out: &mut Overrides,
) {
    let text = actual.get().trim_start();
    let canon_obj = match canon {
        None => Some(Node::obj()),
        Some(n) => {
            let mut n = n.clone();
            n.expand();
            matches!(n, Node::Obj(_)).then_some(n)
        }
    };
    if let (Some(Node::Obj(centries)), Some(aentries)) = (&canon_obj, entries_of(actual)) {
        let simple_keys = aentries.iter().all(|(k, _)| plain_key(k))
            && centries.iter().all(|(k, _)| !k.contains('.'));
        if simple_keys {
            let before = out.set.len() + out.unset.len();
            for (k, v) in &aentries {
                let child = centries.get(k.as_str());
                path.push(k.clone());
                let recurse = v.get().trim_start().starts_with('{')
                    && keys.recurse.contains(&k.as_str())
                    && !(child.is_none() && typed_object(v));
                if child.is_none() && !recurse {
                    out.set.push((path.join("."), RawJson::from(*v), None));
                } else {
                    diff_at(child, v, path, keys, out);
                }
                path.pop();
            }
            let present: HashSet<&str> = aentries.iter().map(|(k, _)| k.as_str()).collect();
            for (ck, cv) in centries {
                if !present.contains(ck.as_str()) {
                    path.push(ck.clone());
                    out.unset
                        .push((path.join("."), cv.to_value().unwrap_or(Value::Null)));
                    path.pop();
                }
            }
            if canon.is_none() && before == out.set.len() + out.unset.len() {
                out.set.push((path.join("."), RawJson::from(actual), None));
            }
            return;
        }
    }
    if let Some(Node::Arr(citems)) = canon.map(|n| {
        let mut n = n.clone();
        n.expand();
        n
    }) && text.starts_with('[')
        && let Ok(aitems) = serde_json::from_str::<Vec<&RawValue>>(actual.get())
        && aitems.len() == citems.len()
    {
        for (i, (c, a)) in citems.iter().zip(aitems).enumerate() {
            path.push(i.to_string());
            diff_at(Some(c), a, path, keys, out);
            path.pop();
        }
        return;
    }
    diff_whole(canon, actual, path, out);
}

fn diff_whole(canon: Option<&Node>, actual: &RawValue, path: &[String], out: &mut Overrides) {
    let canon_value = canon.and_then(|c| c.to_value().ok());
    let equal = match (&canon_value, serde_json::from_str::<Value>(actual.get())) {
        (Some(c), Ok(a)) => *c == a,
        _ => false,
    };
    if !equal {
        out.set
            .push((path.join("."), RawJson::from(actual), canon_value));
    }
}

fn plain_key(key: &str) -> bool {
    !key.is_empty()
        && !key.contains('.')
        && !key.starts_with(['@', '~'])
        && !key.starts_with(HEADER_PREFIX)
}

fn typed_object(raw: &RawValue) -> bool {
    entries_of(raw).is_some_and(|e| {
        e.iter()
            .any(|(k, v)| k == "type" && v.get().trim() != "\"text\"")
    })
}

pub(crate) fn is_form(path: &str, value: &Value, canon_has: bool) -> bool {
    const PRESENTATION_KEYS: &[&str] = &[
        "obfuscation",
        "sequence_number",
        "~event",
        "id",
        "item_id",
        "status",
        "role",
        "object",
        "filename",
    ];
    const EXPLICIT_DEFAULTS: &[(&str, &str)] = &[
        ("store", "false"),
        ("background", "false"),
        ("stream", "false"),
        ("tools", "[]"),
        ("include", "[]"),
        ("metadata", "{}"),
        ("detail", "\"auto\""),
        ("truncation", "\"disabled\""),
        ("service_tier", "\"auto\""),
        ("annotations", "[]"),
        ("logprobs", "[]"),
        ("summary", "[]"),
        ("content", "[]"),
        ("stream_options", "{}"),
        ("text", "{}"),
        ("verbosity", "\"medium\""),
        ("include_obfuscation", "true"),
        ("include_obfuscation", "false"),
    ];
    let last = path.rsplit('.').next().unwrap_or(path);
    if PRESENTATION_KEYS.contains(&last) || value.is_null() {
        return true;
    }
    if path.ends_with("format.type") && value == "text" {
        return true;
    }
    if last == "type" {
        return canon_has || (path == "type" && value == "message");
    }
    EXPLICIT_DEFAULTS
        .iter()
        .any(|(k, v)| *k == last && serde_json::from_str::<Value>(v).is_ok_and(|d| d == *value))
}

pub(crate) fn apply(
    extras: &Extras,
    dialect: DialectId,
    scope: &str,
    exclude: &[&str],
    node: &mut Node,
) {
    if !extras.is_from(dialect) {
        return;
    }
    let select = |key: &str| -> Option<String> {
        if scope.is_empty() {
            let first = key.split('.').next().unwrap_or(key);
            if first.starts_with('@') || exclude.contains(&first) {
                return None;
            }
            Some(key.to_owned())
        } else if key == scope {
            Some(String::new())
        } else {
            key.strip_prefix(scope)
                .and_then(|r| r.strip_prefix('.'))
                .map(str::to_owned)
        }
    };
    let canon = match extras.form.get(CANON) {
        Some(Value::Object(m)) => Some(m),
        _ => None,
    };
    let segs_of = |rel: &str| -> Vec<String> {
        if rel.is_empty() {
            Vec::new()
        } else {
            rel.split('.').map(str::to_owned).collect()
        }
    };
    let pristine = node.clone();
    let fresh = |key: &str, segs: &[String]| -> bool {
        // Decoding records no path deeper than MAX_DEPTH, and the Node walks recurse once
        // per segment: a deeper key, only ever built by hand, would overflow the stack.
        if segs.len() > MAX_DEPTH {
            return false;
        }
        let refs: Vec<&str> = segs.iter().map(String::as_str).collect();
        let now = pristine.value_at(&refs);
        match (canon.and_then(|m| m.get(key)), now) {
            (None, None) => true,
            (Some(then), Some(now)) => values_equal(then, &now),
            _ => false,
        }
    };
    if let Some(Value::Array(list)) = extras.form.get(UNSET) {
        for p in list.iter().filter_map(Value::as_str) {
            if let Some(rel) = select(p) {
                let segs = segs_of(&rel);
                if fresh(p, &segs) {
                    let refs: Vec<&str> = segs.iter().map(String::as_str).collect();
                    node.unset_path(&refs);
                }
            }
        }
    }
    for (key, raw) in &extras.fields {
        if key.starts_with(HEADER_PREFIX) {
            continue;
        }
        if let Some(rel) = select(key) {
            let segs = segs_of(&rel);
            if fresh(key, &segs) {
                let refs: Vec<&str> = segs.iter().map(String::as_str).collect();
                node.set_path(&refs, Node::raw(raw));
            }
        }
    }
    if let Some(Value::Object(map)) = extras.form.get(SET) {
        for (key, value) in map {
            if let Some(rel) = select(key) {
                let segs = segs_of(&rel);
                if fresh(key, &segs) {
                    let refs: Vec<&str> = segs.iter().map(String::as_str).collect();
                    node.set_path(&refs, Node::Val(value.clone()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_kpp::{DialectId, Extras};

    const D: DialectId = DialectId::OpenAiResponses;

    fn raw(s: &str) -> Box<RawValue> {
        serde_json::from_str(s).unwrap()
    }

    fn round_trip(canon: Node, actual: &str, managed: &[&str]) -> Extras {
        let a = raw(actual);
        let o = Overrides::diff_with(&canon, &a, managed, &["text", "format"]);
        let mut e = Extras::new(D);
        o.record(&mut e, "");
        let mut n = canon.clone();
        apply(&e, D, "", &[], &mut n);
        let out = serde_json::to_string(&n).unwrap();
        assert!(
            values_equal(
                &serde_json::from_str(actual).unwrap(),
                &serde_json::from_str(&out).unwrap()
            ),
            "{out}"
        );
        e
    }

    #[test]
    fn records_unknown_nested_null_and_missing_keys() {
        let canon = Node::obj()
            .with("model", Node::str("m"))
            .with("temperature", Node::float(0.0))
            .with("annotations", Node::Arr(vec![]));
        let e = round_trip(
            canon,
            r#"{"model":"m","temperature":0,"text":{"verbosity":"low","format":{"type":"text"}},"x":{"big":123456789012345678901234567890},"store":null}"#,
            &[],
        );
        assert_eq!(e.fields.get("text.verbosity").unwrap().get(), "\"low\"");
        assert_eq!(
            e.fields.get("x").unwrap().get(),
            r#"{"big":123456789012345678901234567890}"#
        );
        assert_eq!(
            e.form.get("unset").unwrap(),
            &serde_json::json!(["annotations"])
        );
        let set = e.form.get("set").unwrap();
        assert_eq!(set["store"], Value::Null);
        assert_eq!(set["text.format.type"], "text");
        assert!(!e.fields.contains_key("temperature"), "0 equals 0.0");
    }

    #[test]
    fn empty_objects_and_managed_arrays() {
        let canon = Node::obj().with("input", Node::Arr(vec![Node::obj()]));
        let o = Overrides::diff_with(
            &canon,
            &raw(r#"{"input":[{"a":1},{"b":2}],"text":{}}"#),
            &["input"],
            &["text"],
        );
        let mut e = Extras::new(D);
        o.record(&mut e, "");
        assert!(e.fields.is_empty(), "managed arrays are the caller's job");
        assert_eq!(e.form.get("set").unwrap()["text"], serde_json::json!({}));
    }

    #[test]
    fn same_length_arrays_recurse_and_other_arrays_replace() {
        let part = |t: &str| {
            Node::obj()
                .with("type", Node::str("summary_text"))
                .with("text", Node::str(t))
        };
        let canon = Node::obj().with("summary", Node::Arr(vec![part("a")]));
        let e = round_trip(
            canon.clone(),
            r#"{"summary":[{"type":"summary_text","text":"a","x":1}]}"#,
            &[],
        );
        assert!(e.fields.contains_key("summary.0.x"));
        let e = round_trip(canon, r#"{"summary":[]}"#, &[]);
        assert_eq!(e.form.get("set").unwrap()["summary"], serde_json::json!([]));
    }

    #[test]
    fn scoped_records_apply_only_to_their_scope() {
        let canon = Node::obj().with("delta", Node::str("x"));
        let o = Overrides::diff(&canon, &raw(r#"{"delta":"x","obfuscation":"abc"}"#), &[]);
        let mut e = Extras::new(D);
        o.record(&mut e, "@done");
        let mut main = canon.clone();
        apply(&e, D, "", &[], &mut main);
        assert!(!main.has("obfuscation"));
        let mut scoped = canon;
        apply(&e, D, "@done", &[], &mut scoped);
        assert_eq!(scoped.get_str("obfuscation").unwrap(), "abc");
        let mut other = Node::obj();
        apply(&e, DialectId::OpenAiChat, "@done", &[], &mut other);
        assert!(!other.has("obfuscation"), "foreign extras are ignored");
    }

    #[test]
    fn classification() {
        assert!(is_form("detail", &serde_json::json!("auto"), false));
        assert!(!is_form("detail", &serde_json::json!("low"), false));
        assert!(is_form("x", &Value::Null, false));
        assert!(!is_form("x_future_field", &serde_json::json!({}), false));
        assert!(is_form(
            "stream_options.include_obfuscation",
            &serde_json::json!(false),
            false
        ));
        assert!(!is_form("text.verbosity", &serde_json::json!("low"), false));
        assert!(is_form("type", &serde_json::json!("output_text"), true));
        assert!(is_form("type", &serde_json::json!("message"), false));
        assert!(!is_form(
            "text.format.type",
            &serde_json::json!("grammar"),
            false
        ));
        assert!(is_form(
            "text.format.type",
            &serde_json::json!("text"),
            false
        ));
    }

    #[test]
    fn typed_objects_the_canonical_encoding_lacks_stay_whole_and_semantic() {
        let canon = Node::obj().with("model", Node::str("m"));
        let o = Overrides::diff_with(
            &canon,
            &raw(r#"{"model":"m","text":{"format":{"type":"grammar","syntax":"lark"}}}"#),
            &[],
            &["text", "format"],
        );
        let mut e = Extras::new(D);
        o.record(&mut e, "");
        assert_eq!(
            e.fields.get("text.format").unwrap().get(),
            r#"{"type":"grammar","syntax":"lark"}"#
        );
        assert!(e.form.get("set").is_none());
    }

    #[test]
    fn stale_overrides_lose_to_changed_typed_fields() {
        let canon = Node::obj()
            .with("model", Node::str("m"))
            .with("input", Node::Arr(vec![]));
        let o = Overrides::diff(
            &canon,
            &raw(r#"{"model":"m","max_output_tokens":null,"x":1}"#),
            &[],
        );
        let mut e = Extras::new(D);
        o.record(&mut e, "");
        let mut same = canon.clone();
        apply(&e, D, "", &[], &mut same);
        assert_eq!(
            serde_json::to_value(&same).unwrap(),
            serde_json::json!({"model": "m", "max_output_tokens": null, "x": 1})
        );
        let mut changed = Node::obj()
            .with("model", Node::str("m"))
            .with("input", Node::Arr(vec![Node::str("hi")]))
            .with("max_output_tokens", Node::int(100));
        apply(&e, D, "", &[], &mut changed);
        assert_eq!(
            serde_json::to_value(&changed).unwrap(),
            serde_json::json!({"model": "m", "input": ["hi"], "max_output_tokens": 100, "x": 1})
        );
    }
}
