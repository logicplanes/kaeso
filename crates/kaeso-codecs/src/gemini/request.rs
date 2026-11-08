use kaeso_kpp::{
    Block, Extras, FunctionTool, GenerateRequest, JsonSchemaSpec, KppError, Message, Origin,
    OutputSpec, RawItem, RawJson, ReasoningConfig, ReasoningEffort, ReasoningMode,
    ReasoningSummary, Request, Role, StreamMode, ToolChoice, ToolDef,
};
use serde_json::Value;
use serde_json::value::RawValue;

use super::D;
use super::parts::{Calls, DUMMY_SIGNATURE, PartDecoder, PartEncoder};
use super::path::GeminiAction;
use super::wire::{
    ContentW, CountTokensW, FunctionCallingConfigW, FunctionDeclarationW, GenerationConfigW, Raw,
    RequestW, ThinkingConfigW, ToolConfigW, ToolW,
};
use crate::codec::{EgressCtx, EncodedRequest, HeaderList};
use crate::cross::{Setting, app_field, check_foreign};
use crate::wire::{
    Ext, add_nested_extra, extra_for, extras_from, nested_extra_for, note_integers, spelled_number,
    to_body, verbatim_from_form, verbatim_into_form, without_position,
};

const OBJECTS: &str = "objects";
const GENERATION_CONFIG: &str = "generationConfig";
const TOOL_CONFIG: &str = "toolConfig";
const CALLING_CONFIG: &str = "toolConfig.functionCallingConfig";
const SYSTEM: &str = "systemInstruction";
const FORM_ROLE: &str = "role";
const FORM_CONTENT: &str = "content";
const FORM_GROUP: &str = "group";
const FORM_SCHEMA: &str = "schema";
const FORM_MODEL: &str = "model";
const FORM_COUNT: &str = "count";
const FORM_LEVEL: &str = "level";
const FORM_INCLUDE: &str = "include_thoughts";
const FORM_CANDIDATES: &str = "candidate_count";
const FORM_STOP: &str = "stop";
const FORM_MIME: &str = "response_mime_type";
const FORM_SDK_NAMES: &str = "sdk_names";
const FORM_TOP_K_FLOAT: &str = "top_k_float";
const JSON_MIME: &str = "application/json";
/// Gemini takes at most five stop sequences (`GenerationConfig.stopSequences`).
const MAX_STOP: usize = 5;

fn invalid(detail: impl std::fmt::Display) -> KppError {
    KppError::invalid_request(format!(
        "The request body is not a valid Gemini API request: {detail}."
    ))
}

fn parse<T: serde::de::DeserializeOwned + serde::Serialize>(
    body: &[u8],
) -> Result<Ext<T>, KppError> {
    let raw: &RawValue =
        serde_json::from_slice(body).map_err(|_| invalid("the body is not valid JSON"))?;
    if !raw.get().starts_with('{') {
        return Err(invalid("the body must be a JSON object"));
    }
    Ext::from_raw(raw).map_err(|e| {
        let text = without_position(&e);
        let err = invalid(&text);
        match [
            "contents",
            "systemInstruction",
            "tools",
            "toolConfig",
            "generationConfig",
            "generateContentRequest",
            "role",
            "parts",
        ]
        .into_iter()
        .find(|f| text.contains(&format!("`{f}`")))
        {
            Some(f) => err.with_param(f),
            None => err,
        }
    })
}

/// Decodes a Gemini request body for `action` on `model`, as the path names them.
pub(crate) fn decode(
    model: &str,
    action: GeminiAction,
    body: &[u8],
    _headers: &HeaderList,
) -> Result<Request, KppError> {
    let origin = Origin::new(D);
    let model = model.strip_prefix("models/").unwrap_or(model);
    let (body, sdk_names) = super::sdk::to_camel(body);
    let mut req = decode_action(model, action, &body, &origin)?;
    if sdk_names && let Request::Generate(r) | Request::CountTokens(r) = &mut req {
        r.extras.form.insert(FORM_SDK_NAMES.into(), true.into());
    }
    Ok(req)
}

fn decode_action(
    model: &str,
    action: GeminiAction,
    body: &[u8],
    origin: &Origin,
) -> Result<Request, KppError> {
    let origin = origin.clone();
    match action {
        GeminiAction::GenerateContent | GeminiAction::StreamGenerateContent => {
            let w = parse::<RequestW>(body)?;
            let mut req = decode_generate(w, model, &origin)?;
            if action == GeminiAction::StreamGenerateContent {
                req.stream = StreamMode::on(true);
            }
            Ok(Request::Generate(req))
        }
        GeminiAction::CountTokens => {
            let Ext {
                known,
                extra,
                verbatim,
            } = parse::<CountTokensW>(body)?;
            let mut req = match known.generate_content_request {
                Some(inner) => {
                    let mut req = decode_generate(inner, model, &origin)?;
                    req.extras.form.insert(FORM_COUNT.into(), "wrapped".into());
                    if !known.contents.is_empty() {
                        return Err(invalid(
                            "send either `contents` or `generateContentRequest`, not both",
                        )
                        .with_param("contents"));
                    }
                    req
                }
                None => {
                    let mut req = GenerateRequest::new(model);
                    req.extras = Extras::new(origin.clone());
                    decode_contents(known.contents, &origin, &mut req)?;
                    req
                }
            };
            for (k, v) in extra {
                req.extras.fields.insert(format!("count:{k}"), v);
            }
            verbatim_into_form(&mut req.extras, "count", verbatim);
            Ok(Request::CountTokens(req))
        }
        GeminiAction::EmbedContent | GeminiAction::BatchEmbedContents => {
            Err(KppError::unsupported_by(
                format!("`{}`", action.as_str()),
                D,
                "Send embedding requests to the Gemini embeddings codec.",
            ))
        }
    }
}

