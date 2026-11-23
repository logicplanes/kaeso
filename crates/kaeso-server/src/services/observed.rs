//! Requests and tool calls a source reported: Kaeso never made them, so it only logs and counts
//! them, as the gateway does its own, under the source that sent them. Nothing here can refuse a
//! call; a refused record only means Kaeso doesn't keep it.

use std::collections::HashMap;
use std::time::Duration;

use kaeso_core::catalogue::Fields;
use kaeso_core::id::RequestId;
use kaeso_core::sources::{Counted, SourceKind};
use kaeso_kpp::{Block, DialectId, GenerateRequest, Message, Request, Response, Usage};
use kaeso_store::StoreError;
use kaeso_store::clickhouse::tool_calls::ToolCallRow;
use kaeso_store::pg::sources::{Activity, Source};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::model_api::hooks::{Caller, Source as Origin};
use crate::state::AppState;

/// How long Kaeso remembers a record it logged, so an exporter that sends it again isn't counted
/// twice. OpenTelemetry exporters give up on a batch within minutes.
const REMEMBERED: Duration = Duration::from_secs(7 * 24 * 3600);
const VALKEY_DEADLINE: Duration = Duration::from_secs(2);
/// The longest error message a log entry keeps of what a source said went wrong.
const MAX_ERROR: usize = 1_000;
/// The most characters of a tool's or a model's name, as the console shows them.
const MAX_NAME: usize = 200;

/// A request a source reported, its token counts already the way Kaeso counts them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObservedRequest {
    /// What the source calls the record, unique within the source: the provider's request id,
    /// or the record's own id. A record sent again with the same id is logged once.
    pub external_id: String,
    pub started_at: Option<OffsetDateTime>,
    pub duration: Duration,
    pub ttfb: Option<Duration>,
    /// The provider's id as KPMR names it, when the source says or Kaeso can tell.
    pub provider: Option<String>,
    pub model: String,
    /// 200 for an answered request, else the provider's HTTP status or 500.
    pub status: u16,
    pub error_message: Option<String>,
    pub tokens: Counted,
    /// What the source said the request cost, in USD.
    pub reported_cost: Option<Decimal>,
    pub stream: bool,
    /// The client and its version, as a user agent, such as `claude-cli/2.1.322`.
    pub client: Option<String>,
    /// The person the source says made the request.
    pub email: Option<String>,
    pub tags: Vec<String>,
    /// The format the client speaks with its provider, for the log's request view.
    pub dialect: Option<DialectId>,
    /// What was asked and answered, when the source sends it; kept only where content is kept.
    pub prompt: Option<String>,
    pub reply: Option<String>,
}

/// A tool call a source reported.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObservedToolCall {
    pub external_id: String,
    pub started_at: Option<OffsetDateTime>,
    pub duration: Duration,
    pub tool: String,
    /// The MCP server that has the tool; none for the client's built-in tools.
    pub server: Option<String>,
    pub ok: bool,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub client: Option<String>,
    pub email: Option<String>,
    pub arguments: Option<String>,
    pub result: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Batch {
    pub requests: Vec<ObservedRequest>,
    pub tool_calls: Vec<ObservedToolCall>,
    /// Records the reader couldn't use, and why the last one was refused.
    pub refused: u32,
    pub refusal: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Recorded {
    pub requests: usize,
    pub tool_calls: usize,
    pub duplicates: usize,
    pub refused: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    #[error("{0}")]
    Store(#[from] StoreError),
    #[error("valkey: {0}")]
    Valkey(String),
}

/// The record's request id: the same for the same record of the same source, so even a
/// duplicate that got past the dedupe window writes the same id.
pub fn request_id(source: Uuid, external_id: &str) -> Uuid {
    let mut h = Sha256::new();
    h.update(source.as_bytes());
    h.update(external_id.as_bytes());
    let d = h.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(d.get(..16).unwrap_or(&[0u8; 16]));
    uuid::Builder::from_custom_bytes(b).into_uuid()
}

fn cut(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((at, _)) => s.get(..at).unwrap_or(s).to_owned(),
        None => s.to_owned(),
    }
}

// Marks each id as seen and says which were new; Valkey decides atomically across instances.
async fn first_time(
    state: &AppState,
    source: Uuid,
    ids: &[Uuid],
) -> Result<Vec<bool>, RecordError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut pipe = redis::pipe();
    for id in ids {
        pipe.cmd("SET")
            .arg(state.valkey.key(&format!("obs:{source}:{id}")))
            .arg(1)
            .arg("NX")
            .arg("EX")
            .arg(REMEMBERED.as_secs());
    }
    let mut conn = state.valkey.conn();
    let set: Vec<Option<String>> =
        tokio::time::timeout(VALKEY_DEADLINE, pipe.query_async(&mut conn))
            .await
            .map_err(|_| RecordError::Valkey("timed out".into()))?
            .map_err(|e| RecordError::Valkey(e.to_string()))?;
    Ok(set.into_iter().map(|s| s.is_some()).collect())
}

