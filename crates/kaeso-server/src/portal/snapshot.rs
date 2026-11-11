use std::collections::HashMap;
use std::sync::Arc;

use kaeso_mcp::params::{self, ParamHeader};
use kaeso_mcp::{apps, names};
use kaeso_store::StoreError;
use kaeso_store::pg::tool_access::{self, ProjectToolAccess};
use kaeso_store::pg::{mcp_servers, mcp_tools};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::state::AppState;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalServer {
    pub id: Uuid,
    pub handle: String,
    pub name: String,
    pub url: String,
    pub per_user: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Standing {
    Offered,
    Off,
    Held,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalTool {
    pub id: Uuid,
    pub server: usize,
    pub exposed: String,
    pub upstream: String,
    pub standing: Standing,
    pub listed: String,
    pub params: Vec<ParamHeader>,
}

// An MCP Apps view a server's tool names, under the URI the portal shows for it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalView {
    pub exposed: String,
    pub upstream: String,
    pub server: usize,
    pub tools: Vec<Uuid>,
}

// Everything a call to a project's portal needs, built once per change and shared: the tools
// list is kept as JSON so no request serializes a definition again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalSnapshot {
    pub servers: Vec<PortalServer>,
    pub tools: Vec<PortalTool>,
    pub by_name: HashMap<String, usize>,
    pub access: ProjectToolAccess,
    pub listed: String,
    #[serde(default)]
    pub views: Vec<PortalView>,
}

impl PortalSnapshot {
    pub fn tool(&self, exposed: &str) -> Option<&PortalTool> {
        self.by_name.get(exposed).and_then(|i| self.tools.get(*i))
    }

    pub fn server(&self, tool: &PortalTool) -> Option<&PortalServer> {
        self.servers.get(tool.server)
    }

    pub fn offered(&self) -> impl Iterator<Item = &PortalTool> {
        self.tools
            .iter()
            .filter(|t| t.standing == Standing::Offered)
    }

    pub fn view(&self, exposed: &str) -> Option<&PortalView> {
        self.views.iter().find(|v| v.exposed == exposed)
    }

    // A view calls its own server's tools by the names that server gave them; the portal knows
    // them by their exposed names, so a name only one viewing server uses is taken as that tool.
    pub fn tool_of_view(&self, upstream: &str) -> Option<&PortalTool> {
        let mut found = self
            .offered()
            .filter(|t| t.upstream == upstream && self.views.iter().any(|v| v.server == t.server));
        let first = found.next()?;
        found.next().is_none().then_some(first)
    }
}

// As the server listed it, with the portal's name; `_meta` and the output schema pass through,
// with a view's URI under the portal's name for it.
fn listed(exposed: &str, t: &mcp_tools::ToolRow, view: Option<(&str, &str)>) -> String {
    let mut o = Map::new();
    o.insert("name".into(), Value::String(exposed.to_owned()));
    if let Some(title) = &t.title {
        o.insert("title".into(), Value::String(title.clone()));
    }
    o.insert("description".into(), Value::String(t.description.clone()));
    o.insert("inputSchema".into(), t.input_schema.clone());
    for (key, value) in [
        ("outputSchema", &t.output_schema),
        ("annotations", &t.annotations),
    ] {
        if let Some(v) = value {
            o.insert(key.into(), v.clone());
        }
    }
    if let Some(meta) = &t.meta {
        let mut meta = meta.clone();
        if let Some((upstream, shown)) = view {
            apps::rename(&mut meta, upstream, shown);
        }
        o.insert("_meta".into(), meta);
    }
    Value::Object(o).to_string()
}

pub async fn build(state: &AppState, project: Uuid) -> Result<PortalSnapshot, StoreError> {
    let rows = mcp_servers::of_project(&state.pg, project).await?;
    let mut index = HashMap::new();
    let servers: Vec<PortalServer> = rows
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            index.insert(s.id, i);
            PortalServer {
                id: s.id,
                handle: s.handle,
                name: s.name,
                url: s.url,
                per_user: s.credential_mode == "per_user",
            }
        })
        .collect();
    let mut tools = Vec::new();
    let mut by_name = HashMap::new();
    let mut views: Vec<PortalView> = Vec::new();
    for r in mcp_tools::of_project(&state.pg, project).await? {
        let Some(&server) = index.get(&r.tool.server_id) else {
            continue;
        };
        let exposed = names::exposed(&r.server_handle, &r.tool.name);
        let view = r
            .tool
            .meta
            .as_ref()
            .and_then(apps::view_of)
            .and_then(|uri| Some((uri.to_owned(), apps::exposed(&r.server_handle, uri)?)));
        if let Some((upstream, shown)) = &view {
            match views.iter_mut().find(|v| &v.exposed == shown) {
                Some(v) => v.tools.push(r.tool.id),
                None => views.push(PortalView {
                    exposed: shown.clone(),
                    upstream: upstream.clone(),
                    server,
                    tools: vec![r.tool.id],
                }),
            }
        }
        let standing = match (r.enabled, r.tool.is_approved()) {
            (false, _) => Standing::Off,
            (true, false) => Standing::Held,
            (true, true) => Standing::Offered,
        };
        by_name.insert(exposed.clone(), tools.len());
        tools.push(PortalTool {
            id: r.tool.id,
            server,
            listed: listed(
                &exposed,
                &r.tool,
                view.as_ref().map(|(u, e)| (u.as_str(), e.as_str())),
            ),
            params: params::param_headers(&r.tool.input_schema).unwrap_or_default(),
            exposed,
            upstream: r.tool.name,
            standing,
        });
    }
    let access = {
        let mut conn = state.pg.acquire().await?;
        tool_access::project_access(&mut conn, project).await?
    };
    let listed = tools
        .iter()
        .filter(|t| t.standing == Standing::Offered)
        .map(|t| t.listed.as_str())
        .collect::<Vec<_>>()
        .join(",");
    Ok(PortalSnapshot {
        servers,
        tools,
        by_name,
        access,
        listed,
        views,
    })
}

pub async fn get(state: &AppState, project: Uuid) -> Result<Arc<PortalSnapshot>, StoreError> {
    let st = state.clone();
    state
        .caches
        .portal
        .get_or_refresh(&project.to_string(), move || async move {
            build(&st, project).await.map(Some)
        })
        .await?
        .ok_or_else(|| StoreError::Config("a portal snapshot vanished while it was built".into()))
}

pub async fn forget(state: &AppState, project: Uuid) {
    state.caches.portal.invalidate(&project.to_string()).await;
}

pub async fn forget_server(state: &AppState, server: Uuid) {
    match mcp_servers::projects(&state.pg, server).await {
        Ok(projects) => {
            for p in projects {
                forget(state, p).await;
            }
        }
        Err(e) => {
            tracing::warn!(server = %server, error = %e, "the portals using an MCP server couldn't be refreshed; they catch up within 30 seconds");
        }
    }
}
