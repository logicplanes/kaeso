use std::collections::HashMap;

use kaeso_codecs::continuity::{
    CarriedReasoning, ReasoningCollector, Resolved, Unresolved, referenced_tool_calls,
    resolve_kept_references, resolve_message_references, restore_reasoning,
    tool_calls_missing_reasoning,
};
use kaeso_crypto::Sealed;
use kaeso_kpp::{DialectId, GenerateRequest, Request, StreamEvent};
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

/// What of a reply's reasoning Kaeso keeps for the next turn, for the API key that asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carry {
    pub key_id: Uuid,
    /// Every reasoning block and the text before a tool call, for an OpenAI Responses app that
    /// may point to them with an `item_reference`; otherwise only the reasoning a provider must
    /// get back signed, for an app whose format can't carry it.
    pub every: bool,
    /// The text too, where the project allows responses stored at the provider.
    pub text: bool,
}

impl Carry {
    /// What of a whole reply is kept.
    pub fn of_reply(&self, blocks: &[kaeso_kpp::Block]) -> Vec<CarriedReasoning> {
        match (self.every, self.text) {
            (true, true) => kaeso_codecs::continuity::items_for_tool_calls(blocks),
            (true, false) => kaeso_codecs::continuity::every_reasoning_for_tool_calls(blocks),
            (false, _) => kaeso_codecs::continuity::reasoning_for_tool_calls(blocks),
        }
    }
}

/// Whether Kaeso keeps the reasoning of this request's reply: for an OpenAI Chat Completions or
/// Gemini app served in another format, and for an OpenAI Responses app that didn't send
/// `store: false`, which promises to send the reply's items back itself.
pub fn carry(
    state: &AppState,
    key_id: Option<Uuid>,
    (ingress, egress): (DialectId, DialectId),
    req: &Request,
    text: bool,
) -> Option<Carry> {
    let key_id = key_id.filter(|_| enabled(state))?;
    let Request::Generate(g) = req else {
        return None;
    };
    match ingress {
        DialectId::OpenAiChat | DialectId::Gemini if egress != ingress => Some(Carry {
            key_id,
            every: false,
            text: false,
        }),
        DialectId::OpenAiResponses
            if kaeso_codecs::openai_responses::stored_state(g).store != Some(false) =>
        {
            Some(Carry {
                key_id,
                every: true,
                text,
            })
        }
        _ => None,
    }
}

pub struct StreamCarry {
    state: AppState,
    key_id: Uuid,
    text: bool,
    collector: ReasoningCollector,
}

impl StreamCarry {
    pub fn new(state: AppState, carry: Carry) -> StreamCarry {
        StreamCarry {
            state,
            key_id: carry.key_id,
            text: carry.text,
            collector: if carry.every {
                ReasoningCollector::every()
            } else {
                ReasoningCollector::new()
            },
        }
    }

    pub fn observe(&mut self, event: &StreamEvent) {
        self.collector.observe(event);
    }

    pub async fn keep(mut self) {
        let mut carried = self.collector.finish();
        if !self.text {
            carried.retain_mut(|c| {
                c.messages.clear();
                !c.blocks.is_empty()
            });
        }
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
    let ids = tool_calls_missing_reasoning(req);
    if ids.is_empty() {
        return 0;
    }
    let kept = lookup(state, key_id, ids).await;
    let restored = restore_reasoning(req, &kept);
    count("restored", kept.len());
    restored
}

/// Puts back the text before a tool call that an OpenAI Responses app points to with
/// `item_reference` items, from what Kaeso kept of its earlier replies for this key, before the
/// request's guardrails check it. The error is the first reference Kaeso holds nothing for.
pub async fn resolve_messages(
    state: &AppState,
    key_id: Option<Uuid>,
    req: &mut GenerateRequest,
) -> Result<Resolved, Unresolved> {
    let ids = referenced_tool_calls(req);
    let kept = match key_id {
        Some(key_id) if enabled(state) && !ids.is_empty() => lookup(state, key_id, ids).await,
        _ => Vec::new(),
    };
    let resolved = resolve_message_references(req, &kept)?;
    if resolved.messages > 0 {
        count("restored", kept.len());
    }
    Ok(resolved)
}

/// Puts back the reasoning and text an OpenAI Responses app points to with `item_reference`
/// items, from what Kaeso kept of its earlier replies for this key. The error is the first
/// reference Kaeso holds nothing for.
pub async fn resolve_references(
    state: &AppState,
    key_id: Option<Uuid>,
    req: &mut GenerateRequest,
    stateless: bool,
) -> Result<Resolved, Unresolved> {
    let ids = referenced_tool_calls(req);
    let kept = match key_id {
        Some(key_id) if enabled(state) && !ids.is_empty() => lookup(state, key_id, ids).await,
        _ => Vec::new(),
    };
    let resolved = resolve_kept_references(req, &kept, stateless)?;
    if resolved.restored > 0 || resolved.messages > 0 {
        count("restored", kept.len());
    }
    Ok(resolved)
}

async fn lookup(state: &AppState, key_id: Uuid, mut ids: Vec<String>) -> Vec<CarriedReasoning> {
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
            return Vec::new();
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
    kept
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
