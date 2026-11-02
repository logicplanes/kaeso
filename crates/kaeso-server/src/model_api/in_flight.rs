use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tokio_util::task::task_tracker::TaskTrackerToken;

// Every poll of a cancellation token locks it, so requests wait on children of a few shards
// rather than all on one token.
const CUT_SHARDS: usize = 16;

#[derive(Debug)]
pub struct InFlight {
    tracker: TaskTracker,
    cut: CancellationToken,
    shards: Box<[CancellationToken]>,
    finishing: Arc<Finishing>,
}

impl Default for InFlight {
    fn default() -> Self {
        let cut = CancellationToken::new();
        InFlight {
            tracker: TaskTracker::new(),
            shards: (0..CUT_SHARDS).map(|_| cut.child_token()).collect(),
            cut,
            finishing: Arc::default(),
        }
    }
}

#[derive(Debug, Default)]
struct Finishing {
    open: AtomicUsize,
    watchers: AtomicUsize,
    settled: Notify,
}

struct Open(Arc<Finishing>);

impl Drop for Open {
    fn drop(&mut self) {
        if self.0.open.fetch_sub(1, Ordering::SeqCst) == 1
            && self.0.watchers.load(Ordering::SeqCst) > 0
        {
            self.0.settled.notify_waiters();
        }
    }
}

struct Watching<'a>(&'a AtomicUsize);

impl Drop for Watching<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl InFlight {
    pub fn token(&self) -> TaskTrackerToken {
        self.tracker.token()
    }

    pub fn spawn<F>(&self, work: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tracker.spawn(work);
    }

    pub fn finish<F>(&self, work: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.finishing.open.fetch_add(1, Ordering::SeqCst);
        let open = Open(self.finishing.clone());
        self.tracker.spawn(async move {
            work.await;
            drop(open);
        });
    }

    pub async fn settled(&self) {
        let f = &*self.finishing;
        f.watchers.fetch_add(1, Ordering::SeqCst);
        let _watching = Watching(&f.watchers);
        loop {
            let notified = f.settled.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if f.open.load(Ordering::SeqCst) == 0 {
                return;
            }
            notified.await;
        }
    }

    pub fn cutter(&self) -> CancellationToken {
        self.cut.clone()
    }

    pub fn cut(&self) -> CancellationToken {
        self.shards
            .get(crate::shard::of_this_thread(CUT_SHARDS))
            .unwrap_or(&self.cut)
            .child_token()
    }

    pub async fn end(&self, wait: Duration) -> usize {
        self.cut.cancel();
        self.tracker.close();
        let _ = tokio::time::timeout(wait, self.tracker.wait()).await;
        self.tracker.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn every_request_token_is_cut_at_shutdown() {
        let in_flight = InFlight::default();
        let tokens: Vec<_> = (0..CUT_SHARDS * 2).map(|_| in_flight.cut()).collect();
        assert!(tokens.iter().all(|t| !t.is_cancelled()));
        in_flight.end(Duration::ZERO).await;
        assert!(tokens.iter().all(CancellationToken::is_cancelled));
        assert!(in_flight.cut().is_cancelled());
    }

    #[tokio::test]
    async fn settled_waits_for_every_finishing_task() {
        let in_flight = InFlight::default();
        in_flight.settled().await;
        let (release, released) = tokio::sync::oneshot::channel::<()>();
        in_flight.finish(async move {
            let _ = released.await;
        });
        let mut settled = std::pin::pin!(in_flight.settled());
        assert!(futures::poll!(settled.as_mut()).is_pending());
        release.send(()).unwrap();
        settled.await;
        in_flight.settled().await;
    }
}
