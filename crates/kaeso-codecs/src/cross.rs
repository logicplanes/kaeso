use kaeso_kpp::{
    Block, DialectId, Extras, GenerateRequest, HEADER_PREFIX, KppError, Opaque, OpaqueKind, Origin,
    RawJson, ReasoningBlock, ReasoningEffort, Request, Role, Source, ToolChoice, ToolDef,
};
use serde_json::Value;

use crate::wire::type_tag;

pub type Scope = &'static str;

pub const EXTRAS_ALLOWLIST: &[(DialectId, Scope, &str)] = &[
    (
        DialectId::AnthropicMessages,
        "request",
        "header:anthropic-beta",
    ),
    (
        DialectId::AnthropicMessages,
        "request",
        "header:anthropic-version",
    ),
    (DialectId::OpenAiChat, "request", "header:openai-beta"),
    (DialectId::OpenAiResponses, "request", "header:openai-beta"),
    (DialectId::OpenAiEmbeddings, "request", "header:openai-beta"),
    (DialectId::AnthropicMessages, "request", "cache_control"),
    (DialectId::AnthropicMessages, "reasoning", "display"),
    (DialectId::OpenAiChat, "image", "image_url.detail"),
    (DialectId::OpenAiChat, "document", "file.filename"),
    (DialectId::OpenAiChat, "request", "prompt_cache_key"),
    (DialectId::OpenAiResponses, "request", "prompt_cache_key"),
    (DialectId::OpenAiResponses, "request", "client_metadata"),
    (DialectId::OpenAiResponses, "image", "detail"),
];

pub mod reasoning_map {
    use kaeso_kpp::{ReasoningConfig, ReasoningEffort, ReasoningMode};

    pub(crate) const MIN_BUDGET: u32 = 1_024;
    pub const MEDIUM_FROM: u32 = 4_096;
    pub const HIGH_FROM: u32 = 16_384;
    pub(crate) const LOW_BUDGET: u32 = 2_048;
    pub(crate) const MEDIUM_BUDGET: u32 = 8_192;
    pub(crate) const HIGH_BUDGET: u32 = 24_576;

    pub fn effort_for_budget(budget_tokens: u32) -> ReasoningEffort {
        if budget_tokens < MEDIUM_FROM {
            ReasoningEffort::Low
        } else if budget_tokens < HIGH_FROM {
            ReasoningEffort::Medium
        } else {
            ReasoningEffort::High
        }
    }

    pub fn budget_for_effort(effort: &ReasoningEffort) -> Option<u32> {
        match effort {
            ReasoningEffort::Low => Some(LOW_BUDGET),
            ReasoningEffort::Medium => Some(MEDIUM_BUDGET),
            ReasoningEffort::High | ReasoningEffort::XHigh | ReasoningEffort::Max => {
                Some(HIGH_BUDGET)
            }
            _ => None,
        }
    }

    pub fn fit_budget(budget: u32, max_tokens: Option<u32>) -> Option<u32> {
        let ceiling = match max_tokens {
            Some(max) => max.checked_sub(1)?,
            None => budget,
        };
        let fitted = budget.min(ceiling);
        (fitted >= MIN_BUDGET).then_some(fitted)
    }

    pub fn budget_within(budget: u32, max_tokens: Option<u32>) -> (u32, Option<u32>) {
        match max_tokens {
            Some(max) if max <= MIN_BUDGET => (MIN_BUDGET, Some(max.saturating_add(MIN_BUDGET))),
            _ => (fit_budget(budget, max_tokens).unwrap_or(MIN_BUDGET), None),
        }
    }

