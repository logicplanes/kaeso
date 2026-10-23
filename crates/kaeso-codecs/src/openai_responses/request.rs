use kaeso_kpp::{
    Block, Extras, FunctionTool, GenerateRequest, JsonSchemaSpec, KppError, Origin, OutputSpec,
    RawItem, RawJson, ReasoningConfig, ReasoningEffort, ReasoningMode, ReasoningSummary, Request,
    RequestKind, Sampling, Source, StreamMode, ToolChoice, ToolDef,
};
use serde_json::value::RawValue;

use super::DIALECT;
use super::input::{
    DecodedInput, INSTRUCTIONS, SYSTEM_ITEMS, decode_items, encode_messages, encode_system,
    string_input, string_message,
};
use super::node::{Node, entries_of};
use super::overrides::{Overrides, apply};
use super::wire;
use crate::codec::{EgressCtx, EncodedRequest, HeaderList, is_forwarded_header};
use crate::cross::{Setting, app_field, check_foreign};
use crate::wire::{StringOr, to_body, type_tag};

pub(super) const INPUT: &str = "input";
const NATIVE: &str = "native";
const USER_KEY: &str = "user_key";
const NO_OBJECT: &str = "no_object";
const OBJECT: &str = "object";
const ENCRYPTED: &str = "reasoning.encrypted_content";

pub(crate) struct Names<'a> {
    ctx: Option<&'a mut EgressCtx>,
}

impl<'a> Names<'a> {
    pub(crate) fn none() -> Names<'a> {
        Names { ctx: None }
    }

    pub(crate) fn new(ctx: &'a mut EgressCtx) -> Names<'a> {
        Names { ctx: Some(ctx) }
    }

    fn model_reasons(&self) -> Option<bool> {
        self.ctx.as_ref().and_then(|c| c.reasoning)
    }

    pub(crate) fn tool_name(&mut self, name: &str) -> String {
        match self.ctx.as_deref_mut() {
            Some(ctx) => crate::wire::rewrite_name(name, valid_tool_name, 64, &mut ctx.tool_names),
            None => name.to_owned(),
        }
    }

    pub(crate) fn call_id(&mut self, id: &str) -> String {
        if id.len() <= 64 || self.ctx.is_none() {
            return id.to_owned();
        }
        let head: String = id.chars().take(55).collect();
        let sent = format!(
            "{head}_{:08x}",
            crate::wire::fnv1a(id.as_bytes()) & 0xffff_ffff
        );
        if let Some(ctx) = self.ctx.as_deref_mut() {
            ctx.tool_ids.insert(sent.clone(), id.to_owned());
        }
        sent
    }
}

fn valid_tool_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.chars().all(crate::wire::is_name_char)
}

const MIN_MAX_OUTPUT_TOKENS: u32 = 16;

fn invalid(e: impl std::fmt::Display) -> KppError {
    KppError::invalid_request(format!(
        "The request body is not a valid {} request: {e}.",
        DIALECT.display_name()
    ))
}

fn describe(raw: &RawValue, err: &serde_json::Error) -> KppError {
    type Check = fn(&serde_json::Value) -> bool;
    let string: Check = |v| v.is_string() || v.is_null();
    let int: Check = |v| v.is_null() || v.as_u64().is_some_and(|n| u32::try_from(n).is_ok());
    let number: Check = |v| v.is_null() || v.is_number();
    let boolean: Check = |v| v.is_null() || v.is_boolean();
    let strings: Check = |v| {
        v.is_null()
            || v.as_array()
                .is_some_and(|a| a.iter().all(serde_json::Value::is_string))
    };
    let array: Check = |v| v.is_null() || v.is_array();
    let object: Check = |v| v.is_null() || v.is_object();
    let input: Check = |v| v.is_null() || v.is_string() || v.is_array();
    let fields: &[(&str, Check, &str)] = &[
        ("model", string, "a string"),
        ("instructions", string, "a string"),
        ("user", string, "a string"),
        ("safety_identifier", string, "a string"),
        ("previous_response_id", string, "a string"),
        ("max_output_tokens", int, "a non-negative integer"),
        ("temperature", number, "a number"),
        ("top_p", number, "a number"),
        ("parallel_tool_calls", boolean, "true or false"),
        ("stream", boolean, "true or false"),
        ("store", boolean, "true or false"),
        ("background", boolean, "true or false"),
        ("include", strings, "an array of strings"),
        ("tools", array, "an array"),
        ("input", input, "a string or an array of items"),
        ("text", object, "an object"),
        ("reasoning", object, "an object"),
    ];
    for (key, value) in entries_of(raw).unwrap_or_default() {
        let Some((name, check, expected)) = fields.iter().find(|(n, _, _)| *n == key) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(value.get()) else {
            continue;
        };
        if !check(&v) {
            return invalid(format!("`{name}` must be {expected}")).with_param(*name);
        }
    }
    let text = err.to_string();
    let text = text.split(" at line ").next().unwrap_or(&text).to_owned();
    invalid(text)
}

