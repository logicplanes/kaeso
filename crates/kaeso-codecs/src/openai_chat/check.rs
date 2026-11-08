use kaeso_kpp::{
    AudioFormat, Block, DocumentBlock, GenerateRequest, ImageBlock, KppError, Modality, Origin,
    ReasoningEffort, ReasoningMode, Request, Role, Source,
};

use super::DIALECT;
use super::profile::ChatProfile;
use crate::cross::{self, Setting, app_field};

const MAX_STOP: usize = 4;

pub(crate) fn check(req: &Request, target: &Origin, profile: &ChatProfile) -> Result<(), KppError> {
    let r = match req {
        Request::Generate(r) => r,
        Request::CountTokens(_) => {
            return Err(KppError::unsupported_by(
                "token counting",
                DIALECT,
                "Chat Completions has no token-counting endpoint; use an Anthropic or OpenAI Responses model.",
            ));
        }
        _ => {
            return Err(KppError::unsupported_by(
                super::kind_feature(req.kind()),
                DIALECT,
                "Send embeddings requests to OpenAI Embeddings.",
            ));
        }
    };
    cross::check_foreign(req, target)?;
    check_generate(r, profile)?;
    r.validate()
}

fn unsupported(feature: &str, hint: &str, param: impl Into<String>) -> KppError {
    KppError::unsupported_by(feature, DIALECT, hint).with_param(param)
}

// Chat Completions writes text and audio (`ResponseModalities`), with a built-in voice and one
// of six formats (`CreateChatCompletionRequest.audio`).
fn check_output(r: &GenerateRequest) -> Result<(), KppError> {
    if r.extras.is_from(DIALECT) {
        return Ok(());
    }
    let modalities = app_field(r, Setting::Modalities, DIALECT);
    for m in &r.output_modalities {
        match m {
            Modality::Text | Modality::Audio => {}
            Modality::Image => {
                return Err(unsupported(
                    "image output",
                    "It returns text and audio only; ask a Gemini image model from a Gemini app.",
                    modalities,
                ));
            }
            other => {
                return Err(unsupported(
                    &format!("output modality `{}`", other.as_wire()),
                    "It returns text and audio only.",
                    modalities,
                ));
            }
        }
    }
    if !r.output_modalities.contains(&Modality::Audio) {
        if r.audio_output.is_some() {
            return Err(unsupported(
                "audio output settings without audio output",
                "Its audio settings go with audio replies only; ask for audio or remove them.",
                app_field(r, Setting::Voice, DIALECT),
            ));
        }
        return Ok(());
    }
    let voice_param = app_field(r, Setting::Voice, DIALECT);
    let audio = r.audio_output.as_ref();
    match audio.and_then(|a| a.voice.as_deref()) {
        None => Err(unsupported(
            "audio output without a voice",
            "It needs a voice for audio replies; name one of OpenAI's voices, such as `marin`.",
            voice_param,
        )),
        Some(v) if !crate::media::has_voice(crate::media::OPENAI_VOICES, v) => Err(unsupported(
            &format!("voice `{v}`"),
            "Its voices are alloy, ash, ballad, coral, echo, fable, nova, onyx, sage, shimmer, verse, marin and cedar.",
            voice_param,
        )),
        Some(_) => match audio.and_then(|a| a.format.as_ref()) {
            Some(AudioFormat::Other(f)) => Err(unsupported(
                &format!("audio format `{f}`"),
                "Its audio formats are wav, mp3, flac, opus, aac and pcm16.",
                app_field(r, Setting::AudioFormat, DIALECT),
            )),
            _ => Ok(()),
        },
    }
}

