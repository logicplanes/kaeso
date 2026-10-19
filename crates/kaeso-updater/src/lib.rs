#![forbid(unsafe_code)]

pub mod clone;
pub mod compose;
pub mod config;
pub mod docker;
pub mod engine;
pub mod files;
pub mod guard;
pub mod server;
pub mod source;
pub mod verify;

use std::sync::Arc;
use std::time::Duration;

use kaeso_update::RELEASE_WORKFLOW;

use crate::config::Config;
use crate::docker::Docker;
use crate::engine::Engine;
use crate::source::{Endpoints, Source};
use crate::verify::{Sigstore, Trust};

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Config(#[from] config::ConfigError),
    #[error(transparent)]
    Trust(#[from] verify::VerifyError),
    #[error("cannot build an HTTP client: {0}")]
    Client(#[from] reqwest::Error),
    #[error("cannot listen on the updater address: {0}")]
    Listen(std::io::Error),
    #[error("the release addresses are invalid: {0}")]
    Endpoints(#[from] url::ParseError),
}

pub async fn serve(
    config: Config,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), RunError> {
    let source = Source::new(Endpoints::github()?)?;
    let verify = Arc::new(Sigstore::new(
        Trust::embedded()?,
        RELEASE_WORKFLOW,
        source.clone(),
    ));
    let docker = Docker::new(&config.docker)?;
    let engine = Engine::new(config.settings(), docker, source, verify);
    let recovering = Arc::clone(&engine);
    tokio::spawn(async move { recovering.recover().await });
    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .map_err(RunError::Listen)?;
    tracing::info!(listen = %config.listen, version = env!("CARGO_PKG_VERSION"), "the updater listens");
    axum::serve(listener, server::router(engine, config.token))
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(RunError::Listen)
}

pub async fn healthcheck(address: &str) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    else {
        return false;
    };
    match client.get(format!("http://{address}/healthz")).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}