    pub fn openai_effort(cfg: &ReasoningConfig) -> Option<ReasoningEffort> {
        if let Some(effort) = &cfg.effort {
            return Some(effort.clone());
        }
        if cfg.mode == Some(ReasoningMode::Disabled) {
            return Some(ReasoningEffort::None);
        }
        cfg.budget_tokens.map(effort_for_budget)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn budget_to_effort_follows_the_table() {
            assert_eq!(effort_for_budget(1_024), ReasoningEffort::Low);
            assert_eq!(effort_for_budget(4_095), ReasoningEffort::Low);
            assert_eq!(effort_for_budget(4_096), ReasoningEffort::Medium);
            assert_eq!(effort_for_budget(16_383), ReasoningEffort::Medium);
            assert_eq!(effort_for_budget(16_384), ReasoningEffort::High);
            assert_eq!(effort_for_budget(64_000), ReasoningEffort::High);
        }

        #[test]
        fn effort_to_budget_follows_the_table_and_the_api_limits() {
            assert_eq!(budget_for_effort(&ReasoningEffort::Low), Some(2_048));
            assert_eq!(budget_for_effort(&ReasoningEffort::Medium), Some(8_192));
            for e in [
                ReasoningEffort::High,
                ReasoningEffort::XHigh,
                ReasoningEffort::Max,
            ] {
                assert_eq!(budget_for_effort(&e), Some(24_576));
            }
            assert_eq!(budget_for_effort(&ReasoningEffort::Minimal), None);
            assert_eq!(budget_for_effort(&ReasoningEffort::None), None);
            assert_eq!(fit_budget(24_576, Some(16_000)), Some(15_999));
            assert_eq!(fit_budget(2_048, Some(64_000)), Some(2_048));
            assert_eq!(fit_budget(2_048, Some(1_025)), Some(1_024));
            assert_eq!(fit_budget(2_048, Some(1_024)), None);
            assert_eq!(fit_budget(2_048, None), Some(2_048));
            assert_eq!(budget_within(24_576, Some(16_000)), (15_999, None));
            assert_eq!(budget_within(8_192, None), (8_192, None));
            assert_eq!(budget_within(2_048, Some(1_025)), (1_024, None));
            assert_eq!(budget_within(2_048, Some(1_024)), (1_024, Some(2_048)));
            assert_eq!(budget_within(8_192, Some(200)), (1_024, Some(1_224)));
        }

        #[test]
        fn openai_effort_prefers_explicit_effort_then_off_then_budget() {
            let mut c = ReasoningConfig::default();
            assert_eq!(openai_effort(&c), None);
            c.mode = Some(ReasoningMode::Disabled);
            assert_eq!(openai_effort(&c), Some(ReasoningEffort::None));
            c.mode = Some(ReasoningMode::Enabled);
            c.budget_tokens = Some(10_000);
            assert_eq!(openai_effort(&c), Some(ReasoningEffort::Medium));
            c.effort = Some(ReasoningEffort::High);
            assert_eq!(openai_effort(&c), Some(ReasoningEffort::High));
        }
    }
}

pub fn is_allowlisted(origin: DialectId, scope: &str, key: &str) -> bool {
    EXTRAS_ALLOWLIST
        .iter()
        .any(|(d, sc, k)| *d == origin && *sc == scope && *k == key)
}

pub fn is_droppable_value(origin: DialectId, scope: &str, key: &str, value: &RawJson) -> bool {
    match (origin, scope, key) {
        (DialectId::AnthropicMessages, "request", "context_management") => {
            only_thinking_edits(value)
        }
        _ => false,
    }
}

fn only_thinking_edits(value: &RawJson) -> bool {
    let Ok(Value::Object(config)) = serde_json::from_str::<Value>(value.get()) else {
        return false;
    };
    let Some(Value::Array(edits)) = config.get("edits") else {
        return false;
    };
    config.len() == 1
        && !edits.is_empty()
        && edits.iter().all(|e| {
            e.get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| t.starts_with("clear_thinking_"))
        })
}

fn repeats_effort(value: &RawJson, effort: Option<&str>) -> bool {
    let Ok(Value::Object(config)) = serde_json::from_str::<Value>(value.get()) else {
        return false;
    };
    config.iter().all(|(k, v)| match (k.as_str(), effort) {
        ("effort", Some(effort)) => v.as_str() == Some(effort),
        _ => false,
    })
}

