use kaeso_kpp::{
    Base64Source, Block, CacheHint, CacheTtl, DocumentBlock, GenerateRequest, KppError, OpaqueKind,
    OutputSpec, ReasoningEffort, ReasoningMode, Request, Role, Source, ToolChoice, ToolDef,
};
use serde_json::{Map, Value, json};

use super::D;
use crate::codec::{EgressCtx, EncodedRequest, HeaderList};
use crate::cross::{Setting, app_field, check_foreign};

// ToolSpecification.name and ToolUseBlock.toolUseId: 1 to 64 characters of `[a-zA-Z0-9_-]` and
// `[a-zA-Z0-9_.:-]` (https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ToolUseBlock.html).
const MAX_TOOL_TEXT: usize = 64;

fn valid_name(name: &str) -> bool {
    (1..=MAX_TOOL_TEXT).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn valid_id(id: &str) -> bool {
    (1..=MAX_TOOL_TEXT).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'))
}

fn generate(req: &Request) -> Result<&GenerateRequest, KppError> {
    match req {
        Request::Generate(r) => Ok(r),
        other => Err(KppError::unsupported_by(
            format!("{:?} request", other.kind()),
            D,
            "Amazon Bedrock Converse serves message generation only.",
        )),
    }
}

pub(super) fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
    check_foreign(req, &ctx.origin(D))?;
    let r = generate(req)?;
    crate::cross::check_text_only(r, D, "Converse replies")?;
    check_output(r)?;
    check_sampling(r)?;
    check_reasoning(r)?;
    if r.parallel_tool_calls == Some(false) {
        return Err(KppError::unsupported_by(
            "`parallel_tool_calls: false`",
            D,
            "Converse can't be told to call one tool at a time; remove `parallel_tool_calls`.",
        )
        .with_param("parallel_tool_calls"));
    }
    match &r.tool_choice {
        Some(ToolChoice::None) => {
            return Err(KppError::unsupported_by(
                "`tool_choice: none`",
                D,
                "Converse has no choice that forbids tools; leave the tools out instead.",
            )
            .with_param("tool_choice"));
        }
        Some(ToolChoice::Unknown(_)) => {
            return Err(
                KppError::unsupported_by("this `tool_choice`", D, "").with_param("tool_choice")
            );
        }
        _ => {}
    }
    for (i, t) in r.tools.iter().enumerate() {
        match t {
            ToolDef::Function(f) if f.parameters.as_ref().is_none_or(object_schema) => {
                check_cache(f.cache.as_ref(), &format!("tools[{i}]"))?;
            }
            ToolDef::Function(_) => {
                return Err(KppError::unsupported_by(
                    "tool parameters that are not a JSON object schema",
                    D,
                    "Converse takes a JSON object schema as a tool's input schema.",
                )
                .with_param(format!("tools[{i}]")));
            }
            _ => {
                return Err(KppError::unsupported_by(
                    "a tool that isn't a function",
                    D,
                    "Converse calls functions only; remove the hosted tool.",
                )
                .with_param(format!("tools[{i}]")));
            }
        }
    }
    for (i, b) in r.system.iter().enumerate() {
        let param = format!("system[{i}]");
        match b {
            Block::Text(t) => check_cache(t.cache.as_ref(), &param)?,
            other => {
                return Err(KppError::unsupported_by(
                    format!("{} block in the system prompt", other.kind()),
                    D,
                    "The Converse system prompt holds text only.",
                )
                .with_param(param));
            }
        }
    }
    for (mi, m) in r.messages.iter().enumerate() {
        for (bi, b) in m.blocks.iter().enumerate() {
            check_block(b, &format!("messages[{mi}].blocks[{bi}]"), false)?;
        }
    }
    Ok(())
}

// ToolInputSchema: "The top level schema type must be `object`"
// (https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ToolInputSchema.html).
fn object_schema(schema: &kaeso_kpp::RawJson) -> bool {
    serde_json::from_str::<Value>(schema.get())
        .ok()
        .and_then(|v| v.as_object().map(|o| o.get("type").cloned()))
        .is_some_and(|t| t.is_none_or(|t| t == "object"))
}

