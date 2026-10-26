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
fn keywords_a_variant_alone_has_are_named_in_its_line() {
    let req = decode(
        C,
        &chat_tool(
            "draw",
            json!({
                "oneOf": [
                    {
                        "type": "object",
                        "properties": {"a": {"type": "string"}},
                        "required": ["a"],
                        "minProperties": 1,
                        "additionalProperties": false
                    },
                    {"type": "object", "properties": {"b": {"type": "string"}}, "maxProperties": 2}
                ]
            }),
        ),
    );
    let (sent, _) = sent(A, &req, anthropic(&req));
    let tool = tool(A, &sent, "draw");
    assert_eq!(
        tool["input_schema"],
        json!({
            "type": "object",
            "properties": {"a": {"type": "string"}, "b": {"type": "string"}}
        })
    );
    assert_eq!(
        tool["description"],
        "Draws a shape.\n\nThe arguments match one of these variants:\n\
         1. requires a; with additionalProperties false, minProperties 1.\n\
         2. requires nothing; also takes b; with maxProperties 2."
    );
}

#[test]
fn enum_values_merge_only_when_their_annotations_agree() {
    let kind = |value: &str, description: &str| json!({"type": "object", "properties": {"kind": {"const": value, "description": description}}});
    let merged_kind = |schema: Value| {
        let req = decode(C, &chat_tool("draw", schema));
        let (sent, _) = sent(A, &req, anthropic(&req));
        tool(A, &sent, "draw")["input_schema"]["properties"]["kind"].clone()
    };
    assert_eq!(
        merged_kind(json!({"oneOf": [kind("circle", "The shape."), kind("square", "The shape.")]})),
        json!({"description": "The shape.", "enum": ["circle", "square"]})
    );
    assert_eq!(
        merged_kind(
            json!({"oneOf": [kind("circle", "Round."), kind("square", "Four equal sides.")]})
        ),
        json!({"anyOf": [
            {"const": "circle", "description": "Round."},
            {"const": "square", "description": "Four equal sides."}
        ]})
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

fn rename_tool(schema: Value) -> Value {
    json!({
        "model": "m",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Rename the file."}],
        "tools": [{"name": "rename", "description": "Renames a file.", "input_schema": schema}]
    })
}

#[test]
fn grok_gets_roots_it_counts_as_objects_as_sent() {
    for parameters in [
        json!({
            "properties": {"path": {"type": "string"}},
            "allOf": [{"type": "object", "properties": {"to": {"type": "string"}}}]
        }),
        json!({
            "type": "object",
            "allOf": [{"type": "object", "properties": {"to": {"type": "string"}}}]
        }),
        json!({
            "oneOf": [
                {"properties": {"path": {"type": "string"}}},
                {"type": "object", "properties": {"to": {"type": "string"}}}
            ]
        }),
    ] {
        let req = decode(A, &rename_tool(parameters.clone()));
        let (sent, adapted) = sent(R, &req, xai(&req));
        assert!(adapted.is_empty(), "{parameters}");
        assert_eq!(schema(R, tool(R, &sent, "rename")), &parameters);
    }
}

#[test]
fn a_root_all_of_from_an_anthropic_app_reaches_grok_as_exactly_what_it_accepts() {
    let definitions = json!({
        "Path": {
            "type": "object",
            "properties": {"path": {"type": "string"}, "mode": {"type": "string"}},
            "required": ["path"]
        }
    });
    let body = rename_tool(json!({
        "allOf": [
            {"$ref": "#/definitions/Path"},
            {
                "type": "object",
                "properties": {"path": {"minLength": 1}, "to": {"type": "string"}},
                "required": ["to"],
                "additionalProperties": false
            }
        ],
        "definitions": definitions
    }));
    let req = decode(A, &body);
    for target in [R, C] {
        let (sent, adapted) = sent(target, &req, xai(&req));
        assert_eq!(adapted, ["tools[0]"], "{target}");
        let tool = tool(target, &sent, "rename");
        assert_eq!(
            schema(target, tool),
            &json!({
                "type": "object",
                "properties": {
                    "path": {"allOf": [{"type": "string"}, {"minLength": 1}]},
                    "to": {"type": "string"}
                },
                "required": ["path", "to"],
                "additionalProperties": false,
                "definitions": definitions
            }),
            "{target}"
        );
        assert_eq!(description(target, tool), "Renames a file.", "{target}");
    }
}

#[test]
fn unevaluated_properties_close_the_object_they_stand_in() {
    let part = json!({
        "allOf": [
            {
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "unevaluatedProperties": false
            },
            {"type": "object", "properties": {"x": {"type": "string"}}}
        ]
    });
    let root = json!({
        "type": "object",
        "oneOf": [
            {"properties": {"path": {"type": "string"}}, "required": ["path"]},
            {"properties": {"id": {"type": "integer"}}, "required": ["id"]}
        ],
        "unevaluatedProperties": false
    });
    for (schema, want) in [
        (
            part,
            json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "additionalProperties": false
            }),
        ),
        (
            root,
            json!({
                "type": "object",
                "properties": {"id": {"type": "integer"}, "path": {"type": "string"}},
                "additionalProperties": false
            }),
        ),
    ] {
        let req = decode(A, &rename_tool(schema));
        let (sent, adapted) = sent(A, &req, anthropic(&req));
        assert_eq!(adapted, ["tools[0]"]);
        assert_eq!(tool(A, &sent, "rename")["input_schema"], want);
    }
}

