use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use serde_json::json;

use super::{App, Resp};

const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
const REDIRECT: &str = "http://127.0.0.1:33418/callback";

pub struct Client {
    id: String,
    path: String,
    access: String,
    refresh: String,
}

fn form(pairs: &[(&str, &str)]) -> String {
    let mut s = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        s.append_pair(k, v);
    }
    s.finish()
}

async fn form_post(app: &App, path: &str, body: String) -> Resp {
    app.raw(
        Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .unwrap(),
    )
    .await
}

async fn register(app: &App) -> String {
    let r = app
        .raw(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "client_name": "Release helper", "redirect_uris": [REDIRECT], "token_endpoint_auth_method": "none" })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect_status(StatusCode::CREATED);
    r.body["client_id"].as_str().unwrap().to_owned()
}

async fn request(app: &App, client: &str, resource: &str) -> String {
    let q = form(&[
        ("response_type", "code"),
        ("client_id", client),
        ("redirect_uri", REDIRECT),
        ("state", "st4te"),
        ("code_challenge", CHALLENGE),
        ("code_challenge_method", "S256"),
        ("resource", resource),
    ]);
    let r = app
        .raw(
            Request::builder()
                .uri(format!("/oauth/authorize?{q}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{:?}", r.body);
    let to = r.headers[header::LOCATION].to_str().unwrap();
    to.split_once("#request=").unwrap().1.to_owned()
}

async fn finish(app: &App, client: String, path: String, resource: &str, approved: Resp) -> Client {
    let approved = approved.expect_status(StatusCode::OK);
    let to = url::Url::parse(approved.body["redirect_to"].as_str().unwrap()).unwrap();
    let code = to
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned();
    let tokens = form_post(
        app,
        "/oauth/token",
        form(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("client_id", &client),
            ("redirect_uri", REDIRECT),
            ("code_verifier", VERIFIER),
            ("resource", resource),
        ]),
    )
    .await
    .expect_status(StatusCode::OK);
    Client {
        id: client,
        path,
        access: tokens.body["access_token"].as_str().unwrap().to_owned(),
        refresh: tokens.body["refresh_token"].as_str().unwrap().to_owned(),
    }
}

pub async fn agent(app: &App, cookie: &str, org: &str) -> Client {
    let resource = format!("{}/mcp", super::ORIGIN);
    let client = register(app).await;
    let request = request(app, &client, &resource).await;
    let approved = app
        .post(
            "/me/agent-requests/approve",
            json!({ "request": request, "name": "Release helper", "grant": {
                "preset": "observer", "customized": false, "instance": [], "account": [],
                "orgs": [{ "org": org, "included": true, "rights": ["org.view"],
                    "projects": { "all": true, "rights": [], "selected": [] } }],
                "future_orgs": null } }),
            Some(cookie),
        )
        .await;
    finish(app, client, "/mcp".into(), &resource, approved).await
}

pub async fn portal(app: &App, cookie: &str, project: &str) -> Client {
    let path = format!("/mcp/{project}");
    let resource = format!("{}{path}", super::ORIGIN);
    let client = register(app).await;
    let request = request(app, &client, &resource).await;
    let approved = app
        .post(
            "/me/agent-requests/approve-portal",
            json!({ "request": request }),
            Some(cookie),
        )
        .await;
    finish(app, client, path, &resource, approved).await
}

impl Client {
    pub async fn ping(&self, app: &App) -> StatusCode {
        app.raw(
            Request::builder()
                .method(Method::POST)
                .uri(&self.path)
                .header(header::AUTHORIZATION, format!("Bearer {}", self.access))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ACCEPT, "application/json, text/event-stream")
                .header("mcp-protocol-version", "2025-11-25")
                .body(Body::from(
                    json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .status
    }

    pub async fn stopped(&self, app: &App) -> bool {
        let call = self.ping(app).await;
        let renewed = form_post(
            app,
            "/oauth/token",
            form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", &self.refresh),
                ("client_id", &self.id),
            ]),
        )
        .await;
        call == StatusCode::UNAUTHORIZED
            && renewed.status == StatusCode::BAD_REQUEST
            && renewed.body["error"] == "invalid_grant"
    }
}
