use kaeso_kpp::RawJson;
use serde::de::{MapAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use serde_json::value::RawValue;

#[derive(Clone, Debug)]
pub(crate) enum Node {
    Raw(RawJson),
    Obj(Vec<(String, Node)>),
    Arr(Vec<Node>),
    Val(Value),
}

impl Node {
    pub(crate) fn obj() -> Node {
        Node::Obj(Vec::new())
    }

    pub(crate) fn str(s: impl Into<String>) -> Node {
        Node::Val(Value::String(s.into()))
    }

    pub(crate) fn int(n: u64) -> Node {
        Node::Val(Value::from(n))
    }

    pub(crate) fn null() -> Node {
        Node::Val(Value::Null)
    }

    pub(crate) fn bool(b: bool) -> Node {
        Node::Val(Value::Bool(b))
    }

    pub(crate) fn float(f: f64) -> Node {
        Node::Val(serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number))
    }

    pub(crate) fn raw(r: &RawJson) -> Node {
        Node::Raw(r.clone())
    }

    pub(crate) fn raw_value(r: &RawValue) -> Node {
        Node::Raw(RawJson::from(r))
    }

    #[must_use]
    pub(crate) fn with(mut self, key: &str, value: Node) -> Node {
        self.insert(key, value);
        self
    }

    #[must_use]
    pub(crate) fn with_opt(self, key: &str, value: Option<Node>) -> Node {
        match value {
            Some(v) => self.with(key, v),
            None => self,
        }
    }

    pub(crate) fn insert(&mut self, key: &str, value: Node) {
        self.expand();
        if !matches!(self, Node::Obj(_)) {
            *self = Node::obj();
        }
        if let Node::Obj(entries) = self {
            match entries.iter_mut().find(|(k, _)| k == key) {
                Some(entry) => entry.1 = value,
                None => entries.push((key.to_owned(), value)),
            }
        }
    }

    pub(crate) fn remove(&mut self, key: &str) {
        self.expand();
        if let Node::Obj(entries) = self {
            entries.retain(|(k, _)| k != key);
        }
    }

    pub(crate) fn has(&self, key: &str) -> bool {
        match self {
            Node::Obj(entries) => entries.iter().any(|(k, _)| k == key),
            Node::Raw(r) => entries_of(r.as_raw()).is_some_and(|e| e.iter().any(|(k, _)| k == key)),
            Node::Val(Value::Object(m)) => m.contains_key(key),
            _ => false,
        }
    }

    pub(crate) fn get(&self, key: &str) -> Option<Node> {
        match self {
            Node::Obj(entries) => entries
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone()),
            Node::Raw(r) => entries_of(r.as_raw())?
                .into_iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| Node::raw_value(v)),
            Node::Val(Value::Object(m)) => m.get(key).cloned().map(Node::Val),
            _ => None,
        }
    }

    pub(crate) fn get_str(&self, key: &str) -> Option<String> {
        match self.get(key)?.to_value().ok()? {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key)?.to_value().ok()?.as_i64()
    }

    pub(crate) fn elements(&self) -> Vec<Node> {
        match self {
            Node::Arr(items) => items.clone(),
            Node::Raw(r) => serde_json::from_str::<Vec<&RawValue>>(r.get())
                .map(|v| v.into_iter().map(Node::raw_value).collect())
                .unwrap_or_default(),
            Node::Val(Value::Array(items)) => items.iter().cloned().map(Node::Val).collect(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn expand(&mut self) {
        let expanded = match self {
            Node::Raw(r) => {
                if let Some(entries) = entries_of(r.as_raw()) {
                    Some(Node::Obj(
                        entries
                            .into_iter()
                            .map(|(k, v)| (k, Node::raw_value(v)))
                            .collect(),
                    ))
                } else if r.get().trim_start().starts_with('[') {
                    serde_json::from_str::<Vec<&RawValue>>(r.get())
                        .ok()
                        .map(|items| Node::Arr(items.into_iter().map(Node::raw_value).collect()))
                } else {
                    None
                }
            }
            Node::Val(Value::Object(m)) => Some(Node::Obj(
                m.iter()
                    .map(|(k, v)| (k.clone(), Node::Val(v.clone())))
                    .collect(),
            )),
            Node::Val(Value::Array(items)) => {
                Some(Node::Arr(items.iter().cloned().map(Node::Val).collect()))
            }
            _ => None,
        };
        if let Some(node) = expanded {
            *self = node;
        }
    }

    pub(crate) fn to_value(&self) -> Result<Value, serde_json::Error> {
        match self {
            Node::Raw(r) => r.to_value(),
            Node::Val(v) => Ok(v.clone()),
            Node::Obj(entries) => {
                let mut map = serde_json::Map::new();
                for (k, v) in entries {
                    map.insert(k.clone(), v.to_value()?);
                }
                Ok(Value::Object(map))
            }
            Node::Arr(items) => items
                .iter()
                .map(Node::to_value)
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
        }
    }

    pub(crate) fn to_raw(&self) -> Result<RawJson, serde_json::Error> {
        RawJson::from_serialize(self)
    }

    pub(crate) fn value_at(&self, path: &[&str]) -> Option<Value> {
        let Some((first, rest)) = path.split_first() else {
            return self.to_value().ok();
        };
        let child = match self.get(first) {
            Some(c) => c,
            None => {
                let i = first.parse::<usize>().ok()?;
                self.elements().into_iter().nth(i)?
            }
        };
        child.value_at(rest)
    }

    pub(crate) fn set_path(&mut self, path: &[&str], value: Node) {
        let Some((first, rest)) = path.split_first() else {
            *self = value;
            return;
        };
        self.expand();
        if let Node::Arr(items) = self
            && let Some(item) = first.parse::<usize>().ok().and_then(|i| items.get_mut(i))
        {
            item.set_path(rest, value);
            return;
        }
        if rest.is_empty() {
            self.insert(first, value);
            return;
        }
        if !self.has(first) {
            self.insert(first, Node::obj());
        }
        if let Node::Obj(entries) = self
            && let Some((_, child)) = entries.iter_mut().find(|(k, _)| k == first)
        {
            child.set_path(rest, value);
        }
    }

    pub(crate) fn unset_path(&mut self, path: &[&str]) {
        let Some((first, rest)) = path.split_first() else {
            return;
        };
        self.expand();
        if rest.is_empty() {
            self.remove(first);
            return;
        }
        match self {
            Node::Arr(items) => {
                if let Some(item) = first.parse::<usize>().ok().and_then(|i| items.get_mut(i)) {
                    item.unset_path(rest);
                }
            }
            Node::Obj(entries) => {
                if let Some((_, child)) = entries.iter_mut().find(|(k, _)| k == first) {
                    child.unset_path(rest);
                }
            }
            _ => {}
        }
    }
}

impl From<Value> for Node {
    fn from(v: Value) -> Node {
        Node::Val(v)
    }
}

impl Serialize for Node {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Node::Raw(r) => r.serialize(serializer),
            Node::Val(v) => v.serialize(serializer),
            Node::Obj(entries) => {
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (k, v) in entries {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
            Node::Arr(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
        }
    }
}

pub(crate) fn entries_of(raw: &RawValue) -> Option<Vec<(String, &RawValue)>> {
    struct Entries<'a>(Vec<(String, &'a RawValue)>);
    impl<'de: 'a, 'a> Deserialize<'de> for Entries<'a> {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct V;
            impl<'de> Visitor<'de> for V {
                type Value = Entries<'de>;
                fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str("a JSON object")
                }
                fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                    let mut out: Vec<(String, &'de RawValue)> = Vec::new();
                    while let Some((k, v)) = map.next_entry::<String, &'de RawValue>()? {
                        if out.iter().any(|(seen, _)| *seen == k) {
                            return Err(serde::de::Error::custom(format!("duplicate key `{k}`")));
                        }
                        out.push((k, v));
                    }
                    Ok(Entries(out))
                }
            }
            d.deserialize_map(V)
        }
    }
    if !raw.get().trim_start().starts_with('{') {
        return None;
    }
    serde_json::from_str::<Entries<'_>>(raw.get())
        .ok()
        .map(|e| e.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(s: &str) -> RawJson {
        RawJson::from_string(s.to_owned()).unwrap()
    }

    #[test]
    fn raw_children_keep_their_bytes_when_edited() {
        let mut n = Node::raw(&raw(r#"{"b": {"z": 1,  "a": 2}, "a": 1}"#));
        n.set_path(&["c", "d"], Node::int(3));
        n.set_path(&["a"], Node::int(5));
        let out = serde_json::to_string(&n).unwrap();
        assert_eq!(out, r#"{"b":{"z": 1,  "a": 2},"a":5,"c":{"d":3}}"#);
        n.unset_path(&["b", "z"]);
        assert_eq!(
            serde_json::to_string(&n).unwrap(),
            r#"{"b":{"a":2},"a":5,"c":{"d":3}}"#
        );
    }

    #[test]
    fn numeric_segments_index_arrays() {
        let mut n = Node::obj().with("xs", Node::Arr(vec![Node::obj(), Node::obj()]));
        n.set_path(&["xs", "1", "k"], Node::bool(true));
        assert_eq!(
            serde_json::to_string(&n).unwrap(),
            r#"{"xs":[{},{"k":true}]}"#
        );
        assert_eq!(n.get("xs").unwrap().elements().len(), 2);
        assert!(n.has("xs"));
        assert_eq!(n.value_at(&["xs", "1", "k"]), Some(Value::Bool(true)));
        assert_eq!(n.value_at(&["xs", "5"]), None);
        assert!(entries_of(&raw(r#"{"a":1,"a":2}"#).into_inner()).is_none());
        assert_eq!(Node::raw(&raw(r#"{"s":"v"}"#)).get_str("s").unwrap(), "v");
    }
}
