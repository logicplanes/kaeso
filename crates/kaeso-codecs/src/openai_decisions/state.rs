//! Structured state and asset references, as a decision request of another format brings them:
//! JSON that fits `input_json`, and images a reference stands for, written as parts in its place.

use indexmap::IndexMap;
use kaeso_kpp::{Asset, ImageBlock, RawJson};
use serde_json::Value;
use serde_json::value::RawValue;

/// `input_json` takes at most this many container levels
/// (<https://developers.openai.com/api/reference/resources/decisions/methods/create>).
pub(super) const MAX_JSON_DEPTH: usize = 32;
/// … this many values …
pub(super) const MAX_JSON_VALUES: usize = 10_000;
/// … and this many bytes of compact UTF-8 JSON.
pub(super) const MAX_JSON_BYTES: usize = 256 * 1024;

/// Whether `data` can be an `input_json` part: an object, or an array of objects, within the
/// format's limits.
pub(super) fn fits_input_json(data: &RawJson) -> bool {
    let Ok(value) = data.to_value() else {
        return false;
    };
    let shape = match &value {
        Value::Object(_) => true,
        Value::Array(items) => items.iter().all(Value::is_object),
        _ => false,
    };
    if !shape {
        return false;
    }
    let mut values = 0usize;
    if !within(&value, 1, &mut values) {
        return false;
    }
    serde_json::to_vec(&value).is_ok_and(|compact| compact.len() <= MAX_JSON_BYTES)
}

fn within(value: &Value, depth: usize, values: &mut usize) -> bool {
    *values += 1;
    if *values > MAX_JSON_VALUES {
        return false;
    }
    let children: Box<dyn Iterator<Item = &Value>> = match value {
        Value::Object(map) => Box::new(map.values()),
        Value::Array(items) => Box::new(items.iter()),
        _ => return true,
    };
    if depth > MAX_JSON_DEPTH {
        return false;
    }
    let mut children = children;
    children.all(|v| within(v, depth + 1, values))
}

/// A piece of a string that holds asset references: text, or the image a reference stands for.
#[derive(Debug, PartialEq)]
pub(super) enum Piece<'a> {
    Text(&'a str),
    Image(&'a str, &'a Asset),
}

/// `text` cut where asset references stand, each reference the image it stands for. The
/// earliest reference wins, and of two that start at the same place, the longer one.
pub(super) fn pieces<'a>(text: &'a str, assets: &'a IndexMap<String, Asset>) -> Vec<Piece<'a>> {
    let mut out = Vec::new();
    let mut rest = text;
    loop {
        let found = assets
            .iter()
            .filter(|(r, _)| !r.is_empty())
            .filter_map(|(r, a)| rest.find(r.as_str()).map(|at| (at, r.as_str(), a)))
            .min_by(|x, y| x.0.cmp(&y.0).then(y.1.len().cmp(&x.1.len())));
        let Some((at, reference, asset)) = found else {
            if !rest.is_empty() {
                out.push(Piece::Text(rest));
            }
            return out;
        };
        let (before, after) = rest.split_at(at);
        if !before.is_empty() {
            out.push(Piece::Text(before));
        }
        out.push(Piece::Image(reference, asset));
        rest = after.get(reference.len()..).unwrap_or_default();
    }
}

/// Whether any string in `raw`, not counting object keys, holds an asset reference; the first
/// one found.
pub(super) fn reference_in<'a>(
    raw: &RawValue,
    assets: &'a IndexMap<String, Asset>,
) -> Option<&'a str> {
    if assets.is_empty() {
        return None;
    }
    let value: Value = serde_json::from_str(raw.get()).ok()?;
    let mut strings = Vec::new();
    collect(&value, &mut strings);
    strings.iter().find_map(|s| reference_in_text(s, assets))
}

pub(super) fn reference_in_text<'a>(
    text: &str,
    assets: &'a IndexMap<String, Asset>,
) -> Option<&'a str> {
    assets
        .keys()
        .find(|r| !r.is_empty() && text.contains(r.as_str()))
        .map(String::as_str)
}

fn collect<'v>(value: &'v Value, out: &mut Vec<&'v str>) {
    match value {
        Value::String(s) => out.push(s),
        Value::Array(items) => items.iter().for_each(|v| collect(v, out)),
        Value::Object(map) => map.values().for_each(|v| collect(v, out)),
        _ => {}
    }
}

/// The image an asset holds, as an image block.
pub(super) fn image_of(asset: &Asset) -> ImageBlock {
    ImageBlock::new(asset.source.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_kpp::Source;

    fn raw(s: &str) -> RawJson {
        RawJson::from_string(s.to_owned()).unwrap()
    }

    #[test]
    fn input_json_takes_objects_within_its_limits() {
        assert!(fits_input_json(&raw(r#"{"a": [1, {"b": null}]}"#)));
        assert!(fits_input_json(&raw(r#"[{"a": 1}, {}]"#)));
        assert!(fits_input_json(&raw("[]")));
        for no in [r#"[{"a": 1}, 2]"#, r#""text""#, "5", "null", r#"[[{}]]"#] {
            assert!(!fits_input_json(&raw(no)), "{no}");
        }
        let deep = format!(
            "{}{}",
            "{\"a\":".repeat(32),
            "1".to_owned() + &"}".repeat(32)
        );
        assert!(fits_input_json(&raw(&deep)));
        let deeper = format!(
            "{}{}",
            "{\"a\":".repeat(33),
            "1".to_owned() + &"}".repeat(33)
        );
        assert!(!fits_input_json(&raw(&deeper)));
        let many = serde_json::to_string(&serde_json::json!({"v": vec![0; 9_998]})).unwrap();
        assert!(fits_input_json(&raw(&many)));
        let too_many = serde_json::to_string(&serde_json::json!({"v": vec![0; 9_999]})).unwrap();
        assert!(!fits_input_json(&raw(&too_many)));
        let big =
            serde_json::to_string(&serde_json::json!({"t": "x".repeat(MAX_JSON_BYTES)})).unwrap();
        assert!(!fits_input_json(&raw(&big)));
    }

    #[test]
    fn references_cut_text_into_pieces() {
        let mut assets = IndexMap::new();
        let a = Asset::new(Source::Url("https://example.com/a.png".into()));
        assets.insert("img".to_owned(), a.clone());
        assets.insert("img-2".to_owned(), a.clone());
        assets.insert(String::new(), a.clone());
        assert_eq!(
            pieces("Is img like img-2?", &assets),
            vec![
                Piece::Text("Is "),
                Piece::Image("img", &assets["img"]),
                Piece::Text(" like "),
                Piece::Image("img-2", &assets["img-2"]),
                Piece::Text("?"),
            ]
        );
        assert_eq!(
            pieces("img", &assets),
            vec![Piece::Image("img", &assets["img"])]
        );
        assert_eq!(pieces("none", &assets), vec![Piece::Text("none")]);
        assert_eq!(
            reference_in(
                &serde_json::from_str::<Box<RawValue>>(r#"{"img": ["x img-2"]}"#).unwrap(),
                &assets
            ),
            Some("img")
        );
        assert_eq!(
            reference_in(
                &serde_json::from_str::<Box<RawValue>>(r#"{"img": 1}"#).unwrap(),
                &assets
            ),
            None
        );
    }
}
