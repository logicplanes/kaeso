use axum::Json;
use axum::extract::State;
use kaeso_core::id::OrgId;
use kaeso_core::provider::{PROVIDERS, Provider, ProviderId};
use kaeso_core::providers::CustomProvider;
use kaeso_core::servers::ServerKind;
use serde::Serialize;
use utoipa::ToSchema;

use super::access::Ctx;
use super::error::ApiResult;
use super::pagination::List;
use super::params::Path;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A model provider Kaeso calls, with what the console shows of it.")]
pub struct ProviderView {
    #[doc = "The id, which names the provider in model names such as\n\
        `groq/llama-3.3-70b-versatile`, in provider keys and in settings."]
    pub id: ProviderId,
    #[doc = "The name people see."]
    #[schema(example = "Groq")]
    pub name: String,
    #[doc = "One of the organization's own providers, added under\n\
        `/orgs/{org}/custom-providers`."]
    pub custom: bool,
    #[doc = "Whether requests to it carry a provider key; `false` for a custom\n\
        provider that takes none."]
    pub takes_key: bool,
    #[doc = "The name of the built-in provider with the same id that this custom\n\
        provider hides inside the organization; `null` without one."]
    #[schema(example = json!(null))]
    pub hides_built_in: Option<&'static str>,
    #[doc = "The provider's page for creating API keys; `null` for a custom provider."]
    pub key_page: Option<KeyPageView>,
    #[doc = "How the provider's API keys start; `null` when they have no fixed prefix."]
    #[schema(example = "gsk_")]
    pub key_prefix: Option<&'static str>,
    #[doc = "The formats Kaeso can call the provider with, the default first; a model's\n\
        `egress_dialect` is one of them."]
    #[schema(example = json!(["openai_chat"]))]
    pub dialects: Vec<&'static str>,
    #[doc = "The format in which the provider counts tokens for\n\
        `/v1/messages/count_tokens` and `/v1/responses/input_tokens`; `null` when\n\
        it counts none."]
    #[schema(example = json!(null))]
    pub count_tokens: Option<&'static str>,
    #[doc = "The format of the provider's embeddings models, which serve\n\
        `/v1/embeddings`; `null` when it has none."]
    #[schema(example = json!(null))]
    pub embeddings: Option<&'static str>,
    #[doc = "The format Kaeso sends the provider's rerank models in, which serve\n\
        `/v1/rerank`; `null` when Kaeso calls no rerank API of the provider."]
    #[schema(example = json!(null))]
    pub rerank: Option<&'static str>,
    #[doc = "An account people can sign in to instead of adding an API key, with\n\
        `POST /orgs/{org}/credentials/oauth`; `null` when the provider takes API\n\
        keys only."]
    pub account: Option<AccountSignInView>,
    #[doc = "A service account key people can add instead of an API key, with\n\
        `auth_method` `google_service_account`; `null` when the provider takes\n\
        none."]
    pub service_account: Option<ServiceAccountSignInView>,
    #[doc = "For a provider whose API names the account, such as Cloudflare AI Gateway:\n\
        each key also gives `account_id`, and may give `gateway_id`. `null` for\n\
        other providers."]
    pub account_scope: Option<AccountScopeView>,
    #[doc = "For a provider whose keys each call one region, such as Amazon Bedrock:\n\
        each API key also gives its region as `location`. `null` for other\n\
        providers."]
    pub regions: Option<RegionsView>,
    #[doc = "For a provider whose keys each belong to one resource with an address of its\n\
        own, such as Azure: each key also gives the resource's `endpoint`. `null` for\n\
        other providers."]
    pub key_endpoint: Option<KeyEndpointView>,
    #[doc = "For Amazon Bedrock: the AWS credentials a key may sign its requests with\n\
        instead of a Bedrock API key, with `auth_method` `aws_access_key` or\n\
        `aws_role`. `null` for other providers."]
    pub aws: Option<AwsSignInView>,
    #[doc = "Which format a model id is sent in when its catalogue entry names\n\
        none, the first matching pattern winning; models no pattern matches go\n\
        in the first of `dialects`. Empty for most providers."]
    pub model_routes: Vec<ModelRouteView>,
    #[doc = "The inference server software a custom provider runs, which also picks\n\
        its mark; `null` for a built-in provider and for another server or a proxy."]
    #[schema(example = json!(null))]
    pub server: Option<ServerKind>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A pattern of model ids and the format they are sent in.")]
pub struct ModelRouteView {
    #[doc = "Model ids it covers; `*` stands for any characters."]
    #[schema(example = "claude-*")]
    pub pattern: &'static str,
    #[doc = "The format, one of the provider's `dialects`."]
    #[schema(example = "anthropic_messages")]
    pub dialect: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "How a provider takes a Google Cloud service account key.")]
