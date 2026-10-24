use std::collections::HashSet;

use kaeso_kpp::{Block, DialectId, GenerateRequest, KppError, RawItem, Request, ToolDef};
use serde::Deserialize;
use serde_json::value::RawValue;

use super::DIALECT;
use super::node::Node;
use super::parts::function_call_item;
use super::request::decode_tool;
use crate::cross::foreign;
use crate::wire::{entries_of, type_tag};

pub(crate) const NAMESPACE: &str = "namespace";

#[derive(Deserialize)]
struct NamespaceTool {
    name: String,
    #[serde(default)]
    description: Option<String>,
    tools: Vec<Box<RawValue>>,
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

pub(crate) fn flat_name(namespace: &str, name: &str) -> String {
    format!("{namespace}__{name}")
}

fn is_namespace(item: &RawItem) -> bool {
    item.origin.is(DIALECT)
        && type_tag(item.raw.as_raw()).ok().flatten().as_deref() == Some(NAMESPACE)
}

fn namespace_tool(item: &RawItem) -> Option<NamespaceTool> {
    is_namespace(item)
        .then(|| serde_json::from_str(item.raw.get()).ok())
        .flatten()
}

const REMOVE: &str =
    "Remove it, or send the request to a model served through its original format.";

fn flatten_tool(item: &RawItem, path: &str, target: DialectId) -> Result<Vec<ToolDef>, KppError> {
    let refuse = |feature: String, at: &str| foreign(feature, &item.origin, target, at, REMOVE);
    let unknown = entries_of(item.raw.as_raw())
        .unwrap_or_default()
        .into_iter()
        .find(|(k, _)| !matches!(k.as_str(), "type" | "name" | "description" | "tools"));
    if let Some((key, _)) = unknown {
        return Err(refuse(format!("{path}.{key}"), &format!("{path}.{key}")));
    }
    let ns = namespace_tool(item).ok_or_else(|| refuse("namespace tool".to_owned(), path))?;
    let mut out = Vec::with_capacity(ns.tools.len());
    for (j, raw) in ns.tools.iter().enumerate() {
        let at = format!("{path}.tools[{j}]");
        let mut f = match decode_tool(raw, &item.origin) {
            ToolDef::Function(f) => f,
            _ => {
                let ty = type_tag(raw).ok().flatten();
                let ty = ty.as_deref().unwrap_or("unknown");
                return Err(refuse(format!("{ty} tool in a namespace"), &at));
            }
        };
        if let Some(key) = f.extras.fields.keys().next() {
            return Err(refuse(format!("{at}.{key}"), &format!("{at}.{key}")));
        }
        f.name = flat_name(&ns.name, &f.name);
        f.description = match (ns.description.as_deref(), f.description.take()) {
            (Some(group), Some(own)) => Some(format!("{group}\n\n{own}")),
            (Some(group), None) => Some(group.to_owned()),
            (None, own) => own,
        };
        out.push(ToolDef::Function(f));
    }
    Ok(out)
}

fn call_namespace(call: &kaeso_kpp::ToolCallBlock) -> Option<String> {
    if !call.extras.is_from(DIALECT) {
        return None;
    }
    call.extras.fields.get(NAMESPACE)?.parse::<String>().ok()
}

fn needs_flattening(r: &GenerateRequest) -> bool {
    let tools = r
        .tools
        .iter()
        .any(|t| matches!(t, ToolDef::Unknown(item) if is_namespace(item)));
    tools
        || r.messages.iter().any(|m| {
            m.blocks
                .iter()
                .any(|b| matches!(b, Block::ToolCall(c) if call_namespace(c).is_some()))
        })
}

pub(crate) fn flatten(
    r: &GenerateRequest,
    target: DialectId,
) -> Result<Option<GenerateRequest>, KppError> {
    if !needs_flattening(r) {
        return Ok(None);
    }
    let mut tools = Vec::with_capacity(r.tools.len());
    let mut flattened = HashSet::new();
    for (i, tool) in r.tools.iter().enumerate() {
        match tool {
            ToolDef::Unknown(item) if is_namespace(item) => {
                for t in flatten_tool(item, &format!("tools[{i}]"), target)? {
                    if let ToolDef::Function(f) = &t {
                        flattened.insert(f.name.clone());
                    }
                    tools.push((i, t));
                }
            }
            other => tools.push((i, other.clone())),
        }
    }
    let mut seen = HashSet::new();
    for (i, t) in &tools {
        if let ToolDef::Function(f) = t
            && !seen.insert(f.name.as_str())
            && flattened.contains(&f.name)
        {
            return Err(KppError::unsupported(
                "namespace tool",
                format!(
                    "`{}` names a function tool and a tool in a namespace, and {} has no namespaces. Rename one of them.",
                    f.name,
                    target.display_name()
                ),
            )
            .with_param(format!("tools[{i}]")));
        }
    }
    let mut out = r.clone();
    out.tools = tools.into_iter().map(|(_, t)| t).collect();
    for m in &mut out.messages {
        for b in &mut m.blocks {
            if let Block::ToolCall(c) = b
                && let Some(ns) = call_namespace(c)
            {
                c.name = flat_name(&ns, &c.name);
                c.extras.fields.shift_remove(NAMESPACE);
            }
        }
    }
    Ok(Some(out))
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Namespaces(Vec<(String, String, String)>);

impl Namespaces {
    pub(crate) fn of(req: Option<&Request>) -> Namespaces {
        let Some(Request::Generate(r) | Request::CountTokens(r)) = req else {
            return Namespaces::default();
        };
        let mut out = Vec::new();
        for t in &r.tools {
            let ToolDef::Unknown(item) = t else { continue };
            let Some(ns) = namespace_tool(item) else {
                continue;
            };
            for child in &ns.tools {
                if let Ok(c) = serde_json::from_str::<Named>(child.get()) {
                    out.push((flat_name(&ns.name, &c.name), ns.name.clone(), c.name));
                }
            }
        }
        Namespaces(out)
    }

    pub(crate) fn split(&self, name: &str) -> Option<(&str, &str)> {
        self.0
            .iter()
            .find(|(flat, _, _)| flat == name)
            .map(|(_, ns, own)| (ns.as_str(), own.as_str()))
    }

    pub(super) fn call_item(
        &self,
        call_id: &str,
        name: &str,
        arguments: &str,
        id_status: Option<(&str, &str)>,
    ) -> Node {
        match self.split(name) {
            Some((ns, own)) => function_call_item(call_id, own, arguments, id_status)
                .with(NAMESPACE, Node::str(ns)),
            None => function_call_item(call_id, name, arguments, id_status),
        }
    }
}
