use kaeso_kpp::{RawJson, Request, RequestKind};
use serde_json::{Value, json};

use super::schema::to_json_schema;
use crate::anthropic_messages::AnthropicMessages;
use crate::codec::{Codec, EgressCtx, HeaderList};
use crate::openai_chat::OpenAiChat;
use crate::openai_responses::OpenAiResponses;
use crate::test_support::fixtures;

fn translated(schema: &str) -> String {
    to_json_schema(&RawJson::from_string(schema.to_owned()).unwrap())
        .get()
        .to_owned()
}

fn value(schema: &str) -> Value {
    serde_json::from_str(&translated(schema)).unwrap()
}

#[test]
fn gemini_types_become_lower_case_json_schema_types_at_every_depth() {
    let schema = r#"{"type":"OBJECT","properties":{"city":{"type":"STRING","description":"City"},"tags":{"type":"ARRAY","items":{"type":"string"}},"choice":{"anyOf":[{"type":"INTEGER"},{"type":"BOOLEAN"}]},"extra":{"type":"OBJECT","additionalProperties":{"type":"NUMBER"}},"none":{"type":"NULL"}},"required":["city"]}"#;
    assert_eq!(
        translated(schema),
        r#"{"type":"object","properties":{"city":{"type":"string","description":"City"},"tags":{"type":"array","items":{"type":"string"}},"choice":{"anyOf":[{"type":"integer"},{"type":"boolean"}]},"extra":{"type":"object","additionalProperties":{"type":"number"}},"none":{"type":"null"}},"required":["city"]}"#
    );
}

