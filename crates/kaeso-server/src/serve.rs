use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo, TokioTimer};
use hyper_util::server::conn::auto::Builder;
use kaeso_core::clock::{SharedClock, SystemClock};
use kaeso_crypto::{KeyRing, KeyRingError};
use kaeso_mail::{Instance, MailError, Mailer, SmtpSecurity, SmtpSettings, Transport};
use kaeso_store::StoreError;
use kaeso_store::clickhouse::ClickHouse;
use kaeso_store::pg::{self, instance};
use kaeso_store::valkey::Valkey;
use metrics_exporter_prometheus::PrometheusHandle;
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tower::ServiceExt;

use crate::config::{Config, MailTransport, SmtpTls};
use crate::state::{AppState, Parts};

#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error(transparent)]
    KeyRing(#[from] KeyRingError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("cannot listen on {addr}: {error}")]
    Bind {
        addr: SocketAddr,
        error: std::io::Error,
    },
    #[error("serving failed: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Mail(#[from] MailError),
    #[error(transparent)]
    MasterKey(#[from] crate::services::master_key::MasterKeyError),
}

const POSTGRES_STARTUP_WAIT: Duration = Duration::from_secs(60);
const MAIL_FLUSH_WAIT: Duration = Duration::from_secs(10);
const HTTP2_PING_INTERVAL: Duration = Duration::from_secs(30);
const HTTP2_PING_TIMEOUT: Duration = Duration::from_secs(20);
const ACCEPT_RETRY_WAIT: Duration = Duration::from_secs(1);
const POSTGRES_RETRY_WAIT: Duration = Duration::from_secs(2);
const CLICKHOUSE_MIGRATION_RETRY_WAIT: Duration = Duration::from_secs(5);
const USAGE_FLUSH_INTERVAL: Duration = Duration::from_secs(10);
const METRICS_SHUTDOWN_GRACE: Duration = Duration::from_secs(1);
const IN_FLIGHT_SHUTDOWN_WAIT: Duration = Duration::from_secs(10);
const LOG_FLUSH_SHUTDOWN_WAIT: Duration = Duration::from_secs(10);
const OVERDUE_ANSWER_WAIT: Duration = Duration::from_secs(1);
const METRICS_MAX_CONNECTIONS: usize = 64;

#[derive(Clone, Copy, Debug)]
pub struct Connections {
    pub header_read_timeout: Duration,
    pub max: usize,
}

#[derive(Debug, Default)]
pub struct Overrides {
    pub clock: Option<SharedClock>,
    pub mail: Option<Transport>,
    pub limits: Option<Arc<dyn crate::model_api::hooks::Limits>>,
    pub sink: Option<Arc<dyn crate::model_api::hooks::Sink>>,
    pub dns: Option<Arc<dyn crate::services::dns::TxtLookup>>,
    pub accounts: Option<crate::services::provider_accounts::Endpoints>,
}

pub async fn prepare(config: Config) -> Result<AppState, ServeError> {
    prepare_with(config, Overrides::default()).await
}

pub async fn prepare_with(config: Config, overrides: Overrides) -> Result<AppState, ServeError> {
    let keyring = KeyRing::parse(&config.master_keys)?;
    let pool = pg::connect(&config.postgres.url, config.postgres.pool_size)?;
    migrate_postgres(&pool).await?;
    crate::services::master_key::check(&pool, &keyring).await?;
    let clickhouse = config
        .clickhouse
        .url
        .as_ref()
        .map(|url| ClickHouse::new(url, &config.clickhouse.database))
        .transpose()?;
    let valkey = Valkey::new(&config.valkey.url, &config.valkey.key_prefix)?;
    instance::seed(&pool, &instance_defaults(&config)).await?;
    pg::updates::seed(&pool, &crate::services::updates::defaults(&config)).await?;
    let transport = overrides.mail.unwrap_or_else(|| mail_transport(&config));
    let mailer = Mailer::start(
        transport,
        &config.smtp.from,
        Instance::new(config.public_url.as_str()),
    )?;
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
        dns: overrides.dns,
        accounts: overrides.accounts,
    });
    crate::services::models_dev::init(&state).await?;
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
                tokio::time::sleep(POSTGRES_RETRY_WAIT).await;
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
        timezone: i.timezone.trim().to_owned(),
        setup_completed_at: None,
    }
}

