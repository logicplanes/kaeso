use kaeso_kpp::{
    DialectId, GenerateRequest, Modality, ModelInfo, ReasoningEffort, ReasoningMode, Request,
    ToolChoice,
};

use kaeso_codecs::cross::reasoning_map;
use sha2::{Digest, Sha256};

use super::Effective;
use crate::estimate::OutputCap;

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
        ("budget", G) => "generationConfig.thinkingConfig.thinkingBudget",
        ("budget", _) => "thinking.budget_tokens",
        ("effort", A) => "output_config.effort",
        ("effort", R) => "reasoning.effort",
        ("effort", G) => "generationConfig.thinkingConfig.thinkingLevel",
        ("effort", _) => "reasoning_effort",
        ("format", A) => "output_config.format",
        ("format", R) => "text.format",
        ("format", _) => "response_format",
        ("messages", R) => "input",
        ("messages", G) => "contents",
        ("modalities", G) => "generationConfig.responseModalities",
        ("temperature", G) => "generationConfig.temperature",
        ("top_p", G) => "generationConfig.topP",
        ("top_k", G) => "generationConfig.topK",
        ("reasoning", A) => "thinking",
        ("user", A) => "metadata.user_id",
        ("reasoning", R) => "reasoning",
        ("reasoning", G) => "generationConfig.thinkingConfig",
        ("reasoning", _) => "reasoning_effort",
        ("stop", A) => "stop_sequences",
        ("stop", G) => "generationConfig.stopSequences",
        ("seed", G) => "generationConfig.seed",
        (other, _) => other,
    }
}

