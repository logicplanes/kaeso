use std::sync::LazyLock;

use secrecy::SecretString;
use tokio::sync::Semaphore;
use tokio::task::JoinError;

// Argon2id holds 19 MiB for each hash it runs. Sign-in attempts are throttled per address but
// checked as they arrive, so a burst from a few addresses could otherwise run the gateway out
// of memory; past this bound, hashes wait their turn.
static HASHING: LazyLock<Bounded> = LazyLock::new(|| {
    Bounded::new(
        std::thread::available_parallelism()
            .map_or(2, |n| n.get().div_ceil(2))
            .max(2),
    )
});

struct Bounded(Semaphore);

impl Bounded {
    fn new(at_once: usize) -> Bounded {
        Bounded(Semaphore::new(at_once))
    }

    async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, JoinError> {
        let _turn = self.0.acquire().await;
        tokio::task::spawn_blocking(work).await
    }
}

pub async fn verify(password: SecretString, stored: String) -> Result<bool, JoinError> {
    HASHING
        .run(move || kaeso_crypto::password::verify(&password, &stored))
        .await
}

pub async fn hash(
    password: SecretString,
) -> Result<Result<String, kaeso_crypto::CryptoError>, JoinError> {
    HASHING
        .run(move || kaeso_crypto::password::hash(&password))
        .await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn no_more_hashes_run_at_once_than_the_bound() {
        let bounded = Arc::new(Bounded::new(2));
        let (running, most) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let tasks: Vec<_> = (0..12)
            .map(|_| {
                let (bounded, running, most) = (bounded.clone(), running.clone(), most.clone());
                tokio::spawn(async move {
                    bounded
                        .run(move || {
                            let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                            most.fetch_max(now, Ordering::SeqCst);
                            std::thread::sleep(std::time::Duration::from_millis(20));
                            running.fetch_sub(1, Ordering::SeqCst);
                        })
                        .await
                })
            })
            .collect();
        for task in tasks {
            task.await.unwrap().unwrap();
        }
        assert_eq!(most.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_password_hashed_here_verifies() {
        let password = SecretString::from("correct horse battery staple");
        let stored = hash(password.clone()).await.unwrap().unwrap();
        assert!(verify(password, stored.clone()).await.unwrap());
        assert!(!verify(SecretString::from("wrong"), stored).await.unwrap());
    }
}
