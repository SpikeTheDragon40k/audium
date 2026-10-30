//! Read queries for the UI. All return plain data.

use std::path::PathBuf;

use audium_core::{BookId, TrackId};
use rusqlite::{Connection, OptionalExtension, Row};

use crate::{Result, paths};

/// Separator for aggregated names (cannot occur in cleaned metadata).
const SEP: char = '\u{1f}';

fn split_names(joined: Option<String>) -> Vec<String> {
    joined
        .map(|s| {
            s.split(SEP)
                .filter(|n| !n.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookFilter {
    All,
    /// In the "Needs review" queue and not dismissed.
    NeedsReview,
    /// Started and not finished, most recently played first.
    ContinueListening,
    /// Newest additions first (at most [`RECENT_LIMIT`]).
    RecentlyAdded,
    /// Finished, most recently finished first.
    Finished,
}

pub const RECENT_LIMIT: usize = 30;

/// SQL (on the `p` progress alias) for "finished and not restarted since".
pub(crate) const FINISHED_SQL: &str =
    "(p.finished_at IS NOT NULL AND p.finished_at >= p.updated_at)";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookSummary {
    pub id: BookId,
    pub title: String,
    pub authors: Vec<String>,
    pub narrators: Vec<String>,
    pub series: Option<String>,
    pub series_index: Option<String>,
    pub duration_ms: i64,
    pub cover_path: Option<String>,
    pub available: bool,
    pub confidence: u8,
    pub track_count: i64,
    pub review_reasons: Vec<String>,
    pub added_at: i64,
    /// Saved book-wide position (0 when never played).
    pub position_ms: i64,
    pub finished: bool,
    pub last_played_at: Option<i64>,
}

fn contributors_sql(role: &str) -> String {
    format!(
        "(SELECT group_concat(name, char(31)) FROM (
             SELECT c.name FROM book_contributors bc JOIN contributors c ON c.id = bc.contributor_id
             WHERE bc.book_id = b.id AND bc.role = '{role}' ORDER BY bc.position))"
    )
}

fn reasons(json: Option<String>) -> Vec<String> {
    json.and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default()
}

pub fn list_books(conn: &Connection, filter: BookFilter) -> Result<Vec<BookSummary>> {
    let (where_sql, order_sql, limit) = match filter {
        BookFilter::All => (String::new(), "b.title_sort, b.id", None),
        BookFilter::NeedsReview => (
            "WHERE rq.book_id IS NOT NULL AND rq.dismissed_at IS NULL".into(),
            "b.title_sort, b.id",
            None,
        ),
        BookFilter::ContinueListening => (
            format!("WHERE p.book_id IS NOT NULL AND NOT {FINISHED_SQL}"),
            "p.updated_at DESC, b.id",
            None,
        ),
        BookFilter::RecentlyAdded => (
            String::new(),
            "b.added_at DESC, b.id DESC",
            Some(RECENT_LIMIT),
        ),
        BookFilter::Finished => (
            format!("WHERE {FINISHED_SQL}"),
            "p.finished_at DESC, b.id",
            None,
        ),
    };
    query_books(conn, &where_sql, order_sql, limit, &[])
}

fn query_books(
    conn: &Connection,
    where_sql: &str,
    order_sql: &str,
    limit: Option<usize>,
    params: &[&dyn rusqlite::ToSql],
) -> Result<Vec<BookSummary>> {
    let limit_sql = limit.map(|n| format!("LIMIT {n}")).unwrap_or_default();
    let sql = format!(
        "SELECT b.id, b.title, {authors}, {narrators}, s.name, b.series_index, b.duration_ms, b.cover_path,
                b.available, b.confidence,
                (SELECT count(*) FROM tracks WHERE book_id = b.id),
                CASE WHEN rq.dismissed_at IS NULL THEN rq.reasons END, b.added_at,
                COALESCE(p.book_position_ms, 0), COALESCE({FINISHED_SQL}, 0), p.updated_at
         FROM books b
         LEFT JOIN series s ON s.id = b.series_id
         LEFT JOIN review_queue rq ON rq.book_id = b.id
         LEFT JOIN progress p ON p.book_id = b.id
         {where_sql}
         ORDER BY {order_sql}
         {limit_sql}",
        authors = contributors_sql("author"),
        narrators = contributors_sql("narrator"),
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params, |r| {
        Ok(BookSummary {
            id: BookId(r.get(0)?),
            title: r.get(1)?,
            authors: split_names(r.get(2)?),
            narrators: split_names(r.get(3)?),
            series: r.get(4)?,
            series_index: r.get(5)?,
            duration_ms: r.get(6)?,
            cover_path: r.get(7)?,
            available: r.get(8)?,
            confidence: r.get::<_, i64>(9)?.clamp(0, 100) as u8,
            track_count: r.get(10)?,
            review_reasons: reasons(r.get(11)?),
            added_at: r.get(12)?,
            position_ms: r.get(13)?,
            finished: r.get(14)?,
            last_played_at: r.get(15)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// One book's summary.
pub fn book_summary(conn: &Connection, id: BookId) -> Result<Option<BookSummary>> {
    Ok(
        query_books(conn, "WHERE b.id = ?1", "b.id", None, &[&id.0])?
            .into_iter()
            .next(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookDetail {
    pub summary: BookSummary,
    pub uid: String,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    /// Field -> source.
    pub provenance: Vec<(String, String)>,
}

pub fn book(conn: &Connection, id: BookId) -> Result<Option<BookDetail>> {
    let Some(summary) = list_books(conn, BookFilter::All)?
        .into_iter()
        .find(|b| b.id == id)
    else {
        return Ok(None);
    };
    let detail = conn
        .query_row(
            "SELECT uid, subtitle, description, year, genre, publisher, language, provenance FROM books WHERE id = ?1",
            [id.0],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get::<_, String>(7)?,
                ))
            },
        )
        .optional()?;
    Ok(detail.map(
        |(uid, subtitle, description, year, genre, publisher, language, provenance)| {
            let mut provenance: Vec<(String, String)> =
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&provenance)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|(k, v)| Some((k, v.as_str()?.to_owned())))
                    .collect();
            provenance.sort();
            BookDetail {
                summary,
                uid,
                subtitle,
                description,
                year,
                genre,
                publisher,
                language,
                provenance,
            }
        },
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackRow {
    pub id: TrackId,
    pub path: PathBuf,
    pub display_path: String,
    pub duration_ms: i64,
    pub title: Option<String>,
    pub disc: Option<u32>,
    pub number: Option<u32>,
    pub missing: bool,
}

pub fn tracks(conn: &Connection, book: BookId) -> Result<Vec<TrackRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, display_path, duration_ms, title, disc, number, missing
         FROM tracks WHERE book_id = ?1 ORDER BY position, id",
    )?;
    let rows = stmt.query_map([book.0], |r: &Row<'_>| {
        Ok(TrackRow {
            id: TrackId(r.get(0)?),
            path: paths::from_bytes(&r.get::<_, Vec<u8>>(1)?),
            display_path: r.get(2)?,
            duration_ms: r.get(3)?,
            title: r.get(4)?,
            disc: r.get(5)?,
            number: r.get(6)?,
            missing: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterRow {
    pub title: String,
    pub start_ms: i64,
}

pub fn chapters(conn: &Connection, book: BookId) -> Result<Vec<ChapterRow>> {
    let mut stmt =
        conn.prepare("SELECT title, start_ms FROM chapters WHERE book_id = ?1 ORDER BY position")?;
    let rows = stmt.query_map([book.0], |r| {
        Ok(ChapterRow {
            title: r.get(0)?,
            start_ms: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub books: i64,
    pub needs_review: i64,
    pub unavailable: i64,
}

pub fn counts(conn: &Connection) -> Result<Counts> {
    Ok(conn.query_row(
        "SELECT (SELECT count(*) FROM books),
                (SELECT count(*) FROM review_queue WHERE dismissed_at IS NULL),
                (SELECT count(*) FROM books WHERE available = 0)",
        [],
        |r| {
            Ok(Counts {
                books: r.get(0)?,
                needs_review: r.get(1)?,
                unavailable: r.get(2)?,
            })
        },
    )?)
}
