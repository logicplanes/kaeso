use kaeso_kpp::{DialectId, KppError, RawJson, ToolDef};
use serde::Deserialize;
use serde_json::value::RawValue;
use serde_json::{Map, Value};

use crate::adapt::Draft;
use crate::codec::EgressCtx;
use crate::openai_responses::namespaces;

/// What a provider takes at the root of a function tool's parameter schema.
///
/// Where a provider takes less than the app sent, Kaeso merges the root union into one
/// object schema and appends a summary of its variants to the tool's description.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ToolSchemaRoot {
    /// Any schema, as the app wrote it.
    AsSent,
    /// An object, or a `oneOf` or `anyOf` union whose every branch is an inline object
    /// schema; a root `allOf` only on an object.
    ObjectUnions,
    /// An object without `oneOf`, `anyOf` or `allOf` at its root.
    Object,
}

const UNIONS: [&str; 2] = ["oneOf", "anyOf"];
const COMBINATORS: [&str; 3] = ["oneOf", "anyOf", "allOf"];
const DEFS: [&str; 2] = ["$defs", "definitions"];
const ROOT_ONLY: [&str; 6] = [
    "$schema",
    "$id",
    "$defs",
    "definitions",
    "title",
    "description",
];
const ANNOTATIONS: [&str; 10] = [
    "description",
    "title",
    "$comment",
    "$schema",
    "$id",
    "examples",
    "default",
    "deprecated",
    "readOnly",
    "writeOnly",
];
const UNEVALUATED: &str = "unevaluatedProperties";
const UNLISTED_EVALUATION: [&str; 5] = [
    "patternProperties",
    "dependentSchemas",
    "if",
    "then",
    "else",
];
const MAX_VARIANTS: usize = 64;
const MAX_DEPTH: usize = 32;

#[derive(Deserialize)]
struct RootKeys<'a> {
    #[serde(rename = "oneOf", borrow, default)]
    one_of: Option<&'a RawValue>,
    #[serde(rename = "anyOf", borrow, default)]
    any_of: Option<&'a RawValue>,
    #[serde(rename = "allOf", borrow, default)]
    all_of: Option<&'a RawValue>,
}

fn has_combinator(raw: &RawJson) -> bool {
    serde_json::from_str::<RootKeys<'_>>(raw.get())
        .is_ok_and(|k| k.one_of.is_some() || k.any_of.is_some() || k.all_of.is_some())
}

fn wanted(target: DialectId, ctx: &EgressCtx) -> (ToolSchemaRoot, &'static str) {
    match target {
        DialectId::AnthropicMessages => (ToolSchemaRoot::Object, "Anthropic"),
        DialectId::OpenAiChat => (
            ctx.chat_profile.request.tool_schema_root,
            ctx.chat_profile.provider,
        ),
        DialectId::OpenAiResponses => (
            ctx.responses_profile.tool_schema_root,
            ctx.responses_profile.provider,
        ),
        _ => (ToolSchemaRoot::AsSent, ""),
    }
}

pub(crate) fn for_target(
    d: &mut Draft<'_>,
    target: DialectId,
    ctx: &EgressCtx,
) -> Result<(), KppError> {
    let (wanted, provider) = wanted(target, ctx);
    if wanted == ToolSchemaRoot::AsSent {
        return Ok(());
    }
    let mut merged = Vec::new();
    for (i, tool) in d.get().tools.iter().enumerate() {
        let ToolDef::Function(f) = tool else {
            continue;
        };
        let Some(raw) = f.parameters.as_ref().filter(|p| has_combinator(p)) else {
            continue;
        };
        let Ok(Value::Object(root)) = raw.to_value() else {
            continue;
        };
        if !needs_merging(&root, wanted) {
            continue;
        }
        let path = namespaces::app_path(d.original(), &f.name).unwrap_or(format!("tools[{i}]"));
        let one = merge(&root).map_err(|fault| fault.refusal(&f.name, provider, &path))?;
        let schema = RawJson::from_serialize(&one.schema)
            .map_err(|e| KppError::invalid_request(e.to_string()).with_param(path.clone()))?;
        merged.push((i, path, schema, one.summary));
    }
    if merged.is_empty() {
        return Ok(());
    }
    let r = d.edit();
    let mut paths = Vec::with_capacity(merged.len());
    for (i, path, schema, summary) in merged {
        if let Some(ToolDef::Function(f)) = r.tools.get_mut(i) {
            f.parameters = Some(schema);
            if let Some(summary) = summary {
                f.description = Some(match f.description.take() {
                    Some(own) if !own.is_empty() => format!("{own}\n\n{summary}"),
                    _ => summary,
                });
            }
            paths.push(path);
        }
    }
    for path in paths {
        d.adapt(path);
    }
    Ok(())
}