fn ref_with_siblings(dialect: Option<&str>, sibling: Value) -> Value {
    let mut branch = json!({"$ref": "#/$defs/Move"});
    for (key, value) in sibling.as_object().unwrap() {
        branch[key] = value.clone();
    }
    let mut schema = json!({
        "oneOf": [branch, {"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}],
        "$defs": {"Move": {"type": "object", "properties": {"to": {"type": "string"}}}}
    });
    if let Some(dialect) = dialect {
        schema["$schema"] = json!(dialect);
    }
    schema
}

fn merged_required(schema: Value) -> Result<Value, KppError> {
    let req = decode(C, &chat_tool("move", schema));
    let body = send(A, &req, &mut anthropic(&req))?;
    Ok(tool(A, &body, "move")["description"].clone())
}

#[test]
fn keywords_next_to_a_ref_follow_the_draft_the_schema_names() {
    let required = json!({"required": ["to"]});
    let draft_07 = merged_required(ref_with_siblings(
        Some("http://json-schema.org/draft-07/schema#"),
        required.clone(),
    ))
    .unwrap();
    assert!(
        draft_07
            .as_str()
            .unwrap()
            .ends_with("1. requires nothing; also takes to.\n2. requires id."),
        "{draft_07}"
    );
    let draft_2020 = merged_required(ref_with_siblings(
        Some("https://json-schema.org/draft/2020-12/schema"),
        required.clone(),
    ))
    .unwrap();
    assert!(
        draft_2020
            .as_str()
            .unwrap()
            .ends_with("1. requires to.\n2. requires id."),
        "{draft_2020}"
    );
    let annotated = merged_required(ref_with_siblings(None, json!({"description": "Moves it."})));
    assert!(annotated.is_ok(), "{annotated:?}");

    let err = merged_required(ref_with_siblings(None, required)).unwrap_err();
    assert_eq!(err.param.as_deref(), Some("tools[0]"));
    assert_eq!(
        err.message,
        "The parameters of `move` put keywords next to a `$ref`, which JSON Schema drafts read differently, so Kaeso cannot merge them for Anthropic. Name the draft in `$schema`."
    );
}

#[test]
fn draft_07_schemas_ignore_unevaluated_properties() {
    let req = decode(
        A,
        &rename_tool(json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "allOf": [
                {"type": "object", "properties": {"path": {"type": "string"}}},
                {"type": "object", "properties": {"to": {"type": "string"}}}
            ],
            "unevaluatedProperties": false
        })),
    );
    let (sent, adapted) = sent(A, &req, anthropic(&req));
    assert_eq!(adapted, ["tools[0]"]);
    assert_eq!(
        tool(A, &sent, "rename")["input_schema"],
        json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {"path": {"type": "string"}, "to": {"type": "string"}}
        })
    );
}