fn stateful(feature: &str, hint: &str) -> KppError {
    KppError::unsupported_by(feature, DIALECT, hint).with_param(feature)
}

pub(super) fn decode(
    kind: RequestKind,
    body: &[u8],
    headers: &HeaderList,
) -> Result<Request, KppError> {
    if kind == RequestKind::Embed {
        return Err(KppError::unsupported_by(
            "embeddings",
            DIALECT,
            "Send embeddings requests to `/v1/embeddings`.",
        ));
    }
    let origin = Origin::new(DIALECT);
    let raw: &RawValue =
        serde_json::from_slice(body).map_err(|_| invalid("the body is not valid JSON"))?;
    if entries_of(raw).is_none() {
        return Err(invalid(
            "the body must be a JSON object without duplicate keys",
        ));
    }
    let w: wire::CreateResponse = serde_json::from_str(raw.get()).map_err(|e| describe(raw, &e))?;
    let count = kind == RequestKind::CountTokens;
    if w.model.is_none() && !count {
        return Err(invalid("`model` is required").with_param("model"));
    }
    if !count
        && w.max_output_tokens
            .is_some_and(|m| m < MIN_MAX_OUTPUT_TOKENS)
    {
        return Err(invalid(format!(
            "`max_output_tokens` must be at least {MIN_MAX_OUTPUT_TOKENS}"
        ))
        .with_param("max_output_tokens"));
    }
    let stateless = "Kaeso is stateless: send the whole conversation in `input` instead.";
    if w.previous_response_id.is_some() {
        return Err(stateful("previous_response_id", stateless));
    }
    if w.conversation.is_some() {
        return Err(stateful("conversation", stateless));
    }
    if w.background == Some(true) {
        return Err(stateful(
            "background",
            "Kaeso does not run background responses: send the request without `background` (or with `background: false`).",
        ));
    }
    if w.store == Some(true) {
        return Err(stateful(
            "store",
            "Kaeso does not store responses: send `store: false` or leave `store` out.",
        ));
    }

    let mut req = GenerateRequest::new(w.model.clone().unwrap_or_default());
    req.extras = Extras::new(origin.clone()).with_form(NATIVE, true.into());

    match &w.input {
        None => {}
        Some(StringOr::String(s)) => {
            req.messages = vec![string_message(s.clone())];
            req.extras.form.insert(INPUT.to_owned(), "string".into());
        }
        Some(StringOr::List(items)) => {
            let DecodedInput {
                system,
                system_items,
                messages,
            } = decode_items(items, &origin);
            req.system = system;
            req.messages = messages;
            if !system_items.is_empty() {
                req.extras.form.insert(
                    SYSTEM_ITEMS.to_owned(),
                    serde_json::Value::Array(system_items),
                );
                if w.instructions.is_some() {
                    req.extras.form.insert(INSTRUCTIONS.to_owned(), true.into());
                }
            }
        }
    }
    if let Some(instructions) = &w.instructions {
        req.system.insert(0, Block::text(instructions.clone()));
    }

    req.tools = w
        .tools
        .iter()
        .flatten()
        .map(|t| decode_tool(t, &origin))
        .collect();
    req.tool_choice = w
        .tool_choice
        .as_deref()
        .map(|c| decode_tool_choice(c, &origin));
    req.parallel_tool_calls = w.parallel_tool_calls;
    req.output = decode_format(w.text.as_ref().and_then(|t| t.format.as_deref()), &origin);

    let mut reasoning = w.reasoning.as_ref().map(|r| {
        let mut cfg = ReasoningConfig::default();
        cfg.effort = r.effort.as_deref().map(ReasoningEffort::from_wire);
        cfg.summary = r.summary.as_deref().map(ReasoningSummary::from_wire);
        cfg.extras = Extras::new(origin.clone());
        if cfg.effort.is_none() && cfg.summary.is_none() {
            cfg.extras.form.insert(OBJECT.to_owned(), true.into());
        }
        cfg
    });
    if !count && w.include.iter().flatten().any(|i| i == ENCRYPTED) {
        let cfg = reasoning.get_or_insert_with(|| {
            let mut cfg = ReasoningConfig::default();
            cfg.extras = Extras::new(origin.clone()).with_form(NO_OBJECT, true.into());
            cfg
        });
        cfg.include_encrypted = true;
    }
    req.reasoning = reasoning;

    if !count {
        let mut sampling = Sampling::default();
        sampling.max_output_tokens = w.max_output_tokens;
        sampling.temperature = w.temperature;
        sampling.top_p = w.top_p;
        req.sampling = sampling;
        req.user = w.safety_identifier.clone().or_else(|| w.user.clone());
        if w.safety_identifier.is_none() && w.user.is_some() {
            req.extras.form.insert(USER_KEY.to_owned(), "user".into());
        }
        if w.stream == Some(true) {
            req.stream = StreamMode::on(true);
        }
    }

    let canon = canonical_body(&req, kind, &origin, &mut Names::none());
    let mut overrides = Overrides::diff_with(
        &canon,
        raw,
        &["input", "tools"],
        &["text", "format", "reasoning", "stream_options"],
    );
    if let OutputSpec::JsonSchema(spec) = &mut req.output {
        spec.extras = Extras::new(origin.clone());
        overrides
            .split_off("text.format")
            .record(&mut spec.extras, "");
    }
    if let Some(cfg) = &mut req.reasoning
        && !cfg.extras.form_flag(DIALECT, NO_OBJECT)
    {
        overrides.split_off("reasoning").record(&mut cfg.extras, "");
    }
    overrides.record(&mut req.extras, "");

    for (name, value) in headers.iter() {
        if is_forwarded_header(DIALECT, name) {
            req.extras.insert_header(name, value);
        }
    }

    Ok(match kind {
        RequestKind::CountTokens => Request::CountTokens(req),
        _ => Request::Generate(req),
    })
}