fn is_inline_object(m: &Map<String, Value>) -> bool {
    !m.contains_key("$ref")
        && match m.get("type") {
            Some(t) => t == "object",
            None => m.get("properties").is_some_and(Value::is_object),
        }
}

fn needs_merging(root: &Map<String, Value>, wanted: ToolSchemaRoot) -> bool {
    match wanted {
        ToolSchemaRoot::AsSent => false,
        ToolSchemaRoot::Object => COMBINATORS.iter().any(|k| root.contains_key(*k)),
        ToolSchemaRoot::ObjectUnions => {
            let foreign_branch = UNIONS.iter().any(|k| match root.get(*k) {
                None => false,
                Some(Value::Array(branches)) => branches
                    .iter()
                    .any(|b| !b.as_object().is_some_and(is_inline_object)),
                Some(_) => true,
            });
            foreign_branch || (root.contains_key("allOf") && !is_inline_object(root))
        }
    }
}

enum Fault {
    NotObject,
    Unresolved(String),
    Circular,
    Conflict(String),
    RefSiblings,
    TooMany,
}

impl Fault {
    fn refusal(self, tool: &str, provider: &str, path: &str) -> KppError {
        let message = match self {
            Fault::NotObject => format!(
                "The parameters of `{tool}` are a union with a branch that is not an object, and {provider} takes only object schemas there. Make every branch an object."
            ),
            Fault::Unresolved(r) => format!(
                "The parameters of `{tool}` are a union that refers to `{r}`, which Kaeso cannot find in the schema's `$defs` or `definitions`, so it cannot merge the union for {provider}. Inline that branch."
            ),
            Fault::Circular => format!(
                "The parameters of `{tool}` are a union whose branches refer back to themselves, so Kaeso cannot merge it for {provider}. Inline the branches."
            ),
            Fault::RefSiblings => format!(
                "The parameters of `{tool}` put keywords next to a `$ref`, which JSON Schema drafts read differently, so Kaeso cannot merge them for {provider}. Name the draft in `$schema`."
            ),
            Fault::Conflict(keyword) => format!(
                "The parameters of `{tool}` combine schemas whose `{keyword}` Kaeso cannot merge into one object for {provider} without changing what the tool accepts. Merge them into one object."
            ),
            Fault::TooMany => format!(
                "The parameters of `{tool}` combine into more than {MAX_VARIANTS} variants, too many to merge into one object for {provider}. Simplify the root union."
            ),
        };
        KppError::unsupported("union at the root of a tool schema", message).with_param(path)
    }
}

struct Merged {
    schema: Value,
    summary: Option<String>,
}

fn merge(root: &Map<String, Value>) -> Result<Merged, Fault> {
    let doc = Doc(root);
    let variants = doc.object_variants(root, 0)?;
    let mut schema = doc.union(&variants);
    for key in ROOT_ONLY {
        if let Some(v) = root.get(key) {
            schema.insert(key.to_owned(), v.clone());
        }
    }
    let summary = (variants.len() > 1).then(|| doc.summary(&variants, &schema));
    Ok(Merged {
        schema: Value::Object(schema),
        summary,
    })
}

