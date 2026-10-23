use axum::Json;
use kaeso_core::id::OrgId;
use kaeso_core::provider::{PROVIDERS, Provider, ProviderId};
use serde::Serialize;
use utoipa::ToSchema;

use super::access::Ctx;
use super::pagination::List;
use super::params::Path;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A model provider Kaeso calls, with what the console shows of it.")]
pub struct ProviderView {
    #[doc = "The id, which names the provider in model names such as\n\
        `groq/llama-3.3-70b-versatile`, in provider keys and in settings."]
    pub id: ProviderId,
    #[doc = "The name people see."]
    #[schema(example = "Groq")]
    pub name: &'static str,
    #[doc = "The provider's page for creating API keys."]
    pub key_page: KeyPageView,
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
    #[doc = "An account people can sign in to instead of adding an API key, with\n\
        `POST /orgs/{org}/credentials/oauth`; `null` when the provider takes API\n\
        keys only."]
    pub account: Option<AccountSignInView>,
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
#[schema(description = "A page on the provider's own site.")]
pub struct KeyPageView {
    #[doc = "The page, an `https` URL."]
    #[schema(example = "https://console.groq.com/keys")]
    pub url: &'static str,
    #[doc = "The page's name as the provider's site shows it, in English."]
    #[schema(example = "API Keys")]
    pub label: &'static str,
}

impl ProviderView {
    fn of(p: &Provider) -> ProviderView {
        ProviderView {
            id: p.provider_id(),
            name: p.name,
            key_page: KeyPageView {
                url: p.key_page.url,
                label: p.key_page.label,
            },
            key_prefix: p.key_prefix,
            dialects: p.dialects.iter().map(|d| d.as_str()).collect(),
            count_tokens: p.count_tokens.map(|d| d.as_str()),
            embeddings: p.embeddings.map(|d| d.as_str()),
            account: p.account.map(|a| AccountSignInView {
                method: a.method.into(),
                product: a.product,
                usage_url: a.usage_url,
                experimental: a.experimental,
            }),
        }
    }
}

#[utoipa::path(
    get,
    path = "/orgs/{org}/providers",
    summary = "Every model provider Kaeso calls, with its name, key page, key prefix and formats.",
    description = "The same list for every organization of the install. A provider key, a\n\
        catalogue model and a setting name a provider by its `id`.",
    tag = "orgs",
    params(("org" = OrgId, Path)),
    responses((status = 200, body = List<ProviderView>))
)]
pub async fn list(Path(_org): Path<String>, _ctx: Ctx) -> Json<List<ProviderView>> {
    Json(List::complete(
        PROVIDERS.iter().map(ProviderView::of).collect(),
    ))
}
