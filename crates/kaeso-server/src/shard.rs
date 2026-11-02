use std::sync::atomic::{AtomicUsize, Ordering};

// Each thread keeps one index, so a worker always lands on the same shard and two workers
// rarely share one.
pub fn of_this_thread(shards: usize) -> usize {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    thread_local! {
        static INDEX: usize = NEXT.fetch_add(1, Ordering::Relaxed);
    }
    INDEX.with(|i| i % shards.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_keeps_its_shard_and_threads_spread_over_them() {
        let mine = of_this_thread(16);
        assert_eq!(of_this_thread(16), mine);
        assert!(mine < 16);
        let others: std::collections::HashSet<usize> = (0..8)
            .map(|_| std::thread::spawn(|| of_this_thread(16)).join().unwrap())
            .collect();
        assert!(others.len() > 1);
        assert_eq!(of_this_thread(0), 0);
    }
}
