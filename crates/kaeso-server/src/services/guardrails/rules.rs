use std::collections::{BTreeMap, HashMap};

use kaeso_core::masking::{Masker, Restorer, RuleAction, Scanner, StreamRestore};
use kaeso_kpp::{Block, Delta, GenerateRequest, RawJson, Response, StreamEvent};

/// What built-in rules found in a request, for its verdict, its log and its reply.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Masked {
    /// How many values of each kind they found, such as `api_key`.
    pub found: BTreeMap<String, u32>,
    /// Each kind's detector and action, for the metrics.
    pub actions: BTreeMap<String, (String, RuleAction)>,
    /// The kind of the first value a blocking rule found.
    pub blocked: Option<String>,
    /// Whether any value was replaced in the request.
    pub changed: bool,
    /// The values the log masks, each with its placeholder.
    pub logged: Vec<(String, String)>,
    pub restorer: Option<Restorer>,
}

fn mask_blocks(m: &mut Masker<'_>, blocks: &mut [Block]) -> bool {
    let mut changed = false;
    for block in blocks {
        match block {
            Block::Text(t) => {
                if let Some(out) = m.text(&t.text) {
                    t.text = out;
                    changed = true;
                }
            }
            Block::ToolCall(c) => {
                if let Some(out) = m.text(c.arguments.get())
                    && let Ok(json) = RawJson::from_string(out)
                {
                    c.arguments = json;
                    changed = true;
                }
            }
            Block::ToolResult(r) => changed |= mask_blocks(m, &mut r.content),
            _ => {}
        }
    }
    changed
}

/// Scans the system prompt, the messages' text, tool results and tool call arguments, and masks
/// what the rules mask. Media and tool definitions stay as they are.
pub fn mask_request(scanner: &Scanner, g: &mut GenerateRequest) -> Masked {
    let mut m = scanner.masker();
    let mut changed = mask_blocks(&mut m, &mut g.system);
    for message in &mut g.messages {
        changed |= mask_blocks(&mut m, &mut message.blocks);
    }
    Masked {
        found: m.found.iter().map(|(k, f)| (k.clone(), f.count)).collect(),
        actions: m
            .found
            .iter()
            .filter_map(|(k, f)| f.action.map(|a| (k.clone(), (f.detector.clone(), a))))
            .collect(),
        blocked: m.blocked.clone(),
        changed,
        logged: m.logged().to_vec(),
        restorer: m.restorer(),
    }
}

/// How the app's error names what a blocking rule found; never the value.
pub fn what(kind: &str) -> String {
    match kind {
        "api_key" => "it contains an API key".to_owned(),
        "private_key" => "it contains a private key".to_owned(),
        "token" => "it contains a JSON Web Token".to_owned(),
        "email" => "it contains an email address".to_owned(),
        "phone" => "it contains a phone number".to_owned(),
        "iban" => "it contains an IBAN".to_owned(),
        "card" => "it contains a payment card number".to_owned(),
        "ip" => "it contains an IP address".to_owned(),
        other => format!("it matches the pattern `{other}`"),
    }
}

fn restore_blocks(r: &Restorer, blocks: &mut [Block]) {
    for block in blocks {
        match block {
            Block::Text(t) => t.text = r.restore(&t.text),
            Block::ToolCall(c) => {
                let restored = r.restore(c.arguments.get());
                if let Ok(json) = RawJson::from_string(restored) {
                    c.arguments = json;
                }
            }
            _ => {}
        }
    }
}

/// Puts the masked values back into a whole reply.
pub fn restore_reply(r: &Restorer, reply: &mut Response) {
    restore_blocks(r, &mut reply.blocks);
}

/// Masks the logged values in a reply the log keeps.
pub fn mask_reply(pairs: &[(String, String)], reply: &mut Response) {
    for block in &mut reply.blocks {
        match block {
            Block::Text(t) => t.text = kaeso_core::masking::mask_text(&t.text, pairs),
            Block::ToolCall(c) => {
                let masked = kaeso_core::masking::mask_text(c.arguments.get(), pairs);
                if let Ok(json) = RawJson::from_string(masked) {
                    c.arguments = json;
                }
            }
            _ => {}
        }
    }
}

