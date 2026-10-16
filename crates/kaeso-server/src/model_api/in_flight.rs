use std::future::Future;
use std::time::Duration;

use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};
use tokio_util::task::TaskTracker;
use tokio_util::task::task_tracker::TaskTrackerToken;

#[derive(Debug, Default)]
pub struct InFlight {
    tracker: TaskTracker,
    cut: CancellationToken,
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