fn check_output(r: &GenerateRequest) -> Result<(), KppError> {
    match &r.output {
        OutputSpec::Text => Ok(()),
        _ => Err(KppError::unsupported_by(
            "structured output",
            D,
            "Kaeso doesn't send JSON output formats through Converse yet; describe the format in the prompt.",
        )
        .with_param(app_field(r, Setting::OutputFormat, D))),
    }
}

fn check_sampling(r: &GenerateRequest) -> Result<(), KppError> {
    let s = &r.sampling;
    let unsupported = [
        ("seed", s.seed.is_some()),
        ("frequency_penalty", s.frequency_penalty.is_some()),
        ("presence_penalty", s.presence_penalty.is_some()),
    ];
    if let Some((name, _)) = unsupported.iter().find(|(_, set)| *set) {
        return Err(KppError::unsupported_by(
            *name,
            D,
            format!("Converse takes `maxTokens`, `temperature`, `topP` and stop sequences only; remove `{name}`."),
        )
        .with_param(*name));
    }
    Ok(())
}

// Each model family turns reasoning on with fields of its own in `additionalModelRequestFields`,
// which Kaeso doesn't write yet; a request that asks for none goes as it is.
fn check_reasoning(r: &GenerateRequest) -> Result<(), KppError> {
    let Some(c) = &r.reasoning else {
        return Ok(());
    };
    let off = matches!(c.mode, None | Some(ReasoningMode::Disabled))
        && matches!(c.effort, None | Some(ReasoningEffort::None))
        && c.budget_tokens.is_none_or(|b| b == 0);
    if off {
        return Ok(());
    }
    Err(KppError::unsupported_by(
        "reasoning settings",
        D,
        "Kaeso doesn't send reasoning settings through Converse yet; remove them.",
    )
    .with_param(app_field(r, Setting::Reasoning, D)))
}

fn check_cache(cache: Option<&CacheHint>, param: &str) -> Result<(), KppError> {
    match cache.and_then(|c| c.ttl.as_ref()) {
        Some(CacheTtl::Other(ttl)) => Err(KppError::unsupported_by(
            format!("cache time `{ttl}`"),
            D,
            "Converse caches for 5 minutes or 1 hour.",
        )
        .with_param(param.to_owned())),
        _ => Ok(()),
    }
}

fn check_block(b: &Block, param: &str, in_result: bool) -> Result<(), KppError> {
    let refuse = |what: String, hint: &str| {
        Err(KppError::unsupported_by(what, D, hint).with_param(param.to_owned()))
    };
    match b {
        Block::Text(t) => check_cache(t.cache.as_ref(), param),
        Block::Json(_) if in_result => Ok(()),
        Block::Image(i) => {
            image_format(&i.source, param)?;
            check_cache(i.cache.as_ref(), param)
        }
        Block::Document(d) if d.citations => refuse(
            "document citations".into(),
            "Kaeso doesn't ask Converse for citations yet; turn them off.",
        ),
        Block::Document(d) => {
            document_format(d, param)?;
            check_cache(d.cache.as_ref(), param)
        }
        Block::ToolCall(_) | Block::Reasoning(_) | Block::Refusal(_) if !in_result => Ok(()),
        Block::ToolResult(t) if !in_result => {
            for (i, c) in t.content.iter().enumerate() {
                check_block(c, &format!("{param}.content[{i}]"), true)?;
            }
            check_cache(t.cache.as_ref(), param)
        }
        other => refuse(
            format!("{} block", other.kind()),
            "Converse takes text, images, documents, tool calls and tool results.",
        ),
    }
}

