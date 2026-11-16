use std::collections::HashMap;
use std::time::Duration;

use clickhouse::Row;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::ClickHouse;
use crate::StoreError;

/// How often at most a gateway writes a part again that it wrote before, to move its
/// `last_at` and `expires_at`. A request that relies on a part started within this long after
/// the part's `last_at`, so deletion keeps a part until its `last_at` plus this is gone too.
pub const REWRITE_EVERY: Duration = Duration::from_secs(3600);

pub type Hash = [u8; 16];

const OPEN: &str = "{\"$kaeso_part\":\"";
const CLOSE: &str = "\"}";
const HEX_LEN: usize = 32;

/// The text a part leaves in the request body it was cut from.
pub fn placeholder(hash: &Hash) -> String {
    let mut out = String::with_capacity(OPEN.len() + HEX_LEN + CLOSE.len());
    out.push_str(OPEN);
    for b in hash {
        out.push_str(&format!("{b:02x}"));
    }
    out.push_str(CLOSE);
    out
}

/// Whether a body could be mistaken for one with placeholders; such a body is stored whole.
pub fn mentions_placeholders(body: &str) -> bool {
    body.contains("\"$kaeso_part\"")
}

fn parse_hex(text: &str) -> Option<Hash> {
    let mut out = [0u8; 16];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

fn placeholders(body: &str) -> impl Iterator<Item = (usize, Hash)> + '_ {
    body.match_indices(OPEN).filter_map(move |(at, _)| {
        let hex = body.get(at + OPEN.len()..at + OPEN.len() + HEX_LEN)?;
        let close = body.get(at + OPEN.len() + HEX_LEN..at + OPEN.len() + HEX_LEN + CLOSE.len());
        (close == Some(CLOSE)).then_some(())?;
        parse_hex(hex).map(|h| (at, h))
    })
}

/// The hashes a stored body names.
pub fn hashes(body: &str) -> Vec<Hash> {
    placeholders(body).map(|(_, h)| h).collect()
}

/// The body with every placeholder replaced by its part; a placeholder whose part is missing
/// stays as it is.
pub fn splice(body: &str, parts: &HashMap<Hash, String>) -> String {
    let extra: usize = parts.values().map(String::len).sum();
    let mut out = String::with_capacity(body.len() + extra);
    let mut from = 0;
    for (at, hash) in placeholders(body) {
        let Some(part) = parts.get(&hash) else {
            continue;
        };
        out.push_str(body.get(from..at).unwrap_or_default());
        out.push_str(part);
        from = at + OPEN.len() + HEX_LEN + CLOSE.len();
    }
    out.push_str(body.get(from..).unwrap_or_default());
    out
}

#[derive(Clone, Debug, PartialEq, Row, Serialize, Deserialize)]
pub struct PartRow {
    #[serde(with = "clickhouse::serde::uuid")]
    pub org_id: Uuid,
    #[serde(with = "clickhouse::serde::uuid")]
    pub project_id: Uuid,
    pub hash: Hash,
    pub body: String,
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    pub first_at: OffsetDateTime,
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    pub last_at: OffsetDateTime,
    #[serde(with = "clickhouse::serde::time::datetime")]
    pub expires_at: OffsetDateTime,
}

pub async fn insert<'a>(
    ch: &ClickHouse,
    rows: impl IntoIterator<Item = &'a PartRow>,
) -> Result<(), StoreError> {
    let mut rows = rows.into_iter().peekable();
    if rows.peek().is_none() {
        return Ok(());
    }
    let mut insert = ch
        .client()
        .insert::<PartRow>("request_content_part")
        .await?;
    for r in rows {
        insert.write(r).await?;
    }
    insert.end().await?;
    Ok(())
}

#[derive(Row, Deserialize)]
struct Found {
    hash: Hash,
    body: String,
}

/// The bodies of a project's parts, by hash.
pub async fn bodies(
    ch: &ClickHouse,
    org: Uuid,
    project: Uuid,
    wanted: &[Hash],
) -> Result<HashMap<Hash, String>, StoreError> {
    if wanted.is_empty() {
        return Ok(HashMap::new());
    }
    let mut hex: Vec<String> = wanted.iter().map(hex_of).collect();
    hex.sort_unstable();
    hex.dedup();
    let found = ch
        .client()
        .query(
            "SELECT hash, any(body) AS body FROM request_content_part \
             WHERE org_id = ? AND project_id = ? AND hash IN (SELECT unhex(arrayJoin(?))) \
             GROUP BY hash",
        )
        .bind(org)
        .bind(project)
        .bind(hex)
        .fetch_all::<Found>()
        .await?;
    Ok(found.into_iter().map(|f| (f.hash, f.body)).collect())
}

fn hex_of(hash: &Hash) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_round_trip_and_splice_in_place() {
        let a = [0xabu8; 16];
        let b = [0x01u8; 16];
        let body = format!(
            r#"{{"messages":[{},{},{}],"x":1}}"#,
            placeholder(&a),
            placeholder(&b),
            placeholder(&a)
        );
        assert_eq!(hashes(&body), vec![a, b, a]);
        assert!(mentions_placeholders(&body));
        let parts = HashMap::from([(a, "\"A\"".to_owned()), (b, "{\"b\": 2}".to_owned())]);
        assert_eq!(
            splice(&body, &parts),
            r#"{"messages":["A",{"b": 2},"A"],"x":1}"#
        );
        let missing = HashMap::from([(a, "1".to_owned())]);
        assert_eq!(
            splice(&body, &missing),
            format!(r#"{{"messages":[1,{},1],"x":1}}"#, placeholder(&b))
        );
        assert_eq!(splice("plain", &parts), "plain");
        assert_eq!(placeholder(&a).len(), 50);
        assert_eq!(hex_of(&b), "01".repeat(16));
    }

    #[test]
    fn malformed_placeholders_are_left_alone() {
        for body in [
            r#"{"$kaeso_part":"zz"}"#,
            r#"{"$kaeso_part":"0101010101010101010101010101010"}"#,
            r#"{"$kaeso_part":"01010101010101010101010101010101" }"#,
            r#"{"$kaeso_part":"#,
        ] {
            assert!(hashes(body).is_empty(), "{body}");
            assert_eq!(splice(body, &HashMap::new()), body);
        }
    }
}