// The person each email address names, when the source matches members and the address belongs
// to a member of the source's organization.
async fn members(
    state: &AppState,
    source: &Source,
    emails: impl Iterator<Item = String>,
) -> Result<HashMap<String, Uuid>, StoreError> {
    let mut found = HashMap::new();
    if !source.match_members {
        return Ok(found);
    }
    let mut seen = std::collections::HashSet::new();
    for email in emails {
        let key = email.to_lowercase();
        if !seen.insert(key.clone()) {
            continue;
        }
        let Some(user) = kaeso_store::pg::users::get_by_email(&state.pg, &email).await? else {
            continue;
        };
        if kaeso_store::pg::members::get_org(&state.pg, source.org_id, user.id)
            .await?
            .is_some()
        {
            found.insert(key, user.id);
        }
    }
    Ok(found)
}

fn caller(source: &Source, members: &HashMap<String, Uuid>, email: Option<&str>) -> Caller {
    let matched = email.and_then(|e| members.get(&e.to_lowercase()).copied());
    let (user, service_account) = match matched {
        Some(u) => (Some(u), None),
        None => (source.user_id, source.service_account_id),
    };
    Caller {
        org_id: source.org_id,
        project_id: source.project_id,
        key_id: None,
        user_id: user,
        service_account_id: if user.is_some() {
            None
        } else {
            service_account
        },
    }
}

/// The model's prices and catalogue entry: the project's own entry first, so its price overrides
/// count, else KPMR's; the provider is found from the model when the source named none.
pub struct Priced {
    pub provider: Option<String>,
    pub catalogue_model_id: Option<Uuid>,
    pub prices: Option<Fields>,
}

pub async fn price(state: &AppState, project: Uuid, provider: Option<&str>, model: &str) -> Priced {
    let wanted = |p: &str, m: &str| provider.is_none_or(|want| want == p) && m == model;
    if let Ok(models) = crate::services::catalogue::project_models(state, project).await
        && let Some(entry) = models
            .catalogue
            .entries()
            .iter()
            .find(|e| wanted(&e.provider, &e.model_id) || wanted(&e.provider, e.called()))
        && let Some(effective) =
            crate::services::catalogue::effective(state, entry, &models.providers)
    {
        return Priced {
            provider: Some(entry.provider.clone()),
            catalogue_model_id: Some(entry.id),
            prices: Some(effective.fields),
        };
    }
    let loaded = state.kpmr.current();
    let snapshot = &loaded.snapshot;
    let found = match provider {
        Some(p) => snapshot.model(p, model).map(|m| (p.to_owned(), m)),
        None => snapshot
            .providers
            .iter()
            .find_map(|(p, listed)| listed.models.get(model).map(|m| (p.clone(), m))),
    };
    match found {
        Some((p, m)) => Priced {
            provider: Some(p),
            catalogue_model_id: None,
            prices: Some(Fields::from_kpmr(m)),
        },
        None => Priced {
            provider: provider.map(str::to_owned),
            catalogue_model_id: None,
            prices: None,
        },
    }
}

fn static_provider(id: &str) -> Option<&'static str> {
    kaeso_core::provider::PROVIDERS
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.id)
}