/// Keeps the values built-in rules found out of what the request log stores: the request body
/// and the reply.
pub fn mask_logged(o: &mut crate::model_api::hooks::Outcome) {
    if o.log_masks.is_empty() {
        return;
    }
    if let Some(body) = &o.request_body
        && let Ok(text) = std::str::from_utf8(body)
    {
        o.request_body = Some(bytes::Bytes::from(kaeso_core::masking::mask_text(
            text,
            &o.log_masks,
        )));
    }
    if let Some(reply) = o.reply.as_mut() {
        mask_reply(&o.log_masks, reply);
    }
}

/// Restores placeholders in a stream's text, block by block. A block's text that may end in
/// part of a placeholder waits for the next piece, and goes out at the block's end.
#[derive(Debug)]
pub struct Restoring {
    restorer: Restorer,
    blocks: HashMap<u32, StreamRestore>,
}

impl Restoring {
    pub fn new(restorer: Restorer) -> Restoring {
        Restoring {
            restorer,
            blocks: HashMap::new(),
        }
    }

    pub fn apply(&mut self, events: Vec<StreamEvent>) -> Vec<StreamEvent> {
        let mut out = Vec::with_capacity(events.len());
        for mut event in events {
            match &mut event {
                StreamEvent::BlockDelta(d) => {
                    if let Delta::Text(text) = &mut d.delta {
                        let s = self
                            .blocks
                            .entry(d.index)
                            .or_insert_with(|| self.restorer.stream());
                        *text = s.push(text);
                        if text.is_empty() {
                            continue;
                        }
                    }
                }
                StreamEvent::BlockStop(stop) => {
                    if let Some(mut s) = self.blocks.remove(&stop.index) {
                        let rest = s.finish();
                        if !rest.is_empty() {
                            out.push(StreamEvent::BlockDelta(kaeso_kpp::BlockDelta::new(
                                stop.index,
                                Delta::Text(rest),
                            )));
                        }
                    }
                }
                _ => {}
            }
            out.push(event);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

    use super::*;
    use kaeso_core::masking::{Detector, Rules};
    use kaeso_kpp::{BlockDelta, BlockStop, Message, Role};

    fn scanner(restore: bool) -> Scanner {
        Rules {
            detectors: [
                (Detector::Email, RuleAction::Mask),
                (Detector::Secrets, RuleAction::Mask),
                (Detector::Card, RuleAction::Block),
            ]
            .into_iter()
            .collect(),
            restore,
            ..Rules::default()
        }
        .compile()
        .unwrap()
    }

    #[test]
    fn requests_are_masked_in_text_tool_calls_and_tool_results_but_not_tool_definitions() {
        let mut g = GenerateRequest::new("m")
            .with_system("Reply to ada@example.com")
            .with_message(Message::new(
                Role::Assistant,
                vec![Block::tool_call(
                    "c1",
                    "send",
                    RawJson::from_string(r#"{"to":"ada@example.com"}"#.to_owned()).unwrap(),
                )],
            ))
            .with_message(Message::new(
                Role::User,
                vec![Block::tool_result(
                    "c1",
                    vec![Block::text("sent to bob@example.com")],
                    false,
                )],
            ));
        let mut tool = kaeso_kpp::FunctionTool::new("send", Some(RawJson::empty_object()));
        tool.description = Some("Mail ada@example.com".to_owned());
        g.tools.push(kaeso_kpp::ToolDef::Function(tool));
        let masked = mask_request(&scanner(true), &mut g);
        assert!(masked.changed);
        assert_eq!(masked.found["email"], 3);
        let Block::Text(t) = &g.system[0] else {
            panic!()
        };
        assert_eq!(t.text, "Reply to [EMAIL_1]");
        let Block::ToolCall(c) = &g.messages[0].blocks[0] else {
            panic!()
        };
        assert_eq!(c.arguments.get(), r#"{"to":"[EMAIL_1]"}"#);
        let Block::ToolResult(r) = &g.messages[1].blocks[0] else {
            panic!()
        };
        let Block::Text(t) = &r.content[0] else {
            panic!()
        };
        assert_eq!(t.text, "sent to [EMAIL_2]");
        let kaeso_kpp::ToolDef::Function(tool) = &g.tools[0] else {
            panic!()
        };
        assert_eq!(tool.description.as_deref(), Some("Mail ada@example.com"));
        assert_eq!(
            masked.actions["email"],
            ("email".to_owned(), RuleAction::Mask)
        );

        let mut reply = Response::new(
            "r",
            "m",
            vec![
                Block::text("Done for [EMAIL_1] and [EMAIL_2]."),
                Block::tool_call(
                    "c2",
                    "send",
                    RawJson::from_string(r#"{"to":"[EMAIL_1]"}"#.to_owned()).unwrap(),
                ),
            ],
        );
        restore_reply(masked.restorer.as_ref().unwrap(), &mut reply);
        let Block::Text(t) = &reply.blocks[0] else {
            panic!()
        };
        assert_eq!(t.text, "Done for ada@example.com and bob@example.com.");
        let Block::ToolCall(c) = &reply.blocks[1] else {
            panic!()
        };
        assert_eq!(c.arguments.get(), r#"{"to":"ada@example.com"}"#);
        mask_reply(&masked.logged, &mut reply);
        let Block::Text(t) = &reply.blocks[0] else {
            panic!()
        };
        assert_eq!(t.text, "Done for [EMAIL_1] and [EMAIL_2].");
    }

    #[test]
    fn a_blocking_rule_names_its_kind_and_never_the_value() {
        let mut g = GenerateRequest::new("m").with_message(Message::new(
            Role::User,
            vec![Block::text("4111 1111 1111 1111")],
        ));
        let masked = mask_request(&scanner(false), &mut g);
        assert_eq!(masked.blocked.as_deref(), Some("card"));
        assert!(!masked.changed);
        assert!(masked.restorer.is_none());
        for (kind, said) in [
            ("api_key", "it contains an API key"),
            ("private_key", "it contains a private key"),
            ("token", "it contains a JSON Web Token"),
            ("email", "it contains an email address"),
            ("phone", "it contains a phone number"),
            ("iban", "it contains an IBAN"),
            ("card", "it contains a payment card number"),
            ("ip", "it contains an IP address"),
            ("order_id", "it matches the pattern `order_id`"),
        ] {
            assert_eq!(what(kind), said);
        }
    }

    #[test]
    fn a_stream_is_restored_across_chunks_and_flushed_at_the_block_end() {
        let mut g = GenerateRequest::new("m").with_message(Message::new(
            Role::User,
            vec![Block::text("ada@example.com")],
        ));
        let masked = mask_request(&scanner(true), &mut g);
        let mut r = Restoring::new(masked.restorer.unwrap());
        let delta = |i: u32, t: &str| {
            StreamEvent::BlockDelta(BlockDelta::new(i, Delta::Text(t.to_owned())))
        };
        let text = |events: &[StreamEvent]| -> String {
            events
                .iter()
                .filter_map(|e| match e {
                    StreamEvent::BlockDelta(d) => match &d.delta {
                        Delta::Text(t) => Some(t.clone()),
                        _ => None,
                    },
                    _ => None,
                })
                .collect()
        };
        let mut out = Vec::new();
        out.extend(r.apply(vec![delta(0, "Hi [EMA")]));
        out.extend(r.apply(vec![delta(0, "IL_1], bye [EM")]));
        assert_eq!(text(&out), "Hi ada@example.com, bye ");
        out.extend(r.apply(vec![
            delta(1, "other block"),
            StreamEvent::BlockStop(BlockStop::new(0)),
            delta(0, "x"),
            StreamEvent::BlockDelta(BlockDelta::new(
                0,
                Delta::ToolArguments("[EMAIL_1]".to_owned()),
            )),
        ]));
        assert_eq!(text(&out), "Hi ada@example.com, bye other block[EMx");
        assert!(
            matches!(out.last(), Some(StreamEvent::BlockDelta(d)) if d.delta == Delta::ToolArguments("[EMAIL_1]".to_owned()))
        );
    }
}
