#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use kaeso_core::rights::{OrgRight, ProjectRight};

fn operations() -> Vec<(String, String, serde_json::Value)> {
    let doc = serde_json::to_value(kaeso_server::platform::openapi()).unwrap();
    let mut out = Vec::new();
    for (path, item) in doc["paths"].as_object().unwrap() {
        for (method, op) in item.as_object().unwrap() {
            if ["get", "put", "post", "delete", "patch"].contains(&method.as_str()) {
                out.push((method.to_uppercase(), path.clone(), op.clone()));
            }
        }
    }
    out
}

#[test]
fn every_route_declares_exactly_one_access() {
    let ops = operations();
    assert!(ops.len() > 50, "only {} operations", ops.len());
    for (method, path, op) in ops {
        let route = format!("{method} {path}");
        assert!(path.starts_with("/platform/v1/"), "{route}");
        let access = op["x-kaeso-access"]
            .as_str()
            .unwrap_or_else(|| panic!("{route} declares no access"));
        let rest = path.trim_start_matches("/platform/v1/");
        if rest.starts_with("auth/") {
            assert!(
                access == "public" || access == "session",
                "{route}: {access}"
            );
        } else if rest == "me" || rest.starts_with("me/") {
            let public_ok =
                matches!(rest, "me/email/confirm" | "me/email/cancel") && access == "public";
            assert!(access == "session" || public_ok, "{route}: {access}");
        } else if rest.starts_with("instance/") {
            assert_eq!(access, "instance_admin", "{route}");
        } else {
            let right = access
                .strip_prefix("right:")
                .unwrap_or_else(|| panic!("{route} checks no right: {access}"));
            if path.contains("{project}") {
                assert!(
                    right.parse::<ProjectRight>().is_ok(),
                    "{route}: `{right}` is not a project right"
                );
            } else {
                assert!(
                    path.contains("{org}"),
                    "{route}: a right needs an org in the path"
                );
                assert!(
                    right.parse::<OrgRight>().is_ok(),
                    "{route}: `{right}` is not an org right"
                );
            }
            let description = op["description"].as_str().unwrap_or_default();
            assert!(
                description.contains(&format!("`{right}`")),
                "{route}: the right is not documented"
            );
        }
    }
}

#[test]
fn every_operation_documents_its_responses_and_errors() {
    for (method, path, op) in operations() {
        let responses = op["responses"].as_object().unwrap();
        assert!(
            responses
                .keys()
                .any(|k| k.starts_with('2') || k.starts_with('3')),
            "{method} {path} has no success response"
        );
        assert!(
            responses.contains_key("401"),
            "{method} {path} documents no 401"
        );
        assert!(
            op["tags"].as_array().is_some_and(|t| !t.is_empty()),
            "{method} {path} has no tag"
        );
    }
}

#[test]
fn every_conditional_change_documents_its_precondition() {
    let mut conditional = 0;
    for (method, path, op) in operations() {
        let accepts = op["parameters"].as_array().is_some_and(|ps| {
            ps.iter()
                .any(|p| p["in"] == "header" && p["name"] == "If-Match")
        });
        let responses = op["responses"].as_object().unwrap();
        assert_eq!(
            responses.contains_key("412"),
            accepts,
            "{method} {path}: 412 is documented exactly when `If-Match` is accepted"
        );
        if !accepts {
            continue;
        }
        conditional += 1;
        assert_eq!(
            responses["412"]["$ref"], "#/components/responses/Stale",
            "{method} {path}"
        );
        for (status, response) in responses {
            if status.starts_with('2') && response.get("content").is_some() {
                assert!(
                    response["headers"]["ETag"].is_object(),
                    "{method} {path}: {status} returns the new `ETag`"
                );
            }
        }
    }
    assert_eq!(conditional, 4);
}

#[test]
fn the_committed_openapi_document_is_current() {
    let committed =
        std::fs::read_to_string(format!("{}/../../openapi.json", env!("CARGO_MANIFEST_DIR")))
            .unwrap();
    let current = kaeso_server::platform::openapi().to_pretty_json().unwrap();
    assert!(
        committed.trim_end() == current.trim_end(),
        "openapi.json is out of date: run `cargo run -q -p kaeso -- openapi > openapi.json`"
    );
}