fn content(r: &ObservedRequest, dialect: DialectId, model: &str) -> Option<(String, String)> {
    let prompt = r.prompt.as_deref().filter(|p| !p.is_empty());
    let reply = r.reply.as_deref().filter(|p| !p.is_empty());
    if prompt.is_none() && reply.is_none() {
        return None;
    }
    let request = Request::Generate(
        GenerateRequest::new(model).with_message(Message::user(prompt.unwrap_or_default())),
    );
    let mut ctx = kaeso_codecs::EgressCtx::new(&request);
    let body = kaeso_codecs::dispatch::encode_request(dialect, &request, &mut ctx)
        .ok()
        .and_then(|e| String::from_utf8(e.body).ok())
        .unwrap_or_default();
    let mut response = Response::new(
        r.external_id.clone(),
        model,
        reply.map(|t| vec![Block::text(t)]).unwrap_or_default(),
    );
    response.usage = usage(r.tokens);
    let reply = serde_json::to_string(&response).unwrap_or_default();
    Some((body, reply))
}

fn usage(t: Counted) -> Usage {
    let mut u = Usage::new(t.input, t.output);
    u.cache_read_tokens = t.cache_read;
    u.cache_write_tokens = t.cache_write;
    u.reasoning_tokens = (t.reasoning > 0).then_some(t.reasoning);
    u
}

fn kind_of(source: &Source) -> Option<SourceKind> {
    source.kind.parse().ok()
}

/// Logs and counts a batch a source sent. Duplicates and refused records are counted as such;
/// nothing a source sends ever fails the batch except Kaeso's own databases.
pub async fn record(
    state: &AppState,
    source: &Source,
    batch: Batch,
) -> Result<Recorded, RecordError> {
    let now = state.clock.now();
    let request_ids: Vec<Uuid> = batch
        .requests
        .iter()
        .map(|r| request_id(source.id, &format!("r:{}", r.external_id)))
        .collect();
    let call_ids: Vec<Uuid> = batch
        .tool_calls
        .iter()
        .map(|c| request_id(source.id, &format!("t:{}", c.external_id)))
        .collect();
    let mut all = request_ids.clone();
    all.extend(&call_ids);
    let new = first_time(state, source.id, &all).await?;
    let (new_requests, new_calls) = new.split_at(request_ids.len());
    let emails = batch
        .requests
        .iter()
        .filter_map(|r| r.email.clone())
        .chain(batch.tool_calls.iter().filter_map(|c| c.email.clone()));
    let people = members(state, source, emails).await?;
    let kind = kind_of(source);
    let mut out = Recorded {
        refused: batch.refused as usize,
        ..Recorded::default()
    };
    for ((r, id), fresh) in batch
        .requests
        .into_iter()
        .zip(request_ids)
        .zip(new_requests)
    {
        if !*fresh {
            out.duplicates += 1;
            continue;
        }
        record_request(state, source, kind, &people, r, id, now).await;
        out.requests += 1;
    }
    for ((c, id), fresh) in batch.tool_calls.into_iter().zip(call_ids).zip(new_calls) {
        if !*fresh {
            out.duplicates += 1;
            continue;
        }
        record_call(state, source, &people, c, id, now).await;
        out.tool_calls += 1;
    }
    let at = now;
    let error = batch.refusal.as_deref().map(|e| cut(e, 300));
    if out.requests + out.tool_calls > 0 {
        kaeso_store::pg::sources::touch(&state.pg, source.id, at, None).await?;
    }
    if let Some(e) = &error {
        kaeso_store::pg::sources::touch(&state.pg, source.id, at, Some(e)).await?;
    }
    crate::platform::projects::sources::add_activity(
        state,
        source.id,
        at,
        Activity {
            batches: 1,
            requests: out.requests as i64,
            tool_calls: out.tool_calls as i64,
            duplicates: out.duplicates as i64,
            refused: out.refused as i64,
        },
    )
    .await;
    Ok(out)
}