fn bytes_of(source: &Source, param: &str) -> Result<Base64Source, KppError> {
    let refuse = |hint: &str| {
        Err(
            KppError::unsupported_by("media by link or file id", D, hint)
                .with_param(param.to_owned()),
        )
    };
    match source {
        Source::Base64(s) => Ok(s.clone()),
        Source::Url(u) => match data_url(u) {
            Some(Ok(s)) => Ok(s),
            Some(Err(e)) => Err(e.with_param(param.to_owned())),
            None => refuse("Converse takes media as its bytes; send it inline as base64."),
        },
        _ => refuse("Converse takes media as its bytes; send it inline as base64."),
    }
}

// `data:<media type>;base64,<data>`.
fn data_url(url: &str) -> Option<Result<Base64Source, KppError>> {
    let rest = url
        .get(..5)?
        .eq_ignore_ascii_case("data:")
        .then(|| &url[5..])?;
    let (head, data) = rest.split_once(',')?;
    let media_type = head
        .strip_suffix(";base64")?
        .split(';')
        .next()?
        .to_ascii_lowercase();
    Some(Base64Source::from_base64(media_type, data))
}

// ImageBlock.format (https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ImageBlock.html).
fn image_format(source: &Source, param: &str) -> Result<&'static str, KppError> {
    let bytes = bytes_of(source, param)?;
    match bytes.media_type.as_str() {
        "image/png" => Ok("png"),
        "image/jpeg" | "image/jpg" => Ok("jpeg"),
        "image/gif" => Ok("gif"),
        "image/webp" => Ok("webp"),
        other => Err(KppError::unsupported_by(
            format!("image type `{other}`"),
            D,
            "Converse takes PNG, JPEG, GIF and WebP images.",
        )
        .with_param(param.to_owned())),
    }
}

// DocumentBlock.format (https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_DocumentBlock.html).
fn document_format(d: &DocumentBlock, param: &str) -> Result<&'static str, KppError> {
    let media_type = match &d.source {
        Source::Text(t) => t.media_type.clone(),
        other => bytes_of(other, param)?.media_type,
    };
    match media_type.as_str() {
        "application/pdf" => Ok("pdf"),
        "text/csv" => Ok("csv"),
        "application/msword" => Ok("doc"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Ok("docx"),
        "application/vnd.ms-excel" => Ok("xls"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => Ok("xlsx"),
        "text/html" => Ok("html"),
        "text/plain" => Ok("txt"),
        "text/markdown" => Ok("md"),
        other => Err(KppError::unsupported_by(
            format!("document type `{other}`"),
            D,
            "Converse takes PDF, CSV, Word, Excel, HTML, text and Markdown documents.",
        )
        .with_param(param.to_owned())),
    }
}

// DocumentBlock.name: letters, digits, single spaces, hyphens, parentheses and square brackets.
fn document_name(title: Option<&str>, n: usize) -> String {
    let mut name = String::new();
    for c in title.unwrap_or_default().chars() {
        let c = if c.is_ascii_alphanumeric() || "-()[]".contains(c) {
            c
        } else if c.is_whitespace() {
            ' '
        } else {
            '-'
        };
        if !(c == ' ' && (name.is_empty() || name.ends_with(' '))) {
            name.push(c);
        }
    }
    let name: String = name.trim_end().chars().take(200).collect();
    if name.is_empty() {
        format!("Document {n}")
    } else {
        name
    }
}

fn set(v: &mut Value, key: &str, value: Value) {
    if let Some(o) = v.as_object_mut() {
        o.insert(key.to_owned(), value);
    }
}

fn cache_point(cache: Option<&CacheHint>) -> Option<Value> {
    let c = cache?;
    let mut point = json!({ "type": "default" });
    match c.ttl {
        Some(CacheTtl::FiveMinutes) => set(&mut point, "ttl", json!("5m")),
        Some(CacheTtl::OneHour) => set(&mut point, "ttl", json!("1h")),
        _ => {}
    }
    Some(json!({ "cachePoint": point }))
}

fn block_cache(b: &Block) -> Option<&CacheHint> {
    match b {
        Block::Text(t) => t.cache.as_ref(),
        Block::Image(i) => i.cache.as_ref(),
        Block::Document(d) => d.cache.as_ref(),
        Block::ToolCall(t) => t.cache.as_ref(),
        Block::ToolResult(t) => t.cache.as_ref(),
        _ => None,
    }
}

struct Encoder<'a> {
    ctx: &'a mut EgressCtx,
    documents: usize,
}

