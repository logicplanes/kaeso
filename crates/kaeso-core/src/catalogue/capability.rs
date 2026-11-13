use kaeso_kpp::{
    DialectId, GenerateRequest, Modality, ModelInfo, ReasoningEffort, ReasoningMode, Request,
    ToolChoice,
};

use kaeso_codecs::cross::reasoning_map;
use sha2::{Digest, Sha256};

use super::Effective;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct Unsupported {
    pub capability: &'static str,
    pub param: Option<&'static str>,
    pub message: String,
}

fn refuse(capability: &'static str, param: Option<&'static str>, message: String) -> Unsupported {
    Unsupported {
        capability,
        param,
        message,
    }
}

// A custom provider serves a media API only once its admin turns it on.
fn no_media_api(eff: &Effective, api: &str, fallback: String) -> String {
    match eff.provider.custom() {
        Some(c) => format!(
            "The custom provider `{}` has no {api} turned on; an org admin turns it on under Providers.",
            c.id()
        ),
        None => fallback,
    }
}

fn app_param(ingress: DialectId, field: &'static str) -> &'static str {
    use DialectId::{AnthropicMessages as A, Gemini as G, OpenAiResponses as R};
    match (field, ingress) {
        ("budget", _) => "thinking.budget_tokens",
        ("effort", A) => "output_config.effort",
        ("effort", R) => "reasoning.effort",
        ("effort", _) => "reasoning_effort",
        ("format", A) => "output_config.format",
        ("format", R) => "text.format",
        ("format", _) => "response_format",
        ("messages", R) => "input",
        ("messages", G) => "contents",
        ("modalities", G) => "generationConfig.responseModalities",
        ("reasoning", A) => "thinking",
        ("user", A) => "metadata.user_id",
        ("reasoning", R) => "reasoning",
        ("reasoning", _) => "reasoning_effort",
        (other, _) => other,
    }
}

fn forced_choice(choice: Option<&ToolChoice>, ingress: DialectId) -> Option<&'static str> {
    let anthropic = ingress == DialectId::AnthropicMessages;
    match choice? {
        ToolChoice::Required if anthropic => Some("`tool_choice` `any`"),
        ToolChoice::Tool(_) if anthropic => Some("`tool_choice` `tool`"),
        ToolChoice::Required => Some("`tool_choice` `required`"),
        ToolChoice::Tool(_) => Some("a named function in `tool_choice`"),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub dialect: DialectId,
    pub adaptive_thinking: Option<bool>,
    pub thinking_binding: Option<bool>,
    pub reasoning: Option<bool>,
    pub left_out: Vec<String>,
    pub adapted: Vec<String>,
}

pub fn prepare(
    req: &mut Request,
    ingress: DialectId,
    eff: &Effective,
) -> Result<Plan, Unsupported> {
    let name = eff.model_id.as_str();
    let adaptive_thinking = eff
        .provider
        .dialects()
        .contains(&DialectId::AnthropicMessages)
        .then_some(eff.fields.adaptive_thinking)
        .flatten();
    let thinking_binding = eff
        .provider
        .dialects()
        .contains(&DialectId::AnthropicMessages)
        .then_some(eff.fields.thinking_binding)
        .flatten();
    match req {
        Request::Embed(_) => {
            refuse_decision_only(eff)?;
            if !eff.is_embeddings() {
                return Err(refuse(
                    "embeddings",
                    Some("model"),
                    format!(
                        "The model `{name}` is a generation model; it cannot create embeddings."
                    ),
                ));
            }
            if !eff.provider.dialects().contains(&eff.dialect) {
                return Err(refuse(
                    "embeddings",
                    Some("model"),
                    format!(
                        "Kaeso calls no embeddings models on {}, so `{name}` can't create embeddings.",
                        eff.provider.name()
                    ),
                ));
            }
            Ok(Plan {
                dialect: eff.dialect,
                adaptive_thinking: None,
                thinking_binding: None,
                reasoning: None,
                left_out: Vec::new(),
                adapted: Vec::new(),
            })
        }
        Request::Generate(g) => {
            refuse_decision_only(eff)?;
            let mut adapted = adaptive_to_budget(g, ingress, eff);
            let mut left_out = leave_out_reasoning(g, ingress, eff);
            left_out.extend(leave_out_temperature(g, ingress, eff));
            adapted.extend(nearest_effort(g, ingress, eff));
            adapted.extend(lowest_effort_for_off(g, ingress, eff));
            adapted.extend(fit_end_user(g, ingress, eff));
            adapted.extend(fit_output_limit(g, ingress, eff));
            check_generation(g, ingress, eff)?;
            let dialect = eff.dialect;
            if dialect == DialectId::AnthropicMessages && g.sampling.max_output_tokens.is_none() {
                g.sampling.max_output_tokens = eff
                    .fields
                    .output_limit
                    .map(|l| u32::try_from(l).unwrap_or(u32::MAX));
            }
            Ok(Plan {
                dialect,
                adaptive_thinking,
                thinking_binding,
                reasoning: eff.fields.reasoning,
                left_out,
                adapted,
            })
        }
        Request::CountTokens(g) => {
            refuse_decision_only(eff)?;
            let Some(dialect) = eff.provider.count_tokens_for(eff.dialect) else {
                return Err(refuse(
                    "token counting",
                    Some("model"),
                    format!("The model `{name}` cannot count tokens."),
                ));
            };
            let mut adapted = adaptive_to_budget(g, ingress, eff);
            let left_out = leave_out_reasoning(g, ingress, eff);
            adapted.extend(nearest_effort(g, ingress, eff));
            adapted.extend(lowest_effort_for_off(g, ingress, eff));
            adapted.extend(fit_end_user(g, ingress, eff));
            check_generation(g, ingress, eff)?;
            Ok(Plan {
                dialect,
                adaptive_thinking,
                thinking_binding,
                reasoning: eff.fields.reasoning,
                left_out,
                adapted,
            })
        }
        Request::Image(r) => {
            let Some(images) = eff.provider.images(name) else {
                return Err(refuse(
                    "images",
                    Some("model"),
                    no_media_api(
                        eff,
                        "Images API",
                        format!(
                            "{} offers no image API Kaeso can call, so `{name}` can't make images.",
                            eff.provider.name()
                        ),
                    ),
                ));
            };
            let produces = eff
                .fields
                .output_modalities
                .as_ref()
                .is_none_or(|o| o.iter().any(|x| x == "image"));
            if !produces {
                return Err(refuse(
                    "image output",
                    Some("model"),
                    format!("The model `{name}` does not produce images."),
                ));
            }
            let accepts = eff
                .fields
                .input_modalities
                .as_ref()
                .is_none_or(|i| i.iter().any(|x| x == "image"));
            if r.required_capabilities().images && !accepts {
                return Err(refuse(
                    "image input",
                    Some("image"),
                    format!("The model `{name}` does not accept images."),
                ));
            }
            Ok(Plan {
                dialect: images.dialect,
                adaptive_thinking: None,
                thinking_binding: None,
                reasoning: None,
                left_out: Vec::new(),
                adapted: Vec::new(),
            })
        }
        Request::Speech(r) => {
            let Some(speech) = eff.provider.speech(name) else {
                return Err(refuse(
                    "speech",
                    Some("model"),
                    no_media_api(
                        eff,
                        "Speech API",
                        format!(
                            "{} offers no speech API Kaeso can call, so `{name}` can't speak.",
                            eff.provider.name()
                        ),
                    ),
                ));
            };
            if !produces(eff, "audio") {
                return Err(refuse(
                    "audio output",
                    Some("model"),
                    format!("The model `{name}` does not produce audio."),
                ));
            }
            if r.required_capabilities().audio_input && !eff.accepts("audio") {
                return Err(refuse(
                    "audio input",
                    Some("audio"),
                    format!("The model `{name}` does not accept audio."),
                ));
            }
            Ok(media_plan(speech.dialect))
        }
        Request::Transcription(_) => {
            let Some(transcriptions) = eff.provider.transcriptions(name) else {
                return Err(refuse(
                    "transcription",
                    Some("model"),
                    no_media_api(
                        eff,
                        "Transcriptions API",
                        format!(
                            "{} offers no transcription API Kaeso can call, so `{name}` can't transcribe.",
                            eff.provider.name()
                        ),
                    ),
                ));
            };
            if !eff.accepts("audio") {
                return Err(refuse(
                    "audio input",
                    Some("file"),
                    format!("The model `{name}` does not accept audio."),
                ));
            }
            if !produces(eff, "text") {
                return Err(refuse(
                    "text output",
                    Some("model"),
                    format!("The model `{name}` does not produce text."),
                ));
            }
            Ok(media_plan(transcriptions.dialect))
        }
        Request::Video(r) => {
            let Some(videos) = eff.provider.videos(name) else {
                let message = if eff.provider.has_videos() {
                    format!("The model `{name}` does not make videos.")
                } else {
                    no_media_api(
                        eff,
                        "Videos API",
                        format!(
                            "{} offers no video API Kaeso can call, so `{name}` can't make videos.",
                            eff.provider.name()
                        ),
                    )
                };
                return Err(refuse("videos", Some("model"), message));
            };
            if !produces(eff, "video") {
                return Err(refuse(
                    "video output",
                    Some("model"),
                    format!("The model `{name}` does not make videos."),
                ));
            }
            if r.required_capabilities().images && !eff.accepts("image") {
                return Err(refuse(
                    "image input",
                    Some("input_reference"),
                    format!("The model `{name}` does not accept images."),
                ));
            }
            Ok(media_plan(videos.dialect))
        }
        Request::Decide(d) => {
            let Some(decisions) = eff.provider.decisions(name) else {
                let message = if eff.provider.has_decisions() {
                    generation_only(name)
                } else {
                    format!(
                        "{} offers no decisions API Kaeso can call, so `{name}` can't make decisions.",
                        eff.provider.name()
                    )
                };
                return Err(refuse("decisions", Some("model"), message));
            };
            check_decision(d, ingress, eff)?;
            Ok(media_plan(decisions.dialect))
        }
        _ => Err(refuse(
            "endpoint",
            None,
            format!("The model `{name}` cannot serve this endpoint."),
        )),
    }
}

fn generation_only(name: &str) -> String {
    format!("The model `{name}` is a generation model; it can't answer decision questions.")
}

// The app's own name for what a decision request is about.
fn decision_input_param(ingress: DialectId) -> &'static str {
    match ingress {
        DialectId::SystemOne => "state",
        _ => "input",
    }
}

