//! Drawing a key from a project's pool of keys for one provider.

use kaeso_kpp::{ErrorKind, KppError};
use kaeso_store::pg::credentials::PoolMember;
use sha2::{Digest, Sha256};
use time::Duration;
use uuid::Uuid;

use crate::model_api::pipeline::Stage;

// A refused, signed-out or empty key waits as long as a refused provider does in a route; a
// rate-limited one as long as the provider asks, within the same bounds.
const ACCOUNT: Duration = Duration::minutes(5);
const LONGEST_RETRY_AFTER: Duration = Duration::minutes(5);
const RATE_LIMITED: Duration = Duration::seconds(30);

/// The pool's keys in the order a request with `seed` tries them. Weighted rendezvous hashing:
/// each key scores `u^(1/weight)` with `u` drawn from the seed and the key, so a key comes first
/// for a share of seeds that is its share of the weight, the same seed always gives the same order
/// on every gateway, and adding or removing a key moves only the callers that drew it.
pub fn order(pool: &[PoolMember], seed: &str) -> Vec<Uuid> {
    let mut scored: Vec<(f64, Uuid)> = pool
        .iter()
        .map(|m| {
            let digest = Sha256::new()
                .chain_update(seed.as_bytes())
                .chain_update(m.credential_id.as_bytes())
                .finalize();
            let mut first = [0_u8; 8];
            first.copy_from_slice(digest.get(..8).unwrap_or(&[0; 8]));
            // 53 random bits, as a float strictly between 0 and 1.
            let bits = u64::from_be_bytes(first) >> 11;
            #[allow(clippy::cast_precision_loss)]
            let u = (bits as f64 + 0.5) / (1_u64 << 53) as f64;
            let weight = f64::from(m.weight.max(1));
            (u.powf(1.0 / weight), m.credential_id)
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, id)| id).collect()
}

/// Whether a failed attempt gives way to the pool's next key: `None` when the failure isn't the
/// key's, otherwise how long to hold the key (`Some(None)` holds nothing, as for a model the
/// key's account doesn't serve, which says nothing about its other models).
pub fn gives_way(stage: &Stage, err: &KppError) -> Option<Option<Duration>> {
    match stage {
        Stage::KeyRefused | Stage::SignedOut(_) | Stage::NoApiAccess(_) => Some(Some(ACCOUNT)),
        Stage::Unserved(_) | Stage::NeedsApiKey { .. } => Some(None),
        Stage::Provider => match err.kind {
            ErrorKind::Billing => Some(Some(ACCOUNT)),
            ErrorKind::RateLimited => Some(Some(
                err.retry_after
                    .and_then(|d| Duration::try_from(d).ok())
                    .map_or(RATE_LIMITED, |d| {
                        d.clamp(Duration::SECOND, LONGEST_RETRY_AFTER)
                    }),
            )),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(n: u128, weight: i16) -> PoolMember {
        PoolMember {
            credential_id: Uuid::from_u128(n),
            weight,
        }
    }

    #[test]
    fn a_caller_keeps_its_key_and_weights_hold_over_many_callers() {
        let pool = [member(1, 1), member(2, 3)];
        let first = order(&pool, "key_a:openai");
        assert_eq!(
            first,
            order(&pool, "key_a:openai"),
            "the same seed, the same order"
        );
        assert_eq!(first.len(), 2);
        let heavy = (0..4000)
            .filter(|i| order(&pool, &format!("caller{i}:openai"))[0] == Uuid::from_u128(2))
            .count();
        assert!(
            (2800..3200).contains(&heavy),
            "a weight of 3 in 4 draws 3/4: {heavy}"
        );
        let alone = [member(7, 100)];
        assert_eq!(order(&alone, "x"), [Uuid::from_u128(7)]);
    }

    #[test]
    fn removing_a_key_moves_only_the_callers_that_drew_it() {
        let three = [member(1, 1), member(2, 1), member(3, 1)];
        let two = [member(1, 1), member(2, 1)];
        for i in 0..500 {
            let seed = format!("caller{i}");
            let before = order(&three, &seed)[0];
            if before != Uuid::from_u128(3) {
                assert_eq!(order(&two, &seed)[0], before, "{seed}");
            }
        }
    }

    #[test]
    fn only_the_keys_own_failures_give_way() {
        let limited = KppError::new(ErrorKind::RateLimited, "slow down")
            .with_retry_after(std::time::Duration::from_secs(7));
        assert_eq!(
            gives_way(&Stage::Provider, &limited),
            Some(Some(Duration::seconds(7)))
        );
        let unbounded = KppError::new(ErrorKind::RateLimited, "slow down")
            .with_retry_after(std::time::Duration::from_secs(3600));
        assert_eq!(
            gives_way(&Stage::Provider, &unbounded),
            Some(Some(LONGEST_RETRY_AFTER))
        );
        let plain = KppError::new(ErrorKind::RateLimited, "slow down");
        assert_eq!(
            gives_way(&Stage::Provider, &plain),
            Some(Some(RATE_LIMITED))
        );
        let empty = KppError::new(ErrorKind::Billing, "no credit");
        assert_eq!(gives_way(&Stage::Provider, &empty), Some(Some(ACCOUNT)));
        let other = KppError::new(ErrorKind::Overloaded, "busy");
        assert_eq!(gives_way(&Stage::Provider, &other), None);
        assert_eq!(gives_way(&Stage::KeyRefused, &other), Some(Some(ACCOUNT)));
        assert_eq!(gives_way(&Stage::Unserved("m".into()), &other), Some(None));
        assert_eq!(gives_way(&Stage::Request, &other), None);
    }
}
