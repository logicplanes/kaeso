#![cfg(test)]

use std::path::Path;
use std::sync::Arc;

use kaeso_codecs::openai_chat::ChatProfile;
use kaeso_codecs::openai_responses::ResponsesProfile;
use kaeso_codecs::{EgressCtx, EncodeCtx, HeaderList, ToolSchemaRoot, dispatch};
use kaeso_kpp::{DialectId, ErrorKind, KppError, Request, RequestKind};
use serde_json::{Value, json};

const A: DialectId = DialectId::AnthropicMessages;
const C: DialectId = DialectId::OpenAiChat;
const R: DialectId = DialectId::OpenAiResponses;

fn fixture(dialect: DialectId, path: &str) -> Vec<u8> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(dialect.as_str())
        .join(path);
    std::fs::read(file).unwrap()
}

fn codex_desktop() -> Value {
    serde_json::from_slice(&fixture(R, "requests/codex-desktop-root-union.json")).unwrap()
}

fn automation_update(body: &Value) -> Value {
    body["tools"][0]["tools"][0]["parameters"].clone()
}

fn decode(dialect: DialectId, body: &Value) -> Request {
    let mut req = dispatch::decode_request(
        dialect,
        RequestKind::Generate,
        &serde_json::to_vec(body).unwrap(),
        &HeaderList::new(),
    )
    .unwrap();
    if let Request::Generate(g) = &mut req {
        g.sampling.max_output_tokens.get_or_insert(4096);
    }
    req
}

fn xai(req: &Request) -> EgressCtx {
    EgressCtx::new(req)
        .with_provider("xai")
        .with_responses_profile(
            ResponsesProfile::compatible("SpaceX")
                .xai_web_search()
                .without_null_reasoning_content()
                .without_openai_encrypted_reasoning()
                .tool_schema_root(ToolSchemaRoot::ObjectUnions),
        )
        .with_chat_profile(
            ChatProfile::compatible("SpaceX").tool_schema_root(ToolSchemaRoot::ObjectUnions),
        )
}

fn anthropic(req: &Request) -> EgressCtx {
    EgressCtx::new(req).with_provider("anthropic")
}

fn send(target: DialectId, req: &Request, ctx: &mut EgressCtx) -> Result<Value, KppError> {
    dispatch::check(target, req, ctx)?;
    let enc = dispatch::encode_request(target, req, ctx)?;
    Ok(serde_json::from_slice(&enc.body).unwrap())
}

fn sent(target: DialectId, req: &Request, mut ctx: EgressCtx) -> (Value, Vec<String>) {
    let body = send(target, req, &mut ctx).unwrap_or_else(|e| panic!("{target}: {e}"));
    assert!(ctx.left_out.is_empty(), "{target}: {:?}", ctx.left_out);
    (body, ctx.adapted)
}

fn tool<'a>(target: DialectId, body: &'a Value, name: &str) -> &'a Value {
    body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| match target {
            C => t["function"]["name"] == name,
            _ => t["name"] == name,
        })
        .unwrap_or_else(|| panic!("{target}: no tool {name} in {body}"))
}

fn schema(target: DialectId, tool: &Value) -> &Value {
    match target {
        A => &tool["input_schema"],
        C => &tool["function"]["parameters"],
        _ => &tool["parameters"],
    }
}

fn description(target: DialectId, tool: &Value) -> &str {
    match target {
        C => tool["function"]["description"].as_str().unwrap(),
        _ => tool["description"].as_str().unwrap(),
    }
}

const AUTOMATION_UPDATE: &str = "mcp__codex_app__automation_update";

#[test]
fn codex_desktops_automation_update_reaches_claude_as_one_object() {
    let body = codex_desktop();
    let req = decode(R, &body);
    let (sent, adapted) = sent(A, &req, anthropic(&req));
    assert_eq!(adapted, ["tools[0].tools[0]"]);
    let tool = tool(A, &sent, AUTOMATION_UPDATE);
    let input = schema(A, tool);
    for key in ["oneOf", "anyOf", "allOf"] {
        assert!(input.get(key).is_none(), "{key} at the root: {input}");
    }
    assert_eq!(input["$defs"], automation_update(&body)["$defs"]);
    insta::assert_json_snapshot!("anthropic_automation_update", tool);
    let list_threads = tool_of(&sent, "mcp__codex_app__list_threads");
    assert_eq!(
        list_threads["input_schema"],
        body["tools"][0]["tools"][1]["parameters"]
    );
}

