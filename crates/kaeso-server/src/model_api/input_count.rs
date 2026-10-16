use std::time::Duration;

use kaeso_codecs::{EgressCtx, dispatch};
use kaeso_core::id::{OrgId, ProjectId};
use kaeso_core::provider::AuthStyle;
use kaeso_kpp::{DialectId, KppError, Reply, Request};
use kaeso_upstream::{Call, ResponseBody, Timeouts};

use super::pipeline::base_url;
use crate::services::credentials;
use crate::state::AppState;

const COUNT_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) struct InputCounter {
    pub(super) state: AppState,
    pub(super) org: OrgId,
    pub(super) project: ProjectId,
    pub(super) provider: &'static str,
    pub(super) auth: AuthStyle,
    pub(super) dialect: DialectId,
    pub(super) app_req: Request,
    pub(super) req: Request,
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

impl InputCounter {
    pub(super) async fn count(&self) -> Input {
        if matches!(
            self.dialect,
            DialectId::AnthropicMessages | DialectId::OpenAiResponses
        ) {
            match self.ask_provider().await {
                Ok(n) => return Input::Counted(n),
                Err(e) => tracing::warn!(
                    provider = self.provider,
                    error = %e,
                    "cannot count the input of a stream that ended early; estimating it from its size"
                ),
            }
        }
        let bytes = u64::try_from(self.sent_bytes).unwrap_or(u64::MAX);
        Input::Estimated(kaeso_core::estimate::tokens_of_bytes(bytes))
    }

    async fn ask_provider(&self) -> Result<u64, KppError> {
        let as_count = |r: &Request| match r {
            Request::Generate(g) => Ok(Request::CountTokens(g.clone())),
            _ => Err(KppError::provider("only generations are counted")),
        };
        let app_req = as_count(&self.app_req)?;
        let req = as_count(&self.req)?;
        let mut ctx = EgressCtx::new(&app_req).with_provider(self.provider);
        if let Some(a) = self.adaptive_thinking {
            ctx = ctx.with_adaptive_thinking(a);
        }
        if let Some(r) = self.reasoning {
            ctx = ctx.with_reasoning(r);
        }
        let encoded = dispatch::encode_request(self.dialect, &req, &mut ctx)?;
        let credential = credentials::resolve(&self.state, self.org, self.project, self.provider)
            .await
            .map_err(|e| KppError::provider(e.to_string()))?;
        let base = base_url(&self.state, self.provider)
            .ok_or_else(|| KppError::provider(format!("no base URL for `{}`", self.provider)))?;
        let resp = self
            .state
            .upstream
            .send(Call {
                base_url: base,
                auth: self.auth,
                credential: &credential.secret,
                request: &encoded,
                stream: false,
                timeouts: Timeouts {
                    request: self.timeouts.request.min(COUNT_TIMEOUT),
                    ..self.timeouts
                },
            })
            .await
            .map_err(|e| KppError::provider(e.to_string()))?;
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
