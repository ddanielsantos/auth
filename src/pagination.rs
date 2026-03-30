use crate::error::AppError;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Deserialize, IntoParams)]
pub struct CursorParams {
    /// Opaque cursor from a previous page's `next_cursor`. Omit for the first page.
    pub cursor: Option<String>,
    /// Number of items to return. Default: 20, max: 100.
    pub limit: Option<i64>,
}

impl CursorParams {
    pub fn limit(&self) -> i64 {
        self.limit.unwrap_or(20).clamp(1, 100)
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CursorPage<T: Serialize + ToSchema> {
    pub items: Vec<T>,
    /// Cursor for the next page. `null` when this is the last page.
    pub next_cursor: Option<String>,
}

impl<T: Serialize + ToSchema> CursorPage<T> {
    /// Build a page from `limit + 1` fetched rows.
    /// If `rows.len() > limit`, there is a next page and `next_cursor_fn` is called
    /// with the last returned item to produce the cursor.
    pub fn from_rows(mut rows: Vec<T>, limit: i64, next_cursor_fn: impl Fn(&T) -> String) -> Self {
        let has_next = rows.len() as i64 > limit;
        if has_next {
            rows.truncate(limit as usize);
        }
        let next_cursor = if has_next { rows.last().map(next_cursor_fn) } else { None };
        Self { items: rows, next_cursor }
    }
}

/// Encode `(occurred_at, id)` into an opaque base64url cursor string.
pub fn encode_cursor(occurred_at: OffsetDateTime, id: Uuid) -> String {
    let ts = occurred_at
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    URL_SAFE_NO_PAD.encode(format!("{ts}|{id}").as_bytes())
}

/// Decode a cursor string back into `(occurred_at, id)`.
pub fn decode_cursor(cursor: &str) -> Result<(OffsetDateTime, Uuid), AppError> {
    let bytes = URL_SAFE_NO_PAD.decode(cursor.as_bytes()).map_err(|_| AppError::InvalidCursor)?;
    let raw = String::from_utf8(bytes).map_err(|_| AppError::InvalidCursor)?;
    let (ts_part, id_part) = raw.split_once('|').ok_or(AppError::InvalidCursor)?;

    let occurred_at = OffsetDateTime::parse(ts_part, &time::format_description::well_known::Rfc3339)
        .map_err(|_| AppError::InvalidCursor)?;
    let id = Uuid::parse_str(id_part).map_err(|_| AppError::InvalidCursor)?;

    Ok((occurred_at, id))
}
