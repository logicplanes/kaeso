use kaeso_kpp::{Block, Extras, GenerateRequest};
use serde_json::Value;

use super::DIALECT;
use super::overrides::SET;

const CONTENT: &str = "content";

fn has_null_content(block: &Block) -> bool {
    let Block::Reasoning(r) = block else {
        return false;
    };
    r.extras.is_from(DIALECT)
        && matches!(r.extras.form.get(SET), Some(Value::Object(set)) if set.get(CONTENT).is_some_and(Value::is_null))
}

fn remove_null_content(extras: &mut Extras) {
    if let Some(Value::Object(set)) = extras.form.get_mut(SET) {
        set.remove(CONTENT);
        if set.is_empty() {
            extras.form.shift_remove(SET);
        }
    }
}

pub(crate) fn without_null_reasoning_content(r: &GenerateRequest) -> Option<GenerateRequest> {
    if !r
        .messages
        .iter()
        .any(|m| m.blocks.iter().any(has_null_content))
    {
        return None;
    }
    let mut out = r.clone();
    for block in out.messages.iter_mut().flat_map(|m| m.blocks.iter_mut()) {
        if has_null_content(block)
            && let Block::Reasoning(r) = block
        {
            remove_null_content(&mut r.extras);
        }
    }
    Some(out)
}
