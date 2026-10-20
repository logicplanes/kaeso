#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::collections::{BTreeMap, BTreeSet};

use axum::http::{Method, StatusCode};
use common::{App, Owner};
use kaeso_core::rights::{OrgRight, OrgRights, ProjectRight};
use kaeso_testkit::services::TestServices;
use serde_json::{Value, json};

#[derive(Clone, Debug)]
struct Route {
    method: Method,
    path: String,
    right: String,
}

impl Route {
    fn in_project(&self) -> bool {
        self.path.contains("{project}") && self.right.parse::<ProjectRight>().is_ok()
    }

    fn names_a_project(&self) -> bool {
        self.path.contains("{project}")
    }

    fn at(&self, org: &str, project: &str) -> String {
        let path = self
            .path
            .trim_start_matches("/platform/v1")
            .replace("{org}", org)
            .replace("{project}", project);
        let mut out = String::with_capacity(path.len());
        let mut rest = path.as_str();
        while let Some(start) = rest.find('{') {
            out.push_str(&rest[..start]);
            let end = rest[start..].find('}').unwrap() + start;
            out.push_str("x0");
            rest = &rest[end + 1..];
        }
        out.push_str(rest);
        out
    }
}

impl std::fmt::Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.method, self.path)
    }
}

fn routes() -> Vec<Route> {
    let doc = serde_json::to_value(kaeso_server::platform::openapi()).unwrap();
    let mut out = Vec::new();
    for (path, item) in doc["paths"].as_object().unwrap() {
        for (method, op) in item.as_object().unwrap() {
            let Some(right) = op["x-kaeso-access"]
                .as_str()
                .and_then(|a| a.strip_prefix("right:"))
            else {
                continue;
            };
            out.push(Route {
                method: method.to_uppercase().parse().unwrap(),
                path: path.clone(),
                right: right.to_owned(),
            });
        }
    }
    assert!(out.len() > 100, "only {} routes check a right", out.len());
    out
}

fn body_for(method: &Method) -> Option<Value> {
    (method != Method::GET && method != Method::DELETE).then(|| json!({}))
}

async fn call(app: &App, route: &Route, path: &str, cookie: &str) -> common::Resp {
    app.call(
        route.method.clone(),
        path,
        body_for(&route.method),
        Some(cookie),
    )
    .await
}

async fn call_with_key(app: &App, route: &Route, path: &str, key: &str) -> common::Resp {
    app.with_key(route.method.clone(), path, body_for(&route.method), key)
        .await
}