#[test]
fn a_schema_that_is_already_json_schema_comes_back_as_written() {
    let schema = r#"{ "type": "object", "properties": { "b": {"type": "string"}, "a": {"type": ["integer", "null"], "format": "int32"} }, "x-own": 1 }"#;
    assert_eq!(translated(schema), schema);
    assert_eq!(translated("true"), "true");
    assert_eq!(translated(r#"{"type": "TYPE_UNSPECIFIED"}"#), "{}");
}

#[test]
fn property_names_that_look_like_keywords_are_not_translated() {
    let v = value(
        r#"{"type":"OBJECT","properties":{"type":{"type":"STRING"},"nullable":{"type":"BOOLEAN"},"max_items":{"type":"INTEGER"},"example":{"type":"STRING"}}}"#,
    );
    assert_eq!(
        v["properties"],
        json!({
            "type": {"type": "string"},
            "nullable": {"type": "boolean"},
            "max_items": {"type": "integer"},
            "example": {"type": "string"}
        })
    );
}

#[test]
fn nullable_lets_the_value_be_null() {
    assert_eq!(
        value(r#"{"type":"INTEGER","nullable":true,"minimum":1}"#),
        json!({"type": ["integer", "null"], "minimum": 1})
    );
    assert_eq!(
        value(r#"{"type":"STRING","enum":["c","f"],"format":"enum","nullable":true}"#),
        json!({"enum": ["c", "f", null]}),
        "Anthropic refuses an enum whose values don't match every type of a type list"
    );
    assert_eq!(
        value(r#"{"anyOf":[{"type":"STRING"},{"type":"INTEGER"}],"nullable":true}"#),
        json!({"anyOf": [{"type": "string"}, {"type": "integer"}, {"type": "null"}]})
    );
    assert_eq!(
        value(r#"{"type":"STRING","nullable":false}"#),
        json!({"type": "string"})
    );
    assert_eq!(
        value(r#"{"type":"NULL","nullable":true}"#),
        json!({"type": "null"})
    );
    assert_eq!(
        value(r#"{"description":"anything","nullable":true}"#),
        json!({"description": "anything"})
    );
}

#[test]
fn property_ordering_orders_the_properties_and_is_left_out() {
    let schema = r#"{"properties":{"unit":{"type":"STRING"},"city":{"type":"STRING"},"days":{"type":"INTEGER"},"note":{"type":"STRING"}},"property_ordering":["city","days","unit","missing"],"type":"OBJECT"}"#;
    assert_eq!(
        translated(schema),
        r#"{"properties":{"city":{"type":"string"},"days":{"type":"integer"},"unit":{"type":"string"},"note":{"type":"string"}},"type":"object"}"#
    );
    assert_eq!(
        translated(
            r#"{"type":"object","propertyOrdering":["b","a"],"properties":{"a":{},"b":{}}}"#
        ),
        r#"{"type":"object","properties":{"b":{},"a":{}}}"#
    );
}

#[test]
fn counts_formats_examples_and_snake_case_keys_take_json_schemas_spelling() {
    assert_eq!(
        value(
            r#"{"type":"ARRAY","items":{"type":"STRING","format":"date-time","example":"2026-11-27T00:00:00Z"},"min_items":"1","maxItems":"3","any_of":[{"type":"STRING","max_length":"8","minLength":2}]}"#
        ),
        json!({
            "type": "array",
            "items": {"type": "string", "format": "date-time", "examples": ["2026-11-27T00:00:00Z"]},
            "minItems": 1,
            "maxItems": 3,
            "anyOf": [{"type": "string", "maxLength": 8, "minLength": 2}]
        })
    );
    assert_eq!(
        value(r#"{"type":"OBJECT","min_properties":1,"max_properties":"2","format":"enum"}"#),
        json!({"type": "object", "minProperties": 1, "maxProperties": 2})
    );
}

#[test]
fn integer_and_number_enums_written_as_strings_become_numbers() {
    assert_eq!(
        value(r#"{"type":"INTEGER","format":"enum","enum":["101","201","301"]}"#),
        json!({"type": "integer", "enum": [101, 201, 301]})
    );
    assert_eq!(
        value(r#"{"type":"NUMBER","enum":["1.5",2]}"#),
        json!({"type": "number", "enum": [1.5, 2]})
    );
    assert_eq!(
        value(r#"{"type":"INTEGER","enum":["101","many"]}"#),
        json!({"type": "integer", "enum": ["101", "many"]})
    );
    assert_eq!(
        value(r#"{"type":"STRING","enum":["101"]}"#),
        json!({"type": "string", "enum": ["101"]})
    );
    assert_eq!(
        value(r#"{"type":"INTEGER","enum":["1","2"],"nullable":true}"#),
        json!({"enum": [1, 2, null]})
    );
}

fn decoded(name: &str) -> Request {
    let body = fixtures("gemini", "requests", "json")
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no fixture {name}"))
        .1;
    crate::dispatch::decode_gemini_request(
        "m",
        super::GeminiAction::GenerateContent,
        &body,
        &HeaderList::new(),
    )
    .unwrap()
}

fn sent<C: Codec>(req: &Request) -> Value {
    let mut ctx = EgressCtx::new(req);
    C::check(req, &ctx).unwrap();
    let out = C::encode_request(req, &mut ctx).unwrap();
    serde_json::from_slice(&out.body).unwrap()
}

#[test]
fn google_genais_function_schema_reaches_every_format_as_json_schema() {
    let req = decoded("sdk-parameters-schema");
    let expected = json!({
        "properties": {
            "city": {"description": "City", "type": "string"},
            "days": {"maximum": 7.0, "minimum": 1.0, "type": ["integer", "null"]},
            "unit": {"enum": ["c", "f"], "type": "string"}
        },
        "required": ["city"],
        "type": "object"
    });
    let chat = sent::<OpenAiChat>(&req);
    assert_eq!(chat["tools"][0]["function"]["parameters"], expected);
    let order: Vec<&str> = chat["tools"][0]["function"]["parameters"]["properties"]
        .as_object()
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap();
    assert_eq!(order, ["city", "days", "unit"]);
    assert_eq!(
        sent::<AnthropicMessages>(&req)["tools"][0]["input_schema"],
        expected
    );
    assert_eq!(
        sent::<OpenAiResponses>(&req)["tools"][0]["parameters"],
        expected
    );
}

#[test]
fn a_response_schema_reaches_every_format_as_json_schema_and_gemini_as_written() {
    let req = decoded("sdk-response-schema-pydantic");
    let schema = &sent::<AnthropicMessages>(&req)["output_config"]["format"]["schema"];
    assert_eq!(schema["type"], "object");
    assert_eq!(
        schema["properties"]["population"]["type"],
        json!(["integer", "null"])
    );
    assert_eq!(
        schema["properties"]["landmarks"]["items"],
        json!({"type": "string"})
    );
    assert!(schema.get("property_ordering").is_none(), "{schema}");
    let chat = sent::<OpenAiChat>(&req);
    assert_eq!(
        &chat["response_format"]["json_schema"]["schema"], schema,
        "{chat}"
    );
    let gemini = sent::<super::Gemini>(&req);
    assert_eq!(
        gemini["generationConfig"]["responseSchema"]["type"],
        "OBJECT"
    );
    assert_eq!(
        gemini["generationConfig"]["responseSchema"]["property_ordering"][0],
        "name"
    );
}

#[test]
fn a_gemini_apps_json_schemas_and_other_formats_schemas_are_not_touched() {
    let req = decoded("structured-json-schema");
    let Request::Generate(g) = &req else {
        panic!("not a generation");
    };
    let kaeso_kpp::OutputSpec::JsonSchema(spec) = &g.output else {
        panic!("no schema");
    };
    assert_eq!(
        sent::<OpenAiChat>(&req)["response_format"]["json_schema"]["schema"],
        spec.schema.to_value().unwrap()
    );
    let req = decoded("tools-parallel");
    let Request::Generate(g) = &req else {
        panic!("not a generation");
    };
    let Some(kaeso_kpp::ToolDef::Function(f)) = g.tools.first() else {
        panic!("no tool");
    };
    assert_eq!(
        sent::<OpenAiChat>(&req)["tools"][0]["function"]["parameters"],
        f.parameters.as_ref().unwrap().to_value().unwrap()
    );
    let chat = OpenAiChat::decode_request(
        RequestKind::Generate,
        br#"{"model":"m","max_completion_tokens":5,"messages":[{"role":"user","content":"Hi"}],"tools":[{"type":"function","function":{"name":"f","parameters":{"type":"OBJECT"}}}]}"#,
        &HeaderList::new(),
    )
    .unwrap();
    assert_eq!(
        sent::<AnthropicMessages>(&chat)["tools"][0]["input_schema"],
        json!({"type": "OBJECT"})
    );
}