fn check_decision(
    d: &kaeso_kpp::DecideRequest,
    ingress: DialectId,
    eff: &Effective,
) -> Result<(), Unsupported> {
    use kaeso_kpp::{Block, DecisionInput, Instructions, QuestionKind};
    let name = eff.model_id.as_str();
    let Some(fields) = eff.fields.decisions.as_ref() else {
        return Err(refuse("decisions", Some("model"), generation_only(name)));
    };
    let input = Some(decision_input_param(ingress));
    let caps = d.required_capabilities();
    for (needed, modality, words) in [
        (caps.images, "image", "images"),
        (caps.pdf, "pdf", "PDF documents"),
        (caps.audio_input, "audio", "audio"),
        (caps.video_input, "video", "video"),
    ] {
        if needed && !eff.accepts(modality) {
            return Err(refuse(
                "decision input",
                input,
                format!("The model `{name}` does not accept {words}."),
            ));
        }
    }
    if let Some(q) = d.questions.iter().find(|q| !fields.answers(q.type_name())) {
        return Err(refuse(
            "question type",
            Some("questions"),
            format!(
                "The model `{name}` does not answer {} questions.",
                q.type_name()
            ),
        ));
    }
    let over = |limit: Option<u32>, n: usize| limit.is_some_and(|l| n > l as usize);
    if over(fields.max_questions, d.questions.len()) {
        return Err(refuse(
            "questions",
            Some("questions"),
            format!(
                "The model `{name}` answers at most {} questions per request.",
                fields.max_questions.unwrap_or_default()
            ),
        ));
    }
    for q in &d.questions {
        let (count, limit, words) = match &q.kind {
            QuestionKind::Choice(c) => (c.options.len(), fields.max_options, "options per choice"),
            QuestionKind::Score(s) => (s.levels.len(), fields.max_levels, "levels per score"),
            _ => continue,
        };
        if over(limit, count) {
            return Err(refuse(
                "questions",
                Some("questions"),
                format!(
                    "The model `{name}` takes at most {} {words} question.",
                    limit.unwrap_or_default()
                ),
            ));
        }
    }
    let is_image = |b: &&Block| matches!(b, Block::Image(_));
    let mut images = d.assets.len();
    if let DecisionInput::Messages(messages) = &d.input {
        images += messages.iter().flat_map(|m| &m.blocks).filter(is_image).count();
    }
    for q in &d.questions {
        if let Some(Instructions::Blocks(blocks)) = &q.instructions {
            images += blocks.iter().filter(is_image).count();
        }
    }
    if over(fields.max_images, images) {
        return Err(refuse(
            "images",
            input,
            format!(
                "The model `{name}` takes at most {} images per request.",
                fields.max_images.unwrap_or_default()
            ),
        ));
    }
    Ok(())
}

