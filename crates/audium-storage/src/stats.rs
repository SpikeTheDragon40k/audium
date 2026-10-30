//! Listening statistics queries. Local days come from SQLite's `localtime`
//! (the system time zone, including daylight saving time); the arithmetic is
//! in `audium_core::stats`.

use audium_core::BookId;
use audium_core::stats::Date;
use rusqlite::Connection;

use crate::Result;

/// Listening time per local day. A session running past local midnight is
/// split between the two days (sessions longer than a day are not split
/// further).
pub fn listening_per_day(conn: &Connection) -> Result<Vec<(Date, i64)>> {
    let mut stmt = conn.prepare(
        "WITH s AS (
            SELECT started_at, listened_ms,
                   date(started_at / 1000, 'unixepoch', 'localtime') AS d1,
                   date(ended_at / 1000, 'unixepoch', 'localtime') AS d2,
                   CAST(strftime('%s', date(ended_at / 1000, 'unixepoch', 'localtime'), 'utc') AS INTEGER) * 1000 AS midnight
            FROM sessions WHERE listened_ms > 0),
         parts AS (
            SELECT d1 AS day,
                   CASE WHEN d1 = d2 THEN listened_ms ELSE max(0, min(listened_ms, midnight - started_at)) END AS ms
            FROM s
            UNION ALL
            SELECT d2, listened_ms - max(0, min(listened_ms, midnight - started_at)) FROM s WHERE d1 <> d2)
         SELECT day, sum(ms) FROM parts GROUP BY day",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (day, ms) = row?;
        if let Some(d) = Date::parse(&day) {
            out.push((d, ms));
        }
    }
    Ok(out)
}

/// Today's local date.
pub fn today(conn: &Connection) -> Result<Option<Date>> {
    let s: String = conn.query_row("SELECT date('now', 'localtime')", [], |r| r.get(0))?;
    Ok(Date::parse(&s))
}

/// The local dates on which books were (last) finished.
pub fn finished_dates(conn: &Connection) -> Result<Vec<Date>> {
    let mut stmt = conn.prepare(
        "SELECT date(finished_at / 1000, 'unixepoch', 'localtime') FROM progress WHERE finished_at IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows
        .filter_map(|r| r.ok().and_then(|s| Date::parse(&s)))
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookTotal {
    pub book: BookId,
    pub title: String,
    pub cover_path: Option<String>,
    pub listened_ms: i64,
    pub sessions: i64,
}

/// Books by total listening time, most first.
pub fn per_book_totals(conn: &Connection, limit: usize) -> Result<Vec<BookTotal>> {
    let mut stmt = conn.prepare(
        "SELECT b.id, b.title, b.cover_path, sum(s.listened_ms), count(*)
         FROM sessions s JOIN books b ON b.id = s.book_id
         GROUP BY b.id ORDER BY sum(s.listened_ms) DESC, b.title_sort LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit as i64], |r| {
        Ok(BookTotal {
            book: BookId(r.get(0)?),
            title: r.get(1)?,
            cover_path: r.get(2)?,
            listened_ms: r.get(3)?,
            sessions: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
