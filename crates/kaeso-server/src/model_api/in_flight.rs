use std::future::Future;
use std::time::Duration;

use tokio::sync::watch;
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};
use tokio_util::task::TaskTracker;
use tokio_util::task::task_tracker::TaskTrackerToken;

#[derive(Debug)]
pub struct InFlight {
    tracker: TaskTracker,
    cut: CancellationToken,
    finishing: watch::Sender<usize>,
}

impl Default for InFlight {
    fn default() -> Self {
        InFlight {
            tracker: TaskTracker::new(),
            cut: CancellationToken::new(),
            finishing: watch::Sender::new(0),
        }
    }
}

struct Finishing(watch::Sender<usize>);

impl Drop for Finishing {
    fn drop(&mut self) {
        self.0.send_modify(|n| *n = n.saturating_sub(1));
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
        self.finishing.send_modify(|n| *n += 1);
        let pending = Finishing(self.finishing.clone());
        self.tracker.spawn(async move {
            work.await;
            drop(pending);
        });
    }

    pub async fn settled(&self) {
        let mut finishing = self.finishing.subscribe();
        let _ = finishing.wait_for(|n| *n == 0).await;
    }

    pub fn cutter(&self) -> CancellationToken {
        self.cut.clone()
    }

    pub fn cut(&self) -> WaitForCancellationFuture<'_> {
        self.cut.cancelled()
    }

    pub async fn end(&self, wait: Duration) -> usize {
        self.cut.cancel();
        self.tracker.close();
        let _ = tokio::time::timeout(wait, self.tracker.wait()).await;
        self.tracker.len()
    }
}