fn tool_of<'a>(body: &'a Value, name: &str) -> &'a Value {
    tool(A, body, name)
}

#[test]
fn codex_desktops_automation_update_reaches_grok_as_one_object() {
    let body = codex_desktop();
    let req = decode(R, &body);
    let (sent, adapted) = sent(R, &req, xai(&req));
    assert_eq!(adapted, ["tools[0].tools[0]"]);
    let tool = tool(R, &sent, AUTOMATION_UPDATE);
    insta::assert_json_snapshot!("xai_automation_update", tool);
    let mut ctx = anthropic(&req);
    let claude = send(A, &req, &mut ctx).unwrap();
    let claude = tool_of(&claude, AUTOMATION_UPDATE);
    assert_eq!(tool["parameters"], claude["input_schema"]);
    assert_eq!(tool["description"], claude["description"]);
}

#[test]
fn the_merged_automation_update_names_each_mode_and_its_fields() {
    let req = decode(R, &codex_desktop());
    let (sent, _) = sent(A, &req, anthropic(&req));
    let tool = tool(A, &sent, AUTOMATION_UPDATE);
    let input = schema(A, tool);
    assert_eq!(
        input["properties"]["mode"],
        json!({"type": "string", "enum": ["view", "create", "suggested_create", "update", "suggested_update", "delete"]})
    );
    assert_eq!(input["required"], json!(["mode"]));
    assert_eq!(input["additionalProperties"], json!(false));
    let text = description(A, tool);
    assert!(text.starts_with(
        "Tools provided by the Codex app.\n\nCreate, update, view, or delete recurring automations"
    ));
    assert!(
        text.contains("\n1. mode \"view\": requires mode, id.\n"),
        "{text}"
    );
    assert!(
        text.ends_with("6. mode \"delete\": requires mode, id."),
        "{text}"
    );
}

#[test]
fn openai_and_mistral_get_automation_update_as_codex_sent_it() {
    let body = codex_desktop();
    let req = decode(R, &body);
    let (openai, adapted) = sent(
        R,
        &req,
        EgressCtx::new(&req).with_responses_profile(ResponsesProfile::OPENAI),
    );
    assert!(adapted.is_empty());
    assert_eq!(automation_update(&openai), automation_update(&body));
    let mistral = EgressCtx::new(&req)
        .with_provider("mistral")
        .with_chat_profile(ChatProfile::compatible("Mistral").system_role());
    let (chat, adapted) = sent(C, &req, mistral);
    assert!(adapted.is_empty());
    assert_eq!(
        schema(C, tool(C, &chat, AUTOMATION_UPDATE)),
        &automation_update(&body)
    );
}

#[test]
fn a_tool_call_to_the_merged_tool_reaches_the_app_with_the_models_arguments() {
    let app = Arc::new(decode(R, &codex_desktop()));
    let mut ctx = anthropic(&app);
    send(A, &app, &mut ctx).unwrap();
    let mut reply: Value =
        serde_json::from_slice(&fixture(A, "responses/tools-single.json")).unwrap();
    reply["content"][1]["name"] = json!(AUTOMATION_UPDATE);
    reply["content"][1]["input"] = json!({"mode": "view", "id": "automation-1"});
    let reply = dispatch::decode_response(
        A,
        200,
        &HeaderList::new(),
        &serde_json::to_vec(&reply).unwrap(),
        &ctx,
    )
    .unwrap();
    let out =
        dispatch::encode_response(R, &reply, &EncodeCtx::for_request(Arc::clone(&app))).unwrap();
    let out: Value = serde_json::from_slice(&out).unwrap();
    let call = out["output"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["type"] == "function_call")
        .unwrap();
    assert_eq!(call["namespace"], "mcp__codex_app");
    assert_eq!(call["name"], "automation_update");
    let arguments: Value = serde_json::from_str(call["arguments"].as_str().unwrap()).unwrap();
    assert_eq!(arguments, json!({"mode": "view", "id": "automation-1"}));
}

fn chat_tool(name: &str, parameters: Value) -> Value {
    json!({
        "model": "m",
        "messages": [{"role": "user", "content": "Draw a shape."}],
        "tools": [{"type": "function", "function": {
            "name": name,
            "description": "Draws a shape.",
            "parameters": parameters,
        }}],
    })
}