// A decision-only model answers typed questions and generates nothing, so chat, token counts
// and embeddings are refused before its provider is asked.
fn refuse_decision_only(eff: &Effective) -> Result<(), Unsupported> {
    if !eff.decides_only() {
        return Ok(());
    }
    Err(refuse(
        "decisions",
        Some("model"),
        format!(
            "The model `{}` is a decision model; send it to `/v1/decisions` or `/v1/systemone`.",
            eff.model_id
        ),
    ))
}

fn produces(eff: &Effective, modality: &str) -> bool {
    eff.fields
        .output_modalities
        .as_ref()
        .is_none_or(|o| o.iter().any(|x| x == modality))
}

fn media_plan(dialect: DialectId) -> Plan {
    Plan {
        dialect,
        adaptive_thinking: None,
        thinking_binding: None,
        reasoning: None,
        left_out: Vec::new(),
        adapted: Vec::new(),
    }
}

// Claude Code and Codex send an effort, and Claude Code adaptive thinking, to every model with
// no switch per model; refusing them would make every model without effort levels unusable.
fn leave_out_reasoning(
    g: &mut GenerateRequest,
    ingress: DialectId,
    eff: &Effective,
) -> Vec<String> {
    let mut left_out = Vec::new();
    let Some(r) = g.reasoning.as_mut() else {
        return left_out;
    };
    let reasons = eff.fields.reasoning == Some(true);
    let mut effort_gone = false;
    if r.effort
        .as_ref()
        .is_some_and(|e| *e != ReasoningEffort::None)
        && (!reasons || effort_levels(eff, ingress).is_empty())
    {
        r.effort = None;
        effort_gone = true;
        left_out.push(app_param(ingress, "effort").to_owned());
    }
    if !reasons && r.mode == Some(ReasoningMode::Adaptive) {
        r.mode = None;
        r.extras.fields.shift_remove("display");
        left_out.push(app_param(ingress, "reasoning").to_owned());
    }
    let nothing_left = r.mode.is_none()
        && r.effort.is_none()
        && r.budget_tokens.is_none()
        && r.summary.is_none()
        && !r.include_encrypted
        && r.extras.fields.is_empty();
    if !left_out.is_empty() && nothing_left {
        g.reasoning = None;
    }
    if effort_gone {
        left_out.extend(message_efforts(g, None, None));
    }
    left_out
}

