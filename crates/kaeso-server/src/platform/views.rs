use kaeso_core::id::{OrgId, SessionId, UserId};
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A user as the user and their org admins see them.")]
pub struct UserView {
    #[doc = "The user's id."]
    pub id: UserId,
    #[doc = "The email address; `null` for a deleted user."]
    #[schema(example = "owner@example.com")]
    pub email: Option<String>,
    #[doc = "The display name; `\"Deleted user\"` for a deleted user."]
    #[schema(example = "Ada Lovelace")]
    pub name: String,
    #[doc = "`active`, `deactivated` or `deleted`."]
    #[schema(example = "active")]
    pub state: String,
    #[doc = "Whether the user administers the whole install."]
    pub is_instance_admin: bool,
    #[doc = "The picture's address, under `me/avatars`; `null` without one."]
    #[schema(
        example = "/platform/v1/me/avatars/9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
    )]
    pub avatar_url: Option<String>,
    #[doc = "When the account was created."]
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

pub const DELETED_USER: &str = "Deleted user";

pub fn display_name(name: Option<&str>) -> String {
    name.unwrap_or(DELETED_USER).to_owned()
}

impl UserView {
    pub fn from_row(u: &kaeso_store::pg::users::User) -> UserView {
        UserView {
            id: UserId::from_uuid(u.id),
            email: u.email.clone(),
            name: display_name(u.name.as_deref()),
            state: u.state.clone(),
            is_instance_admin: u.is_instance_admin,
            avatar_url: avatar_url(u.avatar.as_deref()),
            created_at: u.created_at,
        }
    }
}

pub fn avatar_url(hash: Option<&str>) -> Option<String> {
    hash.map(|h| format!("/platform/v1/me/avatars/{h}"))
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A console session.")]
pub struct SessionView {
    #[doc = "The session's id (not the secret cookie value)."]
    pub id: SessionId,
    #[doc = "How the user signed in: `password`, `magic_link`, `oidc`, `setup` or `signup`."]
    #[schema(example = "password")]
    pub method: String,
    #[doc = "Whether a second factor was used."]
    pub second_factor: bool,
    #[doc = "The org whose SSO signed this session in; `null` for other ways in.\n\
        When that org manages the account (`managed_by` in `auth/session`),\n\
        the session counts like a password one everywhere but `instance/*`.\n\
        Otherwise it opens only that org and reads the user's own account,\n\
        and anything else answers `other_sign_in_required`."]
    pub sso_org: Option<OrgId>,
    #[doc = "When the user last signed in on this session. Through SSO, when the\n\
        identity provider last checked them (its `auth_time`); without one,\n\
        more than 15 minutes before the session began."]
    #[serde(with = "time::serde::rfc3339")]
    pub signed_in_at: OffsetDateTime,
    #[doc = "When the session was last used."]
    #[serde(with = "time::serde::rfc3339")]
    pub last_used_at: OffsetDateTime,
    #[doc = "When it ends at the latest (it also ends after 7 days without use)."]
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "The org that manages an account.")]
pub struct ManagedBy {
    #[doc = "The org."]
    pub org: OrgId,
    #[doc = "Its name."]
    #[schema(example = "Acme")]
    pub name: String,
}

impl ManagedBy {
    pub fn new(org: uuid::Uuid, name: impl Into<String>) -> ManagedBy {
        ManagedBy {
            org: OrgId::from_uuid(org),
            name: name.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "An org in a list of the user's memberships.")]
pub struct OrgRef {
    #[doc = "The org's id."]
    pub id: OrgId,
    #[doc = "Its name."]
    #[schema(example = "Acme")]
    pub name: String,
}
