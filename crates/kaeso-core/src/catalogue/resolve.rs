use std::collections::{BTreeSet, HashMap};

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
    #[error("`{0}` is not in the project's model catalogue")]
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalogue {
    entries: Vec<Entry>,
    aliases: Vec<Alias>,
    by_id: HashMap<Uuid, usize>,
    by_alias: HashMap<String, Uuid>,
    by_target: HashMap<Uuid, Vec<usize>>,
    by_provider: HashMap<String, HashMap<String, usize>>,
    by_model: HashMap<String, Vec<usize>>,
}

impl Catalogue {
    pub fn new(entries: Vec<Entry>, aliases: Vec<Alias>) -> Catalogue {
        let mut by_id = HashMap::with_capacity(entries.len());
        let mut by_provider: HashMap<String, HashMap<String, usize>> = HashMap::new();
        let mut by_model: HashMap<String, Vec<usize>> = HashMap::with_capacity(entries.len());
        for (i, e) in entries.iter().enumerate() {
            by_id.entry(e.id).or_insert(i);
            by_provider
                .entry(e.provider.clone())
                .or_default()
                .entry(e.model_id.clone())
                .or_insert(i);
            by_model.entry(e.model_id.clone()).or_default().push(i);
        }
        let mut by_alias = HashMap::with_capacity(aliases.len());
        let mut by_target: HashMap<Uuid, Vec<usize>> = HashMap::with_capacity(aliases.len());
        for (i, a) in aliases.iter().enumerate() {
            by_alias.entry(a.name.clone()).or_insert(a.target);
            by_target.entry(a.target).or_default().push(i);
        }
        Catalogue {
            entries,
            aliases,
            by_id,
            by_alias,
            by_target,
            by_provider,
            by_model,
        }
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn aliases(&self) -> &[Alias] {
        &self.aliases
    }

    pub fn entry(&self, id: Uuid) -> Option<&Entry> {
        self.by_id.get(&id).and_then(|i| self.entries.get(*i))
    }

    pub fn aliases_of(&self, id: Uuid) -> impl Iterator<Item = &Alias> {
        self.by_target
            .get(&id)
            .into_iter()
            .flatten()
            .filter_map(|i| self.aliases.get(*i))
    }

    pub fn sharing(&self, model_id: &str) -> usize {
        self.by_model.get(model_id).map_or(0, Vec::len)
    }

    pub fn resolve(&self, name: &str) -> Result<(&Entry, Via), ResolveError> {
        if let Some(entry) = self.by_alias.get(name).and_then(|id| self.entry(*id)) {
            return Ok((entry, Via::Alias));
        }
        if let Some(entry) = self.prefixed(name) {
            return Ok((entry, Via::Prefixed));
        }
        let native = self.by_model.get(name).map_or(&[][..], Vec::as_slice);
        match native {
            [one] => self
                .entries
                .get(*one)
                .map(|e| (e, Via::Native))
                .ok_or_else(|| ResolveError::NotInCatalogue(name.to_owned())),
            [] => Err(ResolveError::NotInCatalogue(name.to_owned())),
            many => Err(ResolveError::Ambiguous {
                name: name.to_owned(),
                providers: many
                    .iter()
                    .filter_map(|i| self.entries.get(*i))
                    .map(|e| e.provider.clone())
                    .collect(),
            }),
        }
    }

    pub fn shadows(&self, name: &str, snapshot: &crate::models_dev::Snapshot) -> bool {
        self.by_model.contains_key(name)
            || self.prefixed(name).is_some()
            || snapshot
                .providers
                .values()
                .any(|p| p.models.contains_key(name))
    }

    fn prefixed(&self, name: &str) -> Option<&Entry> {
        let (provider, model) = name.split_once('/')?;
        let i = self.by_provider.get(provider)?.get(model)?;
        self.entries.get(*i)
    }
}

pub fn validate_alias_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.chars().count() > 200 {
        return Err("must be 1 to 200 characters".into());
    }
    if name
        .chars()
        .any(|c| c.is_whitespace() || crate::text::hidden(c) || c == '/')
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Access<'a> {
    pub member: Option<&'a BTreeSet<Uuid>>,
    pub key: Option<&'a BTreeSet<Uuid>>,
}

impl Access<'_> {
    pub fn check(&self, entry: Uuid) -> Result<(), AccessLevel> {
        if self.member.is_some_and(|m| !m.contains(&entry)) {
            return Err(AccessLevel::Member);
        }
        if self.key.is_some_and(|k| !k.contains(&entry)) {
            return Err(AccessLevel::Key);
        }
        Ok(())
    }

    pub fn allows(&self, entry: Uuid) -> bool {
        self.check(entry).is_ok()
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