fn decode_tool(raw: &RawValue, origin: &Origin) -> ToolDef {
    let unknown = || ToolDef::Unknown(RawItem::new(origin.clone(), RawJson::from(raw)));
    if type_tag(raw).ok().flatten().as_deref() != Some("function") {
        return unknown();
    }
    let Ok(w) = serde_json::from_str::<wire::FunctionToolWire>(raw.get()) else {
        return unknown();
    };
    let mut tool = FunctionTool::new(w.name, w.parameters.as_deref().map(RawJson::from));
    tool.description = w.description;
    tool.strict = w.strict;
    tool.extras = Extras::new(origin.clone());
    let canon = function_tool_node(&tool, &mut Names::none());
    Overrides::diff(&canon, raw, &[]).record(&mut tool.extras, "");
    ToolDef::Function(tool)
}

fn function_tool_node(tool: &FunctionTool, names: &mut Names<'_>) -> Node {
    Node::obj()
        .with("type", Node::str("function"))
        .with("name", Node::str(names.tool_name(&tool.name)))
        .with_opt("description", tool.description.as_deref().map(Node::str))
        .with_opt("parameters", tool.parameters.as_ref().map(Node::raw))
        .with_opt("strict", tool.strict.map(Node::bool))
}

fn tool_node(tool: &ToolDef, names: &mut Names<'_>) -> Option<Node> {
    match tool {
        ToolDef::Function(f) => {
            let mut node = function_tool_node(f, names);
            apply(&f.extras, DIALECT, "", &[], &mut node);
            Some(node)
        }
        ToolDef::Unknown(item) if item.origin.is(DIALECT) => Some(Node::raw(&item.raw)),
        _ => None,
    }
}

fn decode_tool_choice(raw: &RawValue, origin: &Origin) -> ToolChoice {
    let unknown = || ToolChoice::Unknown(RawItem::new(origin.clone(), RawJson::from(raw)));
    if let Ok(s) = serde_json::from_str::<String>(raw.get()) {
        return match s.as_str() {
            "auto" => ToolChoice::Auto,
            "none" => ToolChoice::None,
            "required" => ToolChoice::Required,
            _ => unknown(),
        };
    }
    let only_type_and_name =
        entries_of(raw).is_some_and(|e| e.iter().all(|(k, _)| k == "type" || k == "name"));
    if only_type_and_name
        && type_tag(raw).ok().flatten().as_deref() == Some("function")
        && let Ok(f) = serde_json::from_str::<wire::ToolChoiceFunction>(raw.get())
    {
        return ToolChoice::Tool(f.name);
    }
    unknown()
}

