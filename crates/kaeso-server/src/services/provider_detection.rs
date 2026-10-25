use std::time::Duration;

use bytes::Bytes;
use kaeso_codecs::anthropic_messages::{COUNT_TOKENS_PATH, MESSAGES_PATH};
use kaeso_codecs::openai_chat::CHAT_COMPLETIONS_PATH;
use kaeso_codecs::openai_embeddings::EMBEDDINGS_PATH;
use kaeso_codecs::openai_responses::{INPUT_TOKENS_PATH, RESPONSES_PATH};
use kaeso_codecs::{HeaderList, dispatch};
use kaeso_core::providers::CustomAuth;
use kaeso_kpp::{DialectId, ModelInfo, ModelsQuery};
use kaeso_upstream::{Call, Outgoing, ResponseBody, Timeouts, UpstreamError};
use secrecy::SecretString;
use url::Url;

use crate::state::AppState;

const MAX_LISTING_PAGES: usize = 50;

const LONGEST_WAIT: Duration = Duration::from_secs(30);
const DETECTION_BUDGET: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub auth: CustomAuth,
    pub generation: DialectId,
    pub embeddings: bool,
    pub count_tokens: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fixed {
    pub auth: Option<CustomAuth>,
    pub generation: Option<DialectId>,
    pub embeddings: Option<bool>,
    pub count_tokens: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedModel {
    pub id: String,
    pub context_limit: Option<u64>,
    pub output_limit: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detected {
    pub settings: Settings,
    pub models: Option<Vec<DetectedModel>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    Generation(&'static str),
    Embeddings,
    CountTokens(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Unreachable,
    AddressRefused,
    KeyNeeded,
    KeyRefused,
    Unreadable,
    NoApi,
    Answered(u16),
    Missing(Missing),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    Present,
    Missing,
    Refused,
    Unclear,
}

impl Route {
    // A body of `{}` is refused by a route that exists before anything is generated or
    // billed; frameworks answer a route they don't have with 404, 405 or 501.
    fn of(status: u16) -> Route {
        match status {
            200..=299 | 400 | 413 | 415 | 422 => Route::Present,
            404 | 405 | 501 => Route::Missing,
            401 | 403 => Route::Refused,
            _ => Route::Unclear,
        }
    }
}

#[derive(Debug)]
enum Listing {
    Anthropic(Vec<ModelInfo>),
    OpenAi(Vec<ModelInfo>),
    Missing,
    Refused,
}

pub fn generation_path(dialect: DialectId) -> &'static str {
    match dialect {
        DialectId::OpenAiResponses => RESPONSES_PATH,
        DialectId::AnthropicMessages => MESSAGES_PATH,
        _ => CHAT_COMPLETIONS_PATH,
    }
}

pub fn counting_path(dialect: DialectId) -> Option<&'static str> {
    match dialect {
        DialectId::OpenAiResponses => Some(INPUT_TOKENS_PATH),
        DialectId::AnthropicMessages => Some(COUNT_TOKENS_PATH),
        _ => None,
    }
}

// vLLM's model card names the context window `max_model_len`, OpenRouter's and LM Studio's
// `context_length`; the OpenAI format has no field for it, so the codec keeps them as extras.
// https://docs.vllm.ai/en/latest/serving/openai_compatible_server.html
// https://openrouter.ai/docs/api-reference/list-available-models
const CONTEXT_EXTRAS: [&str; 2] = ["max_model_len", "context_length"];

pub fn context_limit(m: &ModelInfo) -> Option<u64> {
    m.context_window.or_else(|| {
        CONTEXT_EXTRAS
            .iter()
            .find_map(|name| m.extras.fields.get(*name)?.get().parse().ok())
    })
}

fn detected(models: Vec<ModelInfo>) -> Vec<DetectedModel> {
    let mut out: Vec<DetectedModel> = models
        .into_iter()
        .map(|m| DetectedModel {
            context_limit: context_limit(&m),
            output_limit: m.max_output_tokens,
            id: m.id,
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out.dedup_by(|a, b| a.id == b.id);
    out
}

struct Prober<'a> {
    state: &'a AppState,
    base_url: &'a Url,
    key: &'a SecretString,
    headers: HeaderList,
}

impl Prober<'_> {
    async fn send(
        &self,
        auth: CustomAuth,
        request: Outgoing,
    ) -> Result<(u16, HeaderList, Bytes), Failure> {
        let wait = self
            .state
            .config
            .upstream
            .first_byte_timeout
            .min(LONGEST_WAIT);
        let resp = self
            .state
            .upstream
            .send(Call {
                base_url: self.base_url,
                auth: auth.style(),
                guarded: true,
                credential: self.key,
                request,
                stream: false,
                timeouts: Timeouts {
                    request: wait,
                    first_byte: wait,
                    idle: wait,
                },
                max_body: self.state.config.body_limits.provider_response.as_usize(),
            })
            .await
            .map_err(|e| match e {
                UpstreamError::Refused(_) => Failure::AddressRefused,
                e => {
                    tracing::info!(error = %e, "a server Kaeso detects a custom provider's settings for did not answer");
                    Failure::Unreachable
                }
            })?;
        match resp.body {
            ResponseBody::Full(body) => Ok((resp.status, resp.headers, body)),
            ResponseBody::Stream(_) => Err(Failure::Unreadable),
        }
    }

    async fn page(
        &self,
        auth: CustomAuth,
        dialect: DialectId,
        query: &ModelsQuery,
    ) -> Result<(u16, HeaderList, Bytes), Failure> {
        let request = dispatch::encode_models_request(dialect, query).map_err(|e| {
            tracing::warn!(error = %e.message, "cannot ask a server for its model listing");
            Failure::Unreadable
        })?;
        self.send(auth, Outgoing::from(request)).await
    }

    async fn listing(&self, auth: CustomAuth) -> Result<Listing, Failure> {
        let mut query = ModelsQuery::default();
        let (status, headers, body) = self
            .page(auth, DialectId::AnthropicMessages, &query)
            .await?;
        match Route::of(status) {
            Route::Refused => return Ok(Listing::Refused),
            Route::Missing => return Ok(Listing::Missing),
            _ if !(200..300).contains(&status) => return Err(Failure::Answered(status)),
            _ => {}
        }
        if let Ok(first) =
            dispatch::decode_models(DialectId::AnthropicMessages, status, &headers, &body)
            && !first.models.is_empty()
        {
            let mut models = Vec::new();
            let mut page = first;
            for _ in 0..MAX_LISTING_PAGES {
                let last = page.models.last().map(|m| m.id.clone());
                models.extend(page.models);
                if !page.has_more || last.is_none() || last == query.after_id {
                    break;
                }
                query.after_id = last;
                let (status, headers, body) = self
                    .page(auth, DialectId::AnthropicMessages, &query)
                    .await?;
                page =
                    dispatch::decode_models(DialectId::AnthropicMessages, status, &headers, &body)
                        .map_err(|_| Failure::Unreadable)?;
            }
            return Ok(Listing::Anthropic(models));
        }
        dispatch::decode_models(DialectId::OpenAiChat, status, &headers, &body)
            .map(|list| Listing::OpenAi(list.models))
            .map_err(|_| Failure::Unreadable)
    }

    async fn route(&self, auth: CustomAuth, path: &str) -> Result<Route, Failure> {
        let (status, _, _) = self
            .send(
                auth,
                Outgoing {
                    method: "POST",
                    path: path.to_owned(),
                    headers: self.headers.clone(),
                    body: Bytes::from_static(b"{}"),
                },
            )
            .await?;
        Ok(Route::of(status))
    }

    async fn serves(&self, auth: CustomAuth, path: &str) -> Result<bool, Failure> {
        Ok(self.route(auth, path).await? == Route::Present)
    }

    async fn offers(&self, auth: CustomAuth, path: &str) -> Result<bool, Failure> {
        match self.serves(auth, path).await {
            Err(Failure::Unreachable) => Ok(false),
            served => served,
        }
    }

    async fn accepting(
        &self,
        auths: &[CustomAuth],
        refused: CustomAuth,
        path: &str,
    ) -> Result<Option<CustomAuth>, Failure> {
        for &auth in auths.iter().filter(|&&a| a != refused) {
            if self.route(auth, path).await? != Route::Refused {
                return Ok(Some(auth));
            }
        }
        Ok(None)
    }

    async fn unlisted(&self, auths: &[CustomAuth]) -> Result<(CustomAuth, DialectId), Failure> {
        let mut refused = false;
        for &auth in auths {
            for dialect in [
                DialectId::OpenAiChat,
                DialectId::OpenAiResponses,
                DialectId::AnthropicMessages,
            ] {
                match self.route(auth, generation_path(dialect)).await? {
                    Route::Present => return Ok((auth, dialect)),
                    Route::Refused => refused = true,
                    Route::Missing | Route::Unclear => {}
                }
            }
        }
        Err(if refused {
            Failure::KeyRefused
        } else {
            Failure::NoApi
        })
    }

    async fn generation(&self, auth: CustomAuth, listing: &Listing) -> Result<DialectId, Failure> {
        if matches!(listing, Listing::Anthropic(_)) {
            if self.route(auth, MESSAGES_PATH).await? == Route::Missing
                && self.serves(auth, CHAT_COMPLETIONS_PATH).await?
            {
                return Ok(DialectId::OpenAiChat);
            }
            return Ok(DialectId::AnthropicMessages);
        }
        if self.route(auth, CHAT_COMPLETIONS_PATH).await? != Route::Missing {
            return Ok(DialectId::OpenAiChat);
        }
        if self.serves(auth, RESPONSES_PATH).await? {
            return Ok(DialectId::OpenAiResponses);
        }
        if self.serves(auth, MESSAGES_PATH).await? {
            return Ok(DialectId::AnthropicMessages);
        }
        Ok(DialectId::OpenAiChat)
    }
}

pub async fn detect(
    state: &AppState,
    base_url: &Url,
    key: Option<&SecretString>,
    fixed: Fixed,
) -> Result<Detected, Failure> {
    tokio::time::timeout(DETECTION_BUDGET, settings(state, base_url, key, fixed))
        .await
        .unwrap_or(Err(Failure::Unreachable))
}

async fn settings(
    state: &AppState,
    base_url: &Url,
    key: Option<&SecretString>,
    fixed: Fixed,
) -> Result<Detected, Failure> {
    let auths: Vec<CustomAuth> = match (fixed.auth, key) {
        (Some(CustomAuth::None), _) => vec![CustomAuth::None],
        (Some(_), None) => return Err(Failure::KeyNeeded),
        (Some(auth), Some(_)) => vec![auth],
        (None, Some(_)) => vec![CustomAuth::Bearer, CustomAuth::XApiKey],
        (None, None) => vec![CustomAuth::None],
    };
    let none = SecretString::from(String::new());
    let headers =
        dispatch::encode_models_request(DialectId::AnthropicMessages, &ModelsQuery::default())
            .map(|r| r.headers)
            .unwrap_or_default()
            .with("content-type", "application/json");
    let prober = Prober {
        state,
        base_url,
        key: key.unwrap_or(&none),
        headers,
    };
    let refusal = if key.is_some() {
        Failure::KeyRefused
    } else {
        Failure::KeyNeeded
    };

    let mut listed = None;
    for &auth in &auths {
        match prober.listing(auth).await? {
            Listing::Refused => {}
            listing => {
                listed = Some((auth, listing));
                break;
            }
        }
    }
    let Some((mut auth, listing)) = listed else {
        return Err(refusal);
    };
    if fixed.auth.is_none()
        && auth == CustomAuth::Bearer
        && matches!(listing, Listing::Anthropic(_))
        && matches!(
            prober.listing(CustomAuth::XApiKey).await,
            Ok(Listing::Anthropic(_))
        )
    {
        auth = CustomAuth::XApiKey;
    }

    let (mut auth, generation, models) = match listing {
        Listing::Missing => match fixed.generation {
            Some(g) => (auth, g, None),
            None => {
                let (auth, g) = prober.unlisted(&auths).await.map_err(|e| match e {
                    Failure::KeyRefused => refusal,
                    e => e,
                })?;
                (auth, g, None)
            }
        },
        listing => {
            let g = match fixed.generation {
                Some(g) => g,
                None => prober.generation(auth, &listing).await?,
            };
            let models = match listing {
                Listing::Anthropic(m) | Listing::OpenAi(m) => Some(detected(m)),
                Listing::Missing | Listing::Refused => None,
            };
            (auth, g, models)
        }
    };

    let path = generation_path(generation);
    match prober.route(auth, path).await? {
        Route::Missing if fixed.generation.is_some() => {
            return Err(Failure::Missing(Missing::Generation(path)));
        }
        Route::Refused => {
            let candidates = if fixed.auth.is_some() {
                &[][..]
            } else {
                &auths[..]
            };
            auth = prober
                .accepting(candidates, auth, path)
                .await?
                .ok_or(refusal)?;
        }
        Route::Present | Route::Unclear | Route::Missing => {}
    }

    let embeddings = match fixed.embeddings {
        Some(false) => false,
        Some(true) => {
            if prober.route(auth, EMBEDDINGS_PATH).await? == Route::Missing {
                return Err(Failure::Missing(Missing::Embeddings));
            }
            true
        }
        None => prober.offers(auth, EMBEDDINGS_PATH).await?,
    };

    let count_tokens = match (counting_path(generation), fixed.count_tokens) {
        (None, _) | (_, Some(false)) => false,
        (Some(path), Some(true)) => {
            if prober.route(auth, path).await? == Route::Missing {
                return Err(Failure::Missing(Missing::CountTokens(path)));
            }
            true
        }
        (Some(path), None) => prober.offers(auth, path).await?,
    };

    Ok(Detected {
        settings: Settings {
            auth,
            generation,
            embeddings,
            count_tokens,
        },
        models,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_route_counts_as_served_when_it_refuses_an_empty_body() {
        for status in [200, 400, 413, 415, 422] {
            assert_eq!(Route::of(status), Route::Present, "{status}");
        }
        for status in [404, 405, 501] {
            assert_eq!(Route::of(status), Route::Missing, "{status}");
        }
        for status in [401, 403] {
            assert_eq!(Route::of(status), Route::Refused, "{status}");
        }
        for status in [302, 429, 500, 502] {
            assert_eq!(Route::of(status), Route::Unclear, "{status}");
        }
    }

    #[test]
    fn every_generation_format_names_its_path_and_only_two_count_tokens() {
        assert_eq!(
            generation_path(DialectId::OpenAiChat),
            "/v1/chat/completions"
        );
        assert_eq!(generation_path(DialectId::OpenAiResponses), "/v1/responses");
        assert_eq!(
            generation_path(DialectId::AnthropicMessages),
            "/v1/messages"
        );
        assert_eq!(counting_path(DialectId::OpenAiChat), None);
        assert_eq!(
            counting_path(DialectId::OpenAiResponses),
            Some("/v1/responses/input_tokens")
        );
        assert_eq!(
            counting_path(DialectId::AnthropicMessages),
            Some("/v1/messages/count_tokens")
        );
    }
}