const OUTPUT_CONFIG: &str = "output_config";

// Claude Code repeats the request's effort in `output_config` on its mid-conversation system
// messages; what the request's effort becomes, theirs becomes too, or they would be refused
// as a setting other formats can't carry.
fn message_efforts(g: &mut GenerateRequest, from: Option<&str>, to: Option<&str>) -> Vec<String> {
    let mut named = Vec::new();
    for (i, m) in g.messages.iter_mut().enumerate() {
        if m.role != kaeso_kpp::Role::System || !m.extras.is_from(DialectId::AnthropicMessages) {
            continue;
        }
        let Some(raw) = m.extras.fields.get(OUTPUT_CONFIG) else {
            continue;
        };
        let Ok(serde_json::Value::Object(mut config)) = raw.to_value() else {
            continue;
        };
        let Some(serde_json::Value::String(effort)) = config.get("effort") else {
            continue;
        };
        if from.is_some_and(|f| f != effort) {
            continue;
        }
        match to {
            Some(level) => {
                config.insert("effort".to_owned(), level.into());
            }
            None => {
                config.remove("effort");
            }
        }
        let Ok(written) = kaeso_kpp::RawJson::from_serialize(&config) else {
            continue;
        };
        if config.is_empty() {
            m.extras.fields.shift_remove(OUTPUT_CONFIG);
        } else {
            m.extras.fields.insert(OUTPUT_CONFIG.to_owned(), written);
        }
        named.push(format!("messages[{i}].{OUTPUT_CONFIG}.effort"));
    }
    named
}

