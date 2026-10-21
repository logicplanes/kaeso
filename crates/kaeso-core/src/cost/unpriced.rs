use kaeso_kpp::{DialectId, Extras, Request, ToolDef};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unpriced {
    pub param: &'static str,
    pub feature: String,
}

const PRICED_TIERS: &[&str] = &["auto", "default", "flex", "on_demand", "standard_only"];

const CLIENT_TOOLS: &[&str] = &[
    "custom",
    "local_shell",
    "apply_patch",
    "computer",
    "computer_use_preview",
];

const CLIENT_TOOL_VERSIONS: &[&str] = &["bash_", "text_editor_", "computer_", "memory_"];

const PROVIDER_RUN_FIELDS: &[&str] = &[
    "mcp_servers",
    "web_search_options",
    "plugins",
    "search_parameters",
];

const SERVICE_TIER: &str = "service_tier";

const BETA_HEADER: &str = "anthropic-beta";

const UNPRICED_BETAS: &[&str] = &["context-1m-"];

const SHOWN: usize = 40;

pub fn unpriced(request: &Request, egress: DialectId) -> Option<Unpriced> {
    let Request::Generate(g) = request else {
        return None;
    };
    g.tools
        .iter()
        .find_map(|t| provider_tool(t, egress))
        .or_else(|| request_field(&g.extras, egress))
}

