use kaeso_core::id::{ApiKeyId, OrgPlatformKeyId, ProjectId, ServiceAccountId, UserId};
use kaeso_core::keys::{
    Expiry, KeyKind, NewSecret, ORG_PREFIX, PROJECT_PREFIX, expires_at, max_lifetime,
};
use kaeso_store::pg::keys::{KeyRow, OrgKeyRow};
use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};
use utoipa::ToSchema;

use super::access::{Caller, Ctx};
use super::error::{ApiError, ApiResult};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(description = "What a key may call.")]
pub enum KeyAccess {
    #[default]
    Inference,
    Platform,
    Both,
}

impl KeyAccess {
    pub fn flags(self) -> (bool, bool) {
        match self {
            KeyAccess::Inference => (true, false),
            KeyAccess::Platform => (false, true),
            KeyAccess::Both => (true, true),
        }
    }

    pub fn from_flags(inference: bool, platform: bool) -> KeyAccess {
        match (inference, platform) {
            (true, true) => KeyAccess::Both,
            (false, true) => KeyAccess::Platform,
            _ => KeyAccess::Inference,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, ToSchema)]
#[schema(
    description = "When a new key expires: `expires_in_days` (1, 7, 30 or 90), or\n\
    `expires_on` (a date, UTC), or neither for never."
)]
pub struct ExpiryRequest {
    #[doc = "1, 7, 30 or 90."]
    #[schema(example = 30)]
    pub expires_in_days: Option<u32>,
    #[doc = "A date; the key stops at its start (UTC)."]
    #[schema(value_type = Option<String>, format = Date, example = "2026-12-31")]
    pub expires_on: Option<Date>,
}

impl ExpiryRequest {
    pub fn resolve(
        &self,
        now: OffsetDateTime,
        max_days: Option<i32>,
    ) -> ApiResult<Option<OffsetDateTime>> {
        let choice = match (self.expires_in_days, self.expires_on) {
            (Some(d), None) => Expiry::Days(d),
            (None, Some(date)) => Expiry::Date(date),
            (None, None) => Expiry::Never,
            (Some(_), Some(_)) => {
                return Err(ApiError::bad_request(
                    "Send either `expires_in_days` or `expires_on`, not both.",
                ));
            }
        };
        expires_at(choice, now, max_days)
            .map_err(|e| ApiError::invalid("expires_in_days", e.to_string()))
    }
}

pub fn project_max(ctx: &Ctx) -> ApiResult<Option<i32>> {
    Ok(max_lifetime(
        ctx.org()?.org.max_key_lifetime_days,
        ctx.project()?.project.max_key_lifetime_days,
    ))
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A key's principal or creator.")]
pub struct Party {
    #[doc = "`user`, `service_account`, `api_key`, `org_platform_key` or `system`."]
    #[schema(example = "user")]
    pub kind: String,
    #[doc = "Its id."]
    pub id: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "An API key, without its secret.")]
pub struct KeyView {
    #[doc = "Id."]
    pub id: ApiKeyId,
    #[doc = "The project."]
    pub project_id: ProjectId,
    #[doc = "Name."]
    #[schema(example = "support-bot")]
    pub name: String,
    #[doc = "Whose rights it acts with."]
    pub principal: Party,
    #[doc = "What it may call."]
    pub access: KeyAccess,
    #[doc = "A redacted form for recognising it, e.g. `sk-kso-AbC123…wxyz`."]
    #[schema(example = "sk-kso-AbC123…wxyz")]
    pub redacted: String,
    #[doc = "`active`, `disabled`, `expired` or `archived`."]
    #[schema(example = "active")]
    pub state: &'static str,
    #[doc = "Expiry; `null` for never."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
    #[doc = "Last use."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
    #[doc = "Who created it."]
    pub created_by: Party,
    #[doc = "Created."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

fn state_of(
    archived: bool,
    disabled: bool,
    expires_at: Option<OffsetDateTime>,
    now: OffsetDateTime,
) -> &'static str {
    if archived {
        "archived"
    } else if expires_at.is_some_and(|e| e <= now) {
        "expired"
    } else if disabled {
        "disabled"
    } else {
        "active"
    }
}

