#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::collections::BTreeSet;

use kaeso_core::rights::{OrgRight, ProjectRight};

const CHECKED_IN_HANDLERS: &[(&str, &str)] = &[
    (
        "costs.org.view",
        "orgs/{org}/usage shows costs only with it",
    ),
    (
        "costs.view",
        "projects/{project}/usage shows costs only with it",
    ),
    (
        "logs.content.view",
        "logs/{log} and logs/export include content only with it",
    ),
    (
        "keys.platform_access.grant",
        "key creation checks it for platform access",
    ),
];

struct Row {
    level: String,
    area: String,
    resources: Vec<String>,
    rights: Vec<String>,
    cloud: bool,
}

fn fixture() -> Vec<Row> {
    include_str!("fixtures/platform-areas.txt")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let cols: Vec<&str> = l.split('|').map(str::trim).collect();
            let list = |s: &str| {
                s.split(',')
                    .map(str::trim)
                    .filter(|x| !x.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            };
            Row {
                level: cols[0].to_owned(),
                area: cols[1].to_owned(),
                resources: list(cols[2]),
                rights: list(cols.get(3).copied().unwrap_or("")),
                cloud: cols.get(4).is_some_and(|n| *n == "cloud"),
            }
        })
        .collect()
}

fn routes() -> Vec<(String, String)> {
    let doc = serde_json::to_value(kaeso_server::platform::openapi()).unwrap();
    let mut out = Vec::new();
    for (path, item) in doc["paths"].as_object().unwrap() {
        for (_, op) in item.as_object().unwrap() {
            if let Some(access) = op["x-kaeso-access"].as_str() {
                out.push((
                    path.trim_start_matches("/platform/v1/").to_owned(),
                    access.to_owned(),
                ));
            }
        }
    }
    out
}

#[test]
fn the_fixture_follows_the_spec() {
    let rows = fixture();
    let levels: BTreeSet<&str> = rows.iter().map(|r| r.level.as_str()).collect();
    assert_eq!(
        levels,
        ["auth", "instance", "me", "org", "project"]
            .into_iter()
            .collect()
    );
    assert_eq!(rows.len(), 36, "{} rows", rows.len());
    for r in &rows {
        for right in &r.rights {
            let name = right.trim_end_matches(".*");
            let known = if right.ends_with(".*") {
                OrgRight::ALL.iter().any(|x| x.as_str().starts_with(name))
                    || ProjectRight::ALL
                        .iter()
                        .any(|x| x.as_str().starts_with(name))
            } else {
                name.parse::<OrgRight>().is_ok() || name.parse::<ProjectRight>().is_ok()
            };
            assert!(
                known,
                "{}: `{right}` is not in the rights catalogue",
                r.area
            );
        }
    }
}

#[test]
fn every_area_of_the_spec_is_served() {
    let routes = routes();
    let mut missing = Vec::new();
    for row in fixture().iter().filter(|r| !r.cloud) {
        for res in &row.resources {
            let found = routes
                .iter()
                .any(|(p, _)| p == res || p.starts_with(&format!("{res}/")));
            if !found {
                missing.push(format!(
                    "{} / {}: no route for `{res}`",
                    row.level, row.area
                ));
            }
        }
        let area_rights: Vec<&str> = routes
            .iter()
            .filter(|(p, _)| {
                row.resources
                    .iter()
                    .any(|res| p == res || p.starts_with(&format!("{res}/")))
            })
            .filter_map(|(_, a)| a.strip_prefix("right:"))
            .collect();
        for right in &row.rights {
            let ok = if let Some(prefix) = right.strip_suffix(".*") {
                area_rights
                    .iter()
                    .any(|r| r.starts_with(&format!("{prefix}.")))
            } else {
                area_rights.contains(&right.as_str())
                    || CHECKED_IN_HANDLERS.iter().any(|(r, _)| r == right)
            };
            if !ok {
                missing.push(format!(
                    "{} / {}: no route checks `{right}`",
                    row.level, row.area
                ));
            }
        }
    }
    assert!(missing.is_empty(), "missing:\n{}", missing.join("\n"));
}

#[test]
fn cloud_only_areas_are_not_served_by_a_self_hosted_gateway() {
    let routes = routes();
    for row in fixture().iter().filter(|r| r.cloud) {
        for res in &row.resources {
            assert!(
                !routes.iter().any(|(p, _)| p.starts_with(res.as_str())),
                "{res}"
            );
        }
    }
}