fn shown(text: &str) -> String {
    if text.chars().count() <= SHOWN {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(SHOWN - 1).collect();
    cut.push('…');
    cut
}

fn client_tool(ty: &str) -> bool {
    CLIENT_TOOLS.contains(&ty) || CLIENT_TOOL_VERSIONS.iter().any(|v| ty.starts_with(v))
}

fn provider_tool(tool: &ToolDef, egress: DialectId) -> Option<Unpriced> {
    let ToolDef::Unknown(item) = tool else {
        return None;
    };
    if !item.origin.is(egress) {
        return None;
    }
    let raw = item.raw.to_value().ok();
    let ty = raw
        .as_ref()
        .and_then(|v| v.get("type"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    (!client_tool(ty)).then(|| Unpriced {
        param: "tools",
        feature: if ty.is_empty() {
            "tools".to_owned()
        } else {
            shown(ty)
        },
    })
}

fn is_unset(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}

fn request_field(extras: &Extras, egress: DialectId) -> Option<Unpriced> {
    if !extras.is_from(egress) {
        return None;
    }
    let tier = extras
        .fields
        .get(SERVICE_TIER)
        .and_then(|raw| raw.to_value().ok());
    match tier {
        None | Some(Value::Null) => {}
        Some(Value::String(t)) if PRICED_TIERS.contains(&t.as_str()) => {}
        Some(Value::String(t)) => {
            return Some(Unpriced {
                param: SERVICE_TIER,
                feature: shown(&format!("{SERVICE_TIER}: {t}")),
            });
        }
        Some(_) => {
            return Some(Unpriced {
                param: SERVICE_TIER,
                feature: SERVICE_TIER.to_owned(),
            });
        }
    }
    for field in PROVIDER_RUN_FIELDS {
        let set = extras
            .fields
            .get(*field)
            .and_then(|raw| raw.to_value().ok())
            .is_some_and(|v| !is_unset(&v));
        if set {
            return Some(Unpriced {
                param: field,
                feature: (*field).to_owned(),
            });
        }
    }
    extras
        .headers()
        .filter(|(name, _)| *name == BETA_HEADER)
        .flat_map(|(_, value)| {
            value
                .split(',')
                .map(|b| b.trim().to_owned())
                .collect::<Vec<_>>()
        })
        .find(|beta| UNPRICED_BETAS.iter().any(|p| beta.starts_with(p)))
        .map(|beta| Unpriced {
            param: BETA_HEADER,
            feature: shown(&format!("{BETA_HEADER}: {beta}")),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_codecs::{HeaderList, dispatch};
    use kaeso_kpp::RequestKind;
    use serde_json::json;

    fn decoded(dialect: DialectId, body: &Value, headers: &[(&str, &str)]) -> Request {
        let mut list = HeaderList::new();
        for &(n, v) in headers {
            list = list.with(n, v);
        }
        let bytes = serde_json::to_vec(body).unwrap();
        dispatch::decode_request(dialect, RequestKind::Generate, &bytes, &list).unwrap()
    }

    fn of(dialect: DialectId, body: Value) -> Option<Unpriced> {
        unpriced(&decoded(dialect, &body, &[]), dialect)
    }

    fn found(param: &'static str, feature: &str) -> Option<Unpriced> {
        Some(Unpriced {
            param,
            feature: feature.to_owned(),
        })
    }

    fn fixture(path: &str) -> Value {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../kaeso-codecs/fixtures")
            .join(path);
        serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap()
    }

    #[test]
    fn anthropic_server_tools_and_mcp_servers_are_unpriced_and_client_tools_are_not() {
        let a = DialectId::AnthropicMessages;
        assert_eq!(
            of(
                a,
                fixture("anthropic_messages/requests/web-search-server-tool.json")
            ),
            found("tools", "web_search_20250305")
        );
        let with = |tool: Value| {
            json!({
                "model": "claude-haiku-4-5",
                "max_tokens": 64,
                "messages": [{ "role": "user", "content": "Hi" }],
                "tools": [{ "name": "lookup", "input_schema": { "type": "object" } }, tool],
            })
        };
        assert_eq!(
            of(
                a,
                with(json!({ "type": "code_execution_20250825", "name": "code_execution" }))
            ),
            found("tools", "code_execution_20250825")
        );
        for client in [
            json!({ "type": "bash_20250124", "name": "bash" }),
            json!({ "type": "text_editor_20250728", "name": "str_replace_based_edit_tool" }),
            json!({ "type": "computer_20250124", "name": "computer", "display_width_px": 1024, "display_height_px": 768 }),
            json!({ "type": "memory_20250818", "name": "memory" }),
            json!({ "type": "custom", "name": "other", "input_schema": { "type": "object" } }),
        ] {
            assert_eq!(of(a, with(client.clone())), None, "{client}");
        }
        let mut mcp = with(json!({ "type": "bash_20250124", "name": "bash" }));
        mcp["mcp_servers"] =
            json!([{ "type": "url", "url": "https://mcp.example.com/sse", "name": "example" }]);
        assert_eq!(of(a, mcp.clone()), found("mcp_servers", "mcp_servers"));
        mcp["mcp_servers"] = json!([]);
        assert_eq!(of(a, mcp), None);
    }

    #[test]
    fn responses_and_chat_provider_tools_and_search_fields_are_unpriced() {
        let r = DialectId::OpenAiResponses;
        assert_eq!(
            of(r, fixture("openai_responses/requests/web-search.json")),
            found("tools", "web_search")
        );
        for (ty, extra) in [
            ("image_generation", json!({})),
            (
                "code_interpreter",
                json!({ "container": { "type": "auto" } }),
            ),
            ("file_search", json!({ "vector_store_ids": ["vs_1"] })),
            (
                "mcp",
                json!({ "server_label": "x", "server_url": "https://mcp.example.com" }),
            ),
        ] {
            let mut tool = json!({ "type": ty });
            tool.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            let body = json!({ "model": "gpt-5.5", "input": "Hi", "tools": [tool] });
            assert_eq!(of(r, body), found("tools", ty), "{ty}");
        }
        let client = json!({
            "model": "gpt-5.5",
            "input": "Hi",
            "tools": [
                { "type": "function", "name": "f", "parameters": { "type": "object" } },
                { "type": "custom", "name": "apply_patch" },
                { "type": "local_shell" },
            ],
        });
        assert_eq!(of(r, client), None);

        let c = DialectId::OpenAiChat;
        assert_eq!(
            of(c, fixture("openai_chat/requests/web-search.json")),
            found("web_search_options", "web_search_options")
        );
        assert_eq!(
            of(
                c,
                fixture("openai_chat/providers/xai/requests/live-search-deprecated.json")
            ),
            found("search_parameters", "search_parameters")
        );
        assert_eq!(
            of(c, fixture("openai_chat/requests/tools-custom-grammar.json")),
            None
        );
        let plugins = json!({
            "model": "openai/gpt-5.5",
            "messages": [{ "role": "user", "content": "Hi" }],
            "plugins": [{ "id": "web" }],
        });
        assert_eq!(of(c, plugins), found("plugins", "plugins"));
    }

    #[test]
    fn service_tiers_other_than_standard_or_cheaper_are_unpriced() {
        let chat = |tier: Value| {
            json!({
                "model": "gpt-5.5",
                "messages": [{ "role": "user", "content": "Hi" }],
                "service_tier": tier,
            })
        };
        for priced in ["auto", "default", "flex", "on_demand"] {
            assert_eq!(
                of(DialectId::OpenAiChat, chat(json!(priced))),
                None,
                "{priced}"
            );
        }
        assert_eq!(of(DialectId::OpenAiChat, chat(Value::Null)), None);
        assert_eq!(
            of(DialectId::OpenAiChat, chat(json!("priority"))),
            found("service_tier", "service_tier: priority")
        );
        assert_eq!(
            of(DialectId::OpenAiChat, chat(json!("scale"))),
            found("service_tier", "service_tier: scale")
        );
        assert_eq!(
            of(DialectId::OpenAiChat, chat(json!(1))),
            found("service_tier", "service_tier")
        );
        assert_eq!(
            of(
                DialectId::OpenAiResponses,
                fixture("openai_responses/requests/sampling-and-ids.json")
            ),
            found("service_tier", "service_tier: priority")
        );
        assert_eq!(
            of(
                DialectId::OpenAiChat,
                fixture("openai_chat/requests/all-params-sdk.json")
            ),
            found("web_search_options", "web_search_options"),
            "an `auto` tier passes and the search options after it are judged"
        );
        assert_eq!(
            of(
                DialectId::AnthropicMessages,
                fixture("anthropic_messages/requests/service-tier-standard-only.json")
            ),
            None
        );
    }

    #[test]
    fn the_long_context_beta_is_unpriced_and_other_betas_are_not() {
        let a = DialectId::AnthropicMessages;
        let body = json!({
            "model": "claude-sonnet-4-5",
            "max_tokens": 64,
            "messages": [{ "role": "user", "content": "Hi" }],
        });
        let beta = |value: &str| unpriced(&decoded(a, &body, &[("anthropic-beta", value)]), a);
        assert_eq!(
            beta("claude-code-20250219, context-1m-2025-08-07"),
            found("anthropic-beta", "anthropic-beta: context-1m-2025-08-07")
        );
        assert_eq!(
            beta("claude-code-20250219,interleaved-thinking-2025-05-14"),
            None
        );
    }

    #[test]
    fn features_bound_to_another_dialect_or_outside_generation_are_not_judged() {
        let web = decoded(
            DialectId::AnthropicMessages,
            &fixture("anthropic_messages/requests/web-search-server-tool.json"),
            &[],
        );
        assert_eq!(unpriced(&web, DialectId::OpenAiChat), None);
        let Request::Generate(g) = web else {
            unreachable!()
        };
        assert_eq!(
            unpriced(&Request::CountTokens(g), DialectId::AnthropicMessages),
            None
        );
        let long = format!("web_search_{}", "9".repeat(60));
        let tool = json!({ "type": long, "name": "web_search" });
        let body = json!({
            "model": "claude-haiku-4-5",
            "max_tokens": 64,
            "messages": [{ "role": "user", "content": "Hi" }],
            "tools": [tool],
        });
        let u = of(DialectId::AnthropicMessages, body).unwrap();
        assert_eq!(u.feature.chars().count(), SHOWN);
        assert!(u.feature.ends_with('…'));
    }
}
