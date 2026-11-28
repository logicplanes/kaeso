use kaeso_kpp::{
    DialectId, GenerateRequest, ReasoningConfig, ReasoningEffort, ReasoningMode, ToolChoice,
    ToolDef,
};

use crate::adapt::Draft;
use crate::cross::{self, Setting};

fn forces_a_tool(r: &GenerateRequest) -> bool {
    match r.tool_choice {
        Some(ToolChoice::Tool(_)) => !r.tools.is_empty(),
        // With a server tool such as `deepseek_web_search` declared, DeepSeek takes `required` as
        // `auto` while thinking (https://api-docs.deepseek.com/guides/anthropic_api#tool_choice).
        Some(ToolChoice::Required) => {
            !r.tools.is_empty() && r.tools.iter().all(|t| matches!(t, ToolDef::Function(_)))
        }
        _ => false,
    }
}

fn thinks(r: &GenerateRequest, target: DialectId) -> bool {
    let Some(c) = &r.reasoning else {
        return true;
    };
    match &c.effort {
        // The Messages encoder sends another format's effort `none` as no `thinking` at all,
        // which DeepSeek reads as thinking on.
        Some(ReasoningEffort::None) => {
            target == DialectId::AnthropicMessages
                && !c.extras.is_from(target)
                && c.mode != Some(ReasoningMode::Disabled)
        }
        Some(_) => true,
        None => c.mode != Some(ReasoningMode::Disabled),
    }
}

// DeepSeek's own `thinking` field in a Chat request is the app's explicit choice and stays as
// written, as the Anthropic encoder keeps an Anthropic app's own thinking with a forced tool.
fn deepseek_thinking_from_app(r: &GenerateRequest) -> bool {
    r.extras
        .fields_for(DialectId::OpenAiChat)
        .and_then(|f| f.get("thinking"))
        .is_some_and(|t| t.get().trim() != "null")
}

// DeepSeek answers a forced tool choice in thinking mode with 400 "Thinking mode does not
// support this tool_choice" (https://api-docs.deepseek.com/guides/anthropic_api#tool_choice,
// `errors/recorded-thinking-tool-choice-required.json`), and its models think unless told not
// to, so such a request goes with thinking off, as Anthropic's does with budget thinking.
pub(crate) fn off_for_forced_tools(d: &mut Draft<'_>, target: DialectId) {
    let r = d.get();
    if !forces_a_tool(r) || !thinks(r, target) || deepseek_thinking_from_app(r) {
        return;
    }
    let field = cross::app_field(r, Setting::Reasoning, target);
    let mut off = ReasoningConfig::default();
    if let Some(c) = &r.reasoning {
        off.extras = c.extras.clone();
    }
    match target {
        DialectId::AnthropicMessages => off.mode = Some(ReasoningMode::Disabled),
        _ => off.effort = Some(ReasoningEffort::None),
    }
    d.edit().reasoning = Some(off);
    d.adapt(field.to_owned());
}

#[cfg(test)]
mod tests {
    use kaeso_kpp::{DialectId, Request, RequestKind};
    use serde_json::{Value, json};

    use crate::anthropic_messages::MessagesProfile;
    use crate::codec::{EgressCtx, HeaderList};
    use crate::dispatch;
    use crate::openai_chat::ChatProfile;

    const A: DialectId = DialectId::AnthropicMessages;
    const C: DialectId = DialectId::OpenAiChat;
    const R: DialectId = DialectId::OpenAiResponses;

    const DEEPSEEK_CHAT: ChatProfile = ChatProfile::compatible("DeepSeek")
        .max_tokens()
        .reasoning_fields(&["reasoning_content"])
        .forced_tool_without_thinking();
    const DEEPSEEK_MESSAGES: MessagesProfile =
        MessagesProfile::compatible("DeepSeek").forced_tool_without_thinking();

    fn decoded(dialect: DialectId, body: &Value) -> Request {
        dispatch::decode_request(
            dialect,
            RequestKind::Generate,
            body.to_string().as_bytes(),
            &HeaderList::new(),
        )
        .unwrap()
    }

