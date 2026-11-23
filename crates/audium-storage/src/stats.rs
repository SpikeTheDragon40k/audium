//! Listening statistics queries. Local days come from SQLite's `localtime`
//! (the system time zone, including daylight saving time); the arithmetic is
//! in `audium_core::stats`.

use audium_core::BookId;
use audium_core::stats::Date;
use rusqlite::Connection;

use crate::Result;

/// Deletes every listening session, so the time listened, streaks, chart
/// and per-book totals start again from zero. Progress, bookmarks and
/// finished books are kept (books finished come from `progress`). Returns
/// how many sessions were removed.
pub fn clear_listening(conn: &Connection) -> Result<usize> {
    Ok(conn.execute("DELETE FROM sessions", [])?)
}

/// Listening time per local day. A session running past local midnight is
/// split between the days it covers, as if listened to from its start
/// without pauses (a session only records its total listening time).
pub fn listening_per_day(conn: &Connection) -> Result<Vec<(Date, i64)>> {
    // `cursor` walks from the session start one local midnight at a time;
    // each step gives the day it starts in the part up to the next midnight.
    const NEXT_MIDNIGHT: &str = "CAST(strftime('%s', date(cursor / 1000, 'unixepoch', 'localtime', '+1 day'), 'utc') AS INTEGER) * 1000";
    let part = format!(
        "CASE WHEN {NEXT_MIDNIGHT} >= ended_at THEN remaining
              ELSE max(0, min(remaining, {NEXT_MIDNIGHT} - cursor)) END"
    );
    let mut stmt = conn.prepare(&format!(
        "WITH RECURSIVE walk(day, cursor, ended_at, remaining, ms) AS (
            SELECT NULL, started_at, ended_at, listened_ms, 0 FROM sessions WHERE listened_ms > 0
            UNION ALL
            SELECT date(cursor / 1000, 'unixepoch', 'localtime'), {NEXT_MIDNIGHT}, ended_at,
                   remaining - ({part}), {part}
            FROM walk WHERE remaining > 0)
         SELECT day, sum(ms) FROM walk WHERE day IS NOT NULL AND ms > 0 GROUP BY day"
    ))?;
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
