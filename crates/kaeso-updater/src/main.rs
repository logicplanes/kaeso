use std::process::ExitCode;

use kaeso_updater::config::{Config, healthcheck_address};

#[tokio::main]
async fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        let listen = std::env::var("KAESO_UPDATER_LISTEN").ok();
        return if kaeso_updater::healthcheck(&healthcheck_address(listen.as_deref())).await {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("KAESO_UPDATER_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let config = match Config::load(
        |name| std::env::var(name).ok(),
        |path| std::fs::read_to_string(path),
    ) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "the updater cannot start");
            return ExitCode::FAILURE;
        }
    };
    let shutdown = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = term.recv() => {}
                    _ = tokio::signal::ctrl_c() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    };
    match kaeso_updater::serve(config, shutdown).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "the updater stopped");
            ExitCode::FAILURE
        }
    }
}