struct Doc<'a>(&'a Map<String, Value>);

enum RefSiblings {
    Applied,
    Ignored,
    Unknown,
}

impl<'a> Doc<'a> {
    // Up to draft 7, keywords next to `$ref` are ignored (draft-07 Core §8.3); from
    // 2019-09 on they apply alongside it (2019-09 Core §8.2.4.1).
    fn ref_siblings(&self) -> RefSiblings {
        let dialect = self
            .0
            .get("$schema")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if ["draft-03", "draft-04", "draft-06", "draft-07"]
            .iter()
            .any(|d| dialect.contains(d))
        {
            RefSiblings::Ignored
        } else if ["2019-09", "2020-12"].iter().any(|d| dialect.contains(d)) {
            RefSiblings::Applied
        } else {
            RefSiblings::Unknown
        }
    }

    fn lookup(&self, reference: &str) -> Result<&'a Value, Fault> {
        let unresolved = || Fault::Unresolved(reference.to_owned());
        let (defs, name) = DEFS
            .iter()
            .find_map(|defs| {
                let name = reference.strip_prefix("#/")?.strip_prefix(defs)?;
                Some((*defs, name.strip_prefix('/')?))
            })
            .ok_or_else(unresolved)?;
        if name.contains('/') {
            return Err(unresolved());
        }
        let name = name.replace("~1", "/").replace("~0", "~");
        self.0
            .get(defs)
            .and_then(|d| d.get(&name))
            .ok_or_else(unresolved)
    }

    fn resolved<'s>(&self, mut v: &'s Value) -> &'s Value
    where
        'a: 's,
    {
        for _ in 0..MAX_DEPTH {
            let Some(m) = v.as_object() else {
                return v;
            };
            let only_ref = m
                .keys()
                .all(|k| k == "$ref" || ANNOTATIONS.contains(&k.as_str()));
            match m.get("$ref").and_then(Value::as_str) {
                Some(r) if only_ref => match self.lookup(r) {
                    Ok(target) => v = target,
                    Err(_) => return v,
                },
                _ => return v,
            }
        }
        v
    }

    fn variants(&self, v: &'a Value, depth: usize) -> Result<Vec<Map<String, Value>>, Fault> {
        if depth > MAX_DEPTH {
            return Err(Fault::Circular);
        }
        match v {
            Value::Object(m) => self.object_variants(m, depth),
            Value::Bool(true) => Ok(vec![Map::new()]),
            _ => Err(Fault::NotObject),
        }
    }

    fn object_variants(
        &self,
        m: &'a Map<String, Value>,
        depth: usize,
    ) -> Result<Vec<Map<String, Value>>, Fault> {
        if let Some(reference) = m.get("$ref")
            && m.keys()
                .any(|k| k != "$ref" && !ANNOTATIONS.contains(&k.as_str()))
        {
            match self.ref_siblings() {
                RefSiblings::Applied => {}
                RefSiblings::Ignored => {
                    let target = self.lookup(reference.as_str().unwrap_or_default())?;
                    return self.variants(target, depth + 1);
                }
                RefSiblings::Unknown => return Err(Fault::RefSiblings),
            }
        }
        match m.get("type") {
            None => {}
            Some(t) if t == "object" => {}
            Some(Value::Array(types)) if types.iter().all(|t| t == "object") => {}
            Some(_) => return Err(Fault::NotObject),
        }
        let own: Map<String, Value> = m
            .iter()
            .filter(|(k, _)| {
                !matches!(k.as_str(), "$ref" | UNEVALUATED)
                    && !COMBINATORS.contains(&k.as_str())
                    && !DEFS.contains(&k.as_str())
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut out = vec![own];
        if let Some(reference) = m.get("$ref") {
            let target = self.lookup(reference.as_str().unwrap_or_default())?;
            out = product(&out, &self.variants(target, depth + 1)?)?;
        }
        if let Some(parts) = m.get("allOf") {
            for part in parts.as_array().ok_or(Fault::NotObject)? {
                out = product(&out, &self.variants(part, depth + 1)?)?;
            }
        }
        for key in UNIONS {
            let Some(branches) = m.get(key) else {
                continue;
            };
            let mut alternatives = Vec::new();
            for branch in branches.as_array().ok_or(Fault::NotObject)? {
                alternatives.extend(self.variants(branch, depth + 1)?);
                if alternatives.len() > MAX_VARIANTS {
                    return Err(Fault::TooMany);
                }
            }
            out = product(&out, &alternatives)?;
        }
        if let Some(rule) = m.get(UNEVALUATED) {
            for variant in &mut out {
                seal(variant, rule)?;
            }
        }
        Ok(out)
    }

    fn union(&self, variants: &[Map<String, Value>]) -> Map<String, Value> {
        let mut out = Map::new();
        out.insert("type".to_owned(), Value::from("object"));
        let mut properties = Map::new();
        let names: Vec<&String> = variants
            .iter()
            .filter_map(|v| v.get("properties").and_then(Value::as_object))
            .flat_map(Map::keys)
            .collect();
        for name in names {
            if properties.contains_key(name) {
                continue;
            }
            let schemas: Vec<&Value> = variants
                .iter()
                .filter_map(|v| v.get("properties")?.get(name))
                .collect();
            properties.insert(name.clone(), self.property(&schemas));
        }
        out.insert("properties".to_owned(), Value::Object(properties));
        let required: Vec<Value> = required_of(variants.first())
            .into_iter()
            .filter(|name| variants.iter().all(|v| required_of(Some(v)).contains(name)))
            .map(Value::from)
            .collect();
        if !required.is_empty() {
            out.insert("required".to_owned(), Value::Array(required));
        }
        let closed = variants.iter().map(|v| v.get("additionalProperties"));
        let mut closed = closed.collect::<Vec<_>>().into_iter();
        if let Some(Some(first)) = closed.next()
            && closed.all(|other| other == Some(first))
        {
            out.insert("additionalProperties".to_owned(), first.clone());
        }
        if let Some((first, rest)) = variants.split_first() {
            for (key, value) in first {
                if out.contains_key(key) || key == "required" {
                    continue;
                }
                if rest.iter().all(|v| v.get(key) == Some(value)) {
                    out.insert(key.clone(), value.clone());
                }
            }
        }
        out
    }

    fn property<'s>(&self, schemas: &[&'s Value]) -> Value
    where
        'a: 's,
    {
        let mut distinct: Vec<&'s Value> = Vec::new();
        for s in schemas {
            if !distinct
                .iter()
                .any(|d| self.resolved(d) == self.resolved(s))
            {
                distinct.push(s);
            }
        }
        match distinct.as_slice() {
            [] => Value::Bool(true),
            [one] => (*one).clone(),
            many => self.merged_enum(many).unwrap_or_else(|| {
                Value::Object(Map::from_iter([(
                    "anyOf".to_owned(),
                    Value::Array(many.iter().map(|s| (*s).clone()).collect()),
                )]))
            }),
        }
    }

    fn values<'s>(&self, schema: &'s Value) -> Option<Vec<&'s Value>>
    where
        'a: 's,
    {
        let m = self.resolved(schema).as_object()?;
        if !m.keys().all(|k| {
            matches!(k.as_str(), "type" | "enum" | "const") || ANNOTATIONS.contains(&k.as_str())
        }) {
            return None;
        }
        match (m.get("enum"), m.get("const")) {
            (Some(Value::Array(values)), None) => Some(values.iter().collect()),
            (None, Some(value)) => Some(vec![value]),
            _ => None,
        }
    }

    fn merged_enum<'s>(&self, schemas: &[&'s Value]) -> Option<Value>
    where
        'a: 's,
    {
        let mut values: Vec<Value> = Vec::new();
        for s in schemas {
            for v in self.values(s)? {
                if !values.contains(v) {
                    values.push(v.clone());
                }
            }
        }
        let types: Vec<Option<&Value>> = schemas
            .iter()
            .map(|s| self.resolved(s).get("type"))
            .collect();
        let mut out = Map::new();
        if let Some(Some(t)) = types.first()
            && types.iter().all(|other| *other == Some(*t))
        {
            out.insert("type".to_owned(), (*t).clone());
        }
        out.insert("enum".to_owned(), Value::Array(values));
        Some(Value::Object(out))
    }

    fn summary(&self, variants: &[Map<String, Value>], merged: &Map<String, Value>) -> String {
        let discriminators = self.discriminators(variants);
        let mut out = String::from("The arguments match one of these variants:");
        for (i, v) in variants.iter().enumerate() {
            let properties = v.get("properties").and_then(Value::as_object);
            let tags: Vec<String> = discriminators
                .iter()
                .filter_map(|name| {
                    let values = self.values(properties?.get(*name)?)?;
                    let values: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                    Some(format!("{name} {}", values.join(" or ")))
                })
                .collect();
            let required = required_of(Some(v));
            let optional: Vec<&str> = properties
                .into_iter()
                .flat_map(Map::keys)
                .map(String::as_str)
                .filter(|name| !required.contains(name))
                .collect();
            out.push_str(&format!("\n{}. ", i + 1));
            if !tags.is_empty() {
                out.push_str(&tags.join(", "));
                out.push_str(": ");
            }
            if required.is_empty() {
                out.push_str("requires nothing");
            } else {
                out.push_str(&format!("requires {}", required.join(", ")));
            }
            if !optional.is_empty() {
                out.push_str(&format!("; also takes {}", optional.join(", ")));
            }
            let only_here: Vec<String> = v
                .iter()
                .filter(|(key, value)| {
                    !matches!(key.as_str(), "properties" | "required" | "type")
                        && !ANNOTATIONS.contains(&key.as_str())
                        && merged.get(*key) != Some(*value)
                })
                .map(|(key, value)| format!("{key} {value}"))
                .collect();
            if !only_here.is_empty() {
                out.push_str(&format!("; with {}", only_here.join(", ")));
            }
            out.push('.');
        }
        out
    }

    fn discriminators<'s>(&self, variants: &'s [Map<String, Value>]) -> Vec<&'s str>
    where
        'a: 's,
    {
        let mut names: Vec<&'s str> = Vec::new();
        let all = variants
            .iter()
            .filter_map(|v| v.get("properties").and_then(Value::as_object))
            .flat_map(Map::keys);
        for name in all {
            if names.contains(&name.as_str()) {
                continue;
            }
            let sets: Option<Vec<Vec<&Value>>> = variants
                .iter()
                .filter_map(|v| v.get("properties")?.get(name))
                .map(|schema| self.values(schema))
                .collect();
            if let Some(sets) = sets
                && sets.iter().any(|set| Some(set) != sets.first())
            {
                names.push(name);
            }
        }
        names
    }
}

