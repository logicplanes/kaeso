use std::sync::{Arc, LazyLock, Mutex};

use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::Value;
use sha2::{Digest, Sha256};

const KEY_ID: &str = "kaeso-tests";

static KEY: LazyLock<SigningKey> = LazyLock::new(|| SigningKey::from_bytes(&[42; 32]));

pub static PUBLIC_KEYS: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{KEY_ID}:{}",
        STANDARD.encode(KEY.verifying_key().to_bytes())
    )
});

pub fn sign(body: &[u8]) -> String {
    format!(
        "keyid={KEY_ID}, ed25519={}",
        STANDARD.encode(KEY.sign(body).to_bytes())
    )
}

pub fn built_in() -> Value {
    serde_json::from_str(kaeso_core::kpmr::EMBEDDED).unwrap()
}

pub fn model<'a>(doc: &'a mut Value, provider: &str, model: &str) -> &'a mut Value {
    &mut doc["providers"][provider]["models"][model]
}

pub struct Served {
    pub status: u16,
    pub body: String,
    pub signed: bool,
    pub not_modified: usize,
}

impl Served {
    pub fn new(body: String) -> Arc<Mutex<Served>> {
        Arc::new(Mutex::new(Served {
            status: 200,
            body,
            signed: true,
            not_modified: 0,
        }))
    }
}

pub async fn serve(served: Arc<Mutex<Served>>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = axum::Router::new().route(
        "/v1/catalogue",
        axum::routing::get(move |headers: HeaderMap| {
            let served = served.clone();
            async move {
                let mut s = served.lock().unwrap();
                let etag = format!("\"{}\"", STANDARD.encode(Sha256::digest(s.body.as_bytes())));
                let asked = headers
                    .get(header::IF_NONE_MATCH)
                    .and_then(|v| v.to_str().ok());
                if s.status == 200 && asked == Some(etag.as_str()) {
                    s.not_modified += 1;
                    return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag)]).into_response();
                }
                let mut reply = (
                    StatusCode::from_u16(s.status).unwrap(),
                    [(header::ETAG, etag)],
                    s.body.clone(),
                )
                    .into_response();
                if s.signed {
                    reply
                        .headers_mut()
                        .insert("kpmr-signature", sign(s.body.as_bytes()).parse().unwrap());
                }
                reply
            }
        }),
    );
    tokio::spawn(async move { axum::serve(listener, router).await });
    format!("http://{addr}/v1/catalogue")
}