fn decode_contents(
    contents: Vec<Ext<ContentW>>,
    origin: &Origin,
    req: &mut GenerateRequest,
) -> Result<(), KppError> {
    let mut calls = Calls::default();
    for (i, c) in contents.into_iter().enumerate() {
        let raw = serde_json::value::to_raw_value(&c).map_err(invalid)?;
        let Ext {
            known,
            extra,
            verbatim,
        } = c;
        let mut extras = extras_from(origin, extra, verbatim);
        let role = match known.role.as_deref() {
            Some("user") => Role::User,
            Some("model") => Role::Assistant,
            None => {
                extras.form.insert(FORM_ROLE.into(), "absent".into());
                Role::User
            }
            Some(_) => {
                let mut m = Message::new(
                    Role::User,
                    vec![Block::Unknown(RawItem::new(
                        origin.clone(),
                        RawJson::from(raw),
                    ))],
                );
                let mut e = Extras::new(origin.clone());
                e.form.insert(FORM_CONTENT.into(), "raw".into());
                m.extras = e;
                req.messages.push(m);
                continue;
            }
        };
        let mut dec = PartDecoder {
            origin,
            calls: &mut calls,
            id_seed: i.to_string(),
            generated: 0,
        };
        let mut blocks = Vec::new();
        for (j, part) in known.parts.iter().enumerate() {
            dec.id_seed = format!("{i}_{j}");
            dec.generated = 0;
            dec.part(part, &mut blocks)
                .map_err(|e| invalid(e).with_param(format!("contents[{i}].parts[{j}]")))?;
        }
        let mut m = Message::new(role, blocks);
        m.extras = extras;
        req.messages.push(m);
    }
    Ok(())
}

fn decode_generate(
    w: Ext<RequestW>,
    model: &str,
    origin: &Origin,
) -> Result<GenerateRequest, KppError> {
    let Ext {
        known: w,
        extra,
        verbatim,
    } = w;
    let mut req = GenerateRequest::new(model);
    req.extras = extras_from(origin, extra, verbatim);
    if let Some(m) = w.model {
        req.extras.form.insert(FORM_MODEL.into(), m.into());
    }
    let mut objects = Vec::new();
    if let Some(sys) = w.system_instruction {
        let Ext {
            known,
            extra,
            verbatim,
        } = sys;
        add_nested_extra(&mut req.extras, SYSTEM, extra);
        verbatim_into_form(&mut req.extras, SYSTEM, verbatim);
        if let Some(role) = known.role {
            req.extras
                .form
                .insert(format!("{SYSTEM}.role"), role.into());
        }
        let mut calls = Calls::default();
        let mut dec = PartDecoder {
            origin,
            calls: &mut calls,
            id_seed: "s".into(),
            generated: 0,
        };
        for (j, part) in known.parts.iter().enumerate() {
            dec.part(part, &mut req.system)
                .map_err(|e| invalid(e).with_param(format!("systemInstruction.parts[{j}]")))?;
        }
        objects.push(Value::from(SYSTEM));
    }
    decode_contents(w.contents, origin, &mut req)?;
    for (group, raw) in w.tools.iter().enumerate() {
        decode_tool(raw, group, origin, &mut req)?;
    }
    if let Some(tc) = w.tool_config {
        let Ext {
            known,
            extra,
            verbatim,
        } = tc;
        add_nested_extra(&mut req.extras, TOOL_CONFIG, extra);
        verbatim_into_form(&mut req.extras, TOOL_CONFIG, verbatim);
        if let Some(fc) = known.function_calling_config {
            decode_calling(fc, origin, &mut req)?;
        }
        objects.push(Value::from(TOOL_CONFIG));
    }
    if let Some(gc) = w.generation_config {
        decode_generation(gc, origin, &mut req)?;
        objects.push(Value::from(GENERATION_CONFIG));
    }
    if !objects.is_empty() {
        req.extras
            .form
            .insert(OBJECTS.into(), Value::Array(objects));
    }
    Ok(req)
}

