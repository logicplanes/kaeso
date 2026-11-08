//! ElevenLabs' API (<https://api.elevenlabs.io/openapi.json>, read 2026-11-08): text to speech,
//! text to dialogue, sound effects, music, the voice changer and speech to text, the voice and
//! model listings, and its errors.

mod error;
mod format;
mod listing;
mod reply;
mod request;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_edges;

use kaeso_kpp::{
    DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Reply, Request, RequestKind,
};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};

pub use listing::{VOICES_PATHS, voice_listing_request, voices_reply};
pub use reply::billed_characters;

const D: DialectId = DialectId::ElevenLabs;

/// What an ElevenLabs path asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ElevenLabsOperation {
    /// `POST /v1/text-to-speech/{voice_id}`.
    TextToSpeech,
    /// `POST /v1/text-to-dialogue`.
    TextToDialogue,
    /// `POST /v1/sound-generation`.
    SoundGeneration,
    /// `POST /v1/music`.
    Music,
    /// `POST /v1/voice-changer/{voice_id}`.
    VoiceChanger,
    /// `POST /v1/speech-to-text`.
    SpeechToText,
}

/// An ElevenLabs call as its path and query name it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ElevenLabsCall {
    pub operation: ElevenLabsOperation,
    pub voice_id: Option<String>,
    /// The `/stream` form of the path, which sends the audio as it is made.
    pub stream: bool,
    /// The `/with-timestamps` form, which adds the time of each character.
    pub timestamps: bool,
    /// The query string, without its `?`.
    pub query: String,
}

impl ElevenLabsCall {
    /// The call a path names, such as `/v1/text-to-speech/{voice_id}/stream/with-timestamps`,
    /// or `None` for a path that is no generation call.
    pub fn from_path(path: &str, query: &str) -> Option<ElevenLabsCall> {
        let rest = path.strip_prefix("/v1/")?;
        let parts: Vec<&str> = rest.split('/').collect();
        let tail = |from: usize| -> Option<(bool, bool)> {
            match parts.get(from..)? {
                [] => Some((false, false)),
                ["stream"] => Some((true, false)),
                ["with-timestamps"] => Some((false, true)),
                ["stream", "with-timestamps"] => Some((true, true)),
                _ => None,
            }
        };
        let voice = |i: usize| {
            parts
                .get(i)
                .filter(|v| !v.is_empty())
                .map(|v| (*v).to_owned())
        };
        let (operation, voice_id, (stream, timestamps)) = match *parts.first()? {
            "text-to-speech" => (ElevenLabsOperation::TextToSpeech, Some(voice(1)?), tail(2)?),
            "text-to-dialogue" => (ElevenLabsOperation::TextToDialogue, None, tail(1)?),
            "sound-generation" if parts.len() == 1 => {
                (ElevenLabsOperation::SoundGeneration, None, (false, false))
            }
            "music" => match parts.get(1..)? {
                [] => (ElevenLabsOperation::Music, None, (false, false)),
                ["stream"] => (ElevenLabsOperation::Music, None, (true, false)),
                _ => return None,
            },
            "voice-changer" => {
                let (stream, timestamps) = tail(2)?;
                if timestamps {
                    return None;
                }
                (
                    ElevenLabsOperation::VoiceChanger,
                    Some(voice(1)?),
                    (stream, false),
                )
            }
            "speech-to-text" if parts.len() == 1 => {
                (ElevenLabsOperation::SpeechToText, None, (false, false))
            }
            _ => return None,
        };
        Some(ElevenLabsCall {
            operation,
            voice_id,
            stream,
            timestamps,
            query: query.to_owned(),
        })
    }

    /// The kind of KPP request the call becomes.
    pub fn kind(&self) -> RequestKind {
        match self.operation {
            ElevenLabsOperation::SpeechToText => RequestKind::Transcription,
            _ => RequestKind::Speech,
        }
    }
}

/// ElevenLabs' API, for ElevenLabs-format apps and as a provider. Apps' requests name their
/// operation and voice in the path and are decoded with
/// [`crate::dispatch::decode_elevenlabs_request`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ElevenLabs;

impl crate::codec::sealed::Sealed for ElevenLabs {}
impl crate::codec::sealed::Sealed for reply::Decoder {}
impl crate::codec::sealed::Sealed for reply::Encoder {}

impl ElevenLabs {
    pub(crate) fn decode_call(
        call: &ElevenLabsCall,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        request::decode(call, body, headers)
    }
}

impl Codec for ElevenLabs {
    const DIALECT: DialectId = D;

    /// Decodes a body without its path as text to speech with no voice; apps' requests go
    /// through [`crate::dispatch::decode_elevenlabs_request`].
    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        let operation = match kind {
            RequestKind::Transcription => ElevenLabsOperation::SpeechToText,
            _ => ElevenLabsOperation::TextToSpeech,
        };
        let call = ElevenLabsCall {
            operation,
            voice_id: None,
            stream: false,
            timestamps: false,
            query: String::new(),
        };
        request::decode(&call, body, headers)
    }

    fn encode_response(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        reply::encode(reply, ctx)
    }

    fn stream_encoder(ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        Box::new(reply::Encoder::new(ctx))
    }

    fn response_media_type(reply: &Reply, ctx: &EncodeCtx) -> String {
        reply::media_type(Some(reply), ctx)
    }

    fn stream_media_type(ctx: &EncodeCtx) -> String {
        reply::media_type(None, ctx)
    }

    fn encode_error(err: &KppError) -> EncodedError {
        error::encode(err)
    }

    fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
        request::check(req, ctx)
    }

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        request::encode(req, ctx)
    }

    fn reads_reply_as_stream(ctx: &EgressCtx) -> bool {
        ctx.kind == RequestKind::Speech && !ctx.speech_reply_json
    }

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        reply::decode(status, headers, body, ctx)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        Box::new(reply::Decoder::new(ctx))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        error::decode(status, headers, body)
    }

    fn decode_models_query(query: &str) -> Result<ModelsQuery, KppError> {
        listing::decode_query(query)
    }

    fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError> {
        listing::encode_models(list)
    }

    fn encode_model(model: &ModelInfo) -> Result<Vec<u8>, KppError> {
        listing::encode_model(model)
    }

    fn decode_models(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
    ) -> Result<ModelList, KppError> {
        if !(200..300).contains(&status) {
            return Err(error::decode(status, headers, body));
        }
        listing::decode_models(body)
    }

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError> {
        if !(200..300).contains(&status) {
            return Err(error::decode(status, headers, body));
        }
        listing::decode_model(body)
    }

    fn encode_model_request(_model: &str) -> EncodedRequest {
        EncodedRequest::get(listing::MODELS_PATH, HeaderList::new())
    }

    fn encode_models_request(_query: &ModelsQuery) -> Result<EncodedRequest, KppError> {
        Ok(EncodedRequest::get(listing::MODELS_PATH, HeaderList::new()))
    }
}
