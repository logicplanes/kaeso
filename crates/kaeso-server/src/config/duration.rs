use std::time::Duration;

use serde::Deserialize;

pub(crate) use kaeso_update::duration::parse;

/// For `#[serde(deserialize_with = "duration::deserialize")]`.
pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
    let text = String::deserialize(d)?;
    parse(&text).map_err(serde::de::Error::custom)
}
