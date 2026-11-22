use std::collections::{HashMap, HashSet};
use std::time::Duration;

use kaeso_core::id::{OrgId, ToolId};
use kaeso_mcp::{canonical, params};
use kaeso_store::StoreError;
use kaeso_store::pg::mcp_connections as connections;
use kaeso_store::pg::mcp_servers::{self, Check, ServerRow};
use kaeso_store::pg::mcp_tools::{
    self as store, Approval, Decider, Definition, NewChange, ToolRow,
};
use kaeso_upstream::UpstreamError;
use kaeso_upstream::mcp::{Client, Handshake, Limits, McpError, SecretHeader};
use serde_json::{Value, json};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use crate::cache::hex;
use crate::platform::audit::{self, Action};
use crate::services::mcp_connections::{self, ConnectionError};
use crate::state::AppState;

pub const LONG_DESCRIPTION: usize = 2000;
const AUTO_APPROVED: &str = "auto_approved";
const MAX_NAME: usize = 256;
const LIMITS: Limits = Limits {
    first_byte: Duration::from_secs(30),
    idle: Duration::from_secs(30),
    max_body: 4 * 1024 * 1024,
};

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Connection(#[from] ConnectionError),
    #[error(transparent)]
    Mcp(#[from] McpError),
    #[error("the server's address is not a URL")]
    Address,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl SyncError {
    pub fn status(&self) -> &'static str {
        match self {
            SyncError::Connection(ConnectionError::NotConnected | ConnectionError::SignedOut)
            | SyncError::Connection(ConnectionError::Unrenewed)
            | SyncError::Mcp(McpError::Unauthorized { .. }) => "signed_out",
            SyncError::Mcp(McpError::Transport(
                UpstreamError::Refused(_) | UpstreamError::Connect(_) | UpstreamError::Timeout(_),
            ))
            | SyncError::Address => "unreachable",
            _ => "error",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Skip {
    NoName,
    ListedTwice,
    NoInputSchema,
    Header(params::ParamError),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Synced {
    pub added: Vec<String>,
    pub changed: Vec<String>,
    pub auto_approved: Vec<String>,
    pub removed: usize,
    pub skipped: Vec<(String, Skip)>,
}

pub fn definition_of(row: &ToolRow) -> Value {
    canonical::definition(&json!({
        "name": row.name,
        "title": row.title,
        "description": row.description,
        "inputSchema": row.input_schema,
        "outputSchema": row.output_schema,
        "annotations": row.annotations,
    }))
}

async fn fetch(
    state: &AppState,
    server: &ServerRow,
    person: Option<Uuid>,
) -> Result<(Handshake, Vec<Value>), SyncError> {
    let url = Url::parse(&server.url).map_err(|_| SyncError::Address)?;
    let credential = mcp_connections::credential(state, server, person).await?;
    let client = Client {
        upstream: &state.mcp,
        url: &url,
        secret: credential.as_ref().map(|c| SecretHeader {
            name: &c.header,
            value: &c.value,
        }),
        limits: LIMITS,
    };
    let hs = client.handshake().await?;
    let tools = client.list_tools(&hs).await?;
    Ok((hs, tools))
}

// A per-person server lists the same tools for everyone, so the sync signs in as one person who
// connected; until someone has, there is nothing to list and the server stays unchecked.
pub async fn sync(state: &AppState, server: &ServerRow) -> Result<Synced, SyncError> {
    let now = state.clock.now();
    let person = if server.credential_mode == "per_user" {
        match connections::person_for_sync(&state.pg, server.id).await? {
            Some(p) => Some(p),
            None => return Ok(Synced::default()),
        }
    } else {
        None
    };
    match fetch(state, server, person).await {
        Ok((hs, tools)) => {
            let synced = apply(state, server, &tools).await?;
            mcp_servers::record_check(
                &state.pg,
                server.id,
                &Check {
                    status: "ok",
                    protocol_version: Some(hs.version.as_str()),
                    server_info: hs.server_info.as_ref(),
                    last_error: None,
                    at: now,
                },
            )
            .await?;
            crate::services::notifications::system::tools_held(state, server, &synced.changed)
                .await;
            Ok(synced)
        }
        Err(e) => {
            let message = e.to_string();
            mcp_servers::record_check(
                &state.pg,
                server.id,
                &Check {
                    status: e.status(),
                    protocol_version: None,
                    server_info: None,
                    last_error: Some(&message.chars().take(1000).collect::<String>()),
                    at: now,
                },
            )
            .await?;
            Err(e)
        }
    }
}

fn valid_name(v: &Value) -> Option<&str> {
    v.get("name")
        .and_then(Value::as_str)
        .filter(|n| !n.is_empty() && n.chars().count() <= MAX_NAME)
}

pub async fn apply(
    state: &AppState,
    server: &ServerRow,
    tools: &[Value],
) -> Result<Synced, StoreError> {
    let now = state.clock.now();
    let existing: HashMap<String, ToolRow> = store::of_server(&state.pg, server.id)
        .await?
        .into_iter()
        .map(|t| (t.name.clone(), t))
        .collect();
    let auto = server.auto_approve_tool_changes;
    let mut synced = Synced::default();
    let mut present: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    for tool in tools {
        let Some(name) = valid_name(tool) else {
            let shown = tool.get("name").and_then(Value::as_str).unwrap_or_default();
            let shown: String = shown.chars().take(MAX_NAME).collect();
            synced.skipped.push((shown, Skip::NoName));
            continue;
        };
        if !seen.insert(name.to_owned()) {
            synced.skipped.push((name.to_owned(), Skip::ListedTwice));
            continue;
        }
        let Some(input) = tool.get("inputSchema").filter(|s| s.is_object()) else {
            synced.skipped.push((name.to_owned(), Skip::NoInputSchema));
            continue;
        };
        if let Err(e) = params::param_headers(input) {
            synced.skipped.push((name.to_owned(), Skip::Header(e)));
            continue;
        }
        present.push(name.to_owned());
        let hash = canonical::definition_hash(tool);
        let definition = canonical::definition(tool);
        let object = |key: &str| tool.get(key).filter(|v| v.is_object());
        let d = Definition {
            name,
            title: tool.get("title").and_then(Value::as_str),
            description: tool
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            input_schema: input,
            output_schema: object("outputSchema"),
            annotations: object("annotations"),
            meta: object("_meta"),
            hash: &hash,
        };
        let approval = Approval {
            hash: &hash,
            definition: &definition,
        };
        match existing.get(name) {
            None => {
                store::insert(
                    &state.pg,
                    Uuid::now_v7(),
                    server.id,
                    server.org_id,
                    &d,
                    Some(&approval),
                    now,
                )
                .await?;
                synced.added.push(name.to_owned());
            }
            Some(row) => {
                let differs = row.definition_hash.as_slice() != hash.as_slice();
                let approved = row.approved_hash.as_deref() == Some(hash.as_slice());
                let approve_now = auto && !approved;
                if !differs && !approve_now {
                    store::update_definition(
                        &state.pg,
                        row.id,
                        &row.definition_hash,
                        &d,
                        None,
                        now,
                    )
                    .await?;
                    continue;
                }
                let mut tx = state.pg.begin().await?;
                let updated = store::update_definition(
                    &mut *tx,
                    row.id,
                    &row.definition_hash,
                    &d,
                    approve_now.then_some(&approval),
                    now,
                )
                .await?;
                if !updated {
                    continue;
                }
                if differs {
                    store::supersede_open(&mut *tx, row.id, now).await?;
                    let decided = if approve_now {
                        Some((AUTO_APPROVED, Decider::SYSTEM))
                    } else if approved {
                        Some(("restored", Decider::SYSTEM))
                    } else {
                        None
                    };
                    store::record_change(
                        &mut *tx,
                        &NewChange {
                            id: Uuid::now_v7(),
                            org_id: server.org_id,
                            server_id: server.id,
                            tool_id: row.id,
                            from_hash: &row.definition_hash,
                            to_hash: &hash,
                            at: now,
                            decided,
                        },
                    )
                    .await?;
                } else {
                    store::decide(&mut *tx, row.id, &hash, AUTO_APPROVED, Decider::SYSTEM, now)
                        .await?;
                }
                if approve_now {
                    audit::system_event(
                        &mut tx,
                        Some(OrgId::from_uuid(server.org_id)),
                        Action::ToolAutoApprove,
                        Some(("tool", ToolId::from_uuid(row.id).to_string())),
                        Some(json!({
                            "hash": row.approved_hash.as_deref().map(hex),
                            "definition": row.approved,
                        })),
                        Some(json!({ "hash": hex(&hash), "definition": definition })),
                        now,
                    )
                    .await?;
                }
                tx.commit().await?;
                if approve_now {
                    synced.auto_approved.push(name.to_owned());
                } else if differs && !approved {
                    synced.changed.push(name.to_owned());
                }
            }
        }
    }
    synced.removed = store::mark_removed(&state.pg, server.id, &present, now)
        .await?
        .len();
    if !synced.changed.is_empty() {
        tracing::info!(server = %server.id, tools = ?synced.changed, "an MCP server changed tools; they wait for review");
    }
    if synced != Synced::default() {
        crate::portal::snapshot::forget_server(state, server.id).await;
    }
    Ok(synced)
}

pub async fn approve(
    state: &AppState,
    org: Uuid,
    tool: Uuid,
    expected_hash: &[u8],
    by: Decider,
) -> Result<bool, StoreError> {
    let Some(row) = store::get(&state.pg, org, tool).await? else {
        return Ok(false);
    };
    let definition = definition_of(&row);
    let now = state.clock.now();
    let mut tx = state.pg.begin().await?;
    let approved = store::approve(
        &mut *tx,
        org,
        tool,
        expected_hash,
        &Approval {
            hash: &row.definition_hash,
            definition: &definition,
        },
        now,
    )
    .await?;
    if approved {
        store::decide(&mut *tx, tool, &row.definition_hash, "approved", by, now).await?;
    }
    tx.commit().await?;
    if approved {
        crate::portal::snapshot::forget_server(state, row.server_id).await;
    }
    Ok(approved)
}

// What switching auto-approve on approves: every change of the server's tools held for review,
// in the caller's transaction, so the switch and the approvals land together.
pub async fn approve_held(
    tx: &mut sqlx::PgConnection,
    server: &ServerRow,
    by: Decider,
    now: OffsetDateTime,
) -> Result<Vec<(ToolRow, Value)>, StoreError> {
    let mut out = Vec::new();
    for row in store::of_server(&mut *tx, server.id).await? {
        if row.state != "changed" {
            continue;
        }
        let definition = definition_of(&row);
        let approval = Approval {
            hash: &row.definition_hash,
            definition: &definition,
        };
        if store::approve(
            &mut *tx,
            server.org_id,
            row.id,
            &row.definition_hash,
            &approval,
            now,
        )
        .await?
        {
            store::decide(&mut *tx, row.id, &row.definition_hash, "approved", by, now).await?;
            out.push((row, definition));
        }
    }
    Ok(out)
}

// Rejecting keeps the change held and switches the tool off wherever it was on, so nothing uses
// the new definition until the server changes it again and someone approves that.
pub async fn reject(
    state: &AppState,
    org: Uuid,
    tool: Uuid,
    by: Decider,
) -> Result<Option<u64>, StoreError> {
    let Some(row) = store::get(&state.pg, org, tool).await? else {
        return Ok(None);
    };
    if row.state != "changed" {
        return Ok(Some(0));
    }
    let projects = store::projects_using(&state.pg, tool).await?;
    let mut tx = state.pg.begin().await?;
    let switched = store::disable_everywhere(&mut *tx, tool).await?;
    store::decide(
        &mut *tx,
        tool,
        &row.definition_hash,
        "kept_off",
        by,
        state.clock.now(),
    )
    .await?;
    tx.commit().await?;
    for p in projects {
        crate::portal::snapshot::forget(state, p).await;
    }
    Ok(Some(switched))
}