pub(crate) fn creator(kind: &str, id: Option<uuid::Uuid>) -> Party {
    let id = id.map(|id| match kind {
        "user" => UserId::from_uuid(id).to_string(),
        "api_key" => ApiKeyId::from_uuid(id).to_string(),
        "org_platform_key" => OrgPlatformKeyId::from_uuid(id).to_string(),
        _ => id.to_string(),
    });
    Party {
        kind: kind.to_owned(),
        id,
    }
}

impl KeyView {
    pub fn from_row(k: &KeyRow, now: OffsetDateTime) -> KeyView {
        let principal = match (k.user_id, k.service_account_id) {
            (Some(u), _) => Party {
                kind: "user".into(),
                id: Some(UserId::from_uuid(u).to_string()),
            },
            (None, Some(s)) => Party {
                kind: "service_account".into(),
                id: Some(ServiceAccountId::from_uuid(s).to_string()),
            },
            (None, None) => Party {
                kind: "none".into(),
                id: None,
            },
        };
        KeyView {
            id: ApiKeyId::from_uuid(k.id),
            project_id: ProjectId::from_uuid(k.project_id),
            name: k.name.clone(),
            principal,
            access: KeyAccess::from_flags(k.inference, k.platform),
            redacted: format!("{PROJECT_PREFIX}{}…{}", k.secret_prefix, k.secret_last4),
            state: state_of(
                k.archived_at.is_some(),
                k.disabled_at.is_some(),
                k.expires_at,
                now,
            ),
            expires_at: k.expires_at,
            last_used_at: k.last_used_at,
            created_by: creator(&k.created_by_kind, k.created_by_id),
            created_at: k.created_at,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "An org platform key, without its secret.")]
pub struct OrgKeyView {
    #[doc = "Id."]
    pub id: OrgPlatformKeyId,
    #[doc = "Name."]
    pub name: String,
    #[doc = "A redacted form, e.g. `sk-kso-org-AbC123…wxyz`."]
    pub redacted: String,
    #[doc = "`active`, `disabled`, `expired` or `archived`."]
    pub state: &'static str,
    #[doc = "Expiry; `null` for never."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
    #[doc = "Last use."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
    #[doc = "Who created it."]
    pub created_by: Party,
    #[doc = "Created."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl OrgKeyView {
    pub fn from_row(k: &OrgKeyRow, now: OffsetDateTime) -> OrgKeyView {
        OrgKeyView {
            id: OrgPlatformKeyId::from_uuid(k.id),
            name: k.name.clone(),
            redacted: format!("{ORG_PREFIX}{}…{}", k.secret_prefix, k.secret_last4),
            state: state_of(
                k.archived_at.is_some(),
                k.disabled_at.is_some(),
                k.expires_at,
                now,
            ),
            expires_at: k.expires_at,
            last_used_at: k.last_used_at,
            created_by: creator(&k.created_by_kind, k.created_by_id),
            created_at: k.created_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A key with its secret, shown once.")]
pub struct CreatedKey<V> {
    #[serde(flatten)]
    pub key: V,
    #[doc = "The secret. Shown only now; Kaeso stores only its hash."]
    #[schema(example = "sk-kso-AbC123…")]
    pub secret: String,
}

pub fn new_secret(kind: KeyKind) -> NewSecret {
    kaeso_core::keys::generate(kind, kaeso_crypto::random_bytes::<64>)
}

pub fn creator_of(ctx: &Ctx) -> (&'static str, Option<uuid::Uuid>) {
    match &ctx.caller {
        Caller::Session(s) => ("user", Some(s.user_id)),
        Caller::ProjectKey(k) => ("api_key", Some(k.id)),
        Caller::OrgKey(k) => ("org_platform_key", Some(k.id)),
        Caller::Anonymous => ("system", None),
    }
}

pub fn validate_name(name: &str) -> ApiResult<String> {
    super::auth::setup::validate_name("name", name)
}
