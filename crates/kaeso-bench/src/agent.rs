use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::Method;
use serde::Serialize;

use crate::client::{Call, Conn, connect, server_timing};
use crate::harness::AGENT_MODEL;
use crate::stats::Summary;

pub(crate) const PATH: &str = "/v1/messages";
const TOOLS: usize = 24;
const TURNS: usize = 12;
const END: &[u8] = b"event: message_stop";

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct Phase {
    pub(crate) requests: usize,
    pub(crate) overhead_ms: Summary,
    pub(crate) upstream_ms: Summary,
    pub(crate) failed: usize,
    pub(crate) failures: BTreeMap<String, usize>,
}

impl Phase {
    pub(crate) fn why(&self) -> String {
        if self.failures.is_empty() {
            return String::new();
        }
        let listed: Vec<String> = self
            .failures
            .iter()
            .map(|(w, n)| format!("{n} × {w}"))
            .collect();
        format!("; {}", listed.join(", "))
    }
}

// Members in the order the SDKs write them, `type` first: a body with sorted keys, as
// `serde_json::Value` writes it, takes the decoders' slower path and no agent sends one.
#[derive(Serialize)]
struct Body {
    model: &'static str,
    max_tokens: u32,
    stream: bool,
    system: String,
    tools: Vec<Tool>,
    tool_choice: Typed,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Typed {
    r#type: &'static str,
}

#[derive(Serialize)]
struct Tool {
    name: String,
    description: String,
    input_schema: Schema,
}

#[derive(Serialize)]
struct Schema {
    r#type: &'static str,
    properties: Properties,
    required: [&'static str; 2],
    #[serde(rename = "additionalProperties")]
    additional_properties: bool,
}

#[derive(Serialize)]
struct Properties {
    path: Property,
    pattern: Property,
    limit: Property,
    mode: Property,
    options: Options,
}

#[derive(Default, Serialize)]
struct Property {
    r#type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    minimum: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maximum: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    r#enum: Option<[&'static str; 4]>,
    description: &'static str,
}

#[derive(Serialize)]
struct Options {
    r#type: &'static str,
    properties: OptionProperties,
    #[serde(rename = "additionalProperties")]
    additional_properties: bool,
}

#[derive(Serialize)]
struct OptionProperties {
    recursive: Property,
    encoding: Property,
    timeout_ms: Property,
}

#[derive(Serialize)]
struct Message {
    role: &'static str,
    content: Content,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Blocks(Vec<Block>),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Block {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Input,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Serialize)]
struct Input {
    path: String,
    mode: &'static str,
    limit: u32,
}

fn property(r#type: &'static str, description: &'static str) -> Property {
    Property {
        r#type,
        description,
        ..Property::default()
    }
}

fn tool_name(i: usize) -> String {
    format!("workspace_tool_{:02}", i % TOOLS)
}

fn tool(i: usize) -> Tool {
    Tool {
        name: tool_name(i),
        description: format!(
            "Tool {i} of the agent's workspace. It reads or changes files, runs commands and \
             reports what happened, the way a coding agent's tools do. Use it when the task \
             needs step {i}; give absolute paths, keep the output short, and never guess a \
             value the user did not give. The result is text the next turn can quote. {}",
            "It refuses a path outside the workspace and says which one. ".repeat(4)
        ),
        input_schema: Schema {
            r#type: "object",
            properties: Properties {
                path: property("string", "The absolute path to work on."),
                pattern: property("string", "A regular expression to look for."),
                limit: Property {
                    minimum: Some(1),
                    maximum: Some(2000),
                    ..property("integer", "The most lines to return.")
                },
                mode: Property {
                    r#enum: Some(["read", "write", "append", "list"]),
                    ..property("string", "What to do with the path.")
                },
                options: Options {
                    r#type: "object",
                    properties: OptionProperties {
                        recursive: property("boolean", "Also look into folders below the path."),
                        encoding: property("string", "The text encoding of the file."),
                        timeout_ms: property("integer", "Stop after this many milliseconds."),
                    },
                    additional_properties: false,
                },
            },
            required: ["path", "mode"],
            additional_properties: false,
        },
    }
}

fn turns(directives: &str) -> Vec<Message> {
    let mut messages = vec![Message {
        role: "user",
        content: Content::Text(
            "Find out why the nightly export is slow and fix it. Show me what you changed.".into(),
        ),
    }];
    for i in 0..TURNS {
        let id = format!("toolu_bench_{i:02}");
        messages.push(Message {
            role: "assistant",
            content: Content::Blocks(vec![
                Block::Text {
                    text: format!("Step {i}: I'll read the next file before I change anything."),
                },
                Block::ToolUse {
                    id: id.clone(),
                    name: tool_name(i),
                    input: Input {
                        path: format!("/workspace/export/stage_{i}.rs"),
                        mode: "read",
                        limit: 200,
                    },
                },
            ]),
        });
        messages.push(Message {
            role: "user",
            content: Content::Blocks(vec![Block::ToolResult {
                tool_use_id: id,
                content: (0..14)
                    .map(|line| {
                        format!(
                            "{line:>4}  let rows_{i}_{line} = export.stage({i}).read_batch(cursor, {line})?; // \"quoted\" \\ path\n"
                        )
                    })
                    .collect(),
            }]),
        });
    }
    messages.push(Message {
        role: "user",
        content: Content::Text(format!("Go on with the fix. {directives}")),
    });
    messages
}

// What a coding agent sends mid-session: a system prompt, a few dozen tool definitions with
// their schemas, and a conversation of tool calls and their results.
pub(crate) fn body(chunks: u64) -> Bytes {
    let system = format!(
        "You are a coding agent working in the user's repository. {}",
        "Read before you write, keep changes small, run the checks, and say what you did. "
            .repeat(40)
    );
    let body = Body {
        model: AGENT_MODEL,
        max_tokens: 4096,
        stream: true,
        system,
        tools: (0..TOOLS).map(tool).collect(),
        tool_choice: Typed { r#type: "auto" },
        messages: turns(&format!("[mock:gen={}] [mock:tps=4000]", chunks * 4)),
    };
    Bytes::from(serde_json::to_vec(&body).unwrap_or_default())
}

#[derive(Default)]
struct Sample {
    overhead_ms: Vec<f64>,
    upstream_ms: Vec<f64>,
    failures: BTreeMap<String, usize>,
}

impl Sample {
    fn phase(self) -> Phase {
        let failed: usize = self.failures.values().sum();
        Phase {
            requests: self.overhead_ms.len() + failed,
            overhead_ms: Summary::of(&self.overhead_ms),
            upstream_ms: Summary::of(&self.upstream_ms),
            failed,
            failures: self.failures,
        }
    }
}

async fn one(
    addr: SocketAddr,
    key: &str,
    body: &Bytes,
    conn: &mut Option<Conn>,
) -> crate::Result<(Option<f64>, Option<f64>)> {
    if conn.is_none() {
        *conn = Some(connect(addr, None).await?);
    }
    let c = conn.as_mut().ok_or("no connection")?;
    let resp = c
        .send(Call {
            method: Method::POST,
            path: PATH,
            bearer: Some(key),
            origin: None,
            cookie: None,
            body: Some(body.clone()),
        })
        .await?;
    let status = resp.status().as_u16();
    let timing = server_timing(resp.headers());
    let text = resp.into_body().collect().await?.to_bytes();
    if status != 200 {
        return Err(format!("status {status}").into());
    }
    if !text.windows(END.len()).any(|w| w == END) {
        return Err("no message_stop".into());
    }
    Ok((timing.kaeso_ms, timing.upstream_ms))
}

// One stream after another on one connection, as one agent session sends them, each `every`
// after the one before began; `pause` is waited before every request instead.
pub(crate) async fn run(
    addr: SocketAddr,
    key: &str,
    body: &Bytes,
    requests: usize,
    every: Duration,
    pause: Duration,
) -> Phase {
    let mut sample = Sample::default();
    let mut conn: Option<Conn> = None;
    for _ in 0..requests {
        if !pause.is_zero() {
            // The gateway closes a connection that stayed idle this long, as an agent's would be.
            conn = None;
            tokio::time::sleep(pause).await;
        }
        let began = Instant::now();
        match one(addr, key, body, &mut conn).await {
            Ok((kaeso, upstream)) => {
                sample.overhead_ms.extend(kaeso);
                sample.upstream_ms.extend(upstream);
            }
            Err(e) => {
                *sample.failures.entry(e.to_string()).or_default() += 1;
                conn = None;
            }
        }
        tokio::time::sleep(every.saturating_sub(began.elapsed())).await;
    }
    sample.phase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_is_a_tool_heavy_stream_of_about_fifty_kilobytes() {
        let bytes = body(8);
        assert!((45_000..60_000).contains(&bytes.len()), "{}", bytes.len());
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
        assert_eq!(v["model"], AGENT_MODEL);
        assert_eq!(v["stream"], true);
        assert_eq!(v["tools"].as_array().map(Vec::len), Some(TOOLS));
        let last = v["messages"]
            .as_array()
            .and_then(|m| m.last())
            .and_then(|m| m["content"].as_str())
            .unwrap_or_default()
            .to_owned();
        assert!(last.ends_with("[mock:gen=32] [mock:tps=4000]"), "{last}");
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with(r#"{"model":"#), "{text:.40}");
        assert!(text.contains(r#"{"type":"tool_use","id":"#));
        assert!(text.contains(r#"{"type":"tool_result","tool_use_id":"#));
    }
}
