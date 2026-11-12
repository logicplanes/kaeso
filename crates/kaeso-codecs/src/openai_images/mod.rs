mod profile;
mod request;
mod response;
mod stream;
mod wire;

#[cfg(test)]
mod tests;

use kaeso_kpp::{
    DialectId, ImageOperation, KppError, ModelInfo, ModelList, ModelsQuery, Reply, Request,
    RequestKind,
};

use crate::codec::{
    Codec, EgressCtx, EncodeCtx, EncodedError, EncodedRequest, HeaderList, StreamDecoder,
    StreamEncoder,
};

pub use profile::{DeliveryForm, EditsForm, ImageField, ImagesProfile, SizeForm, UserField};
pub use request::{MAX_FORM_PARTS, MAX_IMAGE_BYTES};

const DIALECT: DialectId = DialectId::OpenAiImages;

/// OpenAI's Images API: `/v1/images/generations`, `/v1/images/edits` (JSON or
/// `multipart/form-data`) and `/v1/images/variations`, with streamed partial images, and the
/// providers that speak a form of it, each described by an [`ImagesProfile`]. Apps' requests
/// name their operation in the path and are decoded with
/// [`crate::dispatch::decode_image_request`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct OpenAiImages;

impl crate::codec::sealed::Sealed for OpenAiImages {}
impl crate::codec::sealed::Sealed for stream::Decoder {}
impl crate::codec::sealed::Sealed for stream::Encoder {}

impl OpenAiImages {
    pub(crate) fn decode_call(
        operation: ImageOperation,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        request::decode(operation, crate::multipart::Upload::Borrowed(body), headers)
    }

    pub(crate) fn decode_upload(
        operation: ImageOperation,
        body: crate::multipart::Upload<'_>,
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        request::decode(operation, body, headers)
    }

    pub(crate) fn guess_operation(body: &[u8], headers: &HeaderList) -> ImageOperation {
        request::guess_operation(body, headers)
    }

    pub(crate) fn decode_checked(
        _kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        request::decode(
            request::guess_operation(body, headers),
            crate::multipart::Upload::Borrowed(body),
            headers,
        )
    }
}

pub(crate) fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
    crate::openai_responses::error::decode(DIALECT, status, headers, body)
}

impl Codec for OpenAiImages {
    const DIALECT: DialectId = DIALECT;

    /// Decodes a body without its path: an upload or a JSON body with `images` is an edit,
    /// anything else a generation.
    fn decode_request(
        kind: RequestKind,
        body: &[u8],
        headers: &HeaderList,
    ) -> Result<Request, KppError> {
        crate::decisions::refuse_kind(kind, Self::DIALECT)?;
        crate::limits::request_shape(DIALECT, body)?;
        Self::decode_checked(kind, body, headers)
    }

    fn encode_response(reply: &Reply, ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
        crate::decisions::refuse_reply(reply, Self::DIALECT)?;
        response::encode(reply, ctx)
    }

    fn stream_encoder(ctx: &EncodeCtx) -> Box<dyn StreamEncoder> {
        Box::new(stream::Encoder::for_ctx(ctx))
    }

    fn encode_error(err: &KppError) -> EncodedError {
        crate::openai_responses::error::encode(err)
    }

    fn check(req: &Request, ctx: &EgressCtx) -> Result<(), KppError> {
        crate::decisions::refuse_request(req, Self::DIALECT)?;
        request::check(req, ctx)
    }

    fn encode_request(req: &Request, ctx: &mut EgressCtx) -> Result<EncodedRequest, KppError> {
        crate::decisions::refuse_request(req, Self::DIALECT)?;
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
        Box::new(stream::Decoder::new(ctx))
    }

    fn decode_error(status: u16, headers: &HeaderList, body: &[u8]) -> KppError {
        decode_error(status, headers, body)
    }

    fn decode_models_query(query: &str) -> Result<ModelsQuery, KppError> {
        crate::openai_models::decode_query(DIALECT, query)
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
        crate::openai_models::decode_models(DIALECT, body)
    }

    fn decode_model(status: u16, headers: &HeaderList, body: &[u8]) -> Result<ModelInfo, KppError> {
        if !(200..300).contains(&status) {
            return Err(Self::decode_error(status, headers, body));
        }
        crate::openai_models::decode_model(DIALECT, body)
    }

    fn encode_model_request(model: &str) -> EncodedRequest {
        crate::openai_models::retrieve_request(model)
    }

    fn encode_models_request(query: &ModelsQuery) -> Result<EncodedRequest, KppError> {
        crate::openai_models::list_request(DIALECT, query)
    }
}
