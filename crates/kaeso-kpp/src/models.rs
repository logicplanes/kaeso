use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::KppError;
use crate::extras::Extras;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModelInfo {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owned_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<ModelLifecycle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retires_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl ModelInfo {
    pub fn new(id: impl Into<String>) -> Self {
        ModelInfo {
            id: id.into(),
            ..ModelInfo::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ModelLifecycle {
    Active,
    Deprecated,
    Retired,
    Other(String),
}

impl ModelLifecycle {
    pub fn from_wire(s: &str) -> Self {
        match s {
            "active" => ModelLifecycle::Active,
            "deprecated" => ModelLifecycle::Deprecated,
            "retired" => ModelLifecycle::Retired,
            other => ModelLifecycle::Other(other.to_owned()),
        }
    }

    pub fn as_wire(&self) -> &str {
        match self {
            ModelLifecycle::Active => "active",
            ModelLifecycle::Deprecated => "deprecated",
            ModelLifecycle::Retired => "retired",
            ModelLifecycle::Other(s) => s,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModelList {
    pub models: Vec<ModelInfo>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub has_more: bool,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

impl ModelList {
    pub fn new(models: Vec<ModelInfo>) -> Self {
        ModelList {
            models,
            has_more: false,
            extras: Extras::default(),
        }
    }

    pub fn page(
        all: &[ModelInfo],
        query: &ModelsQuery,
        default_limit: Option<u32>,
    ) -> Result<ModelList, KppError> {
        if query.after_id.is_some() && query.before_id.is_some() {
            return Err(KppError::invalid_request(
                "Send either `before_id` or `after_id`, not both.",
            )
            .with_param("before_id"));
        }
        if query.limit == Some(0) {
            return Err(KppError::invalid_request(
                "The query parameter `limit` must be at least 1.",
            )
            .with_param("limit"));
        }
        let mut seen = HashSet::new();
        let visible: Vec<&ModelInfo> = all
            .iter()
            .filter(|m| seen.insert(m.id.as_str()))
            .filter(|m| match (&m.lifecycle, query.lifecycle.is_empty()) {
                (None, _) => true,
                (Some(l), true) => *l != ModelLifecycle::Retired,
                (Some(l), false) => query.lifecycle.contains(l),
            })
            .collect();
        let position = |param: &str, id: &str| {
            visible.iter().position(|m| m.id == id).ok_or_else(|| {
                KppError::invalid_request(format!(
                    "The query parameter `{param}` names the model `{id}`, which is not in this listing. Use an id from `first_id`/`last_id` of a page listed with the same `lifecycle` filter."
                ))
                .with_param(param)
            })
        };
        let backward = query.before_id.is_some();
        let window: &[&ModelInfo] = if let Some(after) = &query.after_id {
            let i = position("after_id", after)?;
            visible.get(i.saturating_add(1)..).unwrap_or_default()
        } else if let Some(before) = &query.before_id {
            let i = position("before_id", before)?;
            visible.get(..i).unwrap_or_default()
        } else {
            &visible
        };
        let limit = query
            .limit
            .or(default_limit.map(|l| l.max(1)))
            .map_or(window.len(), |l| usize::try_from(l).unwrap_or(usize::MAX));
        let (page, has_more) = if backward {
            let start = window.len().saturating_sub(limit);
            (window.get(start..).unwrap_or_default(), start > 0)
        } else {
            (
                window.get(..limit.min(window.len())).unwrap_or_default(),
                window.len() > limit,
            )
        };
        Ok(ModelList {
            models: page.iter().map(|m| (*m).clone()).collect(),
            has_more,
            extras: Extras::default(),
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModelsQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifecycle: Vec<ModelLifecycle>,
    #[serde(default, skip_serializing_if = "Extras::is_empty")]
    pub extras: Extras,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn models(ids: &[&str]) -> Vec<ModelInfo> {
        ids.iter().map(|id| ModelInfo::new(*id)).collect()
    }

    fn ids(list: &ModelList) -> Vec<&str> {
        list.models.iter().map(|m| m.id.as_str()).collect()
    }

    fn q(limit: Option<u32>, after: Option<&str>, before: Option<&str>) -> ModelsQuery {
        ModelsQuery {
            limit,
            after_id: after.map(str::to_owned),
            before_id: before.map(str::to_owned),
            ..ModelsQuery::default()
        }
    }

    fn page(all: &[ModelInfo], query: &ModelsQuery, default: Option<u32>) -> ModelList {
        ModelList::page(all, query, default).unwrap()
    }

    #[test]
    fn pages_forward_and_backward() {
        let all = models(&["a", "b", "c", "d", "e"]);
        let p = page(&all, &q(Some(2), None, None), None);
        assert_eq!((ids(&p), p.has_more), (vec!["a", "b"], true));
        let p = page(&all, &q(Some(2), Some("b"), None), None);
        assert_eq!((ids(&p), p.has_more), (vec!["c", "d"], true));
        let p = page(&all, &q(Some(2), Some("d"), None), None);
        assert_eq!((ids(&p), p.has_more), (vec!["e"], false));
        let p = page(&all, &q(Some(2), Some("e"), None), None);
        assert_eq!((ids(&p), p.has_more), (vec![], false));
        let p = page(&all, &q(Some(2), None, Some("d")), None);
        assert_eq!((ids(&p), p.has_more), (vec!["b", "c"], true));
        let p = page(&all, &q(Some(5), None, Some("b")), None);
        assert_eq!((ids(&p), p.has_more), (vec!["a"], false));
        let p = page(&all, &q(None, None, None), Some(20));
        assert_eq!((ids(&p).len(), p.has_more), (5, false));
        let p = page(&all, &q(None, None, None), None);
        assert_eq!(ids(&p).len(), 5);
        let p = page(&all, &q(None, None, None), Some(0));
        assert_eq!(
            (ids(&p), p.has_more),
            (vec!["a"], true),
            "default limit 0 counts as 1"
        );
    }

    #[test]
    fn invalid_queries_are_refused_naming_the_parameter() {
        let all = models(&["a", "b"]);
        for (query, param) in [
            (q(Some(2), Some("zzz"), None), "after_id"),
            (q(Some(2), None, Some("zzz")), "before_id"),
            (q(None, Some("a"), Some("b")), "before_id"),
            (q(Some(0), None, None), "limit"),
        ] {
            let e = ModelList::page(&all, &query, Some(20)).unwrap_err();
            assert_eq!(e.kind, crate::ErrorKind::InvalidRequest);
            assert_eq!(e.param.as_deref(), Some(param));
            assert!(e.message.contains(&format!("`{param}`")), "{}", e.message);
        }
        let mut retired = models(&["old"]);
        retired[0].lifecycle = Some(ModelLifecycle::Retired);
        let e = ModelList::page(&retired, &q(None, Some("old"), None), None).unwrap_err();
        assert_eq!(
            e.param.as_deref(),
            Some("after_id"),
            "a filtered-out cursor is unknown"
        );
    }

    fn follow(all: &[ModelInfo], limit: u32, start_before: Option<&str>) -> Vec<String> {
        let mut query = q(Some(limit), None, start_before);
        let mut seen = Vec::new();
        for _ in 0..100 {
            let p = page(all, &query, None);
            seen.extend(p.models.iter().map(|m| m.id.clone()));
            let (Some(first), Some(last)) = (p.models.first(), p.models.last()) else {
                return seen;
            };
            if !p.has_more {
                return seen;
            }
            query = if start_before.is_some() {
                q(Some(limit), None, Some(&first.id))
            } else {
                q(Some(limit), Some(&last.id), None)
            };
        }
        panic!("paging did not terminate: {seen:?}");
    }

    #[test]
    fn duplicate_ids_are_listed_once_and_paging_terminates() {
        let all = models(&["a", "b", "a", "c"]);
        for limit in 1..=4 {
            assert_eq!(follow(&all, limit, None), ["a", "b", "c"], "limit {limit}");
            let mut back = follow(&all, limit, Some("c"));
            back.sort();
            assert_eq!(back, ["a", "b"], "limit {limit}");
        }
        let p = page(&all, &ModelsQuery::default(), None);
        assert_eq!(ids(&p), ["a", "b", "c"]);
    }

    #[test]
    fn lifecycle_filter_defaults_to_active_and_deprecated() {
        let mut all = models(&["live", "old", "gone", "unknown"]);
        all[0].lifecycle = Some(ModelLifecycle::Active);
        all[1].lifecycle = Some(ModelLifecycle::Deprecated);
        all[2].lifecycle = Some(ModelLifecycle::Retired);
        let p = page(&all, &ModelsQuery::default(), None);
        assert_eq!(ids(&p), ["live", "old", "unknown"]);
        let q = ModelsQuery {
            lifecycle: vec![ModelLifecycle::Retired],
            ..ModelsQuery::default()
        };
        assert_eq!(ids(&page(&all, &q, None)), ["gone", "unknown"]);
    }

    #[test]
    fn lifecycle_wire_values_round_trip() {
        for s in ["active", "deprecated", "retired", "sunset"] {
            assert_eq!(ModelLifecycle::from_wire(s).as_wire(), s);
        }
    }
}
