use std::collections::HashMap;

use kaeso_codecs::continuity::{
    CarriedReasoning, ReasoningCollector, restore_reasoning, tool_calls_missing_reasoning,
};
use kaeso_crypto::Sealed;
use kaeso_kpp::{DialectId, GenerateRequest, StreamEvent};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::cache::hex;
use crate::state::AppState;

const MAX_LOOKUPS: usize = 512;

// Bump the version when the stored `CarriedReasoning` changes shape: entries of an older shape are
// then never looked up and expire with their TTL instead of failing to open.
const ENTRY_VERSION: &str = "v2";

fn valkey_key(state: &AppState, key_id: Uuid, tool_call_id: &str) -> String {
    let id = hex(&Sha256::digest(tool_call_id.as_bytes()));
    state
        .valkey
        .key(&format!("reasoning:{ENTRY_VERSION}:{key_id}:{id}"))
}

fn aad(key_id: Uuid, tool_call_id: &str) -> Vec<u8> {
    format!("reasoning:{ENTRY_VERSION}:{key_id}:{tool_call_id}").into_bytes()
}

fn count(outcome: &'static str, n: usize) {
    if n > 0 {
        metrics::counter!(crate::metrics::REASONING_CONTINUITY, "outcome" => outcome)
            .increment(n as u64);
    }
}

pub fn enabled(state: &AppState) -> bool {
    !state.config.continuity.reasoning_ttl.is_zero()
}

pub fn applies(state: &AppState, ingress: DialectId, egress: DialectId) -> bool {
    enabled(state) && ingress == DialectId::OpenAiChat && egress != DialectId::OpenAiChat
}

pub struct StreamCarry {
    state: AppState,
    key_id: Uuid,
    collector: ReasoningCollector,
}

impl StreamCarry {
    pub fn new(state: AppState, key_id: Uuid) -> StreamCarry {
        StreamCarry {
            state,
            key_id,
            collector: ReasoningCollector::new(),
        }
    }

    pub fn observe(&mut self, event: &StreamEvent) {
        self.collector.observe(event);
    }

    pub async fn keep(mut self) {
        let carried = self.collector.finish();
        keep(&self.state, self.key_id, &carried).await;
    }
}

pub async fn keep(state: &AppState, key_id: Uuid, carried: &[CarriedReasoning]) {
    if carried.is_empty() || !enabled(state) {
        return;
    }
    let ttl = u64::try_from(state.config.continuity.reasoning_ttl.as_millis()).unwrap_or(u64::MAX);
    let mut pipe = redis::pipe();
    let mut kept = 0;
    for c in carried {
        let Ok(plain) = serde_json::to_vec(c) else {
            continue;
        };
        let sealed = match kaeso_crypto::seal(&state.keyring, &plain, &aad(key_id, &c.tool_call_id))
        {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "cannot encrypt reasoning to keep");
                continue;
            }
        };
        let key = valkey_key(state, key_id, &c.tool_call_id);
        pipe.cmd("HSET")
            .arg(&key)
            .arg("kid")
            .arg(sealed.key_id.as_bytes())
            .arg("nonce")
            .arg(sealed.nonce)
            .arg("wrapped")
            .arg(sealed.wrapped_key)
            .arg("ct")
            .arg(sealed.ciphertext)
            .ignore()
            .cmd("PEXPIRE")
            .arg(&key)
            .arg(ttl)
            .ignore();
        kept += 1;
    }
    if kept == 0 {
        return;
    }
    let mut conn = state.valkey.bulk();
    let done: Result<(), _> = pipe.query_async(&mut conn).await;
    match done {
        Ok(()) => count("kept", kept),
        Err(e) => {
            metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "reasoning_keep")
                .increment(1);
            count("unavailable", kept);
            tracing::warn!(error = %e, "cannot keep reasoning for the next turn; it continues without it");
        }
    }
}

pub async fn restore(state: &AppState, key_id: Uuid, req: &mut GenerateRequest) -> usize {
    if !enabled(state) {
        return 0;
    }
    let mut ids = tool_calls_missing_reasoning(req);
    if ids.is_empty() {
        return 0;
    }
    if ids.len() > MAX_LOOKUPS {
        ids.drain(..ids.len() - MAX_LOOKUPS);
    }
    let mut pipe = redis::pipe();
    for id in &ids {
        pipe.cmd("HGETALL").arg(valkey_key(state, key_id, id));
    }
    let mut conn = state.valkey.bulk();
    let found: Vec<HashMap<String, Vec<u8>>> = match pipe.query_async(&mut conn).await {
        Ok(f) => f,
        Err(e) => {
            metrics::counter!(crate::metrics::VALKEY_FAILURES, "operation" => "reasoning_restore")
                .increment(1);
            count("unavailable", ids.len());
            tracing::warn!(error = %e, "cannot look up kept reasoning; the request goes on without it");
            return 0;
        }
    };
    let mut kept = Vec::new();
    for (id, fields) in ids.iter().zip(found) {
        if fields.is_empty() {
            continue;
        }
        match open(state, key_id, id, fields) {
            Some(carried) => kept.push(carried),
            None => tracing::warn!("kept reasoning cannot be read; the request goes on without it"),
        }
    }
    count("missing", ids.len() - kept.len());
    let restored = restore_reasoning(req, &kept);
    count("restored", kept.len());
    restored
}

fn open(
    state: &AppState,
    key_id: Uuid,
    tool_call_id: &str,
    mut fields: HashMap<String, Vec<u8>>,
) -> Option<CarriedReasoning> {
    let sealed = Sealed {
        key_id: String::from_utf8(fields.remove("kid")?).ok()?,
        nonce: fields.remove("nonce")?,
        wrapped_key: fields.remove("wrapped")?,
        ciphertext: fields.remove("ct")?,
    };
    let plain = kaeso_crypto::open(&state.keyring, &sealed, &aad(key_id, tool_call_id)).ok()?;
    serde_json::from_slice(secrecy::ExposeSecret::expose_secret(&plain)).ok()
}
