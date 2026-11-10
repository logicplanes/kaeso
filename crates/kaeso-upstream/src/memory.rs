use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const KIB: u64 = 1024;

// Each byte of an answer read whole is charged `per_byte` bytes, the answer and what decoding it
// may take, until the answer is dropped, so many large answers at once can't run out of memory.
#[derive(Clone, Debug)]
pub struct AnswerMemory {
    permits: Arc<Semaphore>,
    kib: u32,
    per_byte: u64,
    wait: Duration,
}

impl AnswerMemory {
    pub fn new(bytes: u64, per_byte: u64, wait: Duration) -> AnswerMemory {
        let kib = u32::try_from((bytes / KIB).max(1)).unwrap_or(u32::MAX);
        AnswerMemory {
            permits: Arc::new(Semaphore::new(kib as usize)),
            kib,
            per_byte: per_byte.max(1),
            wait,
        }
    }

    pub fn free(&self) -> u64 {
        u64::try_from(self.permits.available_permits()).unwrap_or(u64::MAX) * KIB
    }

    pub fn charge(&self) -> Charge {
        Charge {
            memory: self.clone(),
            permit: None,
        }
    }
}

#[derive(Debug)]
pub struct Charge {
    memory: AnswerMemory,
    permit: Option<OwnedSemaphorePermit>,
}

#[derive(Debug)]
pub struct Full;

impl Charge {
    // An answer read whole is capped in size, so one larger than the whole budget takes all of
    // it and is read alone instead of never.
    pub(crate) async fn cover(&mut self, bytes: usize) -> Result<(), Full> {
        let wanted = self.kib_for(bytes).min(self.memory.kib);
        self.take(wanted).await
    }

    // A stream has no size of its own, so it ends once what it holds would pass the budget.
    pub async fn cover_streamed(&mut self, bytes: usize) -> Result<(), Full> {
        let wanted = self.kib_for(bytes);
        if wanted > self.memory.kib {
            return Err(Full);
        }
        self.take(wanted).await
    }

    fn kib_for(&self, bytes: usize) -> u32 {
        let kib = u64::try_from(bytes)
            .unwrap_or(u64::MAX)
            .saturating_mul(self.memory.per_byte)
            .div_ceil(KIB);
        u32::try_from(kib).unwrap_or(u32::MAX)
    }

    async fn take(&mut self, wanted: u32) -> Result<(), Full> {
        let m = &self.memory;
        let held = self
            .permit
            .as_ref()
            .map_or(0, |p| u32::try_from(p.num_permits()).unwrap_or(u32::MAX));
        if wanted <= held {
            return Ok(());
        }
        let more =
            tokio::time::timeout(m.wait, m.permits.clone().acquire_many_owned(wanted - held))
                .await
                .map_err(|_| Full)?
                .map_err(|_| Full)?;
        match &mut self.permit {
            Some(p) => p.merge(more),
            None => self.permit = Some(more),
        }
        Ok(())
    }

    pub(crate) fn hold(self, body: Vec<u8>) -> Bytes {
        Bytes::from_owner(Held {
            body,
            _charge: self,
        })
    }
}

struct Held {
    body: Vec<u8>,
    _charge: Charge,
}

impl AsRef<[u8]> for Held {
    fn as_ref(&self) -> &[u8] {
        &self.body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_answer_holds_its_charge_until_it_is_dropped() {
        let memory = AnswerMemory::new(64 * KIB, 4, Duration::ZERO);
        let mut charge = memory.charge();
        charge.cover(4 * 1024).await.unwrap();
        assert_eq!(memory.free(), 48 * KIB);
        charge.cover(2 * 1024).await.unwrap();
        assert_eq!(memory.free(), 48 * KIB, "a smaller size takes nothing more");
        let body = charge.hold(vec![7; 4096]);
        let copy = body.slice(1..10);
        drop(body);
        assert_eq!(memory.free(), 48 * KIB, "a slice keeps the answer alive");
        drop(copy);
        assert_eq!(memory.free(), 64 * KIB);
    }

    #[tokio::test]
    async fn an_answer_without_room_gives_up_and_one_larger_than_the_budget_takes_it_all() {
        let memory = AnswerMemory::new(64 * KIB, 4, Duration::ZERO);
        let mut first = memory.charge();
        first.cover(usize::MAX).await.unwrap();
        assert_eq!(memory.free(), 0);
        let mut second = memory.charge();
        assert!(second.cover(1).await.is_err());
        drop(first);
        assert!(second.cover(1).await.is_ok());
    }

    #[tokio::test]
    async fn a_stream_ends_once_it_would_hold_more_than_the_whole_budget() {
        let memory = AnswerMemory::new(64 * KIB, 4, Duration::ZERO);
        let mut stream = memory.charge();
        stream.cover_streamed(16 * 1024).await.unwrap();
        assert_eq!(memory.free(), 0);
        assert!(stream.cover_streamed(16 * 1024 + 1).await.is_err());
        drop(stream);
        assert_eq!(memory.free(), 64 * KIB);
    }
}