impl Encoder<'_> {
    fn id(&mut self, original: &str) -> String {
        crate::wire::rewrite_name(original, valid_id, MAX_TOOL_TEXT, &mut self.ctx.tool_ids)
    }

    fn name(&mut self, original: &str) -> String {
        crate::wire::rewrite_name(
            original,
            valid_name,
            MAX_TOOL_TEXT,
            &mut self.ctx.tool_names,
        )
    }

    fn image(&self, source: &Source, param: &str) -> Result<Value, KppError> {
        let format = image_format(source, param)?;
        let bytes = bytes_of(source, param)?;
        Ok(json!({ "image": { "format": format, "source": { "bytes": bytes.to_base64() } } }))
    }

    fn document(&mut self, d: &DocumentBlock, param: &str) -> Result<Value, KppError> {
        self.documents += 1;
        let format = document_format(d, param)?;
        let source = match &d.source {
            Source::Text(t) => json!({ "text": t.data }),
            other => json!({ "bytes": bytes_of(other, param)?.to_base64() }),
        };
        let mut doc = json!({
            "format": format,
            "name": document_name(d.title.as_deref(), self.documents),
            "source": source,
        });
        if let Some(context) = &d.context {
            set(&mut doc, "context", json!(context));
        }
        Ok(json!({ "document": doc }))
    }

    fn block(&mut self, b: &Block, param: &str, out: &mut Vec<Value>) -> Result<(), KppError> {
        let value = match b {
            Block::Text(t) => json!({ "text": t.text }),
            Block::Refusal(r) => json!({ "text": r.text }),
            Block::Image(i) => self.image(&i.source, param)?,
            Block::Document(d) => self.document(d, param)?,
            Block::ToolCall(c) => {
                let input: Value = serde_json::from_str(c.arguments.get()).map_err(|_| {
                    KppError::invalid_request("A tool call's arguments aren't JSON.")
                })?;
                json!({ "toolUse": {
                    "toolUseId": self.id(&c.id),
                    "name": self.name(&c.name),
                    "input": input,
                } })
            }
            Block::ToolResult(t) => {
                let mut content = Vec::new();
                for (i, c) in t.content.iter().enumerate() {
                    let param = format!("{param}.content[{i}]");
                    content.push(match c {
                        Block::Text(x) => json!({ "text": x.text }),
                        Block::Json(j) => json!({ "json": serde_json::from_str::<Value>(j.data.get()).unwrap_or(Value::Null) }),
                        Block::Image(x) => self.image(&x.source, &param)?,
                        Block::Document(x) => self.document(x, &param)?,
                        _ => continue,
                    });
                }
                let mut result = json!({ "toolUseId": self.id(&t.call_id), "content": content });
                if t.is_error {
                    set(&mut result, "status", json!("error"));
                }
                json!({ "toolResult": result })
            }
            Block::Reasoning(r) if r.replayable_to(D) => {
                if let Some(redacted) = r.opaque_of(OpaqueKind::BedrockRedactedReasoning) {
                    json!({ "reasoningContent": { "redactedContent": redacted.data } })
                } else {
                    let mut text = json!({ "text": r.text.concat() });
                    if let Some(sig) = r.opaque_of(OpaqueKind::BedrockReasoningSignature) {
                        set(&mut text, "signature", json!(sig.data));
                    }
                    json!({ "reasoningContent": { "reasoningText": text } })
                }
            }
            // Reasoning of another format carries nothing Converse can check, so it isn't sent.
            _ => return Ok(()),
        };
        out.push(value);
        out.extend(cache_point(block_cache(b)));
        Ok(())
    }
}

