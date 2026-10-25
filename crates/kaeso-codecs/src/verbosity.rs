use kaeso_kpp::{DialectId, GenerateRequest, RawJson};

use crate::adapt::Draft;
use crate::openai_chat::ChatProfile;

const RESPONSES_VERBOSITY: &str = "text.verbosity";

pub(crate) fn of_responses(r: &GenerateRequest) -> Option<&RawJson> {
    r.extras
        .fields_for(DialectId::OpenAiResponses)?
        .get(RESPONSES_VERBOSITY)
}

pub(crate) fn left_out(d: &mut Draft<'_>, target: DialectId, chat: &ChatProfile) {
    let lost = match target {
        DialectId::AnthropicMessages => true,
        DialectId::OpenAiChat => !chat.request.verbosity,
        _ => false,
    };
    if lost && of_responses(d.get()).is_some() {
        d.leave_out(RESPONSES_VERBOSITY.to_owned());
    }
}
