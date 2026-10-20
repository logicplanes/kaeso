use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use hickory_resolver::TokioResolver;
use hickory_resolver::proto::rr::RData;

pub const RECORD_PREFIX: &str = "_kaeso-verify";
pub const VALUE_PREFIX: &str = "kaeso-verify=";

#[derive(Debug, thiserror::Error)]
#[error("DNS lookup failed: {0}")]
pub struct LookupFailed(pub String);

pub trait TxtLookup: Send + Sync + std::fmt::Debug {
    fn txt<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Result<Vec<String>, LookupFailed>>;
}

pub fn record_name(ascii_domain: &str) -> String {
    format!("{RECORD_PREFIX}.{ascii_domain}")
}

pub fn record_value(token: &str) -> String {
    format!("{VALUE_PREFIX}{token}")
}

#[derive(Debug, Default)]
pub struct SystemResolver {
    resolver: tokio::sync::OnceCell<Option<TokioResolver>>,
}

impl SystemResolver {
    async fn resolver(&self) -> Option<&TokioResolver> {
        self.resolver
            .get_or_init(|| async {
                TokioResolver::builder_tokio()
                    .and_then(|b| b.build())
                    .map_err(|e| tracing::warn!(error = %e, "no DNS resolver could be set up"))
                    .ok()
            })
            .await
            .as_ref()
    }
}

impl TxtLookup for SystemResolver {
    fn txt<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Result<Vec<String>, LookupFailed>> {
        Box::pin(async move {
            let resolver = self
                .resolver()
                .await
                .ok_or_else(|| LookupFailed("no resolver".into()))?;
            let fqdn = format!("{}.", name.trim_end_matches('.'));
            match resolver.txt_lookup(fqdn).await {
                Ok(found) => Ok(found
                    .answers()
                    .iter()
                    .filter_map(|r| match &r.data {
                        RData::TXT(txt) => Some(
                            txt.txt_data
                                .iter()
                                .map(|part| String::from_utf8_lossy(part).into_owned())
                                .collect::<String>(),
                        ),
                        _ => None,
                    })
                    .collect()),
                Err(e) if e.is_no_records_found() || e.is_nx_domain() => Ok(Vec::new()),
                Err(e) => Err(LookupFailed(e.to_string())),
            }
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct StaticRecords(Arc<Mutex<HashMap<String, Vec<String>>>>);

impl StaticRecords {
    pub fn set(&self, name: &str, values: &[&str]) {
        if let Ok(mut records) = self.0.lock() {
            records.insert(
                name.to_owned(),
                values.iter().map(|v| (*v).to_owned()).collect(),
            );
        }
    }
}

impl TxtLookup for StaticRecords {
    fn txt<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Result<Vec<String>, LookupFailed>> {
        let found = self
            .0
            .lock()
            .map(|r| r.get(name).cloned().unwrap_or_default())
            .map_err(|_| LookupFailed("records poisoned".into()));
        Box::pin(async move { found })
    }
}
