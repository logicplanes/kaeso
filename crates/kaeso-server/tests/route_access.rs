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
            let public_ok = rest == "me/email/confirm" && access == "public";
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