async fn record_request(
    state: &AppState,
    source: &Source,
    kind: Option<SourceKind>,
    people: &HashMap<String, Uuid>,
    r: ObservedRequest,
    id: Uuid,
    now: OffsetDateTime,
) {
    let model = cut(&r.model, MAX_NAME);
    let provider = r
        .provider
        .clone()
        .or_else(|| kind.and_then(SourceKind::provider).map(str::to_owned));
    let priced = price(state, source.project_id, provider.as_deref(), &model).await;
    let dialect = r.dialect.unwrap_or(DialectId::OpenAiChat);
    let mut o = crate::model_api::pipeline::blank_outcome(RequestId::from_uuid(id));
    o.caller = Some(caller(source, people, r.email.as_deref()));
    o.source = Origin::Observed;
    o.source_id = Some(source.id);
    o.ingress = dialect;
    o.egress = Some(dialect);
    o.kind = kaeso_kpp::RequestKind::Generate;
    o.provider = priced.provider.as_deref().and_then(static_provider);
    o.custom_provider = match (&o.provider, &priced.provider) {
        (None, Some(p)) => Some(cut(p, 64)),
        _ => None,
    };
    o.requested_model = Some(model.clone());
    o.model_id = Some(model.clone());
    o.catalogue_model_id = priced.catalogue_model_id;
    o.prices = priced.prices;
    o.observed_cost = r.reported_cost.filter(|c| !c.is_sign_negative());
    o.stream = r.stream;
    o.status = r.status;
    if r.status >= 400 {
        o.error_kind = Some("provider_error".into());
        o.error_message = r.error_message.as_deref().map(|m| cut(m, MAX_ERROR));
    }
    o.usage = Some(usage(r.tokens));
    o.started_at = r.started_at.unwrap_or(now);
    o.duration = r.duration;
    o.ttfb = r.ttfb;
    o.user_agent = r.client.clone();
    o.tags = r.tags.clone();
    o.upstream_request_id = Some(r.external_id.clone()).filter(|_| r.external_id.len() <= 200);
    let kept = source
        .content
        .then(|| content(&r, dialect, &model))
        .flatten();
    o.content_kept = kept.is_some();
    if let Some((body, _)) = &kept {
        o.request_body = Some(bytes::Bytes::from(body.clone()));
    }
    o.reply = kept.and_then(|(_, reply)| serde_json::from_str::<Response>(&reply).ok());
    crate::services::usage::account(state, &o).await;
    crate::services::logs::record(state, &o).await;
}

async fn record_call(
    state: &AppState,
    source: &Source,
    people: &HashMap<String, Uuid>,
    c: ObservedToolCall,
    id: Uuid,
    now: OffsetDateTime,
) {
    let who = caller(source, people, c.email.as_deref());
    let row = ToolCallRow {
        id,
        org_id: source.org_id,
        project_id: source.project_id,
        server_id: Uuid::nil(),
        started_at: c.started_at.unwrap_or(now),
        tool: cut(&c.tool, MAX_NAME),
        upstream_tool: cut(&c.tool, MAX_NAME),
        caller_kind: if who.user_id.is_some() {
            "user"
        } else {
            "service_account"
        }
        .to_owned(),
        caller_id: who.user_id.or(who.service_account_id).unwrap_or_default(),
        key_id: None,
        grant_id: None,
        client_name: c.client.clone().unwrap_or_default(),
        protocol_version: String::new(),
        source: Origin::Observed.as_str().to_owned(),
        status: if c.ok { "ok" } else { "error" }.to_owned(),
        error_type: c.error_type.clone().unwrap_or_default(),
        error_message: c
            .error_message
            .as_deref()
            .map(|m| cut(m, MAX_ERROR))
            .unwrap_or_default(),
        duration_ms: u32::try_from(c.duration.as_millis()).unwrap_or(u32::MAX),
        upstream_ms: None,
        request_bytes: c.arguments.as_ref().map_or(0, |a| a.len() as u64),
        response_bytes: c.result.as_ref().map_or(0, |r| r.len() as u64),
        has_content: false,
        expires_at: now,
        source_id: source.id,
        server_name: c
            .server
            .as_deref()
            .map(|s| cut(s, MAX_NAME))
            .unwrap_or_default(),
    };
    let content = if source.content && (c.arguments.is_some() || c.result.is_some()) {
        Some((
            c.arguments.unwrap_or_default(),
            c.result.unwrap_or_default(),
        ))
    } else {
        None
    };
    crate::services::logs::record_tool_call(state, row, content).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_record_of_the_same_source_gets_the_same_id() {
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        assert_eq!(request_id(a, "r:req_1"), request_id(a, "r:req_1"));
        assert_ne!(request_id(a, "r:req_1"), request_id(a, "r:req_2"));
        assert_ne!(request_id(a, "r:req_1"), request_id(b, "r:req_1"));
    }

    #[test]
    fn long_text_is_cut_at_a_character() {
        assert_eq!(cut("äöü", 2), "äö");
        assert_eq!(cut("ab", 5), "ab");
    }
}
