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

time::serde::format_description!(iso_date, Date, "[year]-[month]-[day]");

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
    #[serde(default, with = "iso_date::option")]
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

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[schema(
    description = "How to rotate a key: how long the old key keeps working, and when\n\
    the new key expires."
)]
pub struct KeyRotate {
    #[doc = "How long the old key keeps working: 0 (it stops at once), 1, 24 or 168\n\
        hours."]
    #[schema(example = 24)]
    pub grace_hours: u32,
    #[doc = "When the new key expires."]
    #[serde(flatten)]
    pub expiry: ExpiryRequest,
    #[serde(flatten)]
    #[schema(ignore)]
    pub other: crate::platform::json::NoOtherFields,
}

impl KeyRotate {
    pub fn grace_ends_at(&self, now: OffsetDateTime) -> ApiResult<OffsetDateTime> {
        kaeso_core::keys::grace_ends_at(self.grace_hours, now)
            .map_err(|e| ApiError::invalid("grace_hours", e.to_string()))
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
    #[doc = "The key it replaced, when it was made by rotating that key."]
    pub rotated_from: Option<ApiKeyId>,
    #[doc = "The key that replaced it; it stops working at `expires_at`."]
    pub replaced_by: Option<ApiKeyId>,
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
            rotated_from: k.rotated_from.map(ApiKeyId::from_uuid),
            replaced_by: k.replaced_by.map(ApiKeyId::from_uuid),
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
    #[doc = "The platform key it replaced, when it was made by rotating that key."]
    pub rotated_from: Option<OrgPlatformKeyId>,
    #[doc = "The platform key that replaced it; it stops working at `expires_at`."]
    pub replaced_by: Option<OrgPlatformKeyId>,
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
            rotated_from: k.rotated_from.map(OrgPlatformKeyId::from_uuid),
            replaced_by: k.replaced_by.map(OrgPlatformKeyId::from_uuid),
        }
    }
}

#[derive(Serialize, ToSchema)]
#[schema(description = "A key with its secret, shown once.")]
pub struct CreatedKey<V> {
    #[serde(flatten)]
    pub key: V,
    #[doc = "The secret. Shown only now; Kaeso stores only its hash."]
    #[schema(example = "sk-kso-AbC123…")]
    pub secret: String,
}

impl<V: std::fmt::Debug> std::fmt::Debug for CreatedKey<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreatedKey")
            .field("key", &self.key)
            .field("secret", &"[redacted]")
            .finish()
    }
}

pub fn new_secret(kind: KeyKind) -> NewSecret {
    kaeso_core::keys::generate(kind, kaeso_crypto::random_bytes::<64>)
}

pub fn creator_of(ctx: &Ctx) -> (&'static str, Option<uuid::Uuid>) {
    match &ctx.caller {
        Caller::Session(s) => ("user", Some(s.user_id)),
        Caller::Agent(a) => ("user", Some(a.user_id())),
        Caller::ProjectKey(k) => ("api_key", Some(k.id)),
        Caller::OrgKey(k) => ("org_platform_key", Some(k.id)),
        Caller::Anonymous => ("system", None),
    }
}

pub fn validate_name(name: &str) -> ApiResult<String> {
    super::validate::validate_name("name", name)
}

#[cfg(test)]
mod tests {
    use super::CreatedKey;

    #[test]
    fn a_created_key_never_prints_its_secret() {
        let created = CreatedKey {
            key: "key_1",
            secret: "sk-kso-AbC123secret".to_owned(),
        };
        let printed = format!("{created:?}");
        assert!(!printed.contains("AbC123secret"), "{printed}");
        assert!(printed.contains("key_1"), "{printed}");
    }
}