fn decode_tool(
    raw: &Raw,
    group: usize,
    origin: &Origin,
    req: &mut GenerateRequest,
) -> Result<(), KppError> {
    let unknown = || ToolDef::Unknown(RawItem::new(origin.clone(), RawJson::from(raw.as_ref())));
    let Ok(t) = Ext::<ToolW>::from_raw(raw) else {
        req.tools.push(unknown());
        return Ok(());
    };
    if !t.extra.is_empty() || !t.verbatim.is_empty() || t.function_declarations.is_empty() {
        req.tools.push(unknown());
        return Ok(());
    }
    for d in t.known.function_declarations {
        let Ext {
            known,
            extra,
            verbatim,
        } = d;
        let mut extras = extras_from(origin, extra, verbatim);
        let parameters = match (known.parameters_json_schema, known.parameters) {
            (Some(json), openapi) => {
                if let Some(p) = openapi {
                    extras.fields.insert("parameters".into(), RawJson::from(p));
                }
                Some(RawJson::from(json))
            }
            (None, Some(p)) => {
                extras.form.insert(FORM_SCHEMA.into(), "parameters".into());
                Some(RawJson::from(p))
            }
            (None, None) => None,
        };
        extras.form.insert(FORM_GROUP.into(), group.into());
        let mut f = FunctionTool::new(known.name, parameters);
        f.description = known.description;
        f.extras = extras;
        req.tools.push(ToolDef::Function(f));
    }
    Ok(())
}

fn decode_calling(
    fc: Ext<FunctionCallingConfigW>,
    origin: &Origin,
    req: &mut GenerateRequest,
) -> Result<(), KppError> {
    let raw = serde_json::value::to_raw_value(&fc).map_err(invalid)?;
    let Ext {
        known,
        extra,
        verbatim,
    } = fc;
    let names = known.allowed_function_names.as_deref();
    let choice = match (known.mode.as_deref(), names) {
        (Some("AUTO"), None) => Some(ToolChoice::Auto),
        (Some("NONE"), None) => Some(ToolChoice::None),
        (Some("ANY"), None) => Some(ToolChoice::Required),
        (Some("ANY"), Some([name])) => Some(ToolChoice::Tool(name.clone())),
        (None, None) => None,
        _ => {
            req.tool_choice = Some(ToolChoice::Unknown(RawItem::new(
                origin.clone(),
                RawJson::from(raw),
            )));
            req.extras
                .form
                .insert(CALLING_CONFIG.into(), "unknown".into());
            return Ok(());
        }
    };
    req.tool_choice = choice;
    add_nested_extra(&mut req.extras, CALLING_CONFIG, extra);
    verbatim_into_form(&mut req.extras, CALLING_CONFIG, verbatim);
    req.extras.form.insert(CALLING_CONFIG.into(), true.into());
    Ok(())
}

fn level_effort(level: &str) -> ReasoningEffort {
    ReasoningEffort::from_wire(&level.to_ascii_lowercase())
}

fn decode_generation(
    gc: Ext<GenerationConfigW>,
    origin: &Origin,
    req: &mut GenerateRequest,
) -> Result<(), KppError> {
    let Ext {
        known: g,
        extra,
        verbatim,
    } = gc;
    add_nested_extra(&mut req.extras, GENERATION_CONFIG, extra);
    verbatim_into_form(&mut req.extras, GENERATION_CONFIG, verbatim);
    if let Some(n) = g.candidate_count {
        if n > 1 {
            return Err(KppError::unsupported_by(
                "`generationConfig.candidateCount` above 1",
                D,
                "Kaeso returns one candidate per request; send several requests instead.",
            )
            .with_param("generationConfig.candidateCount"));
        }
        req.extras.form.insert(FORM_CANDIDATES.into(), n.into());
    }
    note_integers(
        &mut req.extras,
        &[
            ("temperature", g.temperature.as_ref()),
            ("topP", g.top_p.as_ref()),
            ("presencePenalty", g.presence_penalty.as_ref()),
            ("frequencyPenalty", g.frequency_penalty.as_ref()),
        ],
    );
    let s = &mut req.sampling;
    s.temperature = g.temperature.as_ref().and_then(serde_json::Number::as_f64);
    s.top_p = g.top_p.as_ref().and_then(serde_json::Number::as_f64);
    s.presence_penalty = g
        .presence_penalty
        .as_ref()
        .and_then(serde_json::Number::as_f64);
    s.frequency_penalty = g
        .frequency_penalty
        .as_ref()
        .and_then(serde_json::Number::as_f64);
    if let Some(k) = &g.top_k {
        s.top_k = Some(whole_top_k(k)?);
        if k.is_f64() {
            req.extras.form.insert(FORM_TOP_K_FLOAT.into(), true.into());
        }
    }
    s.max_output_tokens = g.max_output_tokens;
    s.seed = g.seed;
    if let Some(stop) = g.stop_sequences {
        if stop.is_empty() {
            req.extras.form.insert(FORM_STOP.into(), "empty".into());
        }
        req.sampling.stop = stop;
    }
    let schema_field = |schema: Raw, field: &str, extras: &mut Extras| {
        let mut spec = JsonSchemaSpec::new(RawJson::from(schema));
        if field == "responseSchema" {
            extras
                .form
                .insert(FORM_SCHEMA.into(), "responseSchema".into());
        }
        spec.extras = Extras::new(origin.clone());
        OutputSpec::JsonSchema(spec)
    };
    let json = g.response_mime_type.as_deref() == Some(JSON_MIME);
    match (json, g.response_json_schema, g.response_schema) {
        (true, Some(schema), other) => {
            if let Some(o) = other {
                req.extras.fields.insert(
                    format!("{GENERATION_CONFIG}.responseSchema"),
                    RawJson::from(o),
                );
            }
            req.output = schema_field(schema, "responseJsonSchema", &mut req.extras);
        }
        (true, None, Some(schema)) => {
            req.output = schema_field(schema, "responseSchema", &mut req.extras);
        }
        (true, None, None) => req.output = OutputSpec::JsonObject,
        (false, json_schema, schema) => {
            if let Some(m) = &g.response_mime_type {
                req.extras.fields.insert(
                    format!("{GENERATION_CONFIG}.responseMimeType"),
                    RawJson::from_serialize(m).map_err(invalid)?,
                );
            }
            if let Some(s) = json_schema {
                req.extras.fields.insert(
                    format!("{GENERATION_CONFIG}.responseJsonSchema"),
                    RawJson::from(s),
                );
            }
            if let Some(s) = schema {
                req.extras.fields.insert(
                    format!("{GENERATION_CONFIG}.responseSchema"),
                    RawJson::from(s),
                );
            }
        }
    }
    if json {
        req.extras.form.insert(FORM_MIME.into(), JSON_MIME.into());
    }
    if let Some(tc) = g.thinking_config {
        let Ext {
            known: t,
            extra,
            verbatim,
        } = tc;
        let mut c = ReasoningConfig::default();
        c.extras = extras_from(origin, extra, verbatim);
        if let Some(level) = &t.thinking_level {
            let effort = level_effort(level);
            if effort.as_wire().to_ascii_uppercase() != *level {
                c.extras
                    .form
                    .insert(FORM_LEVEL.into(), level.clone().into());
            }
            c.effort = Some(effort);
        }
        match t.thinking_budget {
            None => {}
            Some(0) => c.mode = Some(ReasoningMode::Disabled),
            Some(-1) => c.mode = Some(ReasoningMode::Adaptive),
            Some(n) => match u32::try_from(n) {
                Ok(n) => {
                    c.mode = Some(ReasoningMode::Enabled);
                    c.budget_tokens = Some(n);
                }
                Err(_) => {
                    c.extras.fields.insert(
                        "thinkingBudget".into(),
                        RawJson::from_serialize(&n).map_err(invalid)?,
                    );
                }
            },
        }
        match t.include_thoughts {
            Some(true) => c.summary = Some(ReasoningSummary::Auto),
            Some(false) => {
                c.extras.form.insert(FORM_INCLUDE.into(), false.into());
            }
            None => {}
        }
        req.reasoning = Some(c);
    }
    Ok(())
}

