//! TypeSafe's System One API (`POST /v1/systemone`): typed questions about a state, answered with
//! probabilities, as TypeSafe and the providers that took its format serve it.
//!
//! A request is JSON, or a `multipart/form-data` upload whose first part, `request`, holds the
//! JSON and whose other parts hold the images its `assets` name. Images reach KPP as
//! [`DecideRequest::assets`](kaeso_kpp::DecideRequest::assets) and the references that stand for
//! them; the [`SystemOneProfile`] of the provider decides where they are sent.

mod assets;
pub(crate) mod error;
mod profile;
mod request;
mod response;
mod wire;

#[cfg(test)]
mod tests;

pub use profile::{ContextLimits, Envelope, IdRule, MediaPlacement, ModelField, SystemOneProfile};

use kaeso_kpp::{
    DialectId, KppError, ModelInfo, ModelList, ModelsQuery, Origin, Reply, Request, RequestKind,
    StreamEvent,
};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};
use crate::multipart::Upload;
use crate::wire::{Ext, extras_from};

pub(crate) const DIALECT: DialectId = DialectId::SystemOne;

// TypeSafe's own listing, which names each model and alias under `models`; its description and
// release date stay in extras.
fn typesafe_models(body: &[u8]) -> Option<ModelList> {
    let Ext {
        known: w,
        extra,
        verbatim,
    } = Ext::<wire::ModelsWire>::from_slice(body).ok()?;
    let origin = Origin::new(DIALECT);
    let models = w
        .models
        .into_iter()
        .map(|m| {
            let mut info = ModelInfo::new(m.known.name);
            info.extras = extras_from(&origin, m.extra, m.verbatim);
            info
        })
        .collect();
    let mut list = ModelList::new(models);
    list.extras = extras_from(&origin, extra, verbatim);
    Some(list)
}

/// The path apps send System One requests to.
pub const SYSTEM_ONE_PATH: &str = "/v1/systemone";

// No System One listing is served: no header tells TypeSafe's SDK from OpenAI's, so model
// listings are OpenAI's.
const LISTING: DialectId = DialectId::OpenAiChat;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct SystemOne;

impl crate::codec::sealed::Sealed for SystemOne {}
impl crate::codec::sealed::Sealed for NoStreamEncoder {}
impl crate::codec::sealed::Sealed for NoStreamDecoder {}

fn no_streaming() -> KppError {
    KppError::unsupported_by(
        "streaming",
        DIALECT,
        "Decision requests return one complete response; send the request without streaming.",
    )
}

struct NoStreamEncoder;

impl StreamEncoder for NoStreamEncoder {
    fn encode(&mut self, _event: &StreamEvent) -> Result<Vec<u8>, KppError> {
        Err(no_streaming())
    }

    fn finish(&mut self) -> Result<Vec<u8>, KppError> {
        Ok(Vec::new())
    }
}

struct NoStreamDecoder {
    origin: Origin,
    done: bool,
}

impl NoStreamDecoder {
    fn error(&mut self) -> Vec<StreamEvent> {
        if self.done {
            return Vec::new();
        }
        self.done = true;
        vec![StreamEvent::Error(
            no_streaming().with_origin(self.origin.clone()),
        )]
    }
}

impl StreamDecoder for NoStreamDecoder {
    fn feed(&mut self, _bytes: &[u8]) -> Vec<StreamEvent> {
        self.error()
    }

    fn finish(&mut self) -> Vec<StreamEvent> {
        self.error()
    }
}

impl SystemOne {
    pub(crate) fn decode_checked(
        kind: RequestKind,
        body: Upload<'_>,
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        request::decode(kind, body, headers)
    }
}

impl Codec for SystemOne {
    const DIALECT: DialectId = DIALECT;

    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::limits::request_shape(Self::DIALECT, body)?;
        Self::decode_checked(kind, Upload::Borrowed(body), headers)
    }

    fn encode_response(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        response::encode(reply, ctx)
    }

    fn stream_encoder(_ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        Box::new(NoStreamEncoder)
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

    fn decode_response(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
        ctx: &EgressCtx,
    ) -> Result<Reply, KppError> {
        response::decode(status, headers, body, ctx)
    }

    fn stream_decoder(ctx: &EgressCtx) -> Box<dyn StreamDecoder> {
        Box::new(NoStreamDecoder {
            origin: ctx.origin(DIALECT),
            done: false,
        })
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        error::decode(status, headers, body)
    }

    fn decode_models_query(query: &str) -> Result<ModelsQuery, KppError> {
        crate::openai_models::decode_query(LISTING, query)
    }

    fn encode_models(list: &ModelList) -> Result<Vec<u8>, KppError> {
        crate::openai_models::encode_models(list)
    }

    fn encode_model(model: &ModelInfo) -> Result<Vec<u8>, KppError> {
        crate::openai_models::encode_model(model)
    }

    fn decode_models(
        status: u16,
        headers: &HeaderList,
        body: &[u8],
    ) -> Result<ModelList, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        crate::openai_models::decode_models(LISTING, body)
            .or_else(|e| typesafe_models(body).ok_or(e))
    }

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        crate::openai_models::decode_model(LISTING, body)
    }

    fn encode_model_request(model: &str) -> EncodedRequest {
        crate::openai_models::retrieve_request(model)
    }

    fn encode_models_request(query: &ModelsQuery) -> Result<EncodedRequest, KppError> {
        crate::openai_models::list_request(LISTING, query)
    }
}
