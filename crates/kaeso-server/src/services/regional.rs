//! Requests of a provider whose keys each call one region, such as Amazon Bedrock: the key's
//! region picks the host, and each dialect has its own API root, and where it differs its own
//! auth header, on that host.

use kaeso_codecs::EgressCtx;
use kaeso_core::provider::{AuthStyle, Regional};
use kaeso_core::providers::ProviderRef;
use kaeso_kpp::{DialectId, ErrorKind, KppError};
use url::Url;

use crate::model_api::errors::error;
use crate::services::credentials::Resolved;
use crate::state::AppState;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Egress {
    pub base: Url,
    pub auth: AuthStyle,
    api_root: &'static str,
}

impl Egress {
    pub fn apply(&self, ctx: EgressCtx) -> EgressCtx {
        ctx.with_api_root(self.api_root)
    }
}

#[derive(Debug)]
pub enum EgressError {
    /// The key names no region, which the database refuses for such a provider.
    Setup(KppError),
    /// The model isn't served in a format Kaeso speaks there.
    Refused(KppError),
}

pub fn regional(provider: &ProviderRef) -> Option<Regional> {
    provider.built_in().and_then(|p| p.regional)
}

/// Where a request of a regional provider goes: `None` for every other provider. `routed` says
/// whether the model's catalogue entry names its egress dialect; one that names none is served
/// only through the provider's own API.
pub fn egress(
    state: &AppState,
    provider: &ProviderRef,
    credential: &Resolved,
    dialect: DialectId,
    model: &str,
    routed: bool,
) -> Result<Option<Egress>, EgressError> {
    let Some(regional) = regional(provider) else {
        return Ok(None);
    };
    let api_root = regional
        .api_root(dialect)
        .filter(|_| routed)
        .ok_or_else(|| {
            EgressError::Refused(
            error(
                ErrorKind::InvalidRequest,
                400,
                format!(
                    "{} serves `{model}` only through its own API, which Kaeso doesn't call yet.",
                    provider.name()
                ),
            )
            .with_param("model"),
        )
        })?;
    let Some(region) = credential.region.as_deref() else {
        return Err(EgressError::Setup(KppError::provider(format!(
            "The {} key names no region; edit it and choose its region.",
            provider.name()
        ))));
    };
    Ok(Some(Egress {
        base: base(state, provider, regional, region)?,
        auth: regional
            .auth(dialect)
            .or_else(|| provider.auth())
            .unwrap_or(AuthStyle::Bearer),
        api_root,
    }))
}

// A base URL the operator set, such as a VPC endpoint, serves every region as it is; otherwise
// each region has its own host.
fn base(
    state: &AppState,
    provider: &ProviderRef,
    regional: Regional,
    region: &str,
) -> Result<Url, EgressError> {
    let configured = crate::model_api::pipeline::base_url(state, provider);
    let default = provider.built_in().map(|p| p.default_base_url);
    match configured {
        Some(url) if Some(url.as_str().trim_end_matches('/')) != default => Ok(url.clone()),
        _ => Url::parse(&regional.base_url(region)).map_err(|_| {
            EgressError::Setup(error(
                ErrorKind::ProviderError,
                500,
                "No base URL is configured for this provider.",
            ))
        }),
    }
}

#[cfg(test)]
mod tests {
    use kaeso_codecs::EgressCtx;
    use kaeso_core::provider::provider;
    use kaeso_kpp::{RequestKind, StreamMode};

    use super::*;

    #[test]
    fn each_dialect_goes_under_its_own_root_on_the_regions_host() {
        let r = provider("amazon-bedrock").unwrap().regional.unwrap();
        let chat = Egress {
            base: Url::parse(&r.base_url("eu-west-3")).unwrap(),
            auth: AuthStyle::Bearer,
            api_root: r.api_root(DialectId::OpenAiChat).unwrap(),
        };
        let ctx = chat.apply(EgressCtx::for_kind(RequestKind::Generate, StreamMode::Off));
        assert_eq!(ctx.api_root.as_deref(), Some("/openai/v1"));
        assert_eq!(
            chat.base.as_str(),
            "https://bedrock-runtime.eu-west-3.amazonaws.com/"
        );
    }
}