pub(crate) fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
    let target = ctx.origin(D);
    check_foreign(req, &target)?;
    let (Request::Generate(r) | Request::CountTokens(r)) = req else {
        return Err(KppError::unsupported_by(
            "embedding request",
            D,
            "Send embedding requests to the Gemini embeddings codec.",
        ));
    };
    if r.extras.is_from(D) {
        return Ok(());
    }
    let generate = matches!(req, Request::Generate(_));
    if generate && r.sampling.stop.len() > MAX_STOP {
        let field = app_field(r, Setting::StopSequences, D);
        return Err(KppError::unsupported_by(
            format!("more than {MAX_STOP} stop sequences"),
            D,
            format!("Send at most {MAX_STOP} values in `{field}`."),
        )
        .with_param(field));
    }
    if generate && r.parallel_tool_calls == Some(false) {
        return Err(KppError::unsupported_by(
            "`parallel_tool_calls: false`",
            D,
            "Gemini can't be told to call one function at a time; remove `parallel_tool_calls`.",
        )
        .with_param("parallel_tool_calls"));
    }
    if generate && let OutputSpec::JsonSchema(spec) = &r.output {
        let param = app_field(r, Setting::OutputFormat, D);
        if spec.description.is_some() {
            return Err(KppError::unsupported_by(
                "output schema description",
                D,
                "Gemini structured output takes a schema only. Move the description into the prompt or the schema's own `description`.",
            )
            .with_param(param));
        }
        if spec.strict == Some(false) {
            return Err(KppError::unsupported_by(
                "non-strict output schema",
                D,
                "Gemini always enforces the output schema. Set `strict: true` or remove it.",
            )
            .with_param(param));
        }
    }
    if generate && let Some(c) = &r.reasoning {
        if let Some(ReasoningEffort::Other(e)) = &c.effort {
            let field = app_field(r, Setting::ReasoningEffort, D);
            return Err(KppError::unsupported_by(
                format!("reasoning effort `{e}`"),
                D,
                "Gemini thinking levels are minimal, low, medium and high.",
            )
            .with_param(field));
        }
        if let Some(ReasoningMode::Other(m)) = &c.mode {
            let field = app_field(r, Setting::Reasoning, D);
            return Err(KppError::unsupported_by(
                format!("thinking mode `{m}`"),
                D,
                "Gemini turns thinking on with a level or a budget, and off with a budget of 0.",
            )
            .with_param(field));
        }
    }
    for (i, b) in r.system.iter().enumerate() {
        if !matches!(b, Block::Text(_)) {
            return Err(KppError::unsupported_by(
                format!("{} block in the system prompt", b.kind()),
                D,
                "The Gemini system instruction holds text only.",
            )
            .with_param(format!("system[{i}]")));
        }
    }
    for (mi, m) in r.messages.iter().enumerate() {
        if m.role == Role::System {
            return Err(KppError::unsupported_by(
                "system message",
                D,
                "Gemini takes system text only in `systemInstruction`.",
            )
            .with_param(format!("messages[{mi}]")));
        }
        for (bi, b) in m.blocks.iter().enumerate() {
            if matches!(b, Block::Refusal(_)) {
                return Err(KppError::unsupported_by(
                    "refusal block",
                    D,
                    "Gemini has no refusal part. Remove the refusal from the conversation history or replace it with text.",
                )
                .with_param(format!("messages[{mi}].blocks[{bi}]")));
            }
            if let Block::Document(d) = b
                && (d.title.is_some() || d.context.is_some() || d.citations)
            {
                return Err(KppError::unsupported_by(
                    "document title, context or citations",
                    D,
                    "Gemini takes a document's content only.",
                )
                .with_param(format!("messages[{mi}].blocks[{bi}]")));
            }
        }
    }
    Ok(())
}

