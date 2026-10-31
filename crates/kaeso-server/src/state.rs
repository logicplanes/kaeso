use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use kaeso_core::clock::SharedClock;
use kaeso_crypto::KeyRing;
use kaeso_mail::Mailer;
use kaeso_store::clickhouse::ClickHouse;
use kaeso_store::valkey::Valkey;
use sqlx::PgPool;

use crate::cache::Caches;
use crate::config::Config;

#[derive(Clone, Debug)]
pub struct AppState(Arc<Inner>);

#[derive(Debug)]
pub struct Inner {
    pub config: Config,
    pub pg: PgPool,
    pub clickhouse: Option<ClickHouse>,
    pub valkey: Valkey,
    pub keyring: KeyRing,
    pub clock: SharedClock,
    pub mailer: Mailer,
    pub caches: Caches,
    pub response_cache: crate::services::response_cache::Shared,
    pub models_dev: crate::services::models_dev::ModelsDev,
    pub upstream: kaeso_upstream::Upstream,
    pub limits: Arc<dyn crate::model_api::hooks::Limits>,
    pub sink: Arc<dyn crate::model_api::hooks::Sink>,
    pub usage: crate::services::usage::Batch,
    pub distrusted: crate::services::limits::Distrusted,
    pub rebuilds: crate::services::limits::Rebuilds,
    pub logs: crate::services::logs::Buffer,
    pub in_flight: crate::model_api::in_flight::InFlight,
    pub translating: crate::model_api::memory::Budget,
    pub availability: crate::services::availability::Recall,
    pub dns: Arc<dyn crate::services::dns::TxtLookup>,
    pub accounts: crate::services::provider_accounts::Endpoints,
    clickhouse_migrated: AtomicBool,
}

#[derive(Debug)]
pub struct Parts {
    pub config: Config,
    pub pg: PgPool,
    pub clickhouse: Option<ClickHouse>,
    pub valkey: Valkey,
    pub keyring: KeyRing,
    pub clock: SharedClock,
    pub mailer: Mailer,
    pub limits: Option<Arc<dyn crate::model_api::hooks::Limits>>,
    pub sink: Option<Arc<dyn crate::model_api::hooks::Sink>>,
    pub dns: Option<Arc<dyn crate::services::dns::TxtLookup>>,
    pub accounts: Option<crate::services::provider_accounts::Endpoints>,
}

impl AppState {
    pub fn new(p: Parts) -> AppState {
        let caches = Caches::new(&p.valkey, &p.keyring);
        let response_cache = crate::services::response_cache::Shared::new(&p.keyring);
        let connect_timeout = p.config.upstream.connect_timeout;
        let reach = kaeso_upstream::guard::Reach::networks(
            p.config.custom_providers.allowed_networks.clone(),
        );
        let translating = crate::model_api::memory::Budget::new(p.config.body_limits.in_flight.0);
        caches.listen(&p.valkey);
        AppState(Arc::new(Inner {
            config: p.config,
            pg: p.pg,
            clickhouse: p.clickhouse,
            valkey: p.valkey,
            keyring: p.keyring,
            clock: p.clock,
            mailer: p.mailer,
            caches,
            response_cache,
            models_dev: crate::services::models_dev::ModelsDev::embedded(),
            upstream: kaeso_upstream::Upstream::new(
                connect_timeout,
                concat!("kaeso-gateway/", env!("CARGO_PKG_VERSION")),
            )
            .guarding(reach),
            limits: p
                .limits
                .unwrap_or_else(|| Arc::new(crate::services::limits::ValkeyLimits)),
            sink: p
                .sink
                .unwrap_or_else(|| Arc::new(crate::services::logs::Recorder)),
            usage: crate::services::usage::Batch::default(),
            distrusted: crate::services::limits::Distrusted::default(),
            rebuilds: crate::services::limits::Rebuilds::default(),
            logs: crate::services::logs::Buffer::default(),
            in_flight: crate::model_api::in_flight::InFlight::default(),
            translating,
            availability: crate::services::availability::Recall::default(),
            dns: p
                .dns
                .unwrap_or_else(|| Arc::new(crate::services::dns::SystemResolver::default())),
            accounts: p.accounts.unwrap_or_default(),
            clickhouse_migrated: AtomicBool::new(false),
        }))
    }

    pub fn request_logs(&self) -> bool {
        self.0.clickhouse.is_some()
    }

    pub fn clickhouse_migrated(&self) -> bool {
        self.0.clickhouse_migrated.load(Ordering::Acquire)
    }

    pub(crate) fn set_clickhouse_migrated(&self) {
        self.0.clickhouse_migrated.store(true, Ordering::Release);
    }
}

impl Deref for AppState {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}