/// The model id in the path, every reserved character percent-encoded, as an inference profile
/// ARN's `:` and `/` must be.
pub(super) fn model_path(model: &str, stream: bool) -> String {
    let mut id = String::with_capacity(model.len());
    for b in model.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            id.push(char::from(b));
        } else {
            id.push_str(&format!("%{b:02X}"));
        }
    }
    let op = if stream {
        "converse-stream"
    } else {
        "converse"
    };
    format!("/model/{id}/{op}")
}

pub(super) fn encode(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    let r = generate(req)?;
    let stream = !ctx.stream.is_off();
    let mut enc = Encoder { ctx, documents: 0 };
    let mut body = Map::new();

    let mut system = Vec::new();
    for (i, b) in r.system.iter().enumerate() {
        enc.block(b, &format!("system[{i}]"), &mut system)?;
    }
    if !system.is_empty() {
        body.insert("system".into(), Value::Array(system));
    }

    let mut messages: Vec<Value> = Vec::new();
    for (mi, m) in r.messages.iter().enumerate() {
        let role = if m.role == Role::Assistant {
            "assistant"
        } else {
            "user"
        };
        let mut content = Vec::new();
        for (bi, b) in m.blocks.iter().enumerate() {
            enc.block(b, &format!("messages[{mi}].blocks[{bi}]"), &mut content)?;
        }
        if content.is_empty() {
            continue;
        }
        // Converse wants the roles to alternate, so a run of one role's messages goes as one.
        match messages.last_mut() {
            Some(last) if last.get("role").and_then(Value::as_str) == Some(role) => {
                if let Some(c) = last.get_mut("content").and_then(Value::as_array_mut) {
                    c.extend(content);
                }
            }
            _ => messages.push(json!({ "role": role, "content": content })),
        }
    }
    body.insert("messages".into(), Value::Array(messages));

    let s = &r.sampling;
    let mut inference = Map::new();
    if let Some(n) = s.max_output_tokens {
        inference.insert("maxTokens".into(), json!(n));
    }
    if let Some(t) = s.temperature {
        inference.insert("temperature".into(), json!(t));
    }
    if let Some(p) = s.top_p {
        inference.insert("topP".into(), json!(p));
    }
    if !s.stop.is_empty() {
        inference.insert("stopSequences".into(), json!(s.stop));
    }
    if !inference.is_empty() {
        body.insert("inferenceConfig".into(), Value::Object(inference));
    }

    if !r.tools.is_empty() {
        let mut tools = Vec::new();
        for t in &r.tools {
            let ToolDef::Function(f) = t else { continue };
            let schema = match &f.parameters {
                Some(p) => serde_json::from_str::<Value>(p.get()).unwrap_or_else(|_| json!({})),
                None => json!({ "type": "object", "properties": {} }),
            };
            let mut spec = json!({ "name": enc.name(&f.name), "inputSchema": { "json": schema } });
            if let Some(d) = f.description.as_deref().filter(|d| !d.is_empty()) {
                set(&mut spec, "description", json!(d));
            }
            if let Some(strict) = f.strict {
                set(&mut spec, "strict", json!(strict));
            }
            tools.push(json!({ "toolSpec": spec }));
            tools.extend(cache_point(f.cache.as_ref()));
        }
        let mut config = json!({ "tools": tools });
        match &r.tool_choice {
            Some(ToolChoice::Auto) => set(&mut config, "toolChoice", json!({ "auto": {} })),
            Some(ToolChoice::Required) => set(&mut config, "toolChoice", json!({ "any": {} })),
            Some(ToolChoice::Tool(name)) => {
                set(
                    &mut config,
                    "toolChoice",
                    json!({ "tool": { "name": enc.name(name) } }),
                );
            }
            _ => {}
        }
        body.insert("toolConfig".into(), config);
    }

    let mut headers = HeaderList::new();
    headers.push("content-type", "application/json");
    if stream {
        headers.push("accept", "application/vnd.amazon.eventstream");
    }
    let path = crate::codec::under_api_root(enc.ctx, &model_path(&r.model, stream));
    let body =
        serde_json::to_vec(&Value::Object(body)).map_err(|e| KppError::provider(e.to_string()))?;
    Ok(EncodedRequest::post_json(path, headers, body))
}