fn action_for(req: &Request) -> Option<GeminiAction> {
    match req {
        Request::Generate(r) if r.stream.is_off() => Some(GeminiAction::GenerateContent),
        Request::Generate(_) => Some(GeminiAction::StreamGenerateContent),
        Request::CountTokens(_) => Some(GeminiAction::CountTokens),
        _ => None,
    }
}

pub(crate) fn encode(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
    let Some(action) = action_for(req) else {
        return Err(KppError::unsupported_by(
            "embedding request",
            D,
            "Send embedding requests to the Gemini embeddings codec.",
        ));
    };
    let (Request::Generate(r) | Request::CountTokens(r)) = req else {
        return Err(KppError::unsupported_by("request", D, ""));
    };
    let path = ctx.gemini_profile.path(&r.model, action)?;
    let target = ctx.origin(D);
    let body = if action == GeminiAction::CountTokens {
        let own = r.extras.is_from(D);
        let wrapped = if own {
            r.extras.form_str(D, FORM_COUNT) == Some("wrapped")
        } else {
            !r.system.is_empty() || !r.tools.is_empty() || r.tool_choice.is_some()
        };
        let mut w = CountTokensW::default();
        if wrapped {
            let mut inner = encode_generate(r, ctx, &target, own)?;
            if !own {
                inner.known.model = Some(format!("models/{}", model_id(&r.model)));
                inner.known.generation_config = None;
            }
            w.generate_content_request = Some(inner);
        } else {
            w.contents = encode_contents(r, ctx, &target)?;
        }
        let extra = if own {
            r.extras
                .body_fields()
                .filter_map(|(k, v)| Some((k.strip_prefix("count:")?.to_owned(), v.clone())))
                .collect()
        } else {
            Default::default()
        };
        to_body(&Ext::with(
            w,
            extra,
            verbatim_from_form(&r.extras, D, "count"),
        ))?
    } else {
        to_body(&encode_generate(r, ctx, &target, true)?)?
    };
    let body = if r.extras.is_from(D) && r.extras.form_flag(D, FORM_SDK_NAMES) {
        super::sdk::to_snake(body)
    } else {
        body
    };
    Ok(EncodedRequest::post_json(
        path,
        HeaderList::new().with("content-type", "application/json"),
        body,
    ))
}

// google-genai types `topK` as a float and sends `20.0`; the API takes a whole number.
fn whole_top_k(k: &serde_json::Number) -> Result<u32, KppError> {
    k.as_u64()
        .or_else(|| {
            k.as_f64()
                .filter(|f| f.fract() == 0.0 && *f >= 0.0)
                .map(|f| f as u64)
        })
        .and_then(|k| u32::try_from(k).ok())
        .ok_or_else(|| {
            invalid(format!("`topK` must be a whole number, not `{k}`"))
                .with_param("generationConfig.topK")
        })
}

fn model_id(model: &str) -> &str {
    model.strip_prefix("models/").unwrap_or(model)
}

fn top_extras(r: &GenerateRequest) -> indexmap::IndexMap<String, RawJson> {
    extra_for(&r.extras, D)
        .into_iter()
        .filter(|(k, _)| !k.starts_with("count:"))
        .collect()
}