    fn to_chat(req: &Request, profile: ChatProfile) -> (Value, Vec<String>) {
        let mut ctx = EgressCtx::new(req)
            .with_provider("deepseek")
            .with_chat_profile(profile);
        dispatch::check(C, req, &ctx).unwrap();
        let out = dispatch::encode_request(C, req, &mut ctx).unwrap();
        (serde_json::from_slice(&out.body).unwrap(), ctx.adapted)
    }

    fn to_messages(req: &Request, profile: MessagesProfile) -> (Value, Vec<String>) {
        let mut ctx = EgressCtx::new(req)
            .with_provider("deepseek")
            .with_messages_profile(profile);
        dispatch::check(A, req, &ctx).unwrap();
        let out = dispatch::encode_request(A, req, &mut ctx).unwrap();
        (serde_json::from_slice(&out.body).unwrap(), ctx.adapted)
    }

    fn tool() -> Value {
        json!({"type": "function", "function": {"name": "f", "parameters": {"type": "object", "properties": {"a": {"type": "string"}}}}})
    }

    fn chat(extra: Value) -> Request {
        let mut body = json!({
            "model": "deepseek-flash", "max_tokens": 100,
            "messages": [{"role": "user", "content": "hello"}], "tools": [tool()]
        });
        for (k, v) in extra.as_object().unwrap() {
            body[k] = v.clone();
        }
        decoded(C, &body)
    }

    #[test]
    fn a_forced_tool_goes_with_thinking_off() {
        for choice in [
            json!("required"),
            json!({"type": "function", "function": {"name": "f"}}),
        ] {
            let (body, adapted) = to_chat(&chat(json!({"tool_choice": choice})), DEEPSEEK_CHAT);
            assert_eq!(body["reasoning_effort"], "none", "{choice}");
            assert_eq!(body["tool_choice"], choice);
            assert_eq!(adapted, ["reasoning_effort"]);
        }
        let (body, adapted) = to_chat(
            &chat(json!({"tool_choice": "required", "reasoning_effort": "high"})),
            DEEPSEEK_CHAT,
        );
        assert_eq!(body["reasoning_effort"], "none");
        assert_eq!(adapted, ["reasoning_effort"]);
    }

    #[test]
    fn thinking_stays_where_nothing_forces_it_off() {
        for extra in [
            json!({"tool_choice": "auto"}),
            json!({}),
            json!({"tool_choice": "required", "reasoning_effort": "none"}),
            json!({"tool_choice": "required", "thinking": {"type": "enabled"}}),
            json!({"tool_choice": "required", "thinking": {"type": "disabled"}}),
        ] {
            let (body, adapted) = to_chat(&chat(extra.clone()), DEEPSEEK_CHAT);
            assert!(adapted.is_empty(), "{extra}: {adapted:?}");
            assert_eq!(
                body.get("reasoning_effort").cloned(),
                extra.get("reasoning_effort").cloned()
            );
            assert_eq!(
                body.get("thinking").cloned(),
                extra.get("thinking").cloned()
            );
        }
        let (body, adapted) = to_chat(
            &chat(json!({"tool_choice": "required"})),
            ChatProfile::compatible("Example Cloud"),
        );
        assert!(adapted.is_empty() && body.get("reasoning_effort").is_none());
    }

    #[test]
    fn required_with_a_server_tool_keeps_thinking_and_a_named_tool_does_not() {
        let tools = json!([tool(), {"type": "deepseek_web_search"}]);
        let (body, adapted) = to_chat(
            &chat(json!({"tool_choice": "required", "tools": tools})),
            DEEPSEEK_CHAT,
        );
        assert!(adapted.is_empty(), "DeepSeek takes it as auto: {adapted:?}");
        assert!(body.get("reasoning_effort").is_none(), "{body}");
        let responses = decoded(
            R,
            &json!({"model": "deepseek-flash", "input": "hello", "tool_choice": "required",
                "tools": [{"type": "function", "name": "f", "parameters": {"type": "object"}},
                    {"type": "web_search"}]}),
        );
        let (body, adapted) = to_chat(&responses, DEEPSEEK_CHAT);
        assert_eq!(
            body["reasoning_effort"], "none",
            "Chat leaves the search out, so DeepSeek gets only the function: {body}"
        );
        assert_eq!(adapted, ["reasoning"]);
        let named = json!({"type": "function", "function": {"name": "f"}});
        let (body, adapted) = to_chat(
            &chat(json!({"tool_choice": named, "tools": tools})),
            DEEPSEEK_CHAT,
        );
        assert_eq!(body["reasoning_effort"], "none");
        assert_eq!(adapted, ["reasoning_effort"]);
    }