fn tool_choice_node(choice: &ToolChoice, names: &mut Names<'_>) -> Option<Node> {
    Some(match choice {
        ToolChoice::Auto => Node::str("auto"),
        ToolChoice::None => Node::str("none"),
        ToolChoice::Required => Node::str("required"),
        ToolChoice::Tool(name) => Node::obj()
            .with("type", Node::str("function"))
            .with("name", Node::str(names.tool_name(name))),
        ToolChoice::Unknown(item) if item.origin.is(DIALECT) => Node::raw(&item.raw),
        _ => return None,
    })
}

fn decode_format(format: Option<&RawValue>, _origin: &Origin) -> OutputSpec {
    let Some(raw) = format else {
        return OutputSpec::Text;
    };
    match type_tag(raw).ok().flatten().as_deref() {
        Some("json_object") => OutputSpec::JsonObject,
        Some("json_schema") => match serde_json::from_str::<wire::JsonSchemaFormat>(raw.get()) {
            Ok(w) => {
                let mut spec = JsonSchemaSpec::new(RawJson::from(&*w.schema));
                spec.name = w.name;
                spec.description = w.description;
                spec.strict = w.strict;
                OutputSpec::JsonSchema(spec)
            }
            Err(_) => OutputSpec::Text,
        },
        _ => OutputSpec::Text,
    }
}

pub(super) fn format_node(output: &OutputSpec) -> Option<Node> {
    match output {
        OutputSpec::JsonObject => Some(Node::obj().with("type", Node::str("json_object"))),
        OutputSpec::JsonSchema(spec) => {
            let mut node = Node::obj()
                .with("type", Node::str("json_schema"))
                .with(
                    "name",
                    Node::str(spec.name.as_deref().unwrap_or("response")),
                )
                .with_opt("description", spec.description.as_deref().map(Node::str))
                .with("schema", Node::raw(&spec.schema))
                .with_opt("strict", spec.strict.map(Node::bool));
            apply(&spec.extras, DIALECT, "", &[], &mut node);
            Some(node)
        }
        _ => None,
    }
}

fn reasons(cfg: &ReasoningConfig) -> bool {
    crate::cross::reasoning_map::openai_effort(cfg)
        .is_some_and(|effort| effort != ReasoningEffort::None)
}