fn zod_discriminated_union() -> Value {
    json!({
        "type": "object",
        "anyOf": [
            {
                "type": "object",
                "properties": {
                    "kind": {"type": "string", "const": "circle"},
                    "radius": {"type": "number"}
                },
                "required": ["kind", "radius"],
                "additionalProperties": false
            },
            {
                "type": "object",
                "properties": {
                    "kind": {"type": "string", "const": "rectangle"},
                    "width": {"type": "number"},
                    "height": {"type": "number"}
                },
                "required": ["kind", "width", "height"],
                "additionalProperties": false
            }
        ],
        "$schema": "http://json-schema.org/draft-07/schema#"
    })
}

#[test]
fn a_zod_discriminated_union_from_chat_completions_reaches_claude_merged() {
    let req = decode(C, &chat_tool("draw", zod_discriminated_union()));
    let (sent, adapted) = sent(A, &req, anthropic(&req));
    assert_eq!(adapted, ["tools[0]"]);
    let tool = tool(A, &sent, "draw");
    assert_eq!(
        tool["input_schema"],
        json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "height": {"type": "number"},
                "kind": {"type": "string", "enum": ["circle", "rectangle"]},
                "radius": {"type": "number"},
                "width": {"type": "number"}
            },
            "required": ["kind"],
            "additionalProperties": false
        })
    );
    assert_eq!(
        tool["description"],
        "Draws a shape.\n\nThe arguments match one of these variants:\n\
         1. kind \"circle\": requires kind, radius.\n\
         2. kind \"rectangle\": requires kind, width, height."
    );
}

#[test]
fn grok_gets_a_union_of_inline_objects_as_sent() {
    let req = decode(C, &chat_tool("draw", zod_discriminated_union()));
    for target in [R, C] {
        let (sent, adapted) = sent(target, &req, xai(&req));
        assert!(adapted.is_empty(), "{target}: {adapted:?}");
        assert_eq!(
            schema(target, tool(target, &sent, "draw")),
            &zod_discriminated_union(),
            "{target}"
        );
    }
}

#[test]
fn a_root_all_of_from_an_anthropic_app_reaches_grok_merged() {
    let body = json!({
        "model": "m",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Rename the file."}],
        "tools": [{
            "name": "rename",
            "description": "Renames a file.",
            "input_schema": {
                "allOf": [
                    {"$ref": "#/definitions/Path"},
                    {
                        "type": "object",
                        "properties": {"to": {"type": "string"}},
                        "required": ["to"]
                    }
                ],
                "definitions": {
                    "Path": {
                        "type": "object",
                        "properties": {"path": {"type": "string"}},
                        "required": ["path"],
                        "additionalProperties": false
                    }
                }
            }
        }]
    });
    let req = decode(A, &body);
    for target in [R, C] {
        let (sent, adapted) = sent(target, &req, xai(&req));
        assert_eq!(adapted, ["tools[0]"], "{target}");
        let tool = tool(target, &sent, "rename");
        assert_eq!(
            schema(target, tool),
            &json!({
                "type": "object",
                "properties": {"path": {"type": "string"}, "to": {"type": "string"}},
                "required": ["path", "to"],
                "additionalProperties": false,
                "definitions": body["tools"][0]["input_schema"]["definitions"]
            }),
            "{target}"
        );
        assert_eq!(description(target, tool), "Renames a file.", "{target}");
    }
}

#[test]
fn shared_parts_and_differing_properties_merge_into_one_object() {
    let mut body = chat_tool(
        "set",
        json!({
            "allOf": [
                {"type": "object", "properties": {"id": {"type": "string"}}, "additionalProperties": true},
                {"properties": {"id": {"minLength": 1}}, "additionalProperties": false}
            ],
            "oneOf": [
                {"type": ["object"], "properties": {"value": {"type": "string"}}},
                {"type": "object", "properties": {"value": {"type": "integer"}}},
                true
            ]
        }),
    );
    body["tools"][0]["function"]
        .as_object_mut()
        .unwrap()
        .remove("description");
    let req = decode(C, &body);
    let (sent, adapted) = sent(A, &req, anthropic(&req));
    assert_eq!(adapted, ["tools[0]"]);
    let tool = tool(A, &sent, "set");
    assert_eq!(
        tool["input_schema"],
        json!({
            "type": "object",
            "properties": {
                "id": {"allOf": [{"type": "string"}, {"minLength": 1}]},
                "value": {"anyOf": [{"type": "string"}, {"type": "integer"}]}
            },
            "additionalProperties": false
        })
    );
    assert_eq!(
        tool["description"],
        "The arguments match one of these variants:\n\
         1. requires nothing; also takes id, value.\n\
         2. requires nothing; also takes id, value.\n\
         3. requires nothing; also takes id."
    );
}