fn claude_code_hint(origin: DialectId, scope: &str, key: &str) -> Option<&'static str> {
    let pre_release = match (origin, scope) {
        (DialectId::AnthropicMessages, "request") => {
            key == "context_management" || key == "output_config.task_budget"
        }
        (DialectId::AnthropicMessages, "tool") => true,
        _ => false,
    };
    pre_release.then_some(
        "Remove it, or send the request to a model served through its original format. From Claude Code, set `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1` to stop sending it.",
    )
}

fn file_store(dialect: DialectId) -> &'static str {
    match dialect {
        DialectId::AnthropicMessages => "anthropic",
        _ => "openai",
    }
}

pub fn file_id_usable(origin: &Origin, target: &Origin) -> bool {
    file_store(origin.dialect) == file_store(target.dialect) && same_provider(origin, target)
}

pub fn opaque_replayable(opaque: &Opaque, target: &Origin) -> bool {
    opaque.origin.dialect == target.dialect && same_provider(&opaque.origin, target)
}

pub fn reasoning_replayable(block: &ReasoningBlock, target: &Origin) -> bool {
    let origin_ok = block
        .origin
        .as_ref()
        .is_some_and(|o| o.dialect == target.dialect && same_provider(o, target));
    origin_ok && block.opaque.iter().all(|o| opaque_replayable(o, target))
}

