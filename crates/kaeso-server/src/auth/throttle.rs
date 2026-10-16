use std::time::Duration;

use crate::state::AppState;

#[derive(Clone, Copy, Debug)]
pub struct Throttle {
    pub name: &'static str,
    pub limit: u32,
    pub window: Duration,
}

pub const SIGN_IN_PER_IP: Throttle = Throttle {
    name: "signin_ip",
    limit: 50,
    window: Duration::from_secs(15 * 60),
};
pub const SIGN_IN_PER_ACCOUNT: Throttle = Throttle {
    name: "signin_account",
    limit: 10,
    window: Duration::from_secs(15 * 60),
};
pub const EMAILS_PER_ADDRESS: Throttle = Throttle {
    name: "email",
    limit: 5,
    window: Duration::from_secs(60 * 60),
};
pub const EMAILS_PER_IP: Throttle = Throttle {
    name: "email_ip",
    limit: 30,
    window: Duration::from_secs(60 * 60),
};
pub const TOKENS_PER_IP: Throttle = Throttle {
    name: "token_ip",
    limit: 20,
    window: Duration::from_secs(15 * 60),
};

pub const METHODS_PER_IP: Throttle = Throttle {
    name: "methods_ip",
    limit: 150,
    window: Duration::from_secs(15 * 60),
};

pub async fn hit(state: &AppState, t: Throttle, subject: &str) -> Result<(), u64> {
    let key = state
        .valkey
        .key(&format!("thr:{}:{}", t.name, subject.to_lowercase()));
    let window = t.window.as_secs().max(1);
    let mut conn = state.valkey.conn();
    let result: Result<(i64, i64), _> = redis::pipe()
        .atomic()
        .cmd("SET")
        .arg(&key)
        .arg(0)
        .arg("EX")
        .arg(window)
        .arg("NX")
        .ignore()
        .cmd("INCR")
        .arg(&key)
        .cmd("TTL")
        .arg(&key)
        .query_async(&mut conn)
        .await;
    match result {
        Ok((count, ttl)) if count > i64::from(t.limit) => {
            Err(u64::try_from(ttl.max(1)).unwrap_or(window))
        }
        Ok(_) => Ok(()),
        Err(e) => {
            metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "throttle")
                .increment(1);
            tracing::warn!(error = %e, throttle = t.name, "Valkey is unreachable; throttle not applied");
            Ok(())
        }
    }
}

pub async fn clear(state: &AppState, t: Throttle, subject: &str) {
    let key = state
        .valkey
        .key(&format!("thr:{}:{}", t.name, subject.to_lowercase()));
    let mut conn = state.valkey.conn();
    let _: Result<i64, _> = redis::cmd("DEL").arg(key).query_async(&mut conn).await;
}
