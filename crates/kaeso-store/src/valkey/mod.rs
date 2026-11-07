use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, PoisonError, RwLock};
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
const HEAL_EVERY: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invalidation {
    Connected,
    Disconnected,
    Message(String),
}

#[derive(Clone)]
pub struct Valkey {
    client: redis::Client,
    config: ConnectionManagerConfig,
    manager: Arc<Slot>,
    bulk: Arc<Bulk>,
    prefix: String,
}

struct Slot(RwLock<ConnectionManager>);

impl Slot {
    fn get(&self) -> ConnectionManager {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set(&self, fresh: ConnectionManager) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = fresh;
    }
}

struct Bulk {
    managers: Vec<Slot>,
    next: AtomicUsize,
}

impl Bulk {
    fn pick(&self) -> Option<&Slot> {
        self.managers.get(turn(&self.next, self.managers.len())?)
    }
}

fn stuck(e: &redis::RedisError) -> bool {
    e.is_unrecoverable_error() && !e.is_io_error()
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
    pub fn new(url: &SecretString, prefix: &str, ca: Option<&str>) -> Result<Valkey, StoreError> {
        let client = match ca {
            Some(pem) if !pem.contains("-----BEGIN CERTIFICATE-----") => {
                return Err(StoreError::Config(
                    "KAESO_VALKEY_CA is not a certificate in PEM form".into(),
                ));
            }
            Some(pem) => redis::Client::build_with_tls(
                url.expose_secret(),
                redis::TlsCertificates {
                    client_tls: None,
                    root_cert: Some(pem.as_bytes().to_vec()),
                },
            )
            .map_err(|_| {
                StoreError::Config(
                    "KAESO_VALKEY_URL and KAESO_VALKEY_CA don't make a TLS connection".into(),
                )
            })?,
            None => redis::Client::open(url.expose_secret()).map_err(|_| {
                StoreError::Config("KAESO_VALKEY_URL is not a valid Valkey URL".into())
            })?,
        };
        let config = ConnectionManagerConfig::new()
            .set_connection_timeout(Some(CONNECTION_TIMEOUT))
            .set_response_timeout(Some(RESPONSE_TIMEOUT))
            .set_number_of_retries(1)
            .set_max_delay(MAX_RECONNECT_DELAY);
        let lazy = || {
            ConnectionManager::new_lazy_with_config(client.clone(), config.clone())
                .map(|m| Slot(RwLock::new(m)))
        };
        let manager = Arc::new(lazy()?);
        let managers = (0..BULK_CONNECTIONS)
            .map(|_| lazy())
            .collect::<Result<_, _>>()?;
        Ok(Valkey {
            client,
            config,
            manager,
            bulk: Arc::new(Bulk {
                managers,
                next: AtomicUsize::new(0),
            }),
            prefix: prefix.to_owned(),
        })
    }

    pub fn conn(&self) -> ConnectionManager {
        self.manager.get()
    }

    pub fn bulk(&self) -> ConnectionManager {
        self.bulk.pick().unwrap_or(&self.manager).get()
    }

    // redis-rs replaces a manager's connection only after an I/O error: a connection whose
    // sign-in failed, such as an AUTH that Valkey refused while its password was being changed,
    // stays failed for good (`reconnect_if_io_error` in redis/src/aio/connection_manager.rs,
    // https://github.com/redis-rs/redis-rs/blob/redis-1.8.0/redis/src/aio/connection_manager.rs).
    // Such a manager is swapped for a new one, which signs in again on its next command.
    pub async fn heal(&self) -> usize {
        let mut healed = 0;
        for slot in std::iter::once(&*self.manager).chain(&self.bulk.managers) {
            let got: Result<String, redis::RedisError> =
                redis::cmd("PING").query_async(&mut slot.get()).await;
            if let Err(e) = got
                && stuck(&e)
                && let Ok(fresh) = ConnectionManager::new_lazy_with_config(
                    self.client.clone(),
                    self.config.clone(),
                )
            {
                slot.set(fresh);
                healed += 1;
            }
        }
        healed
    }

    pub fn keep_healthy(&self) {
        let valkey = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(HEAL_EVERY).await;
                let healed = valkey.heal().await;
                if healed > 0 {
                    tracing::warn!(
                        healed,
                        "Valkey connections that failed to sign in were opened again"
                    );
                }
            }
        });
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
    #[tokio::test]
    async fn a_tls_url_connects_with_the_system_roots_or_the_authority_given() {
        use secrecy::SecretString;
        let url = SecretString::from("rediss://:secret@valkey.example:6379");
        assert!(super::Valkey::new(&url, "kaeso:", None).is_ok());
        use base64::Engine;
        let der = rcgen::generate_simple_self_signed(vec!["valkey.example".to_owned()])
            .unwrap()
            .cert
            .der()
            .to_vec();
        let ca = format!(
            "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
            base64::engine::general_purpose::STANDARD.encode(der)
        );
        assert!(super::Valkey::new(&url, "kaeso:", Some(&ca)).is_ok());
        assert!(matches!(
            super::Valkey::new(&url, "kaeso:", Some("not a certificate")),
            Err(crate::StoreError::Config(_))
        ));
    }

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
        let valkey =
            Valkey::new(&SecretString::from("redis://127.0.0.1:1"), "kaeso:", None).unwrap();
        assert_eq!(valkey.bulk.managers.len(), BULK_CONNECTIONS);
        assert_eq!(
            valkey.heal().await,
            0,
            "an unreachable Valkey is an I/O error redis-rs retries"
        );
        let got: Result<String, _> = redis::cmd("PING").query_async(&mut valkey.bulk()).await;
        assert!(got.is_err());
        assert!(valkey.ping().await.is_err());
    }

    // A stand-in for Valkey that refuses every sign-in until `accept` is set, as a Valkey does
    // while it runs with another password.
    async fn fussy_valkey() -> (String, Arc<std::sync::atomic::AtomicBool>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("redis://:secret@{}", listener.local_addr().unwrap());
        let accept = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let open = accept.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let open = open.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0_u8; 4096];
                    loop {
                        let Ok(n) = socket.read(&mut buf).await else {
                            return;
                        };
                        if n == 0 {
                            return;
                        }
                        let text = String::from_utf8_lossy(&buf[..n]).to_uppercase();
                        let mut out = String::new();
                        for command in text.split('*').skip(1) {
                            if command.contains("AUTH") && !open.load(Ordering::SeqCst) {
                                out.push_str("-WRONGPASS invalid username-password pair\r\n");
                            } else if command.contains("PING") {
                                out.push_str("+PONG\r\n");
                            } else {
                                out.push_str("+OK\r\n");
                            }
                        }
                        if socket.write_all(out.as_bytes()).await.is_err() {
                            return;
                        }
                    }
                });
            }
        });
        (url, accept)
    }

    #[tokio::test]
    async fn a_connection_whose_sign_in_failed_is_opened_again() {
        let (url, accept) = fussy_valkey().await;
        let valkey = Valkey::new(&SecretString::from(url), "kaeso:", None).unwrap();
        let first: Result<String, _> = redis::cmd("PING").query_async(&mut valkey.conn()).await;
        assert_eq!(
            first.unwrap_err().kind(),
            redis::ErrorKind::AuthenticationFailed
        );
        accept.store(true, Ordering::SeqCst);
        let again: Result<String, _> = redis::cmd("PING").query_async(&mut valkey.conn()).await;
        assert!(again.is_err(), "redis-rs keeps the failed connection");
        assert!(valkey.heal().await >= 1);
        let healed: String = redis::cmd("PING")
            .query_async(&mut valkey.conn())
            .await
            .unwrap();
        assert_eq!(healed, "PONG");
        assert_eq!(valkey.heal().await, 0, "a healthy connection stays");
    }
}
