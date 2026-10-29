use std::sync::Arc;

use kaeso_core::catalogue::resolve::Catalogue;
use kaeso_core::catalogue::{self as core, Effective};
use kaeso_core::providers::OrgProviders;
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
        .get_or_refresh(&project.to_string(), move || async move {
            let mut conn = pg.acquire().await?;
            catalogue::project_models(&mut conn, project)
                .await
                .map(Some)
        })
        .await?;
    Ok(found.unwrap_or_default())
}

pub async fn project_models_for(
    state: &AppState,
    project: Uuid,
    key: Option<Uuid>,
    service_account: Option<Uuid>,
) -> Result<Arc<ProjectModels>, StoreError> {
    let models = project_models(state, project).await?;
    if models.knows(key, service_account) {
        return Ok(models);
    }
    forget(state, project).await;
    project_models(state, project).await
}

pub async fn forget(state: &AppState, project: Uuid) {
    state
        .caches
        .project_models
        .invalidate(&project.to_string())
        .await;
}

pub fn effective(
    state: &AppState,
    entry: &core::Entry,
    providers: &OrgProviders,
) -> Option<Effective> {
    let loaded = state.models_dev.current();
    core::effective(
        entry,
        &loaded.snapshot,
        state.models_dev.supplement(),
        crate::services::custom_providers::in_effect(state, providers),
    )
}

pub fn request_name(entry: &core::Entry, catalogue: &Catalogue) -> String {
    match catalogue.resolve(&entry.model_id) {
        Ok((found, _)) if found.id == entry.id => entry.model_id.clone(),
        _ => format!("{}/{}", entry.provider, entry.model_id),
    }
}

#[cfg(test)]
mod tests {
    use kaeso_core::catalogue::resolve::Alias;

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
            availability_checked_at: None,
        }
    }

    #[test]
    fn a_name_that_resolves_elsewhere_is_sent_with_its_provider() {
        for (routed, direct) in [
            (
                entry("togetherai", "openai/gpt-oss-120b"),
                entry("openai", "gpt-oss-120b"),
            ),
            (
                entry("togetherai", "deepseek/deepseek-chat"),
                entry("deepseek", "deepseek-chat"),
            ),
            (
                entry("openrouter", "openai/gpt-4o"),
                entry("openai", "gpt-4o"),
            ),
            (
                entry("vercel", "openai/gpt-5-nano"),
                entry("openai", "gpt-5-nano"),
            ),
        ] {
            let both = Catalogue::new(vec![routed.clone(), direct.clone()], vec![]);
            let routed_name = request_name(&routed, &both);
            assert_eq!(
                routed_name,
                format!("{}/{}", routed.provider, routed.model_id)
            );
            assert_eq!(request_name(&direct, &both), direct.model_id);
            for (e, name) in [(&routed, routed_name), (&direct, direct.model_id.clone())] {
                assert_eq!(both.resolve(&name).unwrap().0.id, e.id);
            }
            let only = Catalogue::new(vec![routed.clone()], vec![]);
            assert_eq!(request_name(&routed, &only), routed.model_id);
        }
    }

    #[test]
    fn a_name_several_providers_serve_or_an_alias_takes_is_sent_with_its_provider() {
        let a = entry("openai", "gpt-4o");
        let b = entry("azure", "gpt-4o");
        let c = entry("anthropic", "claude-haiku-4-5");
        let entries = vec![a.clone(), b.clone(), c.clone()];
        let all = Catalogue::new(entries.clone(), vec![]);
        assert_eq!(request_name(&a, &all), "openai/gpt-4o");
        assert_eq!(request_name(&b, &all), "azure/gpt-4o");
        let alias = Alias {
            name: "claude-haiku-4-5".into(),
            target: a.id,
        };
        assert_eq!(
            request_name(&c, &Catalogue::new(entries, vec![alias])),
            "anthropic/claude-haiku-4-5"
        );
    }
}
