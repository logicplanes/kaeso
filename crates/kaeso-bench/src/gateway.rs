use std::io::Write;

use kaeso_server::config::{self, ProcessSource};

use crate::Result;

pub(crate) const READY: &str = "kaeso-bench ready";
pub(crate) const SETUP_TOKEN: &str = " setup-token=";

// In a process of its own, so the gateway's memory is measured apart from the clients and the mock
// provider. It stops when its standard input closes, so it never outlives the benchmark.
pub(crate) async fn serve() -> Result<()> {
    let config = config::load(&ProcessSource)?;
    kaeso_server::telemetry::init_logging(&config.log)?;
    let metrics = kaeso_server::telemetry::init_metrics()?;
    let state = kaeso_server::serve::prepare(config).await?;
    let token = kaeso_server::setup::issue_link(&state)
        .await?
        .and_then(|link| link.split_once("#token=").map(|(_, t)| t.to_owned()));
    match token {
        Some(token) => writeln!(std::io::stdout().lock(), "{READY}{SETUP_TOKEN}{token}")?,
        None => writeln!(std::io::stdout().lock(), "{READY}")?,
    }
    let (closed, stop) = tokio::sync::oneshot::channel::<()>();
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink());
        let _ = closed.send(());
    });
    kaeso_server::serve::run(state, metrics, async move {
        let _ = stop.await;
    })
    .await?;
    Ok(())
}