fn rank(level: &str) -> usize {
    EFFORT_ORDER
        .iter()
        .position(|o| *o == level)
        .unwrap_or(usize::MAX)
}

// Claude Code and Codex send one effort to every model; a model that lists levels but not that
// one gets the nearest level it lists, the lower one on a tie, never `none`.
fn nearest_effort(g: &mut GenerateRequest, ingress: DialectId, eff: &Effective) -> Vec<String> {
    let Some(r) = g.reasoning.as_mut() else {
        return Vec::new();
    };
    let Some(effort) = r.effort.as_ref() else {
        return Vec::new();
    };
    let wire = effort.as_wire().to_owned();
    let levels = effort_levels(eff, ingress);
    if *effort == ReasoningEffort::None
        || rank(&wire) == usize::MAX
        || levels.is_empty()
        || levels.contains(&wire)
    {
        return Vec::new();
    }
    let distance = |l: &String| rank(l).abs_diff(rank(&wire));
    let Some(best) = levels
        .iter()
        .filter(|l| rank(l) != usize::MAX)
        .map(distance)
        .min()
    else {
        return Vec::new();
    };
    let mut nearest: Vec<&String> = levels
        .iter()
        .filter(|l| rank(l) != usize::MAX && distance(l) == best)
        .collect();
    nearest.sort_by_key(|l| rank(l));
    let Some(level) = nearest
        .iter()
        .find(|l| l.as_str() != "none")
        .or(nearest.first())
    else {
        return Vec::new();
    };
    let level = (*level).clone();
    r.effort = Some(ReasoningEffort::from_wire(&level));
    message_efforts(g, Some(&wire), Some(&level));
    vec![app_param(ingress, "effort").to_owned()]
}

// Claude Code sends adaptive thinking to every model; a Claude model without it takes
// `enabled` thinking with a budget, which the effort sets as it does for requests from other
// formats ([reasoning_map]).
fn adaptive_to_budget(g: &mut GenerateRequest, ingress: DialectId, eff: &Effective) -> Vec<String> {
    let Some(r) = g.reasoning.as_mut() else {
        return Vec::new();
    };
    if eff.dialect != DialectId::AnthropicMessages
        || eff.fields.adaptive_thinking != Some(false)
        || eff.fields.reasoning != Some(true)
        || r.mode != Some(ReasoningMode::Adaptive)
    {
        return Vec::new();
    }
    let budget = match r.effort.as_ref() {
        None => reasoning_map::budget_for_effort(&ReasoningEffort::Medium),
        Some(effort) => reasoning_map::budget_for_effort(effort),
    };
    match budget {
        Some(budget) => {
            r.mode = Some(ReasoningMode::Enabled);
            r.budget_tokens = Some(budget);
        }
        None => {
            r.mode = Some(ReasoningMode::Disabled);
        }
    }
    r.extras.fields.shift_remove("display");
    vec![app_param(ingress, "reasoning").to_owned()]
}