pub struct ServiceAccountSignInView {
    #[doc = "The provider's page for creating service accounts and their keys."]
    pub key_page: KeyPageView,
    #[doc = "The locations a key can call, in the provider's documented order."]
    #[schema(example = json!(["global", "us-central1"]))]
    pub locations: Vec<&'static str>,
    #[doc = "The location a new key calls unless another is chosen."]
    #[schema(example = "global")]
    pub default_location: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "An account of a provider that signs in with OAuth.")]
pub struct AccountSignInView {
    #[doc = "The provider key's `auth_method` once signed in."]
    pub method: crate::platform::orgs::credentials::AuthMethodView,
    #[doc = "The product whose account it is, as its owner knows it."]
    #[schema(example = "ChatGPT")]
    pub product: &'static str,
    #[doc = "The provider's page where the account's owner reviews and limits what\n\
        Kaeso uses; `null` without one."]
    #[schema(example = "https://chatgpt.com/settings/usage")]
    pub usage_url: Option<&'static str>,
    #[doc = "The sign-in works but may change or stop when the provider changes it."]
    pub experimental: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The account a provider's keys name, and the gateway they may name.")]
pub struct AccountScopeView {
    #[doc = "The provider's page that says where to find the account id."]
    pub account_page: KeyPageView,
    #[doc = "The header Kaeso names a key's gateway in."]
    #[schema(example = "cf-aig-gateway-id")]
    pub gateway_header: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "How a provider takes AWS credentials.")]
pub struct AwsSignInView {
    #[doc = "The AWS page for creating access keys."]
    pub access_key_page: KeyPageView,
    #[doc = "Whether the caller may add a key that signs in as the gateway's own AWS\n\
        identity: the install allows it (`aws.ambient_credentials`) and the caller is\n\
        an instance admin."]
    pub gateway_identity: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The regions a provider's API keys may call.")]
pub struct RegionsView {
    #[doc = "The provider's page that lists its regions."]
    pub regions_page: KeyPageView,
    #[doc = "The regions Kaeso calls, as the provider writes them."]
    #[schema(example = json!(["us-east-1", "eu-central-1"]))]
    pub regions: Vec<&'static str>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "The resource endpoint a provider's keys carry.")]
pub struct KeyEndpointView {
    #[doc = "The provider's page that says where to find the endpoint."]
    pub page: KeyPageView,
    #[doc = "The host suffixes an endpoint may end in."]
    #[schema(example = json!([".openai.azure.com"]))]
    pub hosts: Vec<&'static str>,
    #[doc = "An endpoint, for showing its form."]
    #[schema(example = "https://example.openai.azure.com")]
    pub example: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A page on the provider's own site.")]
pub struct KeyPageView {
    #[doc = "The page, an `https` URL."]
    #[schema(example = "https://console.groq.com/keys")]
    pub url: &'static str,
    #[doc = "The page's name as the provider's site shows it, in English."]
    #[schema(example = "API Keys")]
    pub label: &'static str,
}

/// The provider whose keys may sign with AWS credentials.
pub(crate) const AWS_SIGNED: &str = "amazon-bedrock";

impl ProviderView {
    fn of(p: &Provider) -> ProviderView {
        ProviderView {
            id: p.provider_id(),
            name: p.name.to_owned(),
            custom: false,
            takes_key: true,
            hides_built_in: None,
            key_page: Some(KeyPageView {
                url: p.key_page.url,
                label: p.key_page.label,
            }),
            key_prefix: p.key_prefix,
            dialects: p.dialects.iter().map(|d| d.as_str()).collect(),
            count_tokens: p.count_tokens.map(|d| d.as_str()),
            embeddings: p.embeddings.map(|d| d.as_str()),
            rerank: p.rerank.map(|r| r.dialect.as_str()),
            account: p.account.map(|a| AccountSignInView {
                method: a.method.into(),
                product: a.product,
                usage_url: a.usage_url,
                experimental: a.experimental,
            }),
            service_account: p.service_account.map(|s| ServiceAccountSignInView {
                key_page: KeyPageView {
                    url: s.key_page.url,
                    label: s.key_page.label,
                },
                locations: s.locations.to_vec(),
                default_location: s.default_location,
            }),
            account_scope: p.account_scope.map(|a| AccountScopeView {
                account_page: KeyPageView {
                    url: a.account_page.url,
                    label: a.account_page.label,
                },
                gateway_header: a.gateway_header,
            }),
            regions: p.regional.map(|r| RegionsView {
                regions_page: KeyPageView {
                    url: r.regions_page.url,
                    label: r.regions_page.label,
                },
                regions: r.regions.to_vec(),
            }),
            key_endpoint: p.key_endpoint.map(|e| KeyEndpointView {
                page: KeyPageView {
                    url: e.page.url,
                    label: e.page.label,
                },
                hosts: e.hosts.to_vec(),
                example: e.example,
            }),
            aws: None,
            model_routes: p
                .routes
                .iter()
                .map(|r| ModelRouteView {
                    pattern: r.pattern,
                    dialect: r.dialect.as_str(),
                })
                .collect(),
            server: None,
        }
    }

