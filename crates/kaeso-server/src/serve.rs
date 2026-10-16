use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use kaeso_core::clock::{SharedClock, SystemClock};
use kaeso_crypto::{KeyRing, KeyRingError};
use kaeso_mail::{Instance, Mailer, SmtpSecurity, SmtpSettings, Transport};
use kaeso_store::StoreError;
use kaeso_store::clickhouse::ClickHouse;
use kaeso_store::pg::{self, instance};
use kaeso_store::valkey::Valkey;
use metrics_exporter_prometheus::PrometheusHandle;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::config::{Config, MailTransport, SmtpTls};
use crate::state::{AppState, Parts};

#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error("{0}")]
    KeyRing(#[from] KeyRingError),
    #[error("{0}")]
    Store(#[from] StoreError),
    #[error("cannot listen on {addr}: {error}")]
    Bind {
        addr: SocketAddr,
        error: std::io::Error,
    },
    #[error("serving failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Mail(String),
    #[error("{0}")]
    MasterKey(#[from] crate::services::master_key::MasterKeyError),
}

const POSTGRES_STARTUP_WAIT: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
pub struct Overrides {
    pub clock: Option<SharedClock>,
    pub mail: Option<Transport>,
    pub limits: Option<Arc<dyn crate::model_api::hooks::Limits>>,
    pub sink: Option<Arc<dyn crate::model_api::hooks::Sink>>,
}

pub async fn prepare(config: Config) -> Result<AppState, ServeError> {
    prepare_with(config, Overrides::default()).await
}

pub async fn prepare_with(config: Config, overrides: Overrides) -> Result<AppState, ServeError> {
    let keyring = KeyRing::parse(&config.master_keys)?;
    let pool = pg::connect(&config.postgres.url, config.postgres.pool_size)?;
    migrate_postgres(&pool).await?;
    crate::services::master_key::check(&pool, &keyring).await?;
    let clickhouse = ClickHouse::new(&config.clickhouse.url, &config.clickhouse.database)?;
    let valkey = Valkey::new(&config.valkey.url, &config.valkey.key_prefix)?;
    instance::seed(&pool, &instance_defaults(&config)).await?;
    let transport = overrides.mail.unwrap_or_else(|| mail_transport(&config));
    let mailer = Mailer::start(
        transport,
        &config.smtp.from,
        Instance::new(config.public_url.as_str()),
    )
    .map_err(|e| ServeError::Mail(e.to_string()))?;
    let state = AppState::new(Parts {
        config,
        pg: pool,
        clickhouse,
        valkey,
        keyring,
        clock: overrides.clock.unwrap_or_else(|| Arc::new(SystemClock)),
        mailer,
        limits: overrides.limits,
        sink: overrides.sink,
    });
    crate::services::models_dev::init(&state).await?;
    crate::services::models_dev::spawn_reloader(&state);
    Ok(state)
}

fn mail_transport(config: &Config) -> Transport {
    match (config.mail.transport, &config.smtp.host) {
        (MailTransport::Smtp, Some(host)) => Transport::Smtp(SmtpSettings {
            host: host.clone(),
            port: config.smtp.port,
            security: match config.smtp.tls {
                SmtpTls::Starttls => SmtpSecurity::Starttls,
                SmtpTls::Tls => SmtpSecurity::Tls,
                SmtpTls::None => SmtpSecurity::None,
            },
            credentials: config.smtp.user.clone().zip(config.smtp.password.clone()),
        }),
        _ => Transport::Log,
    }
}

async fn migrate_postgres(pool: &sqlx::PgPool) -> Result<(), StoreError> {
    let deadline = tokio::time::Instant::now() + POSTGRES_STARTUP_WAIT;
    loop {
        match pg::migrate(pool).await {
            Ok(()) => return Ok(()),
            Err(StoreError::Migrate(msg)) => return Err(StoreError::Migrate(msg)),
            Err(e) if tokio::time::Instant::now() < deadline => {
                tracing::warn!(error = %e, "PostgreSQL is not reachable yet; retrying");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

fn instance_defaults(config: &Config) -> instance::InstanceSettings {
    let i = &config.instance;
    let days = |d: u32| i32::try_from(d).unwrap_or(i32::MAX);
    instance::InstanceSettings {
        org_creation: i.org_creation.as_str().to_owned(),
        account_creation: i.account_creation.as_str().to_owned(),
        allowed_domains: Vec::new(),
        metadata_retention_days: days(i.metadata_retention_days),
        content_retention_days: days(i.content_retention_days),
        max_metadata_retention_days: days(i.max_metadata_retention_days),
        max_content_retention_days: days(i.max_content_retention_days),
        org_deletion: i.org_deletion,
        org_deletion_grace_days: days(i.org_deletion_grace_days),
        setup_completed_at: None,
    }
}

pub fn spawn_clickhouse_migrations(state: AppState, shutdown: CancellationToken) {
    tokio::spawn(async move {
        loop {
            match state.clickhouse.migrate(&state.pg).await {
                Ok(()) => {
                    state.set_clickhouse_migrated();
                    return;
                }
                Err(e @ StoreError::ClickHouseMigrate(_)) => {
                    tracing::error!(error = %e, "ClickHouse migrations failed; request logs are not written");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "ClickHouse is not reachable; retrying its migrations")
                }
            }
            tokio::select! {
                () = shutdown.cancelled() => return,
                () = tokio::time::sleep(Duration::from_secs(5)) => {}
            }
        }
    });
}

pub fn spawn_usage_flusher(state: AppState, shutdown: CancellationToken) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                () = shutdown.cancelled() => return,
                () = tokio::time::sleep(Duration::from_secs(10)) => {}
            }
            if let Err(e) = crate::services::usage::flush(&state).await {
                tracing::warn!(error = %e, "cannot write usage; retrying in 10 seconds");
            }
        }
    });
}