// Apps send a temperature with no switch per model; Claude 5 models take only the default,
// so the model answers at its default instead of refusing every such app.
fn leave_out_temperature(
    g: &mut GenerateRequest,
    ingress: DialectId,
    eff: &Effective,
) -> Vec<String> {
    let Some(t) = g.sampling.temperature else {
        return Vec::new();
    };
    if eff.fields.temperature != Some(false)
        || (t - 1.0).abs() <= f64::EPSILON
        || kpp_turns_thinking_on(g, ingress, eff)
    {
        return Vec::new();
    }
    g.sampling.temperature = None;
    vec!["temperature".to_owned()]
}

const EFFORT_ORDER: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

// A model that always reasons, such as GPT-OSS, refuses `none` when its effort levels don't list
// it; its lowest level is the least it thinks, so that stands in for thinking off (#643).
fn lowest_effort_for_off(
    g: &mut GenerateRequest,
    ingress: DialectId,
    eff: &Effective,
) -> Vec<String> {
    let Some(r) = g.reasoning.as_mut() else {
        return Vec::new();
    };
    let effort_off = r.effort == Some(ReasoningEffort::None);
    let mode_off = r.effort.is_none() && r.mode == Some(ReasoningMode::Disabled);
    if eff.dialect == DialectId::AnthropicMessages
        || eff.fields.reasoning != Some(true)
        || !(effort_off || mode_off)
    {
        return Vec::new();
    }
    let levels = effort_levels(eff, ingress);
    if levels.iter().any(|l| l == "none") {
        return Vec::new();
    }
    let Some(lowest) = levels.iter().min_by_key(|l| {
        EFFORT_ORDER
            .iter()
            .position(|o| o == l)
            .unwrap_or(usize::MAX)
    }) else {
        return Vec::new();
    };
    r.effort = Some(ReasoningEffort::from_wire(lowest));
    if mode_off {
        r.mode = None;
    }
    let field = if effort_off { "effort" } else { "reasoning" };
    vec![app_param(ingress, field).to_owned()]
}

// Anthropic's `metadata.user_id` may be longer than a provider takes, such as Claude Code's
// 150 characters; OpenAI's spec asks for a hash of the user's id there anyway.
fn fit_end_user(g: &mut GenerateRequest, ingress: DialectId, eff: &Effective) -> Vec<String> {
    let Some(limit) = eff.provider.end_user_limit() else {
        return Vec::new();
    };
    match g.user.as_mut() {
        Some(user) if ingress == DialectId::AnthropicMessages && user.chars().count() > limit => {
            *user = Sha256::digest(user.as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            vec![app_param(ingress, "user").to_owned()]
        }
        _ => Vec::new(),
    }
}

// An app in another format can't know the model's output limit, and Claude Code asks every
// model it doesn't know for 32,000 tokens; Cohere and Anthropic refuse more than their maximum.
fn fit_output_limit(g: &mut GenerateRequest, ingress: DialectId, eff: &Effective) -> Vec<String> {
    let limit = eff.fields.output_limit.and_then(|l| u32::try_from(l).ok());
    match (g.sampling.max_output_tokens, limit) {
        (Some(asked), Some(limit)) if ingress != eff.dialect && asked > limit => {
            g.sampling.max_output_tokens = Some(limit);
            vec![output_param(g, ingress).to_owned()]
        }
        _ => Vec::new(),
    }
}

fn output_param(g: &GenerateRequest, ingress: DialectId) -> &'static str {
    match ingress {
        DialectId::AnthropicMessages => "max_tokens",
        DialectId::OpenAiResponses => "max_output_tokens",
        _ if g.extras.form_str(DialectId::OpenAiChat, "max_tokens") == Some("max_tokens") => {
            "max_tokens"
        }
        _ => "max_completion_tokens",
    }
}