fn reasoning_param(g: &GenerateRequest, ingress: DialectId, field: &'static str) -> &'static str {
    match (field, ingress) {
        ("budget", DialectId::OpenAiChat | DialectId::OpenAiResponses) => "reasoning.max_tokens",
        ("effort", DialectId::OpenAiChat)
            if kaeso_codecs::openai_chat::uses_reasoning_object(g) =>
        {
            "reasoning.effort"
        }
        ("reasoning", DialectId::OpenAiChat)
            if kaeso_codecs::openai_chat::uses_reasoning_object(g) =>
        {
            "reasoning"
        }
        _ => app_param(ingress, field),
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
            refuse_reranker(eff)?;
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
            adapted.extend(budget_to_adaptive(g, ingress, eff));
            let mut left_out = leave_out_reasoning(g, ingress, eff);
            left_out.extend(leave_out_sampling(g, ingress, eff));
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
            refuse_transcriber(eff, None)?;
            let Some(dialect) = eff.provider.count_tokens_for(eff.dialect) else {
                return Err(refuse(
                    "token counting",
                    Some("model"),
                    format!("The model `{name}` cannot count tokens."),
                ));
            };
            let mut adapted = adaptive_to_budget(g, ingress, eff);
            adapted.extend(budget_to_adaptive(g, ingress, eff));
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
        Request::Rerank(_) => {
            refuse_decision_only(eff)?;
            if !eff.is_rerank() {
                let kind = if eff.is_embeddings() {
                    "an embeddings model"
                } else {
                    "a generation model"
                };
                return Err(refuse(
                    "rerank",
                    Some("model"),
                    format!("The model `{name}` is {kind}; it cannot rerank documents."),
                ));
            }
            let Some(rerank) = eff.provider.rerank().filter(|r| r.dialect == eff.dialect) else {
                return Err(refuse(
                    "rerank",
                    Some("model"),
                    format!(
                        "Kaeso calls no rerank models on {}, so `{name}` can't rerank documents.",
                        eff.provider.name()
                    ),
                ));
            };
            Ok(media_plan(rerank.dialect))
        }
        Request::Decide(d) => {
            let Some(decisions) = eff.provider.decisions(name) else {
                let message = if eff.provider.has_decisions() && !eff.decides() {
                    generation_only(name)
                } else {
                    no_media_api(
                        eff,
                        "Decisions API",
                        format!(
                            "{} offers no decisions API Kaeso can call, so `{name}` can't make decisions.",
                            eff.provider.name()
                        ),
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
        images += messages
            .iter()
            .flat_map(|m| &m.blocks)
            .filter(is_image)
            .count();
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

// A transcription model isn't served on the provider's generation endpoint, and a request there
// comes back as a 404. Gemini's are, at `generateContent`, where Kaeso sends transcriptions too.
// A transcription model serves only transcriptions. One its provider serves at the generation
// endpoint, as Google serves Gemini's at `generateContent`, takes a generation there that has
// audio to transcribe; a token count is no transcription anywhere.
fn refuse_transcriber(
    eff: &Effective,
    generation: Option<&GenerateRequest>,
) -> Result<(), Unsupported> {
    if !eff.transcribes_only {
        return Ok(());
    }
    let at_generation = eff
        .provider
        .transcriptions(&eff.model_id)
        .is_some_and(|t| t.dialect == eff.dialect);
    if let Some(g) = generation.filter(|_| at_generation) {
        if g.required_capabilities().audio_input {
            return Ok(());
        }
        return Err(refuse(
            "generation",
            Some("model"),
            format!(
                "The model `{}` only transcribes; send it audio.",
                eff.model_id
            ),
        ));
    }
    Err(refuse(
        "generation",
        Some("model"),
        format!(
            "The model `{}` is a transcription model; send it to `/v1/audio/transcriptions`.",
            eff.model_id
        ),
    ))
}

fn refuse_reranker(eff: &Effective) -> Result<(), Unsupported> {
    if !eff.is_rerank() {
        return Ok(());
    }
    Err(refuse(
        "rerank",
        Some("model"),
        format!(
            "The model `{}` is a rerank model; send it to `/v1/rerank`.",
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
    let effort_field = reasoning_param(g, ingress, "effort");
    let reasoning_field = reasoning_param(g, ingress, "reasoning");
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
        left_out.push(effort_field.to_owned());
    }
    if !reasons && r.mode == Some(ReasoningMode::Adaptive) {
        r.mode = None;
        r.extras.fields.shift_remove("display");
        left_out.push(reasoning_field.to_owned());
    }
    let nothing_left = r.mode.is_none()
        && r.effort.is_none()
        && r.budget_tokens.is_none()
        && r.summary.is_none()
        && !r.include_encrypted
        && !r.exclude
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
    let effort_field = reasoning_param(g, ingress, "effort");
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
    vec![effort_field.to_owned()]
}

// Claude Code sends adaptive thinking to every model; a Claude model without it takes
// `enabled` thinking with a budget, which the effort sets as it does for requests from other
// formats ([reasoning_map]).
fn adaptive_to_budget(g: &mut GenerateRequest, ingress: DialectId, eff: &Effective) -> Vec<String> {
    let field = reasoning_param(g, ingress, "reasoning");
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
    vec![field.to_owned()]
}

// Claude Opus 4.7 and later refuse `enabled` thinking, so a budget, such as a Gemini app's
// `thinkingBudget` or an older Anthropic app's `budget_tokens`, would fail on every current
// Claude model; it becomes adaptive thinking with the effort the budget stands for. Anthropic
// took budgets away in the same models as sampling settings, which the catalogue marks as
// taking only the default temperature; Claude 4.6 still takes budgets.
fn budget_to_adaptive(g: &mut GenerateRequest, ingress: DialectId, eff: &Effective) -> Vec<String> {
    let field = reasoning_param(g, ingress, "reasoning");
    let Some(r) = g.reasoning.as_mut() else {
        return Vec::new();
    };
    let refuses_budgets = eff.dialect == DialectId::AnthropicMessages
        && is_claude(eff)
        && eff.fields.reasoning == Some(true)
        && eff.fields.temperature == Some(false)
        && eff.fields.adaptive_thinking != Some(false);
    if !refuses_budgets || !matches!(r.mode, Some(ReasoningMode::Enabled) | None) {
        return Vec::new();
    }
    let Some(budget) = r.budget_tokens.take() else {
        return Vec::new();
    };
    match r.effort {
        Some(ReasoningEffort::None) => r.mode = Some(ReasoningMode::Disabled),
        Some(_) => r.mode = Some(ReasoningMode::Adaptive),
        None => {
            r.mode = Some(ReasoningMode::Adaptive);
            r.effort = Some(reasoning_map::effort_for_budget(budget));
        }
    }
    vec![field.to_owned()]
}

// Apps send a temperature and `top_p` with no switch per model; Claude 5 models take only their
// defaults, so the model answers at its defaults instead of refusing every such app.
fn leave_out_sampling(g: &mut GenerateRequest, ingress: DialectId, eff: &Effective) -> Vec<String> {
    if eff.fields.temperature != Some(false) || kpp_turns_thinking_on(g, ingress, eff) {
        return Vec::new();
    }
    let s = &mut g.sampling;
    let mut left_out = Vec::new();
    if s.temperature
        .is_some_and(|t| (t - 1.0).abs() > f64::EPSILON)
    {
        s.temperature = None;
        left_out.push(app_param(ingress, "temperature").to_owned());
    }
    if !is_claude(eff) {
        return left_out;
    }
    if s.top_p.take().is_some() {
        left_out.push(app_param(ingress, "top_p").to_owned());
    }
    if s.top_k.take().is_some() {
        left_out.push(app_param(ingress, "top_k").to_owned());
    }
    left_out
}

// Claude models that take only their default temperature also refuse `top_p` and `top_k`.
// Other models the catalogue marks that way, such as Gemini 4 on some platforms, take them.
fn is_claude(eff: &Effective) -> bool {
    matches!(
        eff.dialect,
        DialectId::AnthropicMessages | DialectId::BedrockConverse
    ) && eff.model_id.contains("claude")
}

const EFFORT_ORDER: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

// A model that always reasons, such as GPT-OSS, refuses `none` when its effort levels don't list
// it; its lowest level is the least it thinks, so that stands in for thinking off (#643).
fn lowest_effort_for_off(
    g: &mut GenerateRequest,
    ingress: DialectId,
    eff: &Effective,
) -> Vec<String> {
    let effort_field = reasoning_param(g, ingress, "effort");
    let reasoning_field = reasoning_param(g, ingress, "reasoning");
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
    let field = if effort_off {
        effort_field
    } else {
        reasoning_field
    };
    vec![field.to_owned()]
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

// The least `max_output_tokens` OpenAI Responses takes, as the codec's check refuses less.
const RESPONSES_MIN_OUTPUT: u32 = 16;

pub struct ResponsesFit {
    pub cap: Option<OutputCap>,
    pub adapted: Vec<String>,
    pub left_out: Vec<String>,
}

// OpenAI Responses has no `stop` and no `seed` and refuses an output limit under 16 tokens,
// while apps in other formats send all three. Kaeso ends the reply itself at the first stop
// sequence or at the app's limit, as it does for a ChatGPT plan, and leaves out `seed`, which
// OpenAI only ever honoured on a best-effort basis.
pub fn fit_responses(
    g: &mut GenerateRequest,
    ingress: DialectId,
    eff: &Effective,
    cap: Option<OutputCap>,
) -> ResponsesFit {
    let mut fit = ResponsesFit {
        cap,
        adapted: Vec::new(),
        left_out: Vec::new(),
    };
    if g.sampling.seed.take().is_some() {
        fit.left_out.push(app_param(ingress, "seed").to_owned());
    }
    if ingress != DialectId::OpenAiResponses
        && let Some(asked) = g
            .sampling
            .max_output_tokens
            .filter(|m| *m < RESPONSES_MIN_OUTPUT)
    {
        g.sampling.max_output_tokens = Some(RESPONSES_MIN_OUTPUT);
        let cap = OutputCap::new(u64::from(asked));
        fit.cap = Some(if eff.provider.reasoning_beyond_output_limit() {
            cap.of_visible_output()
        } else {
            cap
        });
        fit.adapted.push(output_param(g, ingress).to_owned());
    }
    if !g.sampling.stop.is_empty() {
        let stop = std::mem::take(&mut g.sampling.stop);
        fit.cap = Some(match fit.cap.take() {
            Some(cap) => cap.with_stop_sequences(stop),
            None => OutputCap::at_stop_sequences(stop),
        });
        fit.adapted.push(app_param(ingress, "stop").to_owned());
    }
    fit
}

pub fn output_param(g: &GenerateRequest, ingress: DialectId) -> &'static str {
    match ingress {
        DialectId::AnthropicMessages => "max_tokens",
        DialectId::OpenAiResponses => "max_output_tokens",
        DialectId::Gemini => "generationConfig.maxOutputTokens",
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
    refuse_reranker(eff)?;
    refuse_transcriber(eff, Some(g))?;
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
            Some(reasoning_param(g, ingress, "reasoning")),
            format!("The model `{name}` does not support reasoning."),
        ));
    }
    if let Some(effort) = g.reasoning.as_ref().and_then(|r| r.effort.as_ref()) {
        check_effort(g, effort, ingress, eff)?;
    } else if let Some(budget) = budget_sent_as_effort(g, eff) {
        check_budget(g, budget, ingress, eff)?;
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

fn check_budget(
    g: &GenerateRequest,
    budget: u32,
    ingress: DialectId,
    eff: &Effective,
) -> Result<(), Unsupported> {
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
        Some(reasoning_param(g, ingress, "budget")),
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
    g: &GenerateRequest,
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
        Some(reasoning_param(g, ingress, "effort")),
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