pub async fn run(
    state: AppState,
    metrics: PrometheusHandle,
    signal: impl Future<Output = ()> + Send + 'static,
) -> Result<(), ServeError> {
    let shutdown = CancellationToken::new();
    spawn_clickhouse_migrations(state.clone(), shutdown.clone());
    crate::jobs::spawn_all(&state, &shutdown);
    spawn_usage_flusher(state.clone(), shutdown.clone());
    crate::services::logs::spawn_writer(state.clone(), shutdown.clone());

    match crate::setup::issue_link(&state).await {
        Ok(Some(url)) => tracing::info!(
            setup_url = %url,
            "Kaeso is not set up yet. Open the setup link within 24 hours to create the first user and organization."
        ),
        Ok(None) => {}
        Err(e) => tracing::error!(error = %e, "cannot issue the setup link"),
    }

    let config = &state.config;
    let public = bind(config.listen).await?;
    let internal = bind(config.metrics_listen).await?;
    tracing::info!(listen = %config.listen, metrics = %config.metrics_listen, "Kaeso gateway is listening");

    let grace = config.shutdown_grace;
    let app = crate::app::router(state.clone());
    let metrics_app = crate::metrics::router(metrics);
    let trigger = shutdown.clone();
    tokio::spawn(async move {
        signal.await;
        tracing::info!("shutting down: no new connections; waiting for in-flight requests");
        trigger.cancel();
    });
    let (public, internal) = tokio::join!(
        serve_until(public, app, shutdown.clone(), grace),
        serve_until(
            internal,
            metrics_app,
            shutdown.clone(),
            Duration::from_secs(1)
        ),
    );
    public?;
    internal?;
    let open = state.in_flight.end(Duration::from_secs(10)).await;
    if open > 0 {
        tracing::error!(open, "requests could not record their outcome at shutdown");
    }
    match crate::services::usage::flush(&state).await {
        Ok(n) => tracing::info!(rows = n, "usage written"),
        Err(e) => tracing::error!(error = %e, "cannot write the last usage at shutdown"),
    }
    let written = crate::services::logs::flush_all(&state, Duration::from_secs(10)).await;
    let left = state.logs.len();
    if left > 0 {
        tracing::error!(
            written,
            left,
            "request logs could not all be written at shutdown"
        );
    } else {
        tracing::info!(rows = written, "request logs written");
    }
    state.pg.close().await;
    tracing::info!("Kaeso gateway stopped");
    Ok(())
}

async fn bind(addr: SocketAddr) -> Result<TcpListener, ServeError> {
    TcpListener::bind(addr)
        .await
        .map_err(|error| ServeError::Bind { addr, error })
}

pub async fn serve_until(
    listener: TcpListener,
    router: Router,
    shutdown: CancellationToken,
    grace: Duration,
) -> Result<(), std::io::Error> {
    let server = axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown.clone().cancelled_owned());
    let mut server = tokio::spawn(async move { server.await });
    tokio::select! {
        result = &mut server => return flatten(result),
        () = shutdown.cancelled() => {}
    }
    match tokio::time::timeout(grace, &mut server).await {
        Ok(result) => flatten(result),
        Err(_) => {
            tracing::warn!(grace = ?grace, "requests still open after the shutdown grace period; closing them");
            server.abort();
            Ok(())
        }
    }
}

fn flatten(result: Result<std::io::Result<()>, tokio::task::JoinError>) -> std::io::Result<()> {
    match result {
        Ok(r) => r,
        Err(e) => Err(std::io::Error::other(e)),
    }
}