fn check_generate(r: &GenerateRequest, profile: &ChatProfile) -> Result<(), KppError> {
    check_output(r)?;
    if r.sampling.top_k.is_some() {
        return Err(unsupported(
            "top_k",
            "Remove `top_k`, or route the request to an Anthropic model.",
            "top_k",
        ));
    }
    if r.sampling.stop.len() > MAX_STOP {
        let field = app_field(r, Setting::StopSequences, DIALECT);
        return Err(unsupported(
            field,
            "It accepts at most four stop sequences.",
            field,
        ));
    }
    if let Some(reasoning) = &r.reasoning {
        if let Some(ReasoningMode::Other(value)) = &reasoning.mode {
            return Err(unsupported(
                "reasoning.mode",
                &format!(
                    "It does not know the reasoning mode `{value}`; remove the thinking setting or use `reasoning_effort`."
                ),
                app_field(r, Setting::Reasoning, DIALECT),
            ));
        }
        let own = reasoning.extras.is_from(DIALECT);
        if own && reasoning.summary.is_some() {
            return Err(unsupported(
                "reasoning.summary",
                "It returns no reasoning summaries; use OpenAI Responses for them.",
                "reasoning",
            ));
        }
        if own && reasoning.include_encrypted {
            return Err(unsupported(
                "reasoning.include_encrypted",
                "It returns no encrypted reasoning; use OpenAI Responses for it.",
                "reasoning",
            ));
        }
        if let Some(ReasoningEffort::Other(value)) = &reasoning.effort
            && !reasoning.extras.is_from(DIALECT)
        {
            return Err(unsupported(
                "reasoning.effort",
                &format!(
                    "It does not know the effort `{value}`; use none, minimal, low, medium, high, xhigh or max."
                ),
                app_field(r, Setting::ReasoningEffort, DIALECT),
            ));
        }
    }
    for (i, block) in r.system.iter().enumerate() {
        check_block(block, Place::Instruction, &format!("system[{i}]"), profile)?;
    }
    for (mi, message) in r.messages.iter().enumerate() {
        let place = match message.role {
            Role::User => Place::User,
            Role::Assistant => Place::Assistant,
            _ => Place::Instruction,
        };
        for (bi, block) in message.blocks.iter().enumerate() {
            check_block(
                block,
                place,
                &format!("messages[{mi}].blocks[{bi}]"),
                profile,
            )?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Instruction,
    User,
    Assistant,
}

impl Place {
    fn name(self) -> &'static str {
        match self {
            Place::Instruction => "system, developer and tool messages",
            Place::User => "user messages",
            Place::Assistant => "assistant messages",
        }
    }
}

fn misplaced(kind: &str, place: Place, path: &str) -> KppError {
    unsupported(
        &format!("{kind} in {}", place.name()),
        &format!("It only accepts that content in other roles; move or remove the {kind}."),
        path,
    )
}

fn check_block(
    block: &Block,
    place: Place,
    path: &str,
    profile: &ChatProfile,
) -> Result<(), KppError> {
    match block {
        Block::Text(t) => {
            let own = place == Place::Assistant && t.citations.iter().all(|c| c.origin.is(DIALECT));
            if !t.citations.is_empty() && !own {
                return Err(unsupported(
                    "citations",
                    "Only assistant messages carry citations (as `annotations`); remove them.",
                    path,
                ));
            }
            Ok(())
        }
        Block::Image(image) => {
            if place != Place::User {
                return Err(misplaced("image", place, path));
            }
            check_image(image, path)
        }
        Block::Document(doc) => {
            if place != Place::User {
                return Err(misplaced("document", place, path));
            }
            check_document(doc, path)
        }
        Block::Audio(audio) if place == Place::Assistant => {
            if matches!(audio.source, Source::FileId(_)) || audio.transcript.is_some() {
                return Ok(());
            }
            Err(unsupported(
                "audio in assistant messages",
                "It replays an earlier audio reply only by OpenAI's audio id; send the reply's transcript as text instead.",
                path,
            ))
        }
        Block::Audio(audio) => {
            if place != Place::User {
                return Err(misplaced("audio", place, path));
            }
            match &audio.source {
                Source::Base64(_) => Ok(()),
                _ => Err(unsupported(
                    "audio source",
                    "Its `input_audio` takes base64 data only; send the audio as base64.",
                    path,
                )),
            }
        }
        Block::Video(_) if !profile.request.video_url => Err(unsupported(
            "video block",
            "It has no video content part; send frames of the video as images, or route the request to a model that takes video.",
            path,
        )),
        Block::Video(video) => {
            if place != Place::User {
                return Err(misplaced("video", place, path));
            }
            match &video.source {
                Source::Url(_) | Source::Base64(_) => Ok(()),
                _ => Err(unsupported(
                    "video source",
                    "Its `video_url` takes a URL or base64 data; send the video one of those ways.",
                    path,
                )),
            }
        }
        Block::Refusal(_) => {
            if place != Place::Assistant {
                return Err(misplaced("refusal", place, path));
            }
            Ok(())
        }
        Block::ToolResult(result) => {
            for (i, nested) in result.content.iter().enumerate() {
                check_block(
                    nested,
                    Place::Instruction,
                    &format!("{path}.content[{i}]"),
                    profile,
                )?;
            }
            Ok(())
        }
        Block::ToolCall(_) | Block::Reasoning(_) | Block::Unknown(_) => Ok(()),
        other => Err(unsupported(
            &format!("{} block", other.kind()),
            "Remove the block or send the request to a model served through its original format.",
            path,
        )),
    }
}

fn check_image(image: &ImageBlock, path: &str) -> Result<(), KppError> {
    match &image.source {
        Source::Url(_) | Source::Base64(_) => Ok(()),
        Source::FileId(_) => Err(unsupported(
            "image file id",
            "Images must be sent as a URL or base64 data.",
            path,
        )),
        _ => Err(unsupported(
            "image source",
            "Images must be sent as a URL or base64 data.",
            path,
        )),
    }
}

fn check_document(doc: &DocumentBlock, path: &str) -> Result<(), KppError> {
    match &doc.source {
        Source::Base64(_) | Source::FileId(_) => {}
        Source::Text(_) => {
            return Err(unsupported(
                "text document",
                "Send the text as a text content part instead, or route the request to an Anthropic model.",
                path,
            ));
        }
        Source::Url(_) => {
            return Err(unsupported(
                "document URL",
                "Send the PDF as base64 data or an uploaded file id instead.",
                path,
            ));
        }
        _ => {
            return Err(unsupported(
                "document source",
                "Send the PDF as base64 data or an uploaded file id instead.",
                path,
            ));
        }
    }
    if doc.citations {
        return Err(unsupported(
            "document citations",
            "Route the request to an Anthropic model to cite documents.",
            path,
        ));
    }
    if doc.title.is_some() {
        return Err(unsupported(
            "document title",
            "Remove the title or put it in a text part next to the document.",
            path,
        ));
    }
    if doc.context.is_some() {
        return Err(unsupported(
            "document context",
            "Remove the context or put it in a text part next to the document.",
            path,
        ));
    }
    Ok(())
}
