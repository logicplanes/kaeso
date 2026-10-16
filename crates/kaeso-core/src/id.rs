use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

pub const ENCODED_LEN: usize = 26;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum IdError {
    #[error("expected an id starting with `{expected}`")]
    WrongPrefix { expected: &'static str },
    #[error("the id is malformed")]
    Malformed,
}

pub fn encode_base32(value: u128) -> String {
    let mut out = String::with_capacity(ENCODED_LEN);
    for i in (0..ENCODED_LEN).rev() {
        let shift = i * 5;
        let index = ((value >> shift) & 0x1f) as usize;
        out.push(char::from(ALPHABET.get(index).copied().unwrap_or(b'0')));
    }
    out
}

pub fn decode_base32(s: &str) -> Result<u128, IdError> {
    if s.len() != ENCODED_LEN {
        return Err(IdError::Malformed);
    }
    let mut value: u128 = 0;
    for (i, b) in s.bytes().enumerate() {
        let digit = ALPHABET
            .iter()
            .position(|&a| a == b)
            .ok_or(IdError::Malformed)? as u128;
        if i == 0 && digit > 7 {
            return Err(IdError::Malformed);
        }
        value = (value << 5) | digit;
    }
    Ok(value)
}

pub trait PublicId: Copy + Sized {
    const PREFIX: &'static str;
    fn uuid(&self) -> Uuid;
    fn from_uuid(uuid: Uuid) -> Self;
}

macro_rules! public_id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                $name(Uuid::now_v7())
            }

            pub const fn from_uuid(uuid: Uuid) -> Self {
                $name(uuid)
            }

            pub const fn uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl PublicId for $name {
            const PREFIX: &'static str = $prefix;
            fn uuid(&self) -> Uuid {
                self.0
            }
            fn from_uuid(uuid: Uuid) -> Self {
                $name(uuid)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str($prefix)?;
                f.write_str(&encode_base32(self.0.as_u128()))
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(self, f)
            }
        }

        impl FromStr for $name {
            type Err = IdError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let rest = s
                    .strip_prefix($prefix)
                    .ok_or(IdError::WrongPrefix { expected: $prefix })?;
                Ok($name(Uuid::from_u128(decode_base32(rest)?)))
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }

        #[cfg(feature = "openapi")]
        impl utoipa::PartialSchema for $name {
            fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
                utoipa::openapi::ObjectBuilder::new()
                    .schema_type(utoipa::openapi::schema::Type::String)
                    .pattern(Some(concat!("^", $prefix, "[0-9a-hjkmnp-tv-z]{26}$")))
                    .examples([concat!($prefix, "01k7a3f8m2xq4v9c5r6t8w0yz1")])
                    .into()
            }
        }

        #[cfg(feature = "openapi")]
        impl utoipa::ToSchema for $name {}

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = std::borrow::Cow::<str>::deserialize(d)?;
                s.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

public_id!(UserId, "usr_");
public_id!(OrgId, "org_");
public_id!(ProjectId, "proj_");
public_id!(MembershipId, "mem_");
public_id!(InviteId, "inv_");
public_id!(ServiceAccountId, "svc_");
public_id!(ApiKeyId, "key_");
public_id!(OrgPlatformKeyId, "opk_");
public_id!(CredentialId, "cred_");
public_id!(CatalogueModelId, "model_");
public_id!(AliasId, "alias_");
public_id!(LimitId, "lim_");
public_id!(SessionId, "ses_");
public_id!(RequestId, "req_");
public_id!(AuditEventId, "evt_");
public_id!(ExportId, "exp_");
public_id!(RoleId, "role_");

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn ids_have_their_prefix_and_26_characters() {
        let id = OrgId::new();
        let s = id.to_string();
        assert!(s.starts_with("org_"));
        assert_eq!(s.len(), 4 + ENCODED_LEN);
        assert_eq!(s.parse::<OrgId>().unwrap(), id);
    }

    #[test]
    fn wrong_prefix_and_bad_characters_are_errors() {
        let org = OrgId::new().to_string();
        assert_eq!(
            org.parse::<ProjectId>(),
            Err(IdError::WrongPrefix { expected: "proj_" })
        );
        assert_eq!("org_".parse::<OrgId>(), Err(IdError::Malformed));
        assert_eq!(
            "org_0123456789abcdefghjkmnpqrsu".parse::<OrgId>(),
            Err(IdError::Malformed)
        );
        assert!(
            org.to_uppercase()
                .replace("ORG_", "org_")
                .parse::<OrgId>()
                .is_err()
        );
        assert_eq!(
            "org_80000000000000000000000000".parse::<OrgId>(),
            Err(IdError::Malformed)
        );
    }

    #[test]
    fn ids_sort_by_creation_time() {
        let a = ApiKeyId::new();
        let b = ApiKeyId::new();
        assert!(a.to_string() < b.to_string());
        assert!(a < b);
    }

    #[test]
    fn serde_uses_the_public_form() {
        let id = UserId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        assert_eq!(serde_json::from_str::<UserId>(&json).unwrap(), id);
        assert!(serde_json::from_str::<OrgId>(&json).is_err());
    }

    proptest! {
        #[test]
        fn base32_round_trips(v in any::<u128>()) {
            let s = encode_base32(v);
            prop_assert_eq!(s.len(), ENCODED_LEN);
            prop_assert_eq!(decode_base32(&s).unwrap(), v);
        }

        #[test]
        fn id_round_trips(v in any::<u128>()) {
            let id = RequestId::from_uuid(Uuid::from_u128(v));
            prop_assert_eq!(id.to_string().parse::<RequestId>().unwrap(), id);
        }
    }
}
