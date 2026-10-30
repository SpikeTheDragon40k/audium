//! Playback state: progress (track id + offset, with the derived book-wide
//! position), finished state, per-book speed, listening sessions, settings.

use audium_core::{BookId, TrackId};
use rusqlite::{Connection, OptionalExtension, params};

use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub track_id: Option<TrackId>,
    pub offset_ms: i64,
    pub book_position_ms: i64,
    /// Finished and not restarted since.
    pub finished: bool,
    pub updated_at: i64,
}

pub fn progress(conn: &Connection, book: BookId) -> Result<Option<Progress>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT track_id, offset_ms, book_position_ms, COALESCE({finished}, 0), updated_at
                 FROM progress p WHERE book_id = ?1",
                finished = crate::read::FINISHED_SQL
            ),
            [book.0],
            |r| {
                Ok(Progress {
                    track_id: r.get::<_, Option<i64>>(0)?.map(TrackId),
                    offset_ms: r.get(1)?,
                    book_position_ms: r.get(2)?,
                    finished: r.get(3)?,
                    updated_at: r.get(4)?,
                })
            },
        )
        .optional()?)
}

/// Saves the position. Keeps `finished_at`; a book counts as finished only
/// while its last update is the finishing one (see `FINISHED_SQL`), so
/// `updated_at` is kept strictly after `finished_at`: a save in the same
/// millisecond as finishing still means the book was restarted.
pub fn save_progress(
    conn: &Connection,
    book: BookId,
    track: Option<TrackId>,
    offset_ms: i64,
    book_position_ms: i64,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO progress (book_id, track_id, offset_ms, book_position_ms, started_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(book_id) DO UPDATE SET track_id = excluded.track_id, offset_ms = excluded.offset_ms,
            book_position_ms = excluded.book_position_ms,
            updated_at = max(excluded.updated_at, COALESCE(progress.finished_at + 1, 0))",
        params![book.0, track.map(|t| t.0), offset_ms, book_position_ms, now],
    )?;
    Ok(())
}

/// Saves the final position and marks the book finished.
pub fn mark_finished(
    conn: &Connection,
    book: BookId,
    track: Option<TrackId>,
    offset_ms: i64,
    book_position_ms: i64,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO progress (book_id, track_id, offset_ms, book_position_ms, started_at, updated_at, finished_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?5)
         ON CONFLICT(book_id) DO UPDATE SET track_id = excluded.track_id, offset_ms = excluded.offset_ms,
            book_position_ms = excluded.book_position_ms, updated_at = excluded.updated_at,
            finished_at = excluded.finished_at",
        params![book.0, track.map(|t| t.0), offset_ms, book_position_ms, now],
    )?;
    Ok(())
}

pub fn book_speed(conn: &Connection, book: BookId) -> Result<Option<f64>> {
    Ok(conn
        .query_row(
            "SELECT speed FROM book_settings WHERE book_id = ?1",
            [book.0],
            |r| r.get::<_, Option<f64>>(0),
        )
        .optional()?
        .flatten()
        .filter(|s| s.is_finite() && *s > 0.0))
}

pub fn set_book_speed(conn: &Connection, book: BookId, speed: f64) -> Result<()> {
    conn.execute(
        "INSERT INTO book_settings (book_id, speed) VALUES (?1, ?2)
         ON CONFLICT(book_id) DO UPDATE SET speed = excluded.speed",
        params![book.0, speed],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionRecord {
    pub book: BookId,
    pub started_at: i64,
    pub ended_at: i64,
    pub start_position_ms: i64,
    pub end_position_ms: i64,
    pub listened_ms: i64,
}

pub fn record_session(conn: &Connection, s: &SessionRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO sessions (book_id, started_at, ended_at, start_position_ms, end_position_ms, listened_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![s.book.0, s.started_at, s.ended_at, s.start_position_ms, s.end_position_ms, s.listened_ms],
    )?;
    Ok(())
}

pub fn setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}
