use axum::http::{HeaderMap, HeaderValue, header};
use secrecy::{ExposeSecret, SecretString};

use crate::auth::sessions::ABSOLUTE_LIFETIME;
use crate::state::AppState;

pub const NAME: &str = "__Host-kaeso_session";
pub const DEV_NAME: &str = "kaeso_session";

fn name(state: &AppState) -> &'static str {
    if state.config.is_local_development() {
        DEV_NAME
    } else {
        NAME
    }
}

pub fn session_token(state: &AppState, headers: &HeaderMap) -> Option<String> {
    let wanted = name(state);
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(k, _)| *k == wanted)
        .map(|(_, v)| v.to_owned())
        .filter(|v| !v.is_empty())
}

pub fn set(state: &AppState, token: &SecretString) -> Option<HeaderValue> {
    let secure = if state.config.is_local_development() {
        ""
    } else {
        "; Secure"
    };
    HeaderValue::from_str(&format!(
        "{}={}; HttpOnly{secure}; SameSite=Lax; Path=/; Max-Age={}",
        name(state),
        token.expose_secret(),
        ABSOLUTE_LIFETIME.whole_seconds()
    ))
    .ok()
}

pub fn clear(state: &AppState) -> HeaderValue {
    let secure = if state.config.is_local_development() {
        ""
    } else {
        "; Secure"
    };
    HeaderValue::from_str(&format!(
        "{}=; HttpOnly{secure}; SameSite=Lax; Path=/; Max-Age=0",
        name(state)
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("kaeso_session=; Max-Age=0"))
}
