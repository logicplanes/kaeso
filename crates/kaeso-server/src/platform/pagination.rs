use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::error::ApiError;

pub const DEFAULT_LIMIT: u32 = 20;
pub const MAX_LIMIT: u32 = 100;

#[derive(Clone, Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PageParams {
    #[doc = "How many items to return, 1 to 100. Default 20."]
    #[param(minimum = 1, maximum = 100, example = 20)]
    pub limit: Option<u32>,
    #[doc = "The `next_cursor` of the previous page."]
    pub after: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct Page<C = Uuid> {
    pub limit: u32,
    pub after: Option<C>,
}

impl PageParams {
    pub fn page(&self) -> Result<Page, ApiError> {
        self.page_by(decode)
    }

    pub fn page_by<C>(&self, decode: impl Fn(&str) -> Option<C>) -> Result<Page<C>, ApiError> {
        let limit = self.limit.unwrap_or(DEFAULT_LIMIT);
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(ApiError::invalid(
                "limit",
                format!("`limit` must be between 1 and {MAX_LIMIT}."),
            ));
        }
        let after = match &self.after {
            None => None,
            Some(c) => Some(decode(c).ok_or_else(|| {
                ApiError::invalid("after", "`after` is not a cursor from a previous page.")
            })?),
        };
        Ok(Page { limit, after })
    }
}

impl<C> Page<C> {
    pub fn fetch(&self) -> i64 {
        i64::from(self.limit) + 1
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[schema(description = "A page of results.")]
pub struct List<T> {
    #[doc = "The items."]
    pub data: Vec<T>,
    #[doc = "Whether more items follow."]
    pub has_more: bool,
    #[doc = "Pass as `after` to get the next page; `null` on the last page."]
    pub next_cursor: Option<String>,
}

impl<T> List<T> {
    pub fn complete(data: Vec<T>) -> List<T> {
        List {
            data,
            has_more: false,
            next_cursor: None,
        }
    }

    pub fn from_rows<R>(
        page: &Page,
        rows: Vec<R>,
        id: impl Fn(&R) -> Uuid,
        map: impl Fn(R) -> T,
    ) -> List<T> {
        List::from_page(page, rows, |r| encode(id(r)), map)
    }

    pub fn from_page<R, C>(
        page: &Page<C>,
        mut rows: Vec<R>,
        cursor: impl Fn(&R) -> String,
        map: impl Fn(R) -> T,
    ) -> List<T> {
        let has_more = rows.len() > page.limit as usize;
        rows.truncate(page.limit as usize);
        let next_cursor = if has_more {
            rows.last().map(cursor)
        } else {
            None
        };
        List {
            data: rows.into_iter().map(map).collect(),
            has_more,
            next_cursor,
        }
    }
}

pub fn encode(id: Uuid) -> String {
    URL_SAFE_NO_PAD.encode(id.as_bytes())
}

pub fn decode(cursor: &str) -> Option<Uuid> {
    let bytes = URL_SAFE_NO_PAD.decode(cursor).ok()?;
    Uuid::from_slice(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_and_cursors_are_checked() {
        let p = PageParams::default().page().unwrap();
        assert_eq!((p.limit, p.after), (20, None));
        assert!(
            PageParams {
                limit: Some(0),
                after: None
            }
            .page()
            .is_err()
        );
        assert!(
            PageParams {
                limit: Some(101),
                after: None
            }
            .page()
            .is_err()
        );
        assert!(
            PageParams {
                limit: None,
                after: Some("nope!".into())
            }
            .page()
            .is_err()
        );
        let id = Uuid::now_v7();
        assert_eq!(
            PageParams {
                limit: Some(5),
                after: Some(encode(id))
            }
            .page()
            .unwrap()
            .after,
            Some(id)
        );
    }

    #[test]
    fn pages_know_whether_more_follow() {
        let page = PageParams {
            limit: Some(2),
            after: None,
        }
        .page()
        .unwrap();
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::now_v7()).collect();
        let list = List::from_rows(&page, ids.clone(), |u| *u, |u| u);
        assert_eq!(list.data.len(), 2);
        assert!(list.has_more);
        assert_eq!(list.next_cursor.as_deref().and_then(decode), Some(ids[1]));
        let last = List::from_rows(&page, ids[..2].to_vec(), |u| *u, |u| u);
        assert!(!last.has_more);
        assert!(last.next_cursor.is_none());
    }
}
