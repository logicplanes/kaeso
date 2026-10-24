use kaeso_kpp::RawItem;
use serde::Deserialize;
use serde_json::Value;

use super::DIALECT;

pub(crate) fn finished_item(start: &RawItem, deltas: &[RawItem]) -> RawItem {
    deltas
        .iter()
        .rev()
        .find(|d| d.origin.is(DIALECT))
        .unwrap_or(start)
        .clone()
}

pub(crate) fn reports_item_progress(frame: &RawItem) -> bool {
    #[derive(Deserialize)]
    struct Frame {
        data: Option<Value>,
    }
    frame.origin.is(DIALECT)
        && serde_json::from_str::<Frame>(frame.raw.get())
            .ok()
            .and_then(|f| f.data)
            .is_some_and(|d| {
                d.get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|t| t.starts_with("response."))
                    && d.get("item_id").is_some_and(Value::is_string)
            })
}