pub fn spawn_clickhouse_migrations(state: AppState, shutdown: CancellationToken) {
    let Some(clickhouse) = state.clickhouse.clone() else {
        tracing::info!("request logs are off: KAESO_CLICKHOUSE_URL is not set");
        return;
    };
    tokio::spawn(async move {
        loop {
            match clickhouse.migrate(&state.pg).await {
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
                () = tokio::time::sleep(CLICKHOUSE_MIGRATION_RETRY_WAIT) => {}
            }
        }
    });
}

pub fn spawn_usage_flusher(state: AppState, shutdown: CancellationToken) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                () = shutdown.cancelled() => return,
                () = tokio::time::sleep(USAGE_FLUSH_INTERVAL) => {}
            }
            if let Err(e) = crate::services::usage::flush(&state).await {
                tracing::warn!(error = %e, retry_in = ?USAGE_FLUSH_INTERVAL, "cannot write usage; retrying");
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
    crate::services::models_dev::spawn_reloader(&state);
    spawn_clickhouse_migrations(state.clone(), shutdown.clone());
    crate::jobs::spawn_all(&state, &shutdown);
    spawn_usage_flusher(state.clone(), shutdown.clone());
    crate::services::logs::spawn_writer(state.clone(), shutdown.clone());

    crate::setup::announce(&state).await;

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
    let header_read_timeout = config.server.header_read_timeout;
    let (public, internal) = tokio::join!(
        serve_until(
            public,
            app,
            shutdown.clone(),
            grace,
            Connections {
                header_read_timeout,
                max: config.server.max_connections,
            },
            state.in_flight.cutter()
        ),
        serve_until(
            internal,
            metrics_app,
            shutdown.clone(),
            METRICS_SHUTDOWN_GRACE,
            Connections {
                header_read_timeout,
                max: METRICS_MAX_CONNECTIONS,
            },
            CancellationToken::new()
        ),
    );
    public?;
    internal?;
    let open = state.in_flight.end(IN_FLIGHT_SHUTDOWN_WAIT).await;
    if open > 0 {
        tracing::error!(open, "requests could not record their outcome at shutdown");
    }
    match crate::services::usage::flush(&state).await {
        Ok(n) => tracing::info!(rows = n, "usage written"),
        Err(e) => tracing::error!(error = %e, "cannot write the last usage at shutdown"),
    }
    let written = crate::services::logs::flush_all(&state, LOG_FLUSH_SHUTDOWN_WAIT).await;
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
    if tokio::time::timeout(MAIL_FLUSH_WAIT, state.mailer.flush())
        .await
        .is_err()
    {
        tracing::error!(wait = ?MAIL_FLUSH_WAIT, "queued emails could not all be sent at shutdown");
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
    connections: Connections,
    overdue: CancellationToken,
) -> Result<(), std::io::Error> {
    let kill = CancellationToken::new();
    let server = accept_until(
        listener,
        router,
        shutdown.clone(),
        connections,
        kill.clone(),
    );
    let mut server = tokio::spawn(server);
    tokio::select! {
        result = &mut server => return flatten(result),
        () = shutdown.cancelled() => {}
    }
    if let Ok(result) = tokio::time::timeout(grace, &mut server).await {
        return flatten(result);
    }
    tracing::warn!(grace = ?grace, "requests still open after the shutdown grace period; ending them");
    overdue.cancel();
    if let Ok(result) = tokio::time::timeout(OVERDUE_ANSWER_WAIT, &mut server).await {
        return flatten(result);
    }
    tracing::warn!("connections still open after their requests were ended; closing them");
    kill.cancel();
    flatten(server.await)
}

fn flatten(result: Result<std::io::Result<()>, tokio::task::JoinError>) -> std::io::Result<()> {
    match result {
        Ok(r) => r,
        Err(e) => Err(std::io::Error::other(e)),
    }
}

fn connection_builder(header_read_timeout: Duration) -> Builder<TokioExecutor> {
    let mut builder = Builder::new(TokioExecutor::new());
    builder
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(header_read_timeout);
    builder
        .http2()
        .timer(TokioTimer::new())
        .keep_alive_interval(HTTP2_PING_INTERVAL)
        .keep_alive_timeout(HTTP2_PING_TIMEOUT);
    builder
}

async fn accept_until(
    listener: TcpListener,
    router: Router,
    shutdown: CancellationToken,
    limits: Connections,
    kill: CancellationToken,
) -> std::io::Result<()> {
    let header_read_timeout = limits.header_read_timeout;
    let builder = Arc::new(connection_builder(header_read_timeout));
    let connections = TaskTracker::new();
    let slots = Arc::new(Semaphore::new(limits.max));
    let mut full = false;
    loop {
        let slot = match slots.clone().try_acquire_owned() {
            Ok(slot) => {
                full = false;
                slot
            }
            Err(_) => {
                if !full {
                    tracing::warn!(
                        max = limits.max,
                        "the gateway serves its most connections at once; new ones wait"
                    );
                    full = true;
                }
                tokio::select! {
                    slot = slots.clone().acquire_owned() => match slot {
                        Ok(slot) => slot,
                        Err(_) => break,
                    },
                    () = shutdown.cancelled() => break,
                }
            }
        };
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted,
            () = shutdown.cancelled() => break,
        };
        let (stream, peer) = match accepted {
            Ok(connection) => connection,
            Err(e) if is_connection_error(&e) => continue,
            Err(e) => {
                tracing::warn!(error = %e, "cannot accept a connection; retrying");
                tokio::time::sleep(ACCEPT_RETRY_WAIT).await;
                continue;
            }
        };
        if let Err(e) = stream.set_nodelay(true) {
            tracing::debug!(error = %e, "cannot turn off Nagle's algorithm on a connection");
        }
        let served = serve_connection(
            builder.clone(),
            stream,
            peer,
            router.clone(),
            shutdown.clone(),
            header_read_timeout,
            kill.clone(),
        );
        connections.spawn(async move {
            served.await;
            drop(slot);
        });
    }
    drop(listener);
    connections.close();
    connections.wait().await;
    Ok(())
}