fn same_provider(a: &Origin, b: &Origin) -> bool {
    match (&a.provider, &b.provider) {
        (Some(x), Some(y)) => x == y,
        _ => true,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Setting {
    OutputLimit,
    StopSequences,
    OutputFormat,
    Reasoning,
    ReasoningEffort,
}

pub(crate) fn app_field(r: &GenerateRequest, setting: Setting, target: DialectId) -> &'static str {
    use DialectId::{AnthropicMessages as A, OpenAiChat as C};
    let app = r.extras.origin.as_ref().map_or(target, |o| o.dialect);
    match (setting, app) {
        (Setting::OutputLimit, A) => "max_tokens",
        (Setting::OutputLimit, C) => crate::openai_chat::output_limit_field(r),
        (Setting::OutputLimit, _) => "max_output_tokens",
        (Setting::StopSequences, A) => "stop_sequences",
        (Setting::StopSequences, _) => "stop",
        (Setting::OutputFormat, A) => "output_config.format",
        (Setting::OutputFormat, C) => "response_format",
        (Setting::OutputFormat, _) => "text.format",
        (Setting::Reasoning, A) => "thinking",
        (Setting::Reasoning, C) => "reasoning_effort",
        (Setting::Reasoning, _) => "reasoning",
        (Setting::ReasoningEffort, A) => "output_config.effort",
        (Setting::ReasoningEffort, C) => "reasoning_effort",
        (Setting::ReasoningEffort, _) => "reasoning.effort",
    }
}

pub fn check_foreign(req: &Request, target: &Origin) -> Result<(), KppError> {
    match req {
        Request::Generate(r) | Request::CountTokens(r) => check_generate(r, target),
        Request::Embed(r) => check_extras(&r.extras, target.dialect, "", "request"),
        other => Err(KppError::unsupported_by(
            format!("{:?} request", other.kind()),
            target.dialect,
            "",
        )),
    }
}

fn check_generate(r: &GenerateRequest, target: &Origin) -> Result<(), KppError> {
    let dialect = target.dialect;
    check_extras(&r.extras, dialect, "", "request")?;
    for (i, b) in r.system.iter().enumerate() {
        check_block(b, target, &format!("system[{i}]"))?;
    }
    let effort = r
        .reasoning
        .as_ref()
        .and_then(|c| c.effort.as_ref())
        .map(ReasoningEffort::as_wire);
    for (mi, m) in r.messages.iter().enumerate() {
        let path = format!("messages[{mi}]");
        let own_system = m.role == Role::System && m.extras.is_from(DialectId::AnthropicMessages);
        check_extras_with(&m.extras, dialect, &path, "message", |key, value| {
            own_system && key == "output_config" && repeats_effort(value, effort)
        })?;
        for (bi, b) in m.blocks.iter().enumerate() {
            check_block(b, target, &format!("{path}.blocks[{bi}]"))?;
        }
    }
    for (i, t) in r.tools.iter().enumerate() {
        let path = format!("tools[{i}]");
        match t {
            ToolDef::Function(f) => check_extras(&f.extras, dialect, &path, "tool")?,
            ToolDef::Unknown(item) => {
                if !item.origin.is(dialect) {
                    let ty = type_tag(item.raw.as_raw()).ok().flatten();
                    let name = ty.as_deref().unwrap_or("non-function");
                    let hint = match &ty {
                        Some(ty)
                            if item.origin.is(DialectId::OpenAiResponses) && ty == "web_search" =>
                        {
                            "This model has no hosted web search. Remove the tool, or send the request to a model that has one. From Codex, set `web_search = \"disabled\"`.".to_owned()
                        }
                        Some(ty) if item.origin.is(DialectId::AnthropicMessages) => format!(
                            "Input tag '{ty}' found using 'type' is not a function tool, and only function tools can be sent to another API format. Remove the tool, or send the request to a model served through its original format."
                        ),
                        _ => "Only function tools can be sent to another API format. Remove the tool, or send the request to a model served through its original format.".to_owned(),
                    };
                    return Err(foreign(
                        format!("{name} tool"),
                        &item.origin,
                        dialect,
                        &path,
                        &hint,
                    ));
                }
            }
            _ => {
                return Err(KppError::unsupported_by("tool", dialect, "").with_param(path));
            }
        }
    }
    if let Some(ToolChoice::Unknown(item)) = &r.tool_choice
        && !item.origin.is(dialect)
    {
        let ty = type_tag(item.raw.as_raw())
            .ok()
            .flatten()
            .or_else(|| item.raw.parse::<String>().ok());
        return Err(foreign(
            format!("tool_choice `{}`", ty.as_deref().unwrap_or("unknown")),
            &item.origin,
            dialect,
            "tool_choice",
            "Use `auto`, `none`, `required` or a single named tool instead.",
        ));
    }
    if let kaeso_kpp::OutputSpec::JsonSchema(spec) = &r.output {
        check_extras(&spec.extras, dialect, "output", "output")?;
    }
    if let Some(reasoning) = &r.reasoning {
        check_extras(&reasoning.extras, dialect, "reasoning", "reasoning")?;
    }
    Ok(())
}

fn check_block(block: &Block, target: &Origin, path: &str) -> Result<(), KppError> {
    let dialect = target.dialect;
    match block {
        Block::Reasoning(_) => Ok(()),
        Block::Unknown(item) => {
            if item.origin.is(dialect) {
                return Ok(());
            }
            let ty = type_tag(item.raw.as_raw()).ok().flatten();
            if ty.is_none()
                && let Some(role) = role_of(&item.raw)
            {
                return Err(foreign(
                    format!("`{role}` message"),
                    &item.origin,
                    dialect,
                    path,
                    "Remove the message, or send the request to a model served through its original format.",
                ));
            }
            let hint = if item.origin.is(DialectId::AnthropicMessages)
                && ty.as_deref() == Some("tool_reference")
            {
                "Remove the block, or send the request to a model served through its original format. From Claude Code, unset `ENABLE_TOOL_SEARCH` or set it to `false`."
            } else {
                "Remove the block, or send the request to a model served through its original format."
            };
            Err(foreign(
                format!("{} content block", ty.as_deref().unwrap_or("unknown")),
                &item.origin,
                dialect,
                path,
                hint,
            ))
        }
        Block::Text(t) => {
            if let Some(c) = t.citations.iter().find(|c| !c.origin.is(dialect)) {
                return Err(foreign(
                    "citations".to_owned(),
                    &c.origin,
                    dialect,
                    path,
                    "Remove the citations from earlier assistant turns.",
                ));
            }
            check_extras(&t.extras, dialect, path, "text")
        }
        Block::Image(b) => {
            check_source(&b.source, target, path)?;
            check_extras(&b.extras, dialect, path, "image")
        }
        Block::Document(b) => {
            check_source(&b.source, target, path)?;
            check_extras(&b.extras, dialect, path, "document")
        }
        Block::ToolResult(r) => {
            check_extras(&r.extras, dialect, path, "tool_result")?;
            for (i, b) in r.content.iter().enumerate() {
                check_block(b, target, &format!("{path}.content[{i}]"))?;
            }
            Ok(())
        }
        Block::ToolCall(c) => check_extras(&c.extras, dialect, path, "tool_call"),
        Block::Refusal(r) => check_extras(&r.extras, dialect, path, "refusal"),
        other => Err(
            KppError::unsupported_by(format!("{} block", other.kind()), dialect, "")
                .with_param(path),
        ),
    }
}

fn role_of(raw: &RawJson) -> Option<String> {
    match serde_json::from_str::<Value>(raw.get()).ok()?.get("role")? {
        Value::String(role) => Some(role.clone()),
        _ => None,
    }
}

fn check_source(source: &Source, target: &Origin, path: &str) -> Result<(), KppError> {
    if let Source::FileId(f) = source
        && !file_id_usable(&f.origin, target)
    {
        return Err(foreign(
            "file id".to_owned(),
            &f.origin,
            target.dialect,
            path,
            &format!(
                "Provider file ids (here `{}`) only work with the provider that issued them. Send the file inline (base64 or URL) instead.",
                f.id
            ),
        ));
    }
    Ok(())
}

pub fn check_extras(
    extras: &Extras,
    target: DialectId,
    path: &str,
    scope: Scope,
) -> Result<(), KppError> {
    check_extras_with(extras, target, path, scope, |_, _| false)
}

fn check_extras_with(
    extras: &Extras,
    target: DialectId,
    path: &str,
    scope: Scope,
    droppable: impl Fn(&str, &RawJson) -> bool,
) -> Result<(), KppError> {
    let Some(origin) = &extras.origin else {
        if let Some(key) = extras.fields.keys().next() {
            return Err(KppError::invalid_request(format!(
                "`{key}` has no origin; build `Extras` with `Extras::new(origin)`."
            ))
            .with_param(path));
        }
        return Ok(());
    };
    for (key, value) in extras.fields.iter().filter(|_| !extras.is_from(target)) {
        if is_allowlisted(origin.dialect, scope, key)
            || is_droppable_value(origin.dialect, scope, key, value)
            || droppable(key, value)
        {
            continue;
        }
        let feature = match key.strip_prefix(HEADER_PREFIX) {
            Some(header) => format!("{header} header"),
            None if path.is_empty() => key.clone(),
            None => format!("{path}.{key}"),
        };
        return Err(foreign(
            feature,
            origin,
            target,
            path,
            claude_code_hint(origin.dialect, scope, key).unwrap_or(
                "Remove it, or send the request to a model served through its original format.",
            ),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplyPolicy {
    Keep,
    Carry,
    Omit,
}

pub fn reply_policy(origin: Option<&Origin>, target: DialectId) -> ReplyPolicy {
    match origin {
        Some(o) if o.is(target) => ReplyPolicy::Keep,
        Some(_) => ReplyPolicy::Omit,
        None => ReplyPolicy::Keep,
    }
}

pub fn reasoning_reply_policy(origin: Option<&Origin>, target: DialectId) -> ReplyPolicy {
    match reply_policy(origin, target) {
        ReplyPolicy::Omit
            if matches!(
                target,
                DialectId::OpenAiResponses | DialectId::AnthropicMessages
            ) =>
        {
            ReplyPolicy::Carry
        }
        other => other,
    }
}

pub fn reply_opaque_visible(opaque: &Opaque, target: DialectId) -> bool {
    opaque.origin.is(target)
        && matches!(
            (target, opaque.kind),
            (
                DialectId::AnthropicMessages,
                OpaqueKind::AnthropicThinkingSignature | OpaqueKind::AnthropicRedactedThinking
            ) | (
                DialectId::OpenAiResponses,
                OpaqueKind::OpenAiEncryptedReasoning | OpaqueKind::OpenAiReasoningItemId
            )
        )
}

pub(crate) fn foreign(
    feature: String,
    origin: &Origin,
    target: DialectId,
    path: &str,
    hint: &str,
) -> KppError {
    let named = if feature.contains('`') {
        feature.clone()
    } else {
        format!("`{feature}`")
    };
    let message = format!(
        "{named} comes from an {} request and has no equivalent in {}. {hint}",
        origin.dialect.display_name(),
        target.display_name(),
    );
    let err = KppError::unsupported(feature, message);
    if path.is_empty() {
        err
    } else {
        err.with_param(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_kpp::{
        Citation, DocumentBlock, ErrorKind, FunctionTool, Message, RawItem, RawJson, Role,
        TextBlock,
    };
    use serde_json::json;

    const A: DialectId = DialectId::AnthropicMessages;
    const C: DialectId = DialectId::OpenAiChat;
    const R: DialectId = DialectId::OpenAiResponses;

    fn raw(s: &str) -> RawJson {
        RawJson::from_string(s.to_owned()).unwrap()
    }

    fn check(r: GenerateRequest, target: DialectId) -> Result<(), KppError> {
        check_foreign(&Request::Generate(r), &Origin::new(target))
    }

    #[test]
    fn app_fields_follow_the_dialect_the_request_came_from() {
        let from = |app: Option<DialectId>| {
            let mut r = GenerateRequest::new("m");
            r.extras.origin = app.map(Origin::new);
            r
        };
        let names = |r: &GenerateRequest| {
            [
                Setting::OutputLimit,
                Setting::StopSequences,
                Setting::OutputFormat,
                Setting::Reasoning,
                Setting::ReasoningEffort,
            ]
            .map(|s| app_field(r, s, R))
        };
        assert_eq!(
            names(&from(Some(A))),
            [
                "max_tokens",
                "stop_sequences",
                "output_config.format",
                "thinking",
                "output_config.effort"
            ]
        );
        assert_eq!(
            names(&from(Some(C))),
            [
                "max_completion_tokens",
                "stop",
                "response_format",
                "reasoning_effort",
                "reasoning_effort"
            ]
        );
        let expected_responses = [
            "max_output_tokens",
            "stop",
            "text.format",
            "reasoning",
            "reasoning.effort",
        ];
        assert_eq!(names(&from(Some(R))), expected_responses);
        assert_eq!(names(&from(None)), expected_responses);
        let mut legacy = from(Some(C));
        legacy
            .extras
            .form
            .insert("max_tokens".into(), json!("max_tokens"));
        assert_eq!(app_field(&legacy, Setting::OutputLimit, A), "max_tokens");
        assert_eq!(
            app_field(&from(None), Setting::OutputLimit, A),
            "max_tokens"
        );
    }

    #[test]
    fn extras_without_an_origin_say_how_to_build_them() {
        let extras = Extras::default().with_field("top_k", raw("1"));
        let e = check_extras(&extras, C, "", "request").unwrap_err();
        assert_eq!(
            e.message,
            "`top_k` has no origin; build `Extras` with `Extras::new(origin)`."
        );
        assert_eq!(e.kind, ErrorKind::InvalidRequest);
        let own = Extras::new(A).with_field("top_k", raw("1"));
        assert!(check_extras(&own, A, "", "request").is_ok());
    }

    #[test]
    fn context_management_crosses_only_when_it_only_clears_thinking() {
        let cm = |v: &str| is_droppable_value(A, "request", "context_management", &raw(v));
        assert!(cm(
            r#"{"edits":[{"type":"clear_thinking_20251015","keep":"all"}]}"#
        ));
        assert!(cm(
            r#"{"edits":[{"type":"clear_thinking_20251015","keep":{"type":"thinking_turns","value":2}}]}"#
        ));
        assert!(!cm(r#"{"edits":[]}"#));
        assert!(!cm(r#"{}"#));
        assert!(!cm(r#"{"edits":[{"type":"clear_tool_uses_20250919"}]}"#));
        assert!(!cm(
            r#"{"edits":[{"type":"clear_thinking_20251015"},{"type":"compact_20260112"}]}"#
        ));
        assert!(!cm(
            r#"{"edits":[{"type":"clear_thinking_20251015"}],"x":1}"#
        ));
        assert!(!is_droppable_value(
            R,
            "request",
            "context_management",
            &raw(r#"{"edits":[{"type":"clear_thinking_20251015"}]}"#)
        ));
    }

    #[test]
    fn per_message_output_config_crosses_only_when_it_repeats_the_effort() {
        assert!(repeats_effort(
            &raw(r#"{"effort":"medium"}"#),
            Some("medium")
        ));
        assert!(repeats_effort(&raw("{}"), None));
        assert!(!repeats_effort(
            &raw(r#"{"effort":"high"}"#),
            Some("medium")
        ));
        assert!(!repeats_effort(&raw(r#"{"effort":"medium"}"#), None));
        assert!(!repeats_effort(
            &raw(r#"{"effort":"medium","x":1}"#),
            Some("medium")
        ));
        assert!(!repeats_effort(&raw("null"), Some("medium")));
    }

    #[test]
    fn same_dialect_extras_pass_and_foreign_ones_fail() {
        let mut r = GenerateRequest::new("m").with_message(Message::user("hi"));
        r.extras = Extras::new(A).with_field("service_tier", raw("\"auto\""));
        check(r.clone(), A).unwrap();
        let e = check(r, C).unwrap_err();
        assert_eq!(e.unsupported_feature(), Some("service_tier"));
        assert!(e.message.contains("Anthropic Messages"));
        assert!(e.message.contains("OpenAI Chat Completions"));
    }

    #[test]
    fn form_markers_never_block_and_unbound_fields_always_do() {
        let mut r = GenerateRequest::new("m").with_message(Message::user("hi"));
        r.messages[0].extras = Extras::new(A).with_form("content", json!("string"));
        check(r.clone(), C).unwrap();
        r.messages[0].extras = Extras::default().with_field("x", raw("1"));
        assert_eq!(
            check(r, A).unwrap_err().param.as_deref(),
            Some("messages[0]")
        );
    }

    #[test]
    fn allowlisted_fields_cross_silently_only_in_their_scope() {
        let mut r = GenerateRequest::new("m").with_message(Message::user("hi"));
        let mut e = Extras::new(A).with_field("cache_control", raw(r#"{"type":"ephemeral"}"#));
        e.insert_header(
            "anthropic-beta",
            "claude-code-20250219,interleaved-thinking-2025-05-14",
        );
        r.extras = e;
        check(r.clone(), C).unwrap();

        r.messages[0].extras = Extras::new(A).with_field("cache_control", raw("{}"));
        assert_eq!(
            check(r, C).unwrap_err().unsupported_feature(),
            Some("messages[0].cache_control")
        );
        let mut file = DocumentBlock::new(Source::base64("application/pdf", "JVBERi0="));
        file.extras = Extras::new(R).with_field("detail", raw("\"high\""));
        let r = GenerateRequest::new("m")
            .with_message(Message::new(Role::User, vec![Block::Document(file)]));
        assert!(
            check(r, C).is_err(),
            "`detail` is only allowlisted on images"
        );
    }

    #[test]
    fn foreign_headers_are_named_as_headers() {
        let mut r = GenerateRequest::new("m").with_message(Message::user("hi"));
        let mut e = Extras::new(A);
        e.insert_header("x-future-header", "x");
        r.extras = e;
        let err = check(r, C).unwrap_err();
        assert_eq!(err.unsupported_feature(), Some("x-future-header header"));
    }

    #[test]
    fn unknown_blocks_tools_and_choices_are_bound() {
        let unknown = Block::Unknown(RawItem::new(
            A,
            raw(r#"{"type":"server_tool_use","id":"x"}"#),
        ));
        let r =
            GenerateRequest::new("m").with_message(Message::new(Role::Assistant, vec![unknown]));
        check(r.clone(), A).unwrap();
        let e = check(r, C).unwrap_err();
        assert_eq!(
            e.unsupported_feature(),
            Some("server_tool_use content block")
        );
        assert_eq!(e.param.as_deref(), Some("messages[0].blocks[0]"));

        let r = GenerateRequest::new("m").with_tool(ToolDef::Unknown(RawItem::new(
            A,
            raw(r#"{"type":"web_search_20250305","name":"web_search"}"#),
        )));
        let e = check(r, C).unwrap_err();
        assert_eq!(e.unsupported_feature(), Some("web_search_20250305 tool"));

        let mut r = GenerateRequest::new("m");
        r.tool_choice = Some(ToolChoice::Unknown(RawItem::new(
            C,
            raw(r#"{"type":"allowed_tools"}"#),
        )));
        let e = check(r, A).unwrap_err();
        assert_eq!(e.unsupported_feature(), Some("tool_choice `allowed_tools`"));
    }

    #[test]
    fn file_ids_follow_the_file_store_and_provider() {
        let req = |origin: Origin| {
            let doc = Block::Document(DocumentBlock::new(Source::file_id(origin, "file-abc")));
            GenerateRequest::new("m").with_message(Message::new(
                Role::User,
                vec![Block::tool_result("c", vec![doc], false)],
            ))
        };
        let e = check(req(Origin::new(C)), A).unwrap_err();
        assert_eq!(e.param.as_deref(), Some("messages[0].blocks[0].content[0]"));
        assert_eq!(e.unsupported_feature(), Some("file id"));
        assert!(e.message.contains("file-abc"));
        check(req(Origin::new(C)), R).unwrap();
        let azure = Origin::new(C).with_provider("azure");
        check_foreign(
            &Request::Generate(req(azure.clone())),
            &Origin::new(R).with_provider("azure"),
        )
        .unwrap();
        assert!(
            check_foreign(
                &Request::Generate(req(azure)),
                &Origin::new(R).with_provider("openai"),
            )
            .is_err()
        );
    }

    #[test]
    fn foreign_citations_fail() {
        let mut cited = TextBlock::new("x");
        cited.citations = vec![Citation::new(A, raw(r#"{"type":"char_location"}"#))];
        let r = GenerateRequest::new("m")
            .with_message(Message::new(Role::Assistant, vec![Block::Text(cited)]));
        assert_eq!(
            check(r, C).unwrap_err().unsupported_feature(),
            Some("citations")
        );
    }

    #[test]
    fn reasoning_is_left_to_the_encoder_with_replay_rules() {
        let mut reasoning = ReasoningBlock::new(A);
        reasoning.text.push("t".into());
        reasoning
            .opaque
            .push(Opaque::new(A, OpaqueKind::AnthropicThinkingSignature, "s"));
        reasoning.extras = Extras::new(A).with_field("x", raw("1"));
        let r = GenerateRequest::new("m").with_message(Message::new(
            Role::Assistant,
            vec![Block::Reasoning(reasoning.clone())],
        ));
        check(r, C).unwrap();
        assert!(reasoning_replayable(&reasoning, &Origin::new(A)));
        assert!(!reasoning_replayable(&reasoning, &Origin::new(C)));
        let mut from_bedrock = reasoning.clone();
        from_bedrock.origin = Some(Origin::new(A).with_provider("bedrock"));
        from_bedrock.opaque[0].origin = Origin::new(A).with_provider("bedrock");
        assert!(!reasoning_replayable(
            &from_bedrock,
            &Origin::new(A).with_provider("anthropic")
        ));
    }

    #[test]
    fn tool_and_config_extras_are_checked() {
        let mut f = FunctionTool::new("f", None);
        f.extras = Extras::new(C).with_field("custom", raw("true"));
        let r = GenerateRequest::new("m").with_tool(ToolDef::Function(f));
        assert_eq!(
            check(r, A).unwrap_err().unsupported_feature(),
            Some("tools[0].custom")
        );
    }

    #[test]
    fn reply_policy_keeps_own_and_omits_foreign() {
        assert_eq!(reply_policy(Some(&Origin::new(A)), A), ReplyPolicy::Keep);
        assert_eq!(reply_policy(Some(&Origin::new(A)), C), ReplyPolicy::Omit);
        assert_eq!(reply_policy(None, C), ReplyPolicy::Keep);
        let sig = Opaque::new(A, OpaqueKind::AnthropicThinkingSignature, "s");
        assert!(reply_opaque_visible(&sig, A));
        assert!(!reply_opaque_visible(&sig, C));
    }
}
