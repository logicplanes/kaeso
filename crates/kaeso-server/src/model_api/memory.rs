use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use kaeso_codecs::limits::RequestShape;
use kaeso_kpp::RequestKind;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

const KIB: u64 = 1024;

// Allocation peaks of decoding, cloning and re-encoding crafted 8 MiB bodies: 5 bytes per
// body byte (19 for token ids), 3.8 KiB per input item, 1.8 KiB per block, 1 KiB per tool.
const PER_BODY_BYTE: u64 = 5;
const PER_EMBEDDING_BODY_BYTE: u64 = 20;
const PER_ITEM: u64 = 4 * KIB;
const PER_BLOCK: u64 = 2 * KIB;
const PER_TOOL: u64 = KIB;
// A decoded embedding input holds each token id as a u32.
const PER_TOKEN: u64 = 4;

pub const WAIT: Duration = Duration::from_secs(10);

const SHARES_PER_BUDGET: u32 = 4;

fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

fn structure(shape: &RequestShape) -> u64 {
    count(shape.items)
        .saturating_mul(PER_ITEM)
        .saturating_add(count(shape.blocks).saturating_mul(PER_BLOCK))
        .saturating_add(count(shape.tools).saturating_mul(PER_TOOL))
}

pub fn estimate(kind: RequestKind, shape: &RequestShape) -> u64 {
    let per_byte = if kind == RequestKind::Embed {
        PER_EMBEDDING_BODY_BYTE
    } else {
        PER_BODY_BYTE
    };
    count(shape.bytes)
        .saturating_mul(per_byte)
        .saturating_add(structure(shape))
}

pub fn decoded(shape: &RequestShape) -> u64 {
    count(shape.bytes)
        .saturating_add(count(shape.tokens).saturating_mul(PER_TOKEN))
        .saturating_add(structure(shape))
}

type Holders = Arc<Mutex<HashMap<Uuid, Arc<Semaphore>>>>;

#[derive(Debug)]
pub struct Budget {
    permits: Arc<Semaphore>,
    kib: u32,
    share_kib: u32,
    wait: Duration,
    waiting: AtomicUsize,
    holders: Holders,
}

#[derive(Debug)]
pub struct Busy;

#[derive(Debug)]
pub struct Held {
    global: Option<OwnedSemaphorePermit>,
    share: Option<OwnedSemaphorePermit>,
    holder: Uuid,
    holders: Holders,
}

fn kib_of(bytes: u64) -> u32 {
    u32::try_from(bytes.div_ceil(KIB)).unwrap_or(u32::MAX)
}

impl Budget {
    pub fn new(bytes: u64) -> Budget {
        Budget::with_wait(bytes, WAIT)
    }

    pub fn with_wait(bytes: u64, wait: Duration) -> Budget {
        let kib = u32::try_from((bytes / KIB).max(1)).unwrap_or(u32::MAX);
        Budget {
            permits: Arc::new(Semaphore::new(kib as usize)),
            kib,
            share_kib: (kib / SHARES_PER_BUDGET).max(1),
            wait,
            waiting: AtomicUsize::new(0),
            holders: Holders::default(),
        }
    }

    pub fn waiting(&self) -> usize {
        self.waiting.load(Ordering::SeqCst)
    }

    pub fn free(&self) -> u64 {
        u64::try_from(self.permits.available_permits()).unwrap_or(u64::MAX) * KIB
    }

    pub fn share(&self) -> u64 {
        u64::from(self.share_kib) * KIB
    }

    pub async fn hold(&self, holder: Uuid, bytes: u64) -> Result<Held, Busy> {
        let kib = kib_of(bytes).clamp(1, self.share_kib.min(self.kib));
        let share = self
            .holders
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(holder)
            .or_insert_with(|| Arc::new(Semaphore::new(self.share_kib as usize)))
            .clone();
        self.waiting.fetch_add(1, Ordering::SeqCst);
        let _waiting = Waiting(&self.waiting);
        let mut held = Held {
            global: None,
            share: None,
            holder,
            holders: self.holders.clone(),
        };
        let acquired = tokio::time::timeout(self.wait, async {
            held.share = share.acquire_many_owned(kib).await.ok();
            held.global = self.permits.clone().acquire_many_owned(kib).await.ok();
        })
        .await;
        acquired.map(|()| held).map_err(|_| Busy)
    }
}