fn is_connection_error(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::{ConnectionAborted, ConnectionRefused, ConnectionReset};
    matches!(
        e.kind(),
        ConnectionRefused | ConnectionAborted | ConnectionReset
    )
}

async fn serve_connection(
    builder: Arc<Builder<TokioExecutor>>,
    stream: tokio::net::TcpStream,
    peer: SocketAddr,
    router: Router,
    shutdown: CancellationToken,
    first_request_within: Duration,
    kill: CancellationToken,
) {
    let first_request = CancellationToken::new();
    let arrived = first_request.clone();
    let service = service_fn(move |request: axum::http::Request<Incoming>| {
        arrived.cancel();
        let router = router.clone();
        async move {
            let mut request = request.map(Body::new);
            request.extensions_mut().insert(ConnectInfo(peer));
            router.oneshot(request).await
        }
    });
    let connection = builder.serve_connection(TokioIo::new(stream), service);
    let mut connection = std::pin::pin!(connection);
    let deadline = tokio::time::sleep(first_request_within);
    let mut deadline = std::pin::pin!(deadline);
    let mut arrived = false;
    let mut stopping = false;
    loop {
        tokio::select! {
            result = connection.as_mut() => {
                if let Err(e) = result {
                    tracing::debug!(error = %e, peer = %peer, "a connection ended with an error");
                }
                return;
            }
            () = first_request.cancelled(), if !arrived => arrived = true,
            () = deadline.as_mut(), if !arrived => {
                tracing::debug!(peer = %peer, "no request arrived in time; closing the connection");
                return;
            }
            () = shutdown.cancelled(), if !stopping => {
                stopping = true;
                connection.as_mut().graceful_shutdown();
            }
            () = kill.cancelled() => {
                tracing::debug!(peer = %peer, "closing a connection after the shutdown grace period");
                return;
            }
        }
    }
}
