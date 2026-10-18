use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use kaeso_store::pg::{updates, users};
use kaeso_update::Version;
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::platform::access::Ctx;
use crate::platform::error::ApiResult;
use crate::services::updates::{self as service, Known};
use crate::state::AppState;

const MAX_RELEASES: usize = 10;

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "A Kaeso release the signed-in user hasn't seen yet.")]
pub struct WhatsNewRelease {
    #[doc = "The version."]
    #[schema(example = "40.2.0")]
    pub version: String,
    #[doc = "When it was published; `null` when the gateway doesn't know the release list."]
    #[serde(with = "time::serde::rfc3339::option")]
    pub published_at: Option<OffsetDateTime>,
    #[doc = "Whether it is a pre-release."]
    pub prerelease: bool,
    #[doc = "The release notes, in Markdown; empty when the gateway doesn't know them."]
    pub notes: String,
    #[doc = "The release's page."]
    #[schema(example = "https://github.com/logicplanes/kaeso/releases/tag/v40.2.0")]
    pub url: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(description = "What changed in Kaeso since the signed-in user last saw it.")]
pub struct WhatsNewView {
    #[doc = "The version the gateway runs."]
    #[schema(example = json!(env!("CARGO_PKG_VERSION")))]
    pub current: String,
    #[doc = "The last version the user saw; `null` for someone who never signed in to the console before, who has nothing new to see."]
    pub seen: Option<String>,
    #[doc = "Every release after `seen` up to the running version, newest first, at most 10; pre-releases only while a pre-release runs."]
    pub releases: Vec<WhatsNewRelease>,
}

fn release_page(version: &Version) -> String {
    format!("https://github.com/logicplanes/kaeso/releases/tag/v{version}")
}

fn unseen(running: &Version, seen: &Version, known: &[Known]) -> Vec<WhatsNewRelease> {
    let mut releases: Vec<&Known> = known
        .iter()
        .filter(|k| k.version > *seen && k.version <= *running)
        .filter(|k| k.version == *running || !running.pre.is_empty() || k.version.pre.is_empty())
        .collect();
    releases.sort_by(|a, b| b.version.cmp(&a.version));
    let mut out: Vec<WhatsNewRelease> = releases
        .into_iter()
        .take(MAX_RELEASES)
        .map(|k| WhatsNewRelease {
            version: k.version.to_string(),
            published_at: Some(k.row.published_at),
            prerelease: k.row.prerelease,
            notes: k.row.notes.clone(),
            url: k.row.url.clone(),
        })
        .collect();
    if running > seen && !out.iter().any(|r| r.version == running.to_string()) {
        out.insert(
            0,
            WhatsNewRelease {
                version: running.to_string(),
                published_at: None,
                prerelease: !running.pre.is_empty(),
                notes: String::new(),
                url: release_page(running),
            },
        );
        out.truncate(MAX_RELEASES);
    }
    out
}

#[utoipa::path(
    get,
    path = "/me/whats-new",
    summary = "The Kaeso releases since the signed-in user last saw what's new, with their notes.",
    tag = "me",
    responses((status = 200, body = WhatsNewView))
)]
pub async fn get(State(state): State<AppState>, ctx: Ctx) -> ApiResult<Json<WhatsNewView>> {
    let running = service::running();
    let seen = users::seen_version(&state.pg, ctx.user_id()?.uuid()).await?;
    let releases = match seen.as_deref().map(Version::parse) {
        Some(Ok(seen)) => unseen(
            &running,
            &seen,
            &service::known(updates::releases(&state.pg).await?),
        ),
        _ => Vec::new(),
    };
    Ok(Json(WhatsNewView {
        current: running.to_string(),
        seen,
        releases,
    }))
}

#[utoipa::path(
    post,
    path = "/me/whats-new/seen",
    summary = "Records that the signed-in user saw what's new in the running version.",
    tag = "me",
    responses((status = 204, description = "Seen."))
)]
pub async fn seen(State(state): State<AppState>, ctx: Ctx) -> ApiResult<StatusCode> {
    users::set_seen_version(
        &state.pg,
        ctx.user_id()?.uuid(),
        &service::running().to_string(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use kaeso_store::pg::updates::ReleaseRow;

    fn known(versions: &[(&str, bool)]) -> Vec<Known> {
        service::known(
            versions
                .iter()
                .map(|(v, pre)| ReleaseRow {
                    version: (*v).to_owned(),
                    published_at: OffsetDateTime::UNIX_EPOCH,
                    prerelease: *pre,
                    notes: format!("Notes of {v}"),
                    url: format!("https://example.com/{v}"),
                    manifest: None,
                    manifest_error: None,
                })
                .collect(),
        )
    }

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn lists_the_stable_releases_since_the_last_seen_newest_first() {
        let list = known(&[
            ("1.0.0", false),
            ("1.1.0-rc.1", true),
            ("1.1.0", false),
            ("1.2.0", false),
            ("1.3.0", false),
        ]);
        let out = unseen(&v("1.2.0"), &v("1.0.0"), &list);
        let versions: Vec<&str> = out.iter().map(|r| r.version.as_str()).collect();
        assert_eq!(versions, ["1.2.0", "1.1.0"]);
        assert_eq!(out[0].notes, "Notes of 1.2.0");
        assert!(unseen(&v("1.2.0"), &v("1.2.0"), &list).is_empty());
    }

    #[test]
    fn a_running_pre_release_shows_pre_releases_too() {
        let list = known(&[("1.1.0-rc.1", true), ("1.1.0-rc.2", true)]);
        let out = unseen(&v("1.1.0-rc.2"), &v("1.0.0"), &list);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn a_running_version_the_gateway_has_no_notes_for_still_shows() {
        let out = unseen(&v("2.0.0"), &v("1.0.0"), &[]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].version, "2.0.0");
        assert_eq!(out[0].notes, "");
        assert_eq!(out[0].published_at, None);
        assert_eq!(
            out[0].url,
            "https://github.com/logicplanes/kaeso/releases/tag/v2.0.0"
        );
    }

    #[test]
    fn at_most_ten_releases_show() {
        let names: Vec<String> = (1..=15).map(|m| format!("1.{m}.0")).collect();
        let pairs: Vec<(&str, bool)> = names.iter().map(|n| (n.as_str(), false)).collect();
        let out = unseen(&v("1.15.0"), &v("1.0.0"), &known(&pairs));
        assert_eq!(out.len(), 10);
        assert_eq!(out[0].version, "1.15.0");
    }
}
