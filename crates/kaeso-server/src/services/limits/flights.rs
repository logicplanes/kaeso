use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use futures::future::{BoxFuture, FutureExt, Shared};

type Flight<E> = Shared<BoxFuture<'static, Result<(), Arc<E>>>>;

struct Entry<E> {
    flight: Flight<E>,
    waiters: usize,
}

pub struct Flights<E> {
    running: Mutex<HashMap<String, Entry<E>>>,
}

impl<E> Default for Flights<E> {
    fn default() -> Self {
        Flights {
            running: Mutex::new(HashMap::new()),
        }
    }
}

impl<E> fmt::Debug for Flights<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Flights")
            .field("running", &self.len())
            .finish()
    }
}

impl<E> Flights<E> {
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Entry<E>>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<E: Send + Sync + 'static> Flights<E> {
    pub async fn run<F>(&self, key: String, start: impl FnOnce() -> F) -> Result<(), Arc<E>>
    where
        F: Future<Output = Result<(), E>> + Send + 'static,
    {
        let flight = {
            let mut running = self.lock();
            let entry = running.entry(key.clone()).or_insert_with(|| Entry {
                flight: start().map(|r| r.map_err(Arc::new)).boxed().shared(),
                waiters: 0,
            });
            entry.waiters += 1;
            entry.flight.clone()
        };
        let _leave = Leave {
            flights: self,
            key: &key,
            flight: &flight,
        };
        flight.clone().await
    }
}

struct Leave<'a, E> {
    flights: &'a Flights<E>,
    key: &'a str,
    flight: &'a Flight<E>,
}

impl<E> Drop for Leave<'_, E> {
    fn drop(&mut self) {
        let mut running = self.flights.lock();
        let Some(entry) = running.get_mut(self.key) else {
            return;
        };
        if !entry.flight.ptr_eq(self.flight) {
            return;
        }
        entry.waiters = entry.waiters.saturating_sub(1);
        if entry.waiters == 0 || entry.flight.peek().is_some() {
            running.remove(self.key);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tokio::sync::oneshot;

    use super::*;

    #[derive(Debug, PartialEq)]
    struct Failed;

    fn counted(
        starts: &Arc<AtomicUsize>,
        gate: oneshot::Receiver<Result<(), Failed>>,
    ) -> impl FnOnce() -> BoxFuture<'static, Result<(), Failed>> {
        let starts = starts.clone();
        move || {
            starts.fetch_add(1, Ordering::SeqCst);
            async move { gate.await.unwrap_or(Err(Failed)) }.boxed()
        }
    }

    #[tokio::test]
    async fn concurrent_runs_of_one_key_share_a_single_start() {
        let flights = Flights::<Failed>::default();
        let starts = Arc::new(AtomicUsize::new(0));
        let (open, gate) = oneshot::channel();
        let (_unused, never) = oneshot::channel();
        let (a, b, ()) = tokio::join!(
            flights.run("k".into(), counted(&starts, gate)),
            flights.run("k".into(), counted(&starts, never)),
            async {
                open.send(Ok(())).unwrap();
            },
        );
        assert_eq!((a, b), (Ok(()), Ok(())));
        assert_eq!(starts.load(Ordering::SeqCst), 1);
        assert_eq!(flights.len(), 0);

        let (open, gate) = oneshot::channel();
        open.send(Ok(())).unwrap();
        flights
            .run("k".into(), counted(&starts, gate))
            .await
            .unwrap();
        assert_eq!(
            starts.load(Ordering::SeqCst),
            2,
            "a finished run is not reused"
        );
    }

    #[tokio::test]
    async fn every_waiter_gets_the_shared_error() {
        let flights = Flights::<Failed>::default();
        let starts = Arc::new(AtomicUsize::new(0));
        let (open, gate) = oneshot::channel();
        let (_unused, never) = oneshot::channel();
        let (a, b, ()) = tokio::join!(
            flights.run("k".into(), counted(&starts, gate)),
            flights.run("k".into(), counted(&starts, never)),
            async {
                open.send(Err(Failed)).unwrap();
            },
        );
        let (a, b) = (a.unwrap_err(), b.unwrap_err());
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(*a, Failed);
        assert_eq!(starts.load(Ordering::SeqCst), 1);
        assert_eq!(flights.len(), 0);
    }

    #[tokio::test]
    async fn different_keys_run_on_their_own() {
        let flights = Flights::<Failed>::default();
        let starts = Arc::new(AtomicUsize::new(0));
        let (open_a, gate_a) = oneshot::channel();
        let (open_b, gate_b) = oneshot::channel();
        let (a, b, ()) = tokio::join!(
            flights.run("a".into(), counted(&starts, gate_a)),
            flights.run("b".into(), counted(&starts, gate_b)),
            async {
                open_a.send(Ok(())).unwrap();
                open_b.send(Err(Failed)).unwrap();
            },
        );
        assert_eq!(a, Ok(()));
        assert!(b.is_err());
        assert_eq!(starts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn an_abandoned_run_leaves_nothing_behind() {
        let flights = Flights::<Failed>::default();
        let starts = Arc::new(AtomicUsize::new(0));
        let (_held, gate) = oneshot::channel();
        assert!(
            flights
                .run("k".into(), counted(&starts, gate))
                .now_or_never()
                .is_none()
        );
        assert_eq!(flights.len(), 0);

        let (open, gate) = oneshot::channel();
        open.send(Ok(())).unwrap();
        flights
            .run("k".into(), counted(&starts, gate))
            .await
            .unwrap();
        assert_eq!(starts.load(Ordering::SeqCst), 2);
    }
}