fn check_generation(
    g: &GenerateRequest,
    ingress: DialectId,
    eff: &Effective,
) -> Result<(), Unsupported> {
    let name = eff.model_id.as_str();
    if eff.is_embeddings() {
        return Err(refuse(
            "generation",
            Some("model"),
            format!("The model `{name}` is an embeddings model; it cannot generate text."),
        ));
    }
    let caps = g.required_capabilities();
    let produces = |m: &str| {
        eff.fields
            .output_modalities
            .as_ref()
            .is_none_or(|o| o.iter().any(|x| x == m))
    };
    // Chat Completions asks for audio as `["text", "audio"]`, its text being the transcript, which
    // a speech model writes without producing text of its own.
    let wants_text = g.output_modalities.is_empty()
        || (g.output_modalities.contains(&Modality::Text) && !caps.audio_output);
    if wants_text && !produces("text") {
        return Err(refuse(
            "text output",
            Some("model"),
            format!("The model `{name}` does not produce text."),
        ));
    }
    let modalities = Some(app_param(ingress, "modalities"));
    if caps.audio_output && !produces("audio") {
        return Err(refuse(
            "audio output",
            modalities,
            format!("The model `{name}` does not produce audio."),
        ));
    }
    if caps.image_output && !produces("image") {
        return Err(refuse(
            "image output",
            modalities,
            format!("The model `{name}` does not produce images."),
        ));
    }
    let messages = Some(app_param(ingress, "messages"));
    if caps.images && !eff.accepts("image") {
        return Err(refuse(
            "image input",
            messages,
            format!("The model `{name}` does not accept images."),
        ));
    }
    if caps.pdf && !eff.accepts("pdf") {
        return Err(refuse(
            "pdf input",
            messages,
            format!("The model `{name}` does not accept PDF documents."),
        ));
    }
    if caps.audio_input && !eff.accepts("audio") {
        return Err(refuse(
            "audio input",
            messages,
            format!("The model `{name}` does not accept audio."),
        ));
    }
    if caps.video_input && !eff.accepts("video") {
        return Err(refuse(
            "video input",
            messages,
            format!("The model `{name}` does not accept video."),
        ));
    }
    if caps.tools && eff.fields.tool_call != Some(true) {
        return Err(refuse(
            "tools",
            Some("tools"),
            format!("The model `{name}` does not support tool calling."),
        ));
    }
    if eff.fields.forced_tool_choice == Some(false)
        && let Some(words) = forced_choice(g.tool_choice.as_ref(), ingress)
    {
        return Err(refuse(
            "forced tool choice",
            Some("tool_choice"),
            format!(
                "The model `{name}` does not accept {words}; send `auto` and name the tool in the prompt."
            ),
        ));
    }
    if caps.json_schema && eff.fields.structured_output != Some(true) {
        return Err(refuse(
            "structured output",
            Some(app_param(ingress, "format")),
            format!("The model `{name}` does not support structured output (a JSON schema)."),
        ));
    }
    if caps.reasoning && eff.fields.reasoning != Some(true) {
        return Err(refuse(
            "reasoning",
            Some(app_param(ingress, "reasoning")),
            format!("The model `{name}` does not support reasoning."),
        ));
    }
    if let Some(effort) = g.reasoning.as_ref().and_then(|r| r.effort.as_ref()) {
        check_effort(effort, ingress, eff)?;
    } else if let Some(budget) = budget_sent_as_effort(g, eff) {
        check_budget(budget, ingress, eff)?;
    }
    Ok(())
}

fn budget_sent_as_effort(g: &GenerateRequest, eff: &Effective) -> Option<u32> {
    let r = g.reasoning.as_ref()?;
    let openai = matches!(
        eff.dialect,
        DialectId::OpenAiChat | DialectId::OpenAiResponses
    );
    (openai && r.mode != Some(ReasoningMode::Disabled))
        .then_some(r.budget_tokens)
        .flatten()
}