async fn default_project(app: &App, owner: &Owner) -> String {
    let r = app
        .get(&format!("/orgs/{}/projects", owner.org_id), &owner.cookie)
        .await
        .expect_status(StatusCode::OK);
    r.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["is_default"] == true)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn dependents(catalogue: &Value, scope: &str, right: &str) -> BTreeSet<String> {
    let mut needs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for category in catalogue[scope]["categories"].as_array().unwrap() {
        for r in category["rights"].as_array().unwrap() {
            needs.insert(
                r["id"].as_str().unwrap().to_owned(),
                r["needs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_str().unwrap().to_owned())
                    .collect(),
            );
        }
    }
    let mut out = BTreeSet::from([right.to_owned()]);
    loop {
        let before = out.len();
        for (r, ns) in &needs {
            if ns.iter().any(|n| out.contains(n)) {
                out.insert(r.clone());
            }
        }
        if out.len() == before {
            return out;
        }
    }
}

async fn patch(app: &App, owner: &Owner, path: &str, body: Value) {
    app.call(Method::PATCH, path, Some(body), Some(&owner.cookie))
        .await
        .expect_status(StatusCode::OK);
}

#[tokio::test]
async fn every_org_and_project_route_refuses_a_member_without_its_right() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let (probe, probe_id) = common::join(
        &app,
        &owner,
        json!({ "email": "probe@example.com", "role": "member" }),
        "Probe",
    )
    .await;
    let catalogue = app
        .get(&format!("/orgs/{org}/rights"), &owner.cookie)
        .await
        .expect_status(StatusCode::OK)
        .body;
    let reaching: BTreeSet<&str> = [OrgRight::ProjectsViewAll, OrgRight::ProjectsArchiveAny]
        .into_iter()
        .map(OrgRight::as_str)
        .collect();
    let org_role_rights: Vec<&str> = OrgRights::all()
        .minus(OrgRights::owner_only())
        .names()
        .into_iter()
        .filter(|r| !reaching.contains(r))
        .collect();
    let org_role = app
        .post(
            &format!("/orgs/{org}/roles"),
            json!({ "scope": "org", "name": "Everything in the org", "rights": org_role_rights }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED)
        .body["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let mut by_right: BTreeMap<String, Vec<Route>> = BTreeMap::new();
    for route in routes() {
        by_right.entry(route.right.clone()).or_default().push(route);
    }
    let member = format!("/orgs/{org}/members/{probe_id}");
    let project_member = format!("/orgs/{org}/projects/{project}/members/{probe_id}");
    let mut checked = 0;
    for (right, routes) in &by_right {
        let project_right = right.parse::<ProjectRight>().is_ok();
        if project_right {
            let removed = dependents(&catalogue, "project", right);
            patch(&app, &owner, &member, json!({ "role": org_role })).await;
            patch(
                &app,
                &owner,
                &project_member,
                json!({ "role": "admin", "rights_removed": removed }),
            )
            .await;
        } else {
            let removed = dependents(&catalogue, "org", right);
            let added: Vec<&str> = [OrgRight::OrgDelete, OrgRight::BillingManage]
                .into_iter()
                .map(OrgRight::as_str)
                .filter(|r| !removed.contains(*r))
                .collect();
            patch(
                &app,
                &owner,
                &member,
                json!({ "role": "admin", "rights_added": added, "rights_removed": removed }),
            )
            .await;
        }
        for route in routes {
            let r = call(&app, route, &route.at(&org, &project), &probe).await;
            if right == ProjectRight::ProjectView.as_str() {
                assert_eq!(r.status, StatusCode::NOT_FOUND, "{route}: {:?}", r.body);
                assert_eq!(
                    r.body["error"]["message"], "No such project, or you cannot see it.",
                    "{route}"
                );
            } else {
                assert_eq!(r.status, StatusCode::FORBIDDEN, "{route}: {:?}", r.body);
                assert_eq!(r.body["error"]["right"], right.as_str(), "{route}");
            }
            checked += 1;
        }
    }
    assert!(checked > 100, "only {checked} routes were checked");
    s.cleanup().await;
}

fn hidden(r: &common::Resp, route: &Route, what: &str) {
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{route}: {:?}", r.body);
    assert_eq!(
        r.body["error"]["message"],
        format!("No such {what}, or you cannot see it."),
        "{route}"
    );
}

#[tokio::test]
async fn every_org_and_project_route_hides_other_organizations() {
    let s = TestServices::start().await;
    let app = App::start(&s).await;
    let owner = app.setup().await;
    let org = owner.org_id.clone();
    let project = default_project(&app, &owner).await;
    let (admin, _) = common::join(
        &app,
        &owner,
        json!({ "email": "admin@example.com", "role": "admin" }),
        "Admin",
    )
    .await;
    let other = Owner {
        org_id: app
            .post(
                "/me/orgs",
                json!({ "name": "Elsewhere" }),
                Some(&owner.cookie),
            )
            .await
            .expect_status(StatusCode::CREATED)
            .body["id"]
            .as_str()
            .unwrap()
            .to_owned(),
        cookie: owner.cookie.clone(),
        user_id: owner.user_id.clone(),
    };
    let other_project = default_project(&app, &other).await;
    let org_key = app
        .post(
            &format!("/orgs/{org}/platform-keys"),
            json!({ "name": "automation" }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED)
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let project_key = app
        .post(
            &format!("/orgs/{org}/projects/{project}/keys"),
            json!({ "name": "script", "access": "platform" }),
            Some(&owner.cookie),
        )
        .await
        .expect_status(StatusCode::CREATED)
        .body["secret"]
        .as_str()
        .unwrap()
        .to_owned();

    let mut checked = 0;
    for route in routes() {
        if route.in_project() {
            let elsewhere = route.at(&other.org_id, &other_project);
            let crossed = route.at(&org, &other_project);
            hidden(
                &call(&app, &route, &elsewhere, &admin).await,
                &route,
                "organization",
            );
            hidden(
                &call(&app, &route, &crossed, &admin).await,
                &route,
                "project",
            );
            hidden(
                &call_with_key(&app, &route, &elsewhere, &org_key).await,
                &route,
                "project",
            );
            hidden(
                &call_with_key(&app, &route, &crossed, &org_key).await,
                &route,
                "project",
            );
            hidden(
                &call_with_key(&app, &route, &elsewhere, &project_key).await,
                &route,
                "project",
            );
            hidden(
                &call_with_key(&app, &route, &crossed, &project_key).await,
                &route,
                "project",
            );
        } else {
            let elsewhere = route.at(&other.org_id, &other_project);
            hidden(
                &call(&app, &route, &elsewhere, &admin).await,
                &route,
                "organization",
            );
            hidden(
                &call_with_key(&app, &route, &elsewhere, &org_key).await,
                &route,
                "organization",
            );
            if route.names_a_project() {
                let crossed = route.at(&org, &other_project);
                hidden(
                    &call(&app, &route, &crossed, &admin).await,
                    &route,
                    "project",
                );
                hidden(
                    &call_with_key(&app, &route, &crossed, &org_key).await,
                    &route,
                    "project",
                );
            }
        }
        checked += 1;
    }
    assert!(checked > 100, "only {checked} routes were checked");
    s.cleanup().await;
}