fn encode_generate(
    r: &GenerateRequest,
    ctx: &mut EgressCtx,
    target: &Origin,
    with_extras: bool,
) -> Result<Ext<RequestW>, KppError> {
    let own = r.extras.is_from(D);
    let objects: Vec<&str> = r
        .extras
        .form_value(D, OBJECTS)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let present = |name: &str| {
        objects.contains(&name)
            || !nested_extra_for(&r.extras, D, name).is_empty()
            || !verbatim_from_form(&r.extras, D, name).is_empty()
    };
    let mut w = RequestW {
        model: r.extras.form_str(D, FORM_MODEL).map(str::to_owned),
        ..RequestW::default()
    };
    if !r.system.is_empty() || present(SYSTEM) {
        let mut enc = PartEncoder::new(target);
        for (i, b) in r.system.iter().enumerate() {
            enc.block(b, &format!("system[{i}]"))?;
        }
        let parts = enc
            .finish()
            .into_iter()
            .map(|v| serde_json::value::to_raw_value(&v))
            .collect::<Result<Vec<_>, _>>()
            .map_err(ser)?;
        let content = ContentW {
            role: r
                .extras
                .form_str(D, &format!("{SYSTEM}.role"))
                .map(str::to_owned),
            parts,
        };
        w.system_instruction = Some(Ext::with(
            content,
            nested_extra_for(&r.extras, D, SYSTEM),
            verbatim_from_form(&r.extras, D, SYSTEM),
        ));
    }
    w.contents = encode_contents(r, ctx, target)?;
    w.tools = encode_tools(r)?;
    w.tool_config = encode_tool_config(r, own)?;
    w.generation_config = encode_generation(r, own, &present)?;
    let (extra, verbatim) = if with_extras {
        (top_extras(r), verbatim_from_form(&r.extras, D, ""))
    } else {
        Default::default()
    };
    Ok(Ext::with(w, extra, verbatim))
}

fn ser(e: serde_json::Error) -> KppError {
    KppError::provider(format!(
        "Kaeso could not write a Gemini request ({e}). This is a bug in Kaeso; please report it."
    ))
}

fn only_results(m: &Message) -> bool {
    m.role == Role::User
        && !m.blocks.is_empty()
        && m.blocks.iter().all(|b| matches!(b, Block::ToolResult(_)))
}

// The current turn starts after the last user content that holds more than function
// responses (thought signatures guide: "a user turn that only contains a functionResponse
// is still the same turn").
fn current_turn_start(r: &GenerateRequest) -> usize {
    r.messages
        .iter()
        .rposition(|m| m.role == Role::User && !only_results(m))
        .map_or(0, |i| i + 1)
}

fn encode_contents(
    r: &GenerateRequest,
    ctx: &mut EgressCtx,
    target: &Origin,
) -> Result<Vec<Ext<ContentW>>, KppError> {
    let mut names: Vec<(String, String)> = Vec::new();
    let turn = current_turn_start(r);
    let mut out: Vec<Ext<ContentW>> = Vec::with_capacity(r.messages.len());
    let mut last_foreign_results = false;
    for (mi, m) in r.messages.iter().enumerate() {
        let own = m.extras.is_from(D);
        if own
            && m.extras.form_str(D, FORM_CONTENT) == Some("raw")
            && let [Block::Unknown(item)] = m.blocks.as_slice()
        {
            let content: Ext<ContentW> = Ext::from_raw(item.raw.as_raw()).map_err(ser)?;
            out.push(content);
            last_foreign_results = false;
            continue;
        }
        let role = match m.role {
            Role::User => "user",
            Role::Assistant => "model",
            _ => {
                return Err(KppError::unsupported_by(
                    "system message",
                    D,
                    "Gemini takes system text only in `systemInstruction`.",
                )
                .with_param(format!("messages[{mi}]")));
            }
        };
        let mut enc = PartEncoder::new(target);
        let mut first_call = true;
        for (bi, b) in m.blocks.iter().enumerate() {
            let path = format!("messages[{mi}].blocks[{bi}]");
            match b {
                Block::ToolCall(c) => {
                    names.push((c.id.clone(), c.name.clone()));
                    let dummy = (m.role == Role::Assistant
                        && mi >= turn
                        && first_call
                        && !c.extras.is_from(D)
                        && !enc.has_pending())
                    .then_some(DUMMY_SIGNATURE);
                    if dummy.is_some() {
                        ctx.adapted
                            .push(format!("thought signature of tool call {}", c.id));
                    }
                    first_call = false;
                    enc.call(c, dummy)?;
                }
                Block::ToolResult(res) => {
                    let name = names
                        .iter()
                        .rev()
                        .find(|(id, _)| *id == res.call_id)
                        .map(|(_, n)| n.clone());
                    enc.result(res, name.as_deref(), &path)?;
                }
                other => enc.block(other, &path)?,
            }
        }
        let parts = enc
            .finish()
            .into_iter()
            .map(|v| serde_json::value::to_raw_value(&v))
            .collect::<Result<Vec<_>, _>>()
            .map_err(ser)?;
        let foreign_results = !own && only_results(m);
        if foreign_results
            && last_foreign_results
            && let Some(prev) = out.last_mut()
        {
            prev.known.parts.extend(parts);
            continue;
        }
        last_foreign_results = foreign_results;
        let role = if own && m.extras.form_str(D, FORM_ROLE) == Some("absent") {
            ctx.gemini_profile
                .role_on_every_content
                .then(|| role.to_owned())
        } else {
            Some(role.to_owned())
        };
        let content = ContentW { role, parts };
        out.push(if own {
            Ext::with(
                content,
                extra_for(&m.extras, D),
                verbatim_from_form(&m.extras, D, ""),
            )
        } else {
            Ext::new(content)
        });
    }
    Ok(out)
}

