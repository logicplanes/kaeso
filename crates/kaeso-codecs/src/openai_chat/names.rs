use kaeso_kpp::{Block, DialectId, Extras, GenerateRequest, Message, Role};
use serde_json::Value;

use super::DIALECT;
use super::request::{FORM_MESSAGE, MESSAGE_PREFIX, block_extras_mut};
use crate::adapt::Draft;
use crate::codec::EgressCtx;
use crate::wire::VERBATIM_FORM_KEY;

const NAME: &str = "name";

#[derive(Clone, Copy)]
enum Place {
    LeadingSystem(usize),
    Message(usize),
    ToolResult(usize, usize),
}

impl Place {
    fn key(self) -> &'static str {
        match self {
            Place::LeadingSystem(_) => "message.name",
            _ => NAME,
        }
    }

    fn null_scope(self) -> String {
        match self {
            Place::LeadingSystem(_) => format!("{VERBATIM_FORM_KEY}:{MESSAGE_PREFIX}"),
            _ => VERBATIM_FORM_KEY.to_owned(),
        }
    }

    fn extras(self, r: &mut GenerateRequest) -> Option<&mut Extras> {
        match self {
            Place::LeadingSystem(i) => r.system.get_mut(i).and_then(block_extras_mut),
            Place::Message(i) => r.messages.get_mut(i).map(|m| &mut m.extras),
            Place::ToolResult(i, j) => r
                .messages
                .get_mut(i)
                .and_then(|m| m.blocks.get_mut(j))
                .and_then(block_extras_mut),
        }
    }
}

fn starts_message(block: &Block) -> bool {
    block
        .extras()
        .is_some_and(|e| e.form_value(DIALECT, FORM_MESSAGE).is_some())
}

fn tool_results(m: &Message) -> bool {
    m.role == Role::User
        && !m.blocks.is_empty()
        && m.blocks.iter().all(|b| matches!(b, Block::ToolResult(_)))
}

// The decoder keeps a Chat request's leading system messages in `system`, one group of blocks
// per message, and its tool messages as results in user messages, so each place is paired with
// the index of the message the app wrote.
fn places(r: &GenerateRequest) -> Vec<(Place, usize)> {
    let mut out = Vec::new();
    let mut app = 0;
    for (i, block) in r.system.iter().enumerate() {
        if starts_message(block) {
            out.push((Place::LeadingSystem(i), app));
            app += 1;
        }
    }
    for (i, m) in r.messages.iter().enumerate() {
        if tool_results(m) {
            for j in 0..m.blocks.len() {
                out.push((Place::ToolResult(i, j), app));
                app += 1;
            }
        } else {
            out.push((Place::Message(i), app));
            app += 1;
        }
    }
    out
}

fn holds_name(r: &GenerateRequest, place: Place) -> (bool, bool) {
    let extras = match place {
        Place::LeadingSystem(i) => r.system.get(i).and_then(Block::extras),
        Place::Message(i) => r.messages.get(i).map(|m| &m.extras),
        Place::ToolResult(i, j) => r
            .messages
            .get(i)
            .and_then(|m| m.blocks.get(j))
            .and_then(Block::extras),
    };
    let Some(extras) = extras.filter(|e| e.is_from(DIALECT)) else {
        return (false, false);
    };
    let null = matches!(
        extras.form.get(&place.null_scope()),
        Some(Value::Object(nulls)) if nulls.contains_key(NAME)
    );
    (extras.fields.contains_key(place.key()), null)
}

// OpenAI Chat's `name` tells apart participants of one role, and multi-agent frameworks such as
// AutoGen and CrewAI set it on their messages. No other format has the field, and OpenAI's own
// Responses API dropped it, so it is left out and named instead of refusing every such app.
pub(crate) fn left_out(d: &mut Draft<'_>, target: DialectId, ctx: &EgressCtx) {
    if !d.get().extras.is_from(DIALECT) {
        return;
    }
    let refused_here = ctx
        .chat_profile
        .request
        .refused_message_fields
        .contains(&NAME);
    if target == DIALECT && !refused_here {
        return;
    }
    let found: Vec<(Place, usize, bool, bool)> = places(d.get())
        .into_iter()
        .filter(|(place, _)| target != DIALECT || !matches!(place, Place::ToolResult(..)))
        .filter_map(|(place, app)| {
            let (named, null) = holds_name(d.get(), place);
            (named || null).then_some((place, app, named, null))
        })
        .collect();
    if found.is_empty() {
        return;
    }
    let r = d.edit();
    for &(place, _, named, null) in &found {
        let Some(extras) = place.extras(r) else {
            continue;
        };
        if named {
            extras.fields.shift_remove(place.key());
        }
        let scope = place.null_scope();
        if null && let Some(Value::Object(nulls)) = extras.form.get_mut(&scope) {
            nulls.retain(|key, _| key != NAME);
            if nulls.is_empty() {
                extras.form.shift_remove(&scope);
            }
        }
    }
    for (_, app, named, _) in found {
        if named {
            d.leave_out(format!("messages[{app}].name"));
        }
    }
}