fn check_budget(budget: u32, ingress: DialectId, eff: &Effective) -> Result<(), Unsupported> {
    let levels = effort_levels(eff, ingress);
    let effort = reasoning_map::effort_for_budget(budget);
    if levels.is_empty() || levels.iter().any(|l| l == effort.as_wire()) {
        return Ok(());
    }
    let bands = [
        ("low", 0),
        ("medium", reasoning_map::MEDIUM_FROM),
        ("high", reasoning_map::HIGH_FROM),
    ];
    let supported = |band: &&(&str, u32)| levels.iter().any(|l| l == band.0);
    let at = bands
        .iter()
        .position(|(level, _)| *level == effort.as_wire())
        .unwrap_or(0);
    let above = bands.iter().skip(at + 1).find(supported);
    let below = bands
        .iter()
        .take(at)
        .rposition(|band| supported(&band))
        .and_then(|i| bands.get(i + 1));
    let model = &eff.model_id;
    let effort = effort.as_wire();
    let message = match (above, below) {
        (Some((_, min)), _) => format!(
            "A `budget_tokens` of {budget} means `{effort}`; `{model}` needs at least {min}."
        ),
        (None, Some((_, max))) => format!(
            "A `budget_tokens` of {budget} means `{effort}`; `{model}` needs less than {max}."
        ),
        (None, None) => format!(
            "A `budget_tokens` of {budget} means `{effort}`; `{model}` needs `thinking` off."
        ),
    };
    Err(refuse(
        "effort",
        Some(app_param(ingress, "budget")),
        message,
    ))
}

fn kpp_turns_thinking_on(g: &GenerateRequest, ingress: DialectId, eff: &Effective) -> bool {
    let Some(r) = &g.reasoning else {
        return false;
    };
    ingress != DialectId::AnthropicMessages
        && eff.dialect == DialectId::AnthropicMessages
        && (r
            .effort
            .as_ref()
            .is_some_and(|e| !matches!(e, ReasoningEffort::None | ReasoningEffort::Minimal))
            || matches!(
                r.mode,
                Some(ReasoningMode::Enabled | ReasoningMode::Adaptive)
            )
            || r.budget_tokens.is_some()
            || (r.summary.is_some() && r.effort.is_none() && eff.fields.reasoning == Some(true)))
}

pub fn effort_levels(eff: &Effective, ingress: DialectId) -> Vec<String> {
    let mut levels = eff.fields.efforts.clone().unwrap_or_default();
    if eff.dialect == DialectId::AnthropicMessages
        && eff.fields.adaptive_thinking == Some(false)
        && ingress != DialectId::AnthropicMessages
        && eff.fields.reasoning == Some(true)
    {
        for e in ["low", "medium", "high", "xhigh", "max"] {
            if !levels.iter().any(|l| l == e) {
                levels.push(e.to_owned());
            }
        }
    }
    levels
}

fn check_effort(
    effort: &ReasoningEffort,
    ingress: DialectId,
    eff: &Effective,
) -> Result<(), Unsupported> {
    let name = eff.model_id.as_str();
    let wire = effort.as_wire();
    let levels = effort_levels(eff, ingress);
    if *effort == ReasoningEffort::None
        && (eff.dialect == DialectId::AnthropicMessages
            || levels.is_empty()
            || levels.iter().any(|l| l == "none"))
    {
        return Ok(());
    }
    if levels.iter().any(|l| l == wire) {
        return Ok(());
    }
    let message = format!(
        "The model `{name}` supports the effort levels {}; `{wire}` is not one of them.",
        levels
            .iter()
            .map(|l| format!("`{l}`"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    Err(refuse(
        "effort",
        Some(app_param(ingress, "effort")),
        message,
    ))
}

pub fn model_info(id: &str, eff: &Effective) -> ModelInfo {
    let mut info = ModelInfo::new(id);
    info.display_name = Some(eff.display_name().to_owned());
    info.created_at = eff.fields.release_date.as_deref().and_then(release_seconds);
    info.owned_by = Some(eff.provider.id().to_owned());
    info.context_window = eff.fields.context_limit;
    info.max_output_tokens = eff.fields.output_limit;
    info
}

fn release_seconds(date: &str) -> Option<u64> {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    let d = time::Date::parse(date, &format).ok()?;
    u64::try_from(d.midnight().assume_utc().unix_timestamp()).ok()
}
