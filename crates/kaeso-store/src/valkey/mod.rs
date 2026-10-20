use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use futures::StreamExt;
use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use secrecy::{ExposeSecret, SecretString};
use tokio::sync::mpsc;

use crate::StoreError;

const INVALIDATE: &str = "invalidate";
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(1);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_RECONNECT_DELAY: Duration = Duration::from_millis(200);
const SUBSCRIBE_RETRY_WAIT: Duration = Duration::from_secs(1);
const BULK_CONNECTIONS: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invalidation {
    Connected,
    Disconnected,
    Message(String),
}

#[derive(Clone)]
pub struct Valkey {
    client: redis::Client,
    manager: ConnectionManager,
    bulk: Arc<Bulk>,
    prefix: String,
}

struct Bulk {
    managers: Vec<ConnectionManager>,
    next: AtomicUsize,
}

impl Bulk {
    fn pick(&self) -> Option<&ConnectionManager> {
        self.managers.get(turn(&self.next, self.managers.len())?)
    }
}

fn turn(next: &AtomicUsize, len: usize) -> Option<usize> {
    next.fetch_add(1, Ordering::Relaxed).checked_rem(len)
}

impl std::fmt::Debug for Valkey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Valkey")
            .field("prefix", &self.prefix)
            .finish_non_exhaustive()
    }
}

impl Valkey {
    pub fn new(url: &SecretString, prefix: &str) -> Result<Valkey, StoreError> {
        let client = redis::Client::open(url.expose_secret())
            .map_err(|_| StoreError::Config("KAESO_VALKEY_URL is not a valid Valkey URL".into()))?;
        let config = ConnectionManagerConfig::new()
            .set_connection_timeout(Some(CONNECTION_TIMEOUT))
            .set_response_timeout(Some(RESPONSE_TIMEOUT))
            .set_number_of_retries(1)
            .set_max_delay(MAX_RECONNECT_DELAY);
        let manager = ConnectionManager::new_lazy_with_config(client.clone(), config.clone())?;
        let managers = (0..BULK_CONNECTIONS)
            .map(|_| ConnectionManager::new_lazy_with_config(client.clone(), config.clone()))
            .collect::<Result<_, _>>()?;
        Ok(Valkey {
            client,
            manager,
            bulk: Arc::new(Bulk {
                managers,
                next: AtomicUsize::new(0),
            }),
            prefix: prefix.to_owned(),
        })
    }

    pub fn conn(&self) -> ConnectionManager {
        self.manager.clone()
    }

    pub fn bulk(&self) -> ConnectionManager {
        self.bulk.pick().unwrap_or(&self.manager).clone()
    }

    pub fn key(&self, suffix: &str) -> String {
        format!("{}{suffix}", self.prefix)
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub async fn ping(&self) -> Result<(), StoreError> {
        let _: String = redis::cmd("PING").query_async(&mut self.conn()).await?;
        Ok(())
    }

    pub async fn publish_invalidation(&self, message: &str) -> Result<(), StoreError> {
        let _: i64 = redis::cmd("PUBLISH")
            .arg(self.key(INVALIDATE))
            .arg(message)
            .query_async(&mut self.conn())
            .await?;
        Ok(())
    }

    pub fn subscribe_invalidations(&self) -> mpsc::Receiver<Invalidation> {
        let (tx, rx) = mpsc::channel(1024);
        let client = self.client.clone();
        let channel = self.key(INVALIDATE);
        tokio::spawn(async move {
            loop {
                if tx.is_closed() {
                    return;
                }
                match client.get_async_pubsub().await {
                    Ok(mut pubsub) => {
                        if pubsub.subscribe(&channel).await.is_ok() {
                            if tx.send(Invalidation::Connected).await.is_err() {
                                return;
                            }
                            let mut messages = pubsub.on_message();
                            while let Some(msg) = messages.next().await {
                                let payload: String = msg.get_payload().unwrap_or_default();
                                if tx.send(Invalidation::Message(payload)).await.is_err() {
                                    return;
                                }
                            }
                            if tx.send(Invalidation::Disconnected).await.is_err() {
                                return;
                            }
                        }
                        tracing::warn!("Valkey invalidation subscription ended; reconnecting");
                    }
                    Err(e) => {
                        metrics::counter!("kaeso_valkey_failures_total", "operation" => "subscribe").increment(1);
                        tracing::warn!(error = %e, "cannot subscribe to Valkey invalidations; retrying");
                    }
                }
                tokio::time::sleep(SUBSCRIBE_RETRY_WAIT).await;
            }
        });
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulk_turns_go_round_every_connection_in_order() {
        let next = AtomicUsize::new(0);
        let turns: Vec<_> = (0..5).map(|_| turn(&next, 2)).collect();
        assert_eq!(turns, [Some(0), Some(1), Some(0), Some(1), Some(0)]);
    }

    #[test]
    fn bulk_turns_keep_going_after_the_counter_wraps() {
        let next = AtomicUsize::new(usize::MAX);
        assert_eq!(turn(&next, 2), Some(1));
        assert_eq!(turn(&next, 2), Some(0));
        assert_eq!(turn(&next, 2), Some(1));
    }

    #[test]
    fn no_bulk_connections_means_no_turn() {
        assert_eq!(turn(&AtomicUsize::new(0), 0), None);
    }

    #[tokio::test]
    async fn bulk_connections_are_lazy_while_valkey_is_unreachable() {
        let valkey = Valkey::new(&SecretString::from("redis://127.0.0.1:1"), "kaeso:").unwrap();
        assert_eq!(valkey.bulk.managers.len(), BULK_CONNECTIONS);
        let got: Result<String, _> = redis::cmd("PING").query_async(&mut valkey.bulk()).await;
        assert!(got.is_err());
        assert!(valkey.ping().await.is_err());
    }
}
