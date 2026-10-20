use std::time::Duration;

use crate::state::AppState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outage {
    Allow,
    Refuse,
}

#[derive(Clone, Copy, Debug)]
pub struct Throttle {
    pub name: &'static str,
    pub limit: u32,
    pub window: Duration,
    pub on_outage: Outage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Limited { retry_after: u64 },
    Unavailable,
}

pub const SIGN_IN_PER_IP: Throttle = Throttle {
    name: "signin_ip",
    limit: 50,
    window: Duration::from_secs(15 * 60),
    on_outage: Outage::Refuse,
};
pub const FAILURES_PER_ACCOUNT_AND_IP: Throttle = Throttle {
    name: "signin_account_ip",
    limit: 10,
    window: Duration::from_secs(15 * 60),
    on_outage: Outage::Refuse,
};
pub const FAILURES_PER_ACCOUNT: Throttle = Throttle {
    name: "signin_account",
    limit: 100,
    window: Duration::from_secs(15 * 60),
    on_outage: Outage::Refuse,
};
pub const EMAILS_PER_ADDRESS: Throttle = Throttle {
    name: "email",
    limit: 5,
    window: Duration::from_secs(60 * 60),
    on_outage: Outage::Allow,
};
pub const EMAILS_PER_IP: Throttle = Throttle {
    name: "email_ip",
    limit: 30,
    window: Duration::from_secs(60 * 60),
    on_outage: Outage::Allow,
};
pub const TOKENS_PER_IP: Throttle = Throttle {
    name: "token_ip",
    limit: 20,
    window: Duration::from_secs(15 * 60),
    on_outage: Outage::Allow,
};

pub const METHODS_PER_IP: Throttle = Throttle {
    name: "methods_ip",
    limit: 150,
    window: Duration::from_secs(15 * 60),
    on_outage: Outage::Allow,
};

fn key(state: &AppState, t: Throttle, subject: &str) -> String {
    state
        .valkey
        .key(&format!("thr:{}:{}", t.name, subject.to_lowercase()))
}

fn outage(t: Throttle, e: &redis::RedisError) -> Result<(), Refusal> {
    metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "throttle").increment(1);
    match t.on_outage {
        Outage::Allow => {
            tracing::warn!(error = %e, throttle = t.name, "Valkey is unreachable; throttle not applied");
            Ok(())
        }
        Outage::Refuse => {
            tracing::warn!(error = %e, throttle = t.name, "Valkey is unreachable; attempts refused until it is back");
            Err(Refusal::Unavailable)
        }
    }
}

pub async fn hit(state: &AppState, t: Throttle, subject: &str) -> Result<(), Refusal> {
    let key = key(state, t, subject);
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
        Ok((count, ttl)) if count > i64::from(t.limit) => Err(Refusal::Limited {
            retry_after: u64::try_from(ttl.max(1)).unwrap_or(window),
        }),
        Ok(_) => Ok(()),
        Err(e) => outage(t, &e),
    }
}

pub async fn release(state: &AppState, t: Throttle, subject: &str) {
    let key = key(state, t, subject);
    let mut conn = state.valkey.conn();
    let result: Result<i64, _> = redis::Script::new(
        "if redis.call('EXISTS', KEYS[1]) == 1 and tonumber(redis.call('GET', KEYS[1])) > 0 then return redis.call('DECR', KEYS[1]) end return 0",
    )
    .key(&key)
    .invoke_async(&mut conn)
    .await;
    if let Err(e) = result {
        tracing::warn!(error = %e, throttle = t.name, "a counted attempt was not released");
    }
}

pub async fn clear(state: &AppState, t: Throttle, subject: &str) {
    let key = key(state, t, subject);
    let mut conn = state.valkey.conn();
    let _: Result<i64, _> = redis::cmd("DEL").arg(key).query_async(&mut conn).await;
}
