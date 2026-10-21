use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use kaeso_codecs::limits::RequestShape;
use kaeso_kpp::RequestKind;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const KIB: u64 = 1024;

// Allocation peaks of decoding, cloning and re-encoding crafted 8 MiB bodies: 5 bytes per
// body byte (19 for token ids), 3.8 KiB per input item, 1.8 KiB per block, 1 KiB per tool.
const PER_BODY_BYTE: u64 = 5;
const PER_EMBEDDING_BODY_BYTE: u64 = 20;
const PER_ITEM: u64 = 4 * KIB;
const PER_BLOCK: u64 = 2 * KIB;
const PER_TOOL: u64 = KIB;

pub fn estimate(kind: RequestKind, body: usize, shape: &RequestShape) -> u64 {
    let per_byte = if kind == RequestKind::Embed {
        PER_EMBEDDING_BODY_BYTE
    } else {
        PER_BODY_BYTE
    };
    let count = |n: usize| u64::try_from(n).unwrap_or(u64::MAX);
    count(body)
        .saturating_mul(per_byte)
        .saturating_add(count(shape.items).saturating_mul(PER_ITEM))
        .saturating_add(count(shape.blocks).saturating_mul(PER_BLOCK))
        .saturating_add(count(shape.tools).saturating_mul(PER_TOOL))
}

#[derive(Debug)]
pub struct Budget {
    permits: Arc<Semaphore>,
    kib: u32,
    waiting: AtomicUsize,
}

#[derive(Debug)]
pub struct Held {
    _permit: Option<OwnedSemaphorePermit>,
}

impl Budget {
    pub fn new(bytes: u64) -> Budget {
        let kib = u32::try_from((bytes / KIB).max(1)).unwrap_or(u32::MAX);
        Budget {
            permits: Arc::new(Semaphore::new(kib as usize)),
            kib,
            waiting: AtomicUsize::new(0),
        }
    }

    pub fn waiting(&self) -> usize {
        self.waiting.load(Ordering::SeqCst)
    }

    pub fn free(&self) -> u64 {
        u64::try_from(self.permits.available_permits()).unwrap_or(u64::MAX) * KIB
    }

    pub async fn hold(&self, bytes: u64) -> Held {
        let kib = u32::try_from(bytes.div_ceil(KIB))
            .unwrap_or(u32::MAX)
            .clamp(1, self.kib);
        self.waiting.fetch_add(1, Ordering::SeqCst);
        let _waiting = Waiting(&self.waiting);
        Held {
            _permit: self.permits.clone().acquire_many_owned(kib).await.ok(),
        }
    }
}

struct Waiting<'a>(&'a AtomicUsize);

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(items: usize, blocks: usize, tools: usize) -> RequestShape {
        let mut s = RequestShape::default();
        s.items = items;
        s.blocks = blocks;
        s.tools = tools;
        s
    }

    #[test]
    fn the_estimate_weighs_body_bytes_and_every_counted_structure() {
        assert_eq!(
            estimate(RequestKind::Generate, 1000, &shape(2, 3, 1)),
            5000 + 2 * 4096 + 3 * 2048 + 1024
        );
        assert_eq!(
            estimate(RequestKind::Embed, 1000, &RequestShape::default()),
            20_000
        );
        assert_eq!(
            estimate(RequestKind::Generate, usize::MAX, &shape(usize::MAX, 0, 0)),
            u64::MAX
        );
    }

    #[tokio::test]
    async fn requests_wait_for_memory_and_one_larger_than_the_budget_runs_alone() {
        let budget = Budget::new(10 * KIB);
        let first = budget.hold(6 * KIB).await;
        assert_eq!(budget.free(), 4 * KIB);
        let mut second = Box::pin(budget.hold(5 * KIB));
        assert!(futures::poll!(second.as_mut()).is_pending());
        assert_eq!(budget.waiting(), 1);
        let mut gave_up = Box::pin(budget.hold(KIB * 9));
        assert!(futures::poll!(gave_up.as_mut()).is_pending());
        drop(gave_up);
        assert_eq!(budget.waiting(), 1);
        drop(first);
        let second = second.await;
        assert_eq!((budget.free(), budget.waiting()), (5 * KIB, 0));
        drop(second);
        let huge = budget.hold(u64::MAX).await;
        assert_eq!(budget.free(), 0);
        drop(huge);
        let tiny = budget.hold(0).await;
        assert_eq!(budget.free(), 9 * KIB);
        drop(tiny);
        assert_eq!(budget.free(), 10 * KIB);
    }
}