impl Held {
    pub fn keep(&mut self, bytes: u64) {
        let kib = kib_of(bytes) as usize;
        for slot in [&mut self.global, &mut self.share] {
            match slot {
                Some(_) if kib == 0 => *slot = None,
                Some(permit) => {
                    let extra = permit.num_permits().saturating_sub(kib);
                    drop(permit.split(extra));
                }
                None => {}
            }
        }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        self.share = None;
        let mut holders = self.holders.lock().unwrap_or_else(PoisonError::into_inner);
        if holders
            .get(&self.holder)
            .is_some_and(|s| Arc::strong_count(s) == 1)
        {
            holders.remove(&self.holder);
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

    fn shape(bytes: usize, items: usize, blocks: usize, tools: usize) -> RequestShape {
        let mut s = RequestShape::default();
        s.bytes = bytes;
        s.items = items;
        s.blocks = blocks;
        s.tools = tools;
        s
    }

    #[test]
    fn the_estimate_weighs_content_bytes_and_every_counted_structure() {
        assert_eq!(
            estimate(RequestKind::Generate, &shape(1000, 2, 3, 1)),
            5000 + 2 * 4096 + 3 * 2048 + 1024
        );
        assert_eq!(estimate(RequestKind::Embed, &shape(1000, 0, 0, 0)), 20_000);
        assert_eq!(
            estimate(RequestKind::Generate, &shape(usize::MAX, usize::MAX, 0, 0)),
            u64::MAX
        );
    }

    #[test]
    fn a_decoded_request_keeps_its_content_and_every_counted_structure() {
        assert_eq!(
            decoded(&shape(1000, 2, 3, 1)),
            1000 + 2 * 4096 + 3 * 2048 + 1024
        );
        assert_eq!(decoded(&shape(usize::MAX, 1, 0, 0)), u64::MAX);
        let mut ids = shape(2000, 1, 0, 0);
        ids.tokens = 1000;
        assert_eq!(decoded(&ids), 2000 + 4 * 1000 + 4096);
    }

    #[test]
    fn whitespace_padding_adds_nothing_to_the_estimate() {
        let body = br#"{"model":"m","input":"hi"}"#;
        let mut padded = b"{".to_vec();
        padded.extend(std::iter::repeat_n(b' ', 1 << 20));
        padded.extend_from_slice(&body[1..]);
        let shape_of = |b: &[u8]| {
            kaeso_codecs::limits::request_shape(kaeso_kpp::DialectId::OpenAiResponses, b).unwrap()
        };
        assert_eq!(shape_of(body).bytes, body.len());
        assert_eq!(
            estimate(RequestKind::Generate, &shape_of(&padded)),
            estimate(RequestKind::Generate, &shape_of(body))
        );
    }

    #[tokio::test]
    async fn requests_wait_for_memory_and_one_larger_than_a_share_takes_the_share() {
        let budget = Budget::new(40 * KIB);
        let (a, b, c, d) = (
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        );
        assert_eq!(budget.share(), 10 * KIB);
        let first = budget.hold(a, 6 * KIB).await.unwrap();
        assert_eq!(budget.free(), 34 * KIB);
        let mut second = Box::pin(budget.hold(a, 5 * KIB));
        assert!(
            futures::poll!(second.as_mut()).is_pending(),
            "a's share is spent"
        );
        assert_eq!(budget.waiting(), 1);
        let other = budget.hold(b, 9 * KIB).await.unwrap();
        assert_eq!(budget.free(), 25 * KIB, "another key is not held up");
        drop(first);
        let second = second.await.unwrap();
        assert_eq!((budget.free(), budget.waiting()), (26 * KIB, 0));
        let huge = budget.hold(c, u64::MAX).await.unwrap();
        assert_eq!(budget.free(), 16 * KIB, "a huge request takes one share");
        let rest = budget.hold(d, u64::MAX).await.unwrap();
        assert_eq!(budget.free(), 6 * KIB);
        drop((second, other, huge, rest));
        let tiny = budget.hold(a, 0).await.unwrap();
        assert_eq!(budget.free(), 39 * KIB);
        drop(tiny);
        assert_eq!(budget.free(), 40 * KIB);
        assert!(
            budget.holders.lock().unwrap().is_empty(),
            "no key is left over"
        );
    }

    #[tokio::test]
    async fn a_hold_gives_up_after_its_wait() {
        let budget = Budget::with_wait(8 * KIB, Duration::ZERO);
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        let full = budget.hold(a, 2 * KIB).await.unwrap();
        assert!(
            budget.hold(a, KIB).await.is_err(),
            "a's share of 2 KiB is spent"
        );
        assert!(budget.hold(b, KIB).await.is_ok());
        assert_eq!(budget.waiting(), 0);
        drop(full);
        assert_eq!(budget.free(), 8 * KIB);
        assert!(budget.holders.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn keep_returns_everything_but_what_stays_allocated() {
        let budget = Budget::new(40 * KIB);
        let a = Uuid::now_v7();
        let mut held = budget.hold(a, 8 * KIB).await.unwrap();
        held.keep(3 * KIB);
        assert_eq!(budget.free(), 37 * KIB);
        assert!(
            budget.hold(a, 7 * KIB).await.is_ok(),
            "the share is back too"
        );
        held.keep(KIB * 5);
        assert_eq!(budget.free(), 37 * KIB, "keep never takes more");
        held.keep(0);
        assert_eq!(budget.free(), 40 * KIB);
        drop(held);
        assert_eq!(budget.free(), 40 * KIB);
    }
}