fn encode_tools(r: &GenerateRequest) -> Result<Vec<Raw>, KppError> {
    let mut out = Vec::new();
    let mut group: Option<(Option<u64>, Vec<Ext<FunctionDeclarationW>>)> = None;
    let flush = |group: &mut Option<(Option<u64>, Vec<Ext<FunctionDeclarationW>>)>,
                 out: &mut Vec<Raw>|
     -> Result<(), KppError> {
        if let Some((_, decls)) = group.take() {
            let tool = ToolW {
                function_declarations: decls,
            };
            out.push(serde_json::value::to_raw_value(&tool).map_err(ser)?);
        }
        Ok(())
    };
    for t in &r.tools {
        match t {
            ToolDef::Function(f) => {
                let own = f.extras.is_from(D);
                let id = if own {
                    f.extras.form_value(D, FORM_GROUP).and_then(Value::as_u64)
                } else {
                    None
                };
                let openapi = own && f.extras.form_str(D, FORM_SCHEMA) == Some("parameters");
                let schema = f.parameters.clone().map(RawJson::into_inner);
                let decl = FunctionDeclarationW {
                    name: f.name.clone(),
                    description: f.description.clone(),
                    parameters: if openapi { schema.clone() } else { None },
                    parameters_json_schema: if openapi { None } else { schema },
                };
                let ext = if own {
                    Ext::with(
                        decl,
                        extra_for(&f.extras, D),
                        verbatim_from_form(&f.extras, D, ""),
                    )
                } else {
                    Ext::new(decl)
                };
                match &mut group {
                    Some((g, decls)) if *g == id => decls.push(ext),
                    _ => {
                        flush(&mut group, &mut out)?;
                        group = Some((id, vec![ext]));
                    }
                }
            }
            ToolDef::Unknown(item) => {
                flush(&mut group, &mut out)?;
                out.push(item.raw.clone().into_inner());
            }
            _ => {
                return Err(KppError::unsupported_by("tool", D, ""));
            }
        }
    }
    flush(&mut group, &mut out)?;
    Ok(out)
}

fn strict_tools(r: &GenerateRequest) -> bool {
    r.tools
        .iter()
        .any(|t| matches!(t, ToolDef::Function(f) if f.strict == Some(true)))
}

fn encode_tool_config(
    r: &GenerateRequest,
    own: bool,
) -> Result<Option<Ext<ToolConfigW>>, KppError> {
    let marker = if own {
        r.extras.form_value(D, CALLING_CONFIG)
    } else {
        None
    };
    let calling = match &r.tool_choice {
        Some(ToolChoice::Unknown(item)) if item.origin.is(D) => {
            Some(Ext::from_raw(item.raw.as_raw()).map_err(ser)?)
        }
        choice => {
            let (mode, names) = match choice {
                Some(ToolChoice::Auto) if !own && strict_tools(r) => (Some("VALIDATED"), None),
                Some(ToolChoice::Auto) => (Some("AUTO"), None),
                Some(ToolChoice::None) => (Some("NONE"), None),
                Some(ToolChoice::Required) => (Some("ANY"), None),
                Some(ToolChoice::Tool(n)) => (Some("ANY"), Some(vec![n.clone()])),
                None if !own && strict_tools(r) => (Some("VALIDATED"), None),
                _ => (None, None),
            };
            let w = FunctionCallingConfigW {
                mode: mode.map(str::to_owned),
                allowed_function_names: names,
            };
            if mode.is_some() || marker.is_some() {
                Some(Ext::with(
                    w,
                    nested_extra_for(&r.extras, D, CALLING_CONFIG),
                    verbatim_from_form(&r.extras, D, CALLING_CONFIG),
                ))
            } else {
                None
            }
        }
    };
    let objects = r
        .extras
        .form_value(D, OBJECTS)
        .and_then(Value::as_array)
        .is_some_and(|a| a.iter().any(|v| v == TOOL_CONFIG));
    let extra = nested_extra_for(&r.extras, D, TOOL_CONFIG);
    let verbatim = verbatim_from_form(&r.extras, D, TOOL_CONFIG);
    if calling.is_none() && !objects && extra.is_empty() && verbatim.is_empty() {
        return Ok(None);
    }
    Ok(Some(Ext::with(
        ToolConfigW {
            function_calling_config: calling,
        },
        extra,
        verbatim,
    )))
}

fn thinking_level(effort: &ReasoningEffort) -> Option<&'static str> {
    match effort {
        ReasoningEffort::Minimal => Some("MINIMAL"),
        ReasoningEffort::Low => Some("LOW"),
        ReasoningEffort::Medium => Some("MEDIUM"),
        ReasoningEffort::High | ReasoningEffort::XHigh | ReasoningEffort::Max => Some("HIGH"),
        _ => None,
    }
}

fn wants_thoughts(r: &GenerateRequest, c: &ReasoningConfig) -> bool {
    if c.summary.is_some() {
        return true;
    }
    let anthropic = c.extras.is_from(kaeso_kpp::DialectId::AnthropicMessages);
    let on = matches!(
        c.mode,
        Some(ReasoningMode::Enabled | ReasoningMode::Adaptive)
    );
    let omitted = c
        .extras
        .fields_for(kaeso_kpp::DialectId::AnthropicMessages)
        .and_then(|f| f.get("display"))
        .is_some_and(|d| d.get() == "\"omitted\"");
    anthropic && on && !omitted && r.extras.is_from(kaeso_kpp::DialectId::AnthropicMessages)
}