fn egress_reasoning(
    cfg: &ReasoningConfig,
    model_reasons: Option<bool>,
    count: bool,
) -> ReasoningConfig {
    let thinking_on = matches!(
        cfg.mode,
        Some(ReasoningMode::Adaptive | ReasoningMode::Enabled)
    );
    let on = reasons(cfg) || (thinking_on && model_reasons == Some(true));
    let mut out = cfg.clone();
    if on {
        out.include_encrypted = true;
    }
    let anthropic = cfg.extras.is_from(kaeso_kpp::DialectId::AnthropicMessages);
    let omitted = cfg
        .extras
        .fields
        .get("display")
        .is_some_and(|d| d.get().trim() == r#""omitted""#);
    if on
        && !count
        && anthropic
        && cfg.summary.is_none()
        && !omitted
        && cfg.effort != Some(ReasoningEffort::None)
    {
        out.summary = Some(ReasoningSummary::Auto);
    }
    out
}

fn reasoning_object(cfg: &ReasoningConfig) -> Option<Node> {
    if cfg.extras.form_flag(DIALECT, NO_OBJECT) {
        return None;
    }
    let effort = crate::cross::reasoning_map::openai_effort(cfg);
    if effort.is_none() && cfg.summary.is_none() && !cfg.extras.form_flag(DIALECT, OBJECT) {
        return None;
    }
    let mut node = Node::obj()
        .with_opt("effort", effort.as_ref().map(|e| Node::str(e.as_wire())))
        .with_opt(
            "summary",
            cfg.summary.as_ref().map(|s| Node::str(s.as_wire())),
        );
    apply(&cfg.extras, DIALECT, "", &[], &mut node);
    Some(node)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Input {
    Encoded,
    Left,
}

fn canonical_body(
    req: &GenerateRequest,
    kind: RequestKind,
    target: &Origin,
    names: &mut Names<'_>,
) -> Node {
    canonical_body_with(req, kind, target, names, Input::Encoded)
}

fn canonical_body_with(
    req: &GenerateRequest,
    kind: RequestKind,
    target: &Origin,
    names: &mut Names<'_>,
    input: Input,
) -> Node {
    let count = kind == RequestKind::CountTokens;
    let mut node = Node::obj();
    if !(count && req.model.is_empty()) {
        node.insert("model", Node::str(&req.model));
    }
    let (instructions, system_items) = encode_system(&req.system, &req.extras);
    if input == Input::Encoded {
        let as_string =
            req.extras.form_str(DIALECT, INPUT) == Some("string") && system_items.is_empty();
        match string_input(&req.messages).filter(|_| as_string) {
            Some(s) => node.insert("input", Node::str(s)),
            None => {
                let mut items = system_items;
                let stateless = !req.extras.form_flag(DIALECT, NATIVE);
                items.extend(encode_messages(&req.messages, target, stateless, names));
                node.insert("input", Node::Arr(items));
            }
        }
    }
    if let Some(i) = instructions {
        node.insert("instructions", Node::str(i));
    }
    let tools: Vec<Node> = req
        .tools
        .iter()
        .filter_map(|t| tool_node(t, names))
        .collect();
    if !tools.is_empty() {
        node.insert("tools", Node::Arr(tools));
    }
    if let Some(choice) = req
        .tool_choice
        .as_ref()
        .and_then(|c| tool_choice_node(c, names))
    {
        node.insert("tool_choice", choice);
    }
    if let Some(p) = req.parallel_tool_calls {
        node.insert("parallel_tool_calls", Node::bool(p));
    }
    if let Some(format) = format_node(&req.output) {
        node.insert("text", Node::obj().with("format", format));
    }
    if let Some(cfg) = &req.reasoning {
        let foreign;
        let cfg = if req.extras.form_flag(DIALECT, NATIVE) {
            cfg
        } else {
            foreign = egress_reasoning(cfg, names.model_reasons(), count);
            &foreign
        };
        if let Some(r) = reasoning_object(cfg) {
            node.insert("reasoning", r);
        }
        if cfg.include_encrypted && !count {
            node.insert("include", Node::Arr(vec![Node::str(ENCRYPTED)]));
        }
    }
    if count {
        return node;
    }
    let s = &req.sampling;
    if let Some(m) = s.max_output_tokens {
        node.insert("max_output_tokens", Node::int(u64::from(m)));
    }
    if let Some(t) = s.temperature {
        node.insert("temperature", Node::float(t));
    }
    if let Some(p) = s.top_p {
        node.insert("top_p", Node::float(p));
    }
    if let Some(user) = &req.user {
        let key = if req.extras.form_str(DIALECT, USER_KEY) == Some("user") {
            "user"
        } else {
            "safety_identifier"
        };
        node.insert(key, Node::str(user));
    }
    if matches!(req.stream, StreamMode::On { .. }) {
        node.insert("stream", Node::bool(true));
    }
    if !req.extras.form_flag(DIALECT, NATIVE) {
        node.insert("store", Node::bool(false));
    }
    node
}

pub(super) fn body_node(
    req: &GenerateRequest,
    kind: RequestKind,
    target: &Origin,
    names: &mut Names<'_>,
) -> Node {
    let mut node = canonical_body(req, kind, target, names);
    apply(&req.extras, DIALECT, "", &[], &mut node);
    node
}

pub(super) fn check(req: &Request, target: &Origin) -> Result<(), KppError> {
    check_rules(req)?;
    check_foreign(req, target)?;
    match req {
        Request::Generate(r) => check_output_limit(r),
        _ => Ok(()),
    }
}

fn check_rules(req: &Request) -> Result<(), KppError> {
    match req {
        Request::Generate(r) | Request::CountTokens(r) => check_generate(r),
        Request::Embed(_) => Err(KppError::unsupported_by(
            "embeddings",
            DIALECT,
            "Send embeddings requests through the OpenAI Embeddings format.",
        )),
        _ => Err(KppError::unsupported_by("request kind", DIALECT, "")),
    }
}

fn unsupported(feature: &str, hint: &str) -> KppError {
    KppError::unsupported_by(feature, DIALECT, hint)
}

fn check_output_limit(r: &GenerateRequest) -> Result<(), KppError> {
    match r.sampling.max_output_tokens {
        Some(m) if m < MIN_MAX_OUTPUT_TOKENS => {
            let field = app_field(r, Setting::OutputLimit, DIALECT);
            Err(unsupported(
                &format!("{field} {m}"),
                &format!(
                    "OpenAI Responses needs an output limit of at least {MIN_MAX_OUTPUT_TOKENS} tokens; raise it or route the request to another model."
                ),
            )
            .with_param(field))
        }
        _ => Ok(()),
    }
}

fn check_generate(r: &GenerateRequest) -> Result<(), KppError> {
    let s = &r.sampling;
    if s.top_k.is_some() {
        return Err(unsupported("top_k", "OpenAI models do not support top-k sampling. Remove `top_k` or route the request to an Anthropic model.").with_param("top_k"));
    }
    if s.seed.is_some() {
        return Err(unsupported(
            "seed",
            "The Responses API has no `seed`. Remove it or use the Chat Completions format.",
        )
        .with_param("seed"));
    }
    if s.frequency_penalty.is_some() {
        return Err(unsupported("frequency_penalty", "The Responses API has no `frequency_penalty`. Remove it or use the Chat Completions format.").with_param("frequency_penalty"));
    }
    if s.presence_penalty.is_some() {
        return Err(unsupported("presence_penalty", "The Responses API has no `presence_penalty`. Remove it or use the Chat Completions format.").with_param("presence_penalty"));
    }
    if !s.stop.is_empty() {
        return Err(unsupported("stop sequences", "The Responses API has no `stop` parameter. Remove the stop sequences or use the Chat Completions format.").with_param(app_field(r, Setting::StopSequences, DIALECT)));
    }
    if let Some(cfg) = &r.reasoning {
        match &cfg.mode {
            None
            | Some(ReasoningMode::Adaptive)
            | Some(ReasoningMode::Enabled)
            | Some(ReasoningMode::Disabled) => {}
            Some(other) => {
                let wire = other.as_wire();
                return Err(unsupported(
                    &format!("reasoning mode \"{wire}\""),
                    "Remove the reasoning mode or set a reasoning effort.",
                )
                .with_param(app_field(r, Setting::Reasoning, DIALECT)));
            }
        }
    }
    for (mi, m) in r.messages.iter().enumerate() {
        for (bi, b) in m.blocks.iter().enumerate() {
            check_block(b, &format!("messages[{mi}].blocks[{bi}]"))?;
        }
    }
    Ok(())
}

fn check_block(block: &Block, path: &str) -> Result<(), KppError> {
    match block {
        Block::ToolResult(res) => {
            for (i, b) in res.content.iter().enumerate() {
                check_block(b, &format!("{path}.content[{i}]"))?;
            }
            Ok(())
        }
        Block::Document(d) => {
            if matches!(d.source, Source::Text(_)) {
                return Err(unsupported(
                    "text document",
                    "Send the text as a text block, or as a PDF.",
                )
                .with_param(path));
            }
            if d.title.is_some() {
                return Err(
                    unsupported("document title", "Remove the document title.").with_param(path)
                );
            }
            if d.context.is_some() {
                return Err(unsupported(
                    "document context",
                    "Remove the document context or put it in a text block.",
                )
                .with_param(path));
            }
            if d.citations {
                return Err(unsupported("document citations", "OpenAI cannot cite from input documents. Turn citations off for this document.").with_param(path));
            }
            Ok(())
        }
        Block::Image(i) if matches!(i.source, Source::Text(_)) => Err(unsupported(
            "text image source",
            "Send the image as base64, a URL or a file id.",
        )
        .with_param(path)),
        _ => Ok(()),
    }
}

pub(super) fn encode(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    let target = ctx.origin(DIALECT);
    check(req, &target)?;
    let (r, path) = match req {
        Request::Generate(r) => (r, super::RESPONSES_PATH),
        Request::CountTokens(r) => (r, super::INPUT_TOKENS_PATH),
        _ => return Err(KppError::unsupported_by("request kind", DIALECT, "")),
    };
    let body = body_node(r, req.kind(), &target, &mut Names::new(ctx));
    let mut headers = HeaderList::new().with("content-type", "application/json");
    if r.extras.is_from(DIALECT) {
        for (name, value) in r.extras.headers() {
            if is_forwarded_header(DIALECT, name) {
                headers.push(name, value);
            }
        }
    }
    Ok(EncodedRequest::post_json(path, headers, to_body(&body)?))
}

pub(super) fn echo_body(req: &Request) -> Option<Node> {
    match req {
        Request::Generate(r) | Request::CountTokens(r) => {
            let mut node = canonical_body_with(
                r,
                RequestKind::Generate,
                &Origin::new(DIALECT),
                &mut Names::none(),
                Input::Left,
            );
            apply(&r.extras, DIALECT, "", &[], &mut node);
            Some(node)
        }
        _ => None,
    }
}
