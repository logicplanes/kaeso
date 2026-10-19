use std::sync::Arc;

use kaeso_core::catalogue::resolve::{Alias, resolve};
use kaeso_core::catalogue::{self as core, Effective};
use kaeso_store::StoreError;
use kaeso_store::pg::catalogue::{self, ProjectModels};
use uuid::Uuid;

use crate::state::AppState;

pub async fn project_models(
    state: &AppState,
    project: Uuid,
) -> Result<Arc<ProjectModels>, StoreError> {
    let pg = state.pg.clone();
    let found = state
        .caches
        .project_models
        .get_or_load(&project.to_string(), || async move {
            let mut conn = pg.acquire().await?;
            catalogue::project_models(&mut conn, project)
                .await
                .map(Some)
        })
        .await?;
    Ok(found.unwrap_or_default())
}

pub async fn forget(state: &AppState, project: Uuid) {
    state
        .caches
        .project_models
        .invalidate(&project.to_string())
        .await;
}

pub fn effective(state: &AppState, entry: &core::Entry) -> Option<Effective> {
    let loaded = state.models_dev.current();
    core::effective(entry, &loaded.snapshot, state.models_dev.supplement())
}

pub fn request_name(entry: &core::Entry, entries: &[core::Entry], aliases: &[Alias]) -> String {
    match resolve(&entry.model_id, entries, aliases) {
        Ok((found, _)) if found.id == entry.id => entry.model_id.clone(),
        _ => format!("{}/{}", entry.provider, entry.model_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(provider: &str, model: &str) -> core::Entry {
        core::Entry {
            id: Uuid::now_v7(),
            provider: provider.into(),
            model_id: model.into(),
            overrides: core::Fields::default(),
            custom: false,
            delisted: false,
            last_known: None,
            availability: None,
        }
    }

    #[test]
    fn a_name_that_resolves_elsewhere_is_sent_with_its_provider() {
        for (routed, direct) in [
            (
                entry("openrouter", "openai/gpt-4o"),
                entry("openai", "gpt-4o"),
            ),
            (
                entry("groq", "openai/gpt-oss-20b"),
                entry("openai", "gpt-oss-20b"),
            ),
        ] {
            let both = vec![routed.clone(), direct.clone()];
            let routed_name = request_name(&routed, &both, &[]);
            assert_eq!(
                routed_name,
                format!("{}/{}", routed.provider, routed.model_id)
            );
            assert_eq!(request_name(&direct, &both, &[]), direct.model_id);
            for (e, name) in [(&routed, routed_name), (&direct, direct.model_id.clone())] {
                assert_eq!(resolve(&name, &both, &[]).unwrap().0.id, e.id);
            }
            let only = vec![routed.clone()];
            assert_eq!(request_name(&routed, &only, &[]), routed.model_id);
        }
    }

    #[test]
    fn a_name_several_providers_serve_or_an_alias_takes_is_sent_with_its_provider() {
        let a = entry("openai", "gpt-4o");
        let b = entry("azure", "gpt-4o");
        let c = entry("anthropic", "claude-haiku-4-5");
        let all = vec![a.clone(), b.clone(), c.clone()];
        assert_eq!(request_name(&a, &all, &[]), "openai/gpt-4o");
        assert_eq!(request_name(&b, &all, &[]), "azure/gpt-4o");
        let alias = Alias {
            name: "claude-haiku-4-5".into(),
            target: a.id,
        };
        assert_eq!(
            request_name(&c, &all, std::slice::from_ref(&alias)),
            "anthropic/claude-haiku-4-5"
        );
    }
}