    #[test]
    fn a_null_thinking_is_no_choice_and_an_effort_of_none_is_sent_as_off() {
        let (body, adapted) = to_chat(
            &chat(json!({"tool_choice": "required", "thinking": null})),
            DEEPSEEK_CHAT,
        );
        assert_eq!(body["reasoning_effort"], "none", "{body}");
        assert_eq!(adapted, ["reasoning_effort"]);

        let none = chat(json!({"tool_choice": "required", "reasoning_effort": "none"}));
        let (body, adapted) = to_messages(&none, DEEPSEEK_MESSAGES);
        assert_eq!(body["thinking"], json!({"type": "disabled"}), "{body}");
        assert_eq!(adapted, ["reasoning_effort"]);
    }

    #[test]
    fn reasoning_settings_the_target_refuses_are_still_refused_with_a_forced_tool() {
        let req = decoded(
            R,
            &json!({"model": "deepseek-flash", "input": "hello", "tool_choice": "required",
                "reasoning": {"context": "all_turns"},
                "tools": [{"type": "function", "name": "f", "parameters": {"type": "object"}}]}),
        );
        let ctx = EgressCtx::new(&req)
            .with_provider("deepseek")
            .with_chat_profile(DEEPSEEK_CHAT);
        let e = dispatch::check(C, &req, &ctx).unwrap_err();
        assert!(e.message.contains("reasoning.context"), "{}", e.message);
    }

    #[test]
    fn a_tool_turn_without_its_reasoning_keeps_thinking() {
        let req = decoded(
            C,
            &json!({"model": "deepseek-flash", "max_tokens": 100, "tools": [tool()], "messages": [
                {"role": "user", "content": "Use f."},
                {"role": "assistant", "content": null, "tool_calls": [
                    {"id": "call_00_1", "type": "function", "function": {"name": "f", "arguments": "{}"}}
                ]},
                {"role": "tool", "tool_call_id": "call_00_1", "content": "done"}
            ]}),
        );
        let (body, adapted) = to_chat(&req, DEEPSEEK_CHAT);
        assert!(
            adapted.is_empty(),
            "DeepSeek restores it by the call's id: {adapted:?}"
        );
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn other_formats_name_their_own_reasoning_field() {
        let responses = decoded(
            R,
            &json!({"model": "deepseek-flash", "input": "hello", "tool_choice": "required",
                "tools": [{"type": "function", "name": "f", "parameters": {"type": "object"}}]}),
        );
        let (body, adapted) = to_chat(&responses, DEEPSEEK_CHAT);
        assert_eq!(body["reasoning_effort"], "none");
        assert_eq!(adapted, ["reasoning"]);

        let messages = decoded(
            A,
            &json!({"model": "deepseek-flash", "max_tokens": 2048,
                "messages": [{"role": "user", "content": "hello"}],
                "tools": [{"name": "f", "input_schema": {"type": "object"}}],
                "tool_choice": {"type": "any"}, "thinking": {"type": "enabled", "budget_tokens": 1024}}),
        );
        let (body, adapted) = to_messages(&messages, DEEPSEEK_MESSAGES);
        assert_eq!(body["thinking"], json!({"type": "disabled"}));
        assert_eq!(body["tool_choice"]["type"], "any");
        assert_eq!(adapted, ["thinking"]);
        let (body, adapted) = to_messages(&messages, MessagesProfile::ANTHROPIC);
        assert!(adapted.is_empty(), "{adapted:?}");
        assert_eq!(body["thinking"]["type"], "enabled");
    }
}