fn required_of(v: Option<&Map<String, Value>>) -> Vec<&str> {
    v.and_then(|v| v.get("required"))
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

fn product(
    left: &[Map<String, Value>],
    right: &[Map<String, Value>],
) -> Result<Vec<Map<String, Value>>, Fault> {
    if left.len().saturating_mul(right.len()) > MAX_VARIANTS {
        return Err(Fault::TooMany);
    }
    left.iter()
        .flat_map(|l| right.iter().map(move |r| intersect(l, r)))
        .collect()
}

fn property_names(m: &Map<String, Value>) -> Vec<&str> {
    m.get("properties")
        .and_then(Value::as_object)
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

fn is_open(rule: Option<&Value>) -> bool {
    matches!(rule, None | Some(Value::Bool(true)))
}

// `unevaluatedProperties` sees every property evaluated inside its own object, which after
// merging are the variant's properties, unless a keyword whose evaluation can't be listed
// decides it.
fn seal(variant: &mut Map<String, Value>, rule: &Value) -> Result<(), Fault> {
    match rule {
        Value::Bool(true) => Ok(()),
        Value::Bool(false) if !UNLISTED_EVALUATION.iter().any(|k| variant.contains_key(*k)) => {
            variant
                .entry("additionalProperties")
                .or_insert(Value::Bool(false));
            Ok(())
        }
        _ => Err(Fault::Conflict(UNEVALUATED.to_owned())),
    }
}

// Both sides must hold for the same arguments, so the result may only be a schema that
// accepts exactly what both accept; whatever can't be written that way is refused.
fn intersect(
    left: &Map<String, Value>,
    right: &Map<String, Value>,
) -> Result<Map<String, Value>, Fault> {
    let conflict = |key: &str| Fault::Conflict(key.to_owned());
    let mut out = left.clone();
    for (key, value) in right {
        let Some(mine) = out.get_mut(key) else {
            out.insert(key.clone(), value.clone());
            continue;
        };
        if mine == value || key == "type" || ANNOTATIONS.contains(&key.as_str()) {
            continue;
        }
        match (key.as_str(), mine) {
            ("properties", Value::Object(mine)) => {
                for (name, schema) in value.as_object().into_iter().flatten() {
                    match mine.get_mut(name) {
                        None => {
                            mine.insert(name.clone(), schema.clone());
                        }
                        Some(existing) if existing != schema => {
                            let both = Value::Array(vec![existing.clone(), schema.clone()]);
                            *existing = Value::Object(Map::from_iter([("allOf".to_owned(), both)]));
                        }
                        Some(_) => {}
                    }
                }
            }
            ("required", Value::Array(mine)) => {
                for name in value.as_array().into_iter().flatten() {
                    if !mine.contains(name) {
                        mine.push(name.clone());
                    }
                }
            }
            ("additionalProperties", _) => {}
            _ => return Err(conflict(key)),
        }
    }
    close(&mut out, left, right)?;
    Ok(out)
}

fn close(
    out: &mut Map<String, Value>,
    left: &Map<String, Value>,
    right: &Map<String, Value>,
) -> Result<(), Fault> {
    let conflict = || Fault::Conflict("additionalProperties".to_owned());
    let mut allowed: Option<Vec<&str>> = None;
    for (part, other) in [(left, right), (right, left)] {
        let rule = part.get("additionalProperties");
        if is_open(rule) {
            continue;
        }
        let own = property_names(part);
        if property_names(other).iter().all(|name| own.contains(name)) {
            continue;
        }
        if rule != Some(&Value::Bool(false)) || part.contains_key("patternProperties") {
            return Err(conflict());
        }
        allowed = Some(match allowed {
            None => own,
            Some(kept) => kept.into_iter().filter(|name| own.contains(name)).collect(),
        });
    }
    let rules: Vec<&Value> = [left, right]
        .iter()
        .filter_map(|m| m.get("additionalProperties"))
        .filter(|rule| !is_open(Some(rule)))
        .collect();
    if rules.contains(&&Value::Bool(false)) {
        out.insert("additionalProperties".to_owned(), Value::Bool(false));
    } else if let Some((first, rest)) = rules.split_first() {
        if rest.iter().any(|rule| rule != first) {
            return Err(conflict());
        }
        out.insert("additionalProperties".to_owned(), (*first).clone());
    }
    if let Some(allowed) = allowed {
        if required_of(Some(out))
            .iter()
            .any(|name| !allowed.contains(name))
        {
            return Err(conflict());
        }
        if let Some(Value::Object(properties)) = out.get_mut("properties") {
            properties.retain(|name, _| allowed.contains(&name.as_str()));
        }
    }
    Ok(())
}