#[test]
fn nested_unions_stay_as_the_app_sent_them() {
    let parameters = json!({
        "type": "object",
        "properties": {
            "target": {
                "oneOf": [
                    {"type": "string"},
                    {"type": "object", "properties": {"id": {"type": "integer"}}}
                ]
            }
        },
        "required": ["target"]
    });
    let req = decode(C, &chat_tool("open", parameters.clone()));
    for (target, ctx) in [(A, anthropic(&req)), (R, xai(&req))] {
        let (sent, adapted) = sent(target, &req, ctx);
        assert!(adapted.is_empty(), "{target}");
        let tool = tool(target, &sent, "open");
        assert_eq!(schema(target, tool), &parameters, "{target}");
        assert_eq!(description(target, tool), "Draws a shape.", "{target}");
    }
}

fn refusal(target: DialectId, parameters: Value) -> KppError {
    let req = decode(C, &chat_tool("pick", parameters));
    let mut ctx = match target {
        A => anthropic(&req),
        _ => xai(&req),
    };
    send(target, &req, &mut ctx).unwrap_err()
}

#[test]
fn a_root_union_with_a_branch_that_is_not_an_object_is_refused_naming_the_tool() {
    let parameters = json!({
        "anyOf": [
            {"type": "string"},
            {"type": "object", "properties": {"name": {"type": "string"}}}
        ]
    });
    for (target, provider) in [(A, "Anthropic"), (R, "SpaceX")] {
        let err = refusal(target, parameters.clone());
        assert!(
            matches!(err.kind, ErrorKind::Unsupported { .. }),
            "{target}"
        );
        assert_eq!(err.param.as_deref(), Some("tools[0]"), "{target}");
        assert_eq!(
            err.message,
            format!(
                "The parameters of `pick` are a union with a branch that is not an object, and {provider} takes only object schemas there. Make every branch an object."
            )
        );
    }
}

#[test]
fn a_branch_kaeso_cannot_resolve_is_refused() {
    let err = refusal(
        A,
        json!({"oneOf": [{"$ref": "https://example.com/shape.json"}]}),
    );
    assert_eq!(err.param.as_deref(), Some("tools[0]"));
    assert!(
        err.message
            .contains("refers to `https://example.com/shape.json`"),
        "{}",
        err.message
    );
}

#[test]
fn a_root_union_whose_branches_refer_to_themselves_is_refused() {
    let err = refusal(
        A,
        json!({
            "oneOf": [{"$ref": "#/$defs/Loop"}],
            "$defs": {"Loop": {"anyOf": [{"$ref": "#/$defs/Loop"}]}}
        }),
    );
    assert_eq!(err.param.as_deref(), Some("tools[0]"));
    assert!(
        err.message.contains("refer back to themselves"),
        "{}",
        err.message
    );
}

#[test]
fn a_root_union_that_combines_into_too_many_variants_is_refused() {
    let branches: Vec<Value> = (0..9)
        .map(|i| json!({"type": "object", "properties": {format!("p{i}"): {"type": "string"}}}))
        .collect();
    let err = refusal(A, json!({"oneOf": branches.clone(), "anyOf": branches}));
    assert!(
        err.message.contains("more than 64 variants"),
        "{}",
        err.message
    );
}

#[test]
fn token_counts_for_claude_see_the_merged_tool() {
    let req = match decode(R, &codex_desktop()) {
        Request::Generate(g) => Request::CountTokens(g),
        other => other,
    };
    let mut ctx = anthropic(&req);
    let sent = send(A, &req, &mut ctx).unwrap();
    assert_eq!(ctx.adapted, ["tools[0].tools[0]"]);
    let input = &tool_of(&sent, AUTOMATION_UPDATE)["input_schema"];
    assert!(input.get("oneOf").is_none(), "{input}");
}