#[test]
fn every_schema_field_is_documented() {
    let doc = serde_json::to_value(kaeso_server::platform::openapi()).unwrap();
    let schemas = doc["components"]["schemas"].as_object().unwrap();
    let documented = |v: &serde_json::Value| {
        v.get("description").is_some()
            || ["oneOf", "allOf", "anyOf"].iter().any(|k| {
                v.get(*k)
                    .and_then(|x| x.as_array())
                    .is_some_and(|xs| xs.iter().any(|x| x.get("description").is_some()))
            })
    };
    let mut missing = Vec::new();
    for (name, schema) in schemas {
        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            for (field, v) in props {
                if !documented(v) {
                    missing.push(format!("{name}.{field}"));
                }
            }
        }
    }
    assert!(missing.is_empty(), "undocumented fields: {missing:?}");
}

fn decimal_or_whole(schema: &serde_json::Value, maximum: i64) {
    let variants = schema["oneOf"].as_array().unwrap();
    assert_eq!(variants.len(), 2, "{schema}");
    assert_eq!(variants[0]["type"], "string");
    assert_eq!(variants[0]["pattern"], kaeso_core::decimal::DECIMAL_PATTERN);
    assert_eq!(variants[1]["type"], "integer");
    assert_eq!(variants[1]["minimum"], 0);
    assert_eq!(variants[1]["maximum"], maximum);
}

fn refers_to(property: &serde_json::Value, name: &str) -> bool {
    let target = format!("#/components/schemas/{name}");
    property["$ref"] == target
        || property["oneOf"]
            .as_array()
            .is_some_and(|xs| xs.iter().any(|x| x["$ref"] == target))
}

#[test]
fn request_amounts_and_prices_are_a_decimal_string_or_a_whole_number() {
    let doc = serde_json::to_value(kaeso_server::platform::openapi()).unwrap();
    let schemas = &doc["components"]["schemas"];
    decimal_or_whole(&schemas["LimitAmount"], 1_000_000_000_000);
    decimal_or_whole(&schemas["TokenPrice"], 1_000_000);

    for name in ["LimitCreate", "KeyLimit", "LimitUpdate"] {
        let amount = &schemas[name]["properties"]["amount"];
        assert!(refers_to(amount, "LimitAmount"), "{name}.amount: {amount}");
    }
    for name in ["LimitCreate", "KeyLimit"] {
        let required = schemas[name]["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r == "amount"), "{name}");
        assert!(
            schemas[name]["properties"]["amount"]["$ref"].is_string(),
            "{name}.amount is not nullable"
        );
    }

    for name in ["ModelCreate", "ModelUpdate"] {
        let overrides = &schemas[name]["properties"]["overrides"];
        assert!(refers_to(overrides, "FieldsInput"), "{name}: {overrides}");
    }
    let prices = [
        "input_price",
        "output_price",
        "cache_read_price",
        "cache_write_price",
    ];
    for price in prices {
        let input = &schemas["FieldsInput"]["properties"][price];
        assert!(
            refers_to(input, "TokenPrice"),
            "FieldsInput.{price}: {input}"
        );
        assert!(input["description"].is_string(), "FieldsInput.{price}");
    }
}

#[test]
fn response_amounts_and_prices_stay_decimal_strings() {
    let doc = serde_json::to_value(kaeso_server::platform::openapi()).unwrap();
    let schemas = &doc["components"]["schemas"];
    assert_eq!(
        schemas["LimitView"]["properties"]["amount"]["type"],
        "string"
    );
    let nullable_string = serde_json::json!(["string", "null"]);
    for price in [
        "input_price",
        "output_price",
        "cache_read_price",
        "cache_write_price",
    ] {
        for name in ["Fields", "PlaygroundModel"] {
            assert_eq!(
                schemas[name]["properties"][price]["type"], nullable_string,
                "{name}.{price}"
            );
        }
    }
    for name in ["CatalogueModelView", "Preview"] {
        let properties = schemas[name]["properties"].as_object().unwrap();
        for (field, property) in properties {
            assert!(
                !refers_to(property, "FieldsInput"),
                "{name}.{field} describes a response with a request schema"
            );
        }
    }
}