fn encode_thinking(r: &GenerateRequest, c: &ReasoningConfig) -> Option<Ext<ThinkingConfigW>> {
    if c.extras.is_from(D) {
        let level = c.effort.as_ref().map(|e| {
            c.extras
                .form_str(D, FORM_LEVEL)
                .map_or_else(|| e.as_wire().to_ascii_uppercase(), str::to_owned)
        });
        let budget = match (&c.mode, c.budget_tokens) {
            (Some(ReasoningMode::Disabled), _) => Some(0),
            (Some(ReasoningMode::Adaptive), _) => Some(-1),
            (_, Some(n)) => Some(i64::from(n)),
            _ => None,
        };
        let include = if c.summary.is_some() {
            Some(true)
        } else if c.extras.form_value(D, FORM_INCLUDE) == Some(&Value::Bool(false)) {
            Some(false)
        } else {
            None
        };
        return Some(Ext::with(
            ThinkingConfigW {
                include_thoughts: include,
                thinking_budget: budget,
                thinking_level: level,
            },
            extra_for(&c.extras, D),
            verbatim_from_form(&c.extras, D, ""),
        ));
    }
    let mut t = ThinkingConfigW::default();
    match (&c.effort, &c.mode, c.budget_tokens) {
        (Some(ReasoningEffort::None), _, _) | (_, Some(ReasoningMode::Disabled), _) => {
            t.thinking_budget = Some(0);
        }
        (Some(e), _, _) => t.thinking_level = thinking_level(e).map(str::to_owned),
        (None, _, Some(n)) => t.thinking_budget = Some(i64::from(n)),
        (None, Some(ReasoningMode::Adaptive | ReasoningMode::Enabled), None) => {
            t.thinking_budget = Some(-1);
        }
        (None, _, None) => {}
    }
    let off = t.thinking_budget == Some(0);
    if !off && wants_thoughts(r, c) {
        t.include_thoughts = Some(true);
    }
    if t.include_thoughts.is_none() && t.thinking_budget.is_none() && t.thinking_level.is_none() {
        return None;
    }
    Some(Ext::new(t))
}

fn encode_generation(
    r: &GenerateRequest,
    own: bool,
    present: &dyn Fn(&str) -> bool,
) -> Result<Option<Ext<GenerationConfigW>>, KppError> {
    let s = &r.sampling;
    let number = |v: Option<f64>, key: &str| spelled_number(v, key, &r.extras, D);
    let mut g = GenerationConfigW {
        temperature: number(s.temperature, "temperature"),
        top_p: number(s.top_p, "topP"),
        top_k: s.top_k.and_then(|k| {
            if own && r.extras.form_flag(D, FORM_TOP_K_FLOAT) {
                serde_json::Number::from_f64(f64::from(k))
            } else {
                Some(k.into())
            }
        }),
        max_output_tokens: s.max_output_tokens,
        stop_sequences: if s.stop.is_empty() && r.extras.form_str(D, FORM_STOP) != Some("empty") {
            None
        } else {
            Some(s.stop.clone())
        },
        seed: s.seed,
        presence_penalty: number(s.presence_penalty, "presencePenalty"),
        frequency_penalty: number(s.frequency_penalty, "frequencyPenalty"),
        candidate_count: r
            .extras
            .form_value(D, FORM_CANDIDATES)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok()),
        ..GenerationConfigW::default()
    };
    match &r.output {
        OutputSpec::JsonObject => g.response_mime_type = Some(JSON_MIME.into()),
        OutputSpec::JsonSchema(spec) => {
            g.response_mime_type = Some(JSON_MIME.into());
            let schema = spec.schema.clone().into_inner();
            if own && r.extras.form_str(D, FORM_SCHEMA) == Some("responseSchema") {
                g.response_schema = Some(schema);
            } else {
                g.response_json_schema = Some(schema);
            }
        }
        _ => {}
    }
    if own && r.extras.form_str(D, FORM_MIME) == Some(JSON_MIME) && g.response_mime_type.is_none() {
        g.response_mime_type = Some(JSON_MIME.into());
    }
    g.thinking_config = r.reasoning.as_ref().and_then(|c| encode_thinking(r, c));
    let empty = g.temperature.is_none()
        && g.top_p.is_none()
        && g.top_k.is_none()
        && g.max_output_tokens.is_none()
        && g.stop_sequences.is_none()
        && g.seed.is_none()
        && g.presence_penalty.is_none()
        && g.frequency_penalty.is_none()
        && g.candidate_count.is_none()
        && g.response_mime_type.is_none()
        && g.thinking_config.is_none();
    if empty && !present(GENERATION_CONFIG) {
        return Ok(None);
    }
    Ok(Some(Ext::with(
        g,
        nested_extra_for(&r.extras, D, GENERATION_CONFIG),
        verbatim_from_form(&r.extras, D, GENERATION_CONFIG),
    )))
}