#[test]
fn all_of_parts_that_cannot_be_merged_exactly_are_refused() {
    let closed_path = json!({
        "allOf": [
            {
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
                "additionalProperties": false
            },
            {"type": "object", "properties": {"to": {"type": "string"}}, "required": ["to"]}
        ]
    });
    let counts = json!({
        "allOf": [
            {"type": "object", "properties": {"path": {"type": "string"}}, "minProperties": 1},
            {"type": "object", "properties": {"to": {"type": "string"}}, "minProperties": 2}
        ]
    });
    let unevaluated_path = json!({
        "allOf": [
            {
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "unevaluatedProperties": false
            },
            {"type": "object", "properties": {"to": {"type": "string"}}, "required": ["to"]}
        ]
    });
    let patterns_past_closed = json!({
        "allOf": [
            {
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "additionalProperties": false
            },
            {"type": "object", "patternProperties": {"^x-": {"type": "string"}}}
        ]
    });
    let patterned = json!({
        "type": "object",
        "patternProperties": {"^x-": {"type": "string"}},
        "oneOf": [{"$ref": "#/$defs/Path"}],
        "$defs": {"Path": {"properties": {"path": {"type": "string"}}}},
        "unevaluatedProperties": false
    });
    for (schema, keyword) in [
        (closed_path, "additionalProperties"),
        (counts, "minProperties"),
        (unevaluated_path, "additionalProperties"),
        (patterns_past_closed, "additionalProperties"),
        (patterned, "unevaluatedProperties"),
    ] {
        let req = decode(A, &rename_tool(schema));
        let err = send(R, &req, &mut xai(&req)).unwrap_err();
        assert!(
            matches!(err.kind, ErrorKind::Unsupported { .. }),
            "{keyword}"
        );
        assert_eq!(err.param.as_deref(), Some("tools[0]"));
        assert_eq!(
            err.message,
            format!(
                "The parameters of `rename` combine schemas whose `{keyword}` Kaeso cannot merge into one object for SpaceX without changing what the tool accepts. Merge them into one object."
            )
        );
    }
}

#[test]
fn shared_parts_and_differing_properties_merge_into_one_object() {
    let mut body = chat_tool(
        "set",
        json!({
            "allOf": [
                {"type": "object", "properties": {"id": {"type": "string"}}},
                {"properties": {"id": {"minLength": 1}}}
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
            }
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
fn references_the_merge_would_move_are_refused() {
    for (reference, schema) in [
        (
            "#/oneOf/0/$defs/Point",
            json!({"oneOf": [
                {
                    "type": "object",
                    "properties": {"at": {"$ref": "#/oneOf/0/$defs/Point"}},
                    "$defs": {"Point": {"type": "array"}}
                },
                {"type": "object", "properties": {"id": {"type": "string"}}}
            ]}),
        ),
        (
            "#point",
            json!({"oneOf": [
                {
                    "type": "object",
                    "properties": {"at": {"$ref": "#point"}},
                    "$defs": {"Point": {"$anchor": "point", "type": "array"}}
                },
                {"type": "object", "properties": {"id": {"type": "string"}}}
            ]}),
        ),
        (
            "#",
            json!({"oneOf": [
                {"type": "object", "properties": {"child": {"$ref": "#"}}},
                {"type": "object", "properties": {"id": {"type": "string"}}}
            ]}),
        ),
    ] {
        let err = refusal(A, schema);
        assert_eq!(err.param.as_deref(), Some("tools[0]"), "{reference}");
        assert_eq!(
            err.message,
            format!(
                "The parameters of `pick` refer to `{reference}`, which the merge for Anthropic would move or change. Move that definition into the root `$defs`."
            )
        );
    }
}

#[test]
fn a_root_union_without_branches_is_refused() {
    for key in ["oneOf", "anyOf"] {
        let err = refusal(A, json!({"type": "object", key: []}));
        assert_eq!(err.param.as_deref(), Some("tools[0]"), "{key}");
        assert!(
            err.message.contains("a union without branches"),
            "{}",
            err.message
        );
    }
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
