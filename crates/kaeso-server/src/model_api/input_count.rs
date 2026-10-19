use std::sync::Arc;
use std::time::Duration;

use kaeso_codecs::{EgressCtx, dispatch};
use kaeso_core::provider::AuthStyle;
use kaeso_kpp::{DialectId, KppError, Reply, Request};
use kaeso_upstream::{Call, ResponseBody, Timeouts, UpstreamError};

use super::pipeline::base_url;
use crate::services::credentials::Resolved;
use crate::state::AppState;

const COUNT_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) struct InputCounter {
    pub(super) state: AppState,
    pub(super) credential: Arc<Resolved>,
    pub(super) provider: &'static str,
    pub(super) auth: AuthStyle,
    pub(super) dialect: DialectId,
    pub(super) asked: Option<(Arc<Request>, Request)>,
    pub(super) adaptive_thinking: Option<bool>,
    pub(super) reasoning: Option<bool>,
    pub(super) timeouts: Timeouts,
    pub(super) sent_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Input {
    Counted(u64),
    Estimated(u64),
}

impl Input {
    pub(super) fn tokens(self) -> u64 {
        match self {
            Input::Counted(n) | Input::Estimated(n) => n,
        }
    }
}

fn with_causes(e: &UpstreamError) -> String {
    let mut text = e.to_string();
    let mut cause = std::error::Error::source(e);
    while let Some(c) = cause {
        text.push_str(": ");
        text.push_str(&c.to_string());
        cause = c.source();
    }
    text
}

impl InputCounter {
    pub(super) async fn count(mut self) -> Input {
        let (provider, sent_bytes) = (self.provider, self.sent_bytes);
        if let Some((app_req, req)) = self.asked.take() {
            match self.ask_provider(app_req, req).await {
                Ok(n) => return Input::Counted(n),
                Err(e) => tracing::warn!(
                    provider,
                    error = %e,
                    "cannot count the input of a stream that ended early; estimating it from its size"
                ),
            }
        }
        let bytes = u64::try_from(sent_bytes).unwrap_or(u64::MAX);
        Input::Estimated(kaeso_core::estimate::tokens_of_bytes(bytes))
    }

    async fn ask_provider(self, app_req: Arc<Request>, req: Request) -> Result<u64, KppError> {
        let (Request::Generate(app), Request::Generate(sent)) =
            (Arc::unwrap_or_clone(app_req), req)
        else {
            return Err(KppError::provider("only generations are counted"));
        };
        let app_req = Request::CountTokens(app);
        let req = Request::CountTokens(sent);
        let mut ctx = EgressCtx::new(&app_req).with_provider(self.provider);
        if let Some(a) = self.adaptive_thinking {
            ctx = ctx.with_adaptive_thinking(a);
        }
        if let Some(r) = self.reasoning {
            ctx = ctx.with_reasoning(r);
        }
        let encoded = dispatch::encode_request(self.dialect, &req, &mut ctx)?;
        let base = base_url(&self.state, self.provider)
            .ok_or_else(|| KppError::provider(format!("no base URL for `{}`", self.provider)))?;
        let resp = self
            .state
            .upstream
            .send(Call {
                base_url: base,
                auth: self.auth,
                credential: &self.credential.secret,
                request: encoded.into(),
                stream: false,
                timeouts: Timeouts {
                    request: self.timeouts.request.min(COUNT_TIMEOUT),
                    ..self.timeouts
                },
                max_body: self.state.config.body_limits.provider_response.as_usize(),
            })
            .await
            .map_err(|e| KppError::provider(with_causes(&e)))?;
        let ResponseBody::Full(body) = resp.body else {
            return Err(KppError::provider("the count arrived as a stream"));
        };
        if !(200..300).contains(&resp.status) {
            return Err(dispatch::decode_error(
                self.dialect,
                resp.status,
                &resp.headers,
                &body,
            ));
        }
        match dispatch::decode_response(self.dialect, resp.status, &resp.headers, &body, &ctx)? {
            Reply::CountTokens(c) => Ok(c.input_tokens),
            _ => Err(KppError::provider(
                "the provider did not answer with a count",
            )),
        }
    }
}
