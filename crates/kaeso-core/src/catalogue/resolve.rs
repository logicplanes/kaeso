use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Entry;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alias {
    pub name: String,
    pub target: Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    Alias,
    Prefixed,
    Native,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    #[error(
        "The model `{0}` is not in this project's model catalogue. A project admin can add it under the project's models."
    )]
    NotInCatalogue(String),
    #[error(
        "The model name `{name}` is served by several providers in this project ({}). Use a prefixed name such as `{}/{name}`.",
        .providers.join(", "),
        .providers.first().map_or("provider", String::as_str)
    )]
    Ambiguous {
        name: String,
        providers: Vec<String>,
    },
}

pub fn resolve<'a>(
    name: &str,
    entries: &'a [Entry],
    aliases: &[Alias],
) -> Result<(&'a Entry, Via), ResolveError> {
    if let Some(alias) = aliases.iter().find(|a| a.name == name)
        && let Some(entry) = entries.iter().find(|e| e.id == alias.target)
    {
        return Ok((entry, Via::Alias));
    }
    if let Some((provider, model)) = name.split_once('/')
        && let Some(entry) = entries
            .iter()
            .find(|e| e.provider == provider && e.model_id == model)
    {
        return Ok((entry, Via::Prefixed));
    }
    let native: Vec<&Entry> = entries.iter().filter(|e| e.model_id == name).collect();
    match native.as_slice() {
        [] => Err(ResolveError::NotInCatalogue(name.to_owned())),
        [one] => Ok((one, Via::Native)),
        many => Err(ResolveError::Ambiguous {
            name: name.to_owned(),
            providers: many.iter().map(|e| e.provider.clone()).collect(),
        }),
    }
}

pub fn shadows(name: &str, entries: &[Entry], snapshot: &crate::models_dev::Snapshot) -> bool {
    entries
        .iter()
        .any(|e| e.model_id == name || format!("{}/{}", e.provider, e.model_id) == name)
        || snapshot
            .providers
            .values()
            .any(|p| p.models.contains_key(name))
}

pub fn validate_alias_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.chars().count() > 200 {
        return Err("must be 1 to 200 characters".into());
    }
    if name
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '/')
    {
        return Err("must not contain spaces, control characters or `/`".into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessLevel {
    Member,
    Key,
}

impl AccessLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            AccessLevel::Member => "member",
            AccessLevel::Key => "key",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Access {
    pub member: Option<BTreeSet<Uuid>>,
    pub key: Option<BTreeSet<Uuid>>,
}

impl Access {
    pub fn check(&self, entry: Uuid) -> Result<(), AccessLevel> {
        if self.member.as_ref().is_some_and(|m| !m.contains(&entry)) {
            return Err(AccessLevel::Member);
        }
        if self.key.as_ref().is_some_and(|k| !k.contains(&entry)) {
            return Err(AccessLevel::Key);
        }
        Ok(())
    }

    pub fn allows(&self, entry: Uuid) -> bool {
        self.check(entry).is_ok()
    }
}

pub fn access_message(model: &str, level: AccessLevel) -> String {
    match level {
        AccessLevel::Member => format!(
            "The model `{model}` is not in the models this key's member may use in this project. A project admin can change the member's model access."
        ),
        AccessLevel::Key => format!(
            "The model `{model}` is not in this key's model list. Create a key with access to it, or ask a project admin to change the key's model access."
        ),
    }
}

pub fn check_subset(list: &BTreeSet<Uuid>, allowed: Option<&BTreeSet<Uuid>>) -> Result<(), Uuid> {
    match allowed {
        None => Ok(()),
        Some(allowed) => match list.iter().find(|id| !allowed.contains(id)) {
            None => Ok(()),
            Some(id) => Err(*id),
        },
    }
}