    fn custom(p: &CustomProvider) -> ProviderView {
        ProviderView {
            id: p.id().clone(),
            name: p.name().to_owned(),
            custom: true,
            takes_key: p.auth().style().is_some(),
            hides_built_in: kaeso_core::provider::provider(p.id().as_str()).map(|b| b.name),
            key_page: None,
            key_prefix: None,
            dialects: p.dialects().iter().map(|d| d.as_str()).collect(),
            count_tokens: p.count_tokens().map(|d| d.as_str()),
            embeddings: p.embeddings().map(|d| d.as_str()),
            rerank: None,
            account: None,
            service_account: None,
            account_scope: None,
            regions: None,
            key_endpoint: None,
            aws: None,
            model_routes: Vec::new(),
            server: p.server(),
        }
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/providers",
    summary = "Every model provider Kaeso calls for the organization, with its name, key page,\n\
    key prefix and formats.",
    description = "The built-in providers, the same for every organization, followed by the\n\
        organization's custom providers while the install allows them. A provider\n\
        key and a catalogue model name a provider by its `id`. A custom provider\n\
        with the id of a built-in one takes its place in the organization.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = List<ProviderView>))
)]
pub async fn list(
    State(state): State<AppState>,
    Path(_org): Path<String>,
    ctx: Ctx,
) -> ApiResult<Json<List<ProviderView>>> {
    let org = ctx.org()?.org.id;
    let custom = crate::services::custom_providers::of_org(&state, &state.pg, org).await?;
    let hidden = |id: &str| custom.custom().iter().any(|c| c.id().as_str() == id);
    let gateway_identity =
        state.config.aws.ambient_credentials && ctx.session().is_ok_and(|s| s.is_instance_admin);
    Ok(Json(List::complete(
        PROVIDERS
            .iter()
            .filter(|p| !hidden(p.id))
            .map(|p| {
                let mut view = ProviderView::of(p);
                if p.id == AWS_SIGNED {
                    view.aws = Some(AwsSignInView {
                        access_key_page: KeyPageView {
                            url: "https://console.aws.amazon.com/iam/home#/security_credentials",
                            label: "Security credentials",
                        },
                        gateway_identity,
                    });
                }
                view
            })
            .chain(custom.custom().iter().map(|c| ProviderView::custom(c)))
            .collect(),
    )))
}
