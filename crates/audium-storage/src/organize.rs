//! User organization: collections, bookmarks, metadata overrides, the review
//! queue, and merging or splitting books. None of this touches audio files.

use std::collections::HashMap;

use audium_core::{BookId, BookUid, TrackId};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{Database, Result, StorageError, fts};

// ---------------------------------------------------------------------------
// Collections

/// Creates a collection, or returns the existing one with that name.
pub fn create_collection(conn: &Connection, name: &str, now: i64) -> Result<i64> {
    let name = clean_name(name)?;
    conn.execute(
        "INSERT INTO collections (name, created_at) VALUES (?1, ?2) ON CONFLICT(name) DO NOTHING",
        params![name, now],
    )?;
    Ok(
        conn.query_row("SELECT id FROM collections WHERE name = ?1", [&name], |r| {
            r.get(0)
        })?,
    )
}

pub fn rename_collection(conn: &Connection, id: i64, name: &str) -> Result<()> {
    let name = clean_name(name)?;
    conn.execute(
        "UPDATE collections SET name = ?2 WHERE id = ?1",
        params![id, name],
    )
    .map_err(|e| match e {
        rusqlite::Error::SqliteFailure(f, _)
            if f.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            StorageError::Invalid(format!("a collection named {name:?} already exists"))
        }
        e => e.into(),
    })?;
    Ok(())
}

pub fn delete_collection(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM collections WHERE id = ?1", [id])?;
    Ok(())
}

/// Adds a book at the end of a collection (no-op if it is already in it).
pub fn add_to_collection(conn: &Connection, collection: i64, book: BookId) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO collection_books (collection_id, book_id, position)
         VALUES (?1, ?2, (SELECT COALESCE(max(position) + 1, 0) FROM collection_books WHERE collection_id = ?1))",
        params![collection, book.0],
    )?;
    Ok(())
}

pub fn remove_from_collection(conn: &Connection, collection: i64, book: BookId) -> Result<()> {
    conn.execute(
        "DELETE FROM collection_books WHERE collection_id = ?1 AND book_id = ?2",
        params![collection, book.0],
    )?;
    Ok(())
}

/// The collections a book is in: (id, name).
pub fn collections_of(conn: &Connection, book: BookId) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name FROM collections c JOIN collection_books cb ON cb.collection_id = c.id
         WHERE cb.book_id = ?1 ORDER BY lower(c.name)",
    )?;
    let rows = stmt.query_map([book.0], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn clean_name(name: &str) -> Result<String> {
    audium_core::text::clean(name, 200)
        .ok_or_else(|| StorageError::Invalid("a name is required".into()))
}

// ---------------------------------------------------------------------------
// Bookmarks

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub id: i64,
    pub book: BookId,
    pub track: Option<TrackId>,
    pub offset_ms: i64,
    pub book_position_ms: i64,
    pub title: Option<String>,
    pub note: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub fn add_bookmark(
    conn: &Connection,
    book: BookId,
    track: Option<TrackId>,
    offset_ms: i64,
    book_position_ms: i64,
    now: i64,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO bookmarks (book_id, track_id, offset_ms, book_position_ms, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![book.0, track.map(|t| t.0), offset_ms, book_position_ms, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Sets a bookmark's title and note (empty text clears them).
pub fn update_bookmark(
    conn: &Connection,
    id: i64,
    title: &str,
    note: &str,
    now: i64,
) -> Result<()> {
    let title = audium_core::text::clean(title, 500);
    let note = audium_core::text::clean_multiline(note, 20_000);
    conn.execute(
        "UPDATE bookmarks SET title = ?2, note = ?3, updated_at = ?4 WHERE id = ?1",
        params![id, title, note, now],
    )?;
    Ok(())
}

pub fn delete_bookmark(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM bookmarks WHERE id = ?1", [id])?;
    Ok(())
}

pub fn bookmarks(conn: &Connection, book: BookId) -> Result<Vec<Bookmark>> {
    let mut stmt = conn.prepare(
        "SELECT id, book_id, track_id, offset_ms, book_position_ms, title, note, created_at, updated_at
         FROM bookmarks WHERE book_id = ?1 ORDER BY book_position_ms, id",
    )?;
    let rows = stmt.query_map([book.0], |r| {
        Ok(Bookmark {
            id: r.get(0)?,
            book: BookId(r.get(1)?),
            track: r.get::<_, Option<i64>>(2)?.map(TrackId),
            offset_ms: r.get(3)?,
            book_position_ms: r.get(4)?,
            title: r.get(5)?,
            note: r.get(6)?,
            created_at: r.get(7)?,
            updated_at: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

// ---------------------------------------------------------------------------
// Metadata overrides

/// Fields that can be overridden. Lists (authors, narrators) are stored one
/// name per line.
pub const OVERRIDE_FIELDS: &[&str] = &[
    "title",
    "subtitle",
    "authors",
    "narrators",
    "series",
    "series_index",
    "description",
    "year",
    "genre",
];

/// Sets (or, with an empty value, removes) one override. Takes effect when
/// the book is next re-read.
pub fn set_override(
    conn: &Connection,
    book: BookId,
    field: &str,
    value: &str,
    now: i64,
) -> Result<()> {
    if !OVERRIDE_FIELDS.contains(&field) {
        return Err(StorageError::Invalid(format!(
            "unknown metadata field {field:?}"
        )));
    }
    let value = value.trim();
    if value.is_empty() {
        conn.execute(
            "DELETE FROM overrides WHERE book_id = ?1 AND field = ?2",
            params![book.0, field],
        )?;
    } else {
        conn.execute(
            "INSERT INTO overrides (book_id, field, value, updated_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(book_id, field) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![book.0, field, value, now],
        )?;
    }
    Ok(())
}

pub fn overrides(conn: &Connection, book: BookId) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT field, value FROM overrides WHERE book_id = ?1")?;
    let rows = stmt.query_map([book.0], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

// ---------------------------------------------------------------------------
// Review queue

/// Hides a book from "Needs review" until its reasons change.
pub fn dismiss_review(conn: &Connection, book: BookId, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE review_queue SET dismissed_at = ?2 WHERE book_id = ?1",
        params![book.0, now],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Merge and split

/// Moves every track of `others` into `target` (after its own tracks) and
/// deletes the emptied books. Their bookmarks, sessions and collection
/// memberships move to `target`; their progress and speed are used only if
/// `target` has none. `target` is locked, so rescans keep the grouping.
pub fn merge_books(db: &mut Database, target: BookId, others: &[BookId], now: i64) -> Result<()> {
    let others: Vec<BookId> = others.iter().copied().filter(|b| *b != target).collect();
    if others.is_empty() {
        return Ok(());
    }
    let tx = db.conn_mut().transaction()?;
    let exists: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM books WHERE id = ?1)",
        [target.0],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(StorageError::Invalid(format!(
            "book {target} does not exist"
        )));
    }
    for other in &others {
        let base: i64 = tx.query_row(
            "SELECT COALESCE(max(position) + 1, 0) FROM tracks WHERE book_id = ?1",
            [target.0],
            |r| r.get(0),
        )?;
        tx.execute(
            "UPDATE tracks SET book_id = ?1, position = position + ?3 WHERE book_id = ?2",
            params![target.0, other.0, base],
        )?;
        tx.execute(
            "UPDATE bookmarks SET book_id = ?1 WHERE book_id = ?2",
            params![target.0, other.0],
        )?;
        tx.execute(
            "UPDATE sessions SET book_id = ?1 WHERE book_id = ?2",
            params![target.0, other.0],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO collection_books (collection_id, book_id, position)
             SELECT collection_id, ?1, position FROM collection_books WHERE book_id = ?2",
            params![target.0, other.0],
        )?;
        tx.execute(
            "UPDATE progress SET book_id = ?1 WHERE book_id = ?2 AND NOT EXISTS (SELECT 1 FROM progress WHERE book_id = ?1)",
            params![target.0, other.0],
        )?;
        tx.execute(
            "UPDATE book_settings SET book_id = ?1 WHERE book_id = ?2 AND NOT EXISTS (SELECT 1 FROM book_settings WHERE book_id = ?1)",
            params![target.0, other.0],
        )?;
        tx.execute("DELETE FROM books WHERE id = ?1", [other.0])?;
    }
    renumber_tracks(&tx, target)?;
    tx.execute(
        "UPDATE books SET grouping_locked = 1, updated_at = ?2 WHERE id = ?1",
        params![target.0, now],
    )?;
    tx.execute("DELETE FROM review_queue WHERE book_id = ?1", [target.0])?;
    fts::prune(&tx)?;
    tx.commit()?;
    Ok(())
}

/// Moves `tracks` of `book` into a new book (a copy of the original's
/// metadata until the next re-read). Bookmarks on the moved tracks move
/// with them. Both books are locked. Returns the new book's id.
pub fn split_book(
    db: &mut Database,
    book: BookId,
    tracks: &[TrackId],
    uid: &BookUid,
    now: i64,
) -> Result<BookId> {
    let tx = db.conn_mut().transaction()?;
    let total: i64 = tx.query_row(
        "SELECT count(*) FROM tracks WHERE book_id = ?1",
        [book.0],
        |r| r.get(0),
    )?;
    let ids: Vec<i64> = tracks.iter().map(|t| t.0).collect();
    let moving: i64 = ids
        .iter()
        .map(|id| {
            tx.query_row(
                "SELECT count(*) FROM tracks WHERE id = ?1 AND book_id = ?2",
                params![id, book.0],
                |r| r.get::<_, i64>(0),
            )
        })
        .sum::<rusqlite::Result<i64>>()?;
    if moving == 0 || moving as usize != ids.len() {
        return Err(StorageError::Invalid(
            "the tracks to split off must belong to the book".into(),
        ));
    }
    if moving == total {
        return Err(StorageError::Invalid(
            "a split must leave at least one track in the book".into(),
        ));
    }
    tx.execute(
        "INSERT INTO books (uid, library_id, title, title_sort, subtitle, description, year, genre, publisher, language,
                            series_id, series_index, cover_path, confidence, available, grouping_locked, provenance,
                            added_at, updated_at)
         SELECT ?2, library_id, title, title_sort, subtitle, description, year, genre, publisher, language,
                series_id, series_index, cover_path, confidence, available, 1, provenance, ?3, ?3
         FROM books WHERE id = ?1",
        params![book.0, uid.as_str(), now],
    )?;
    let new_book = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO book_contributors (book_id, contributor_id, role, position)
         SELECT ?2, contributor_id, role, position FROM book_contributors WHERE book_id = ?1",
        params![book.0, new_book],
    )?;
    for id in &ids {
        tx.execute(
            "UPDATE tracks SET book_id = ?2 WHERE id = ?1",
            params![id, new_book],
        )?;
        tx.execute(
            "UPDATE bookmarks SET book_id = ?2 WHERE track_id = ?1",
            params![id, new_book],
        )?;
    }
    renumber_tracks(&tx, book)?;
    renumber_tracks(&tx, BookId(new_book))?;
    tx.execute(
        "UPDATE books SET grouping_locked = 1, updated_at = ?2 WHERE id = ?1",
        params![book.0, now],
    )?;
    tx.execute("DELETE FROM review_queue WHERE book_id = ?1", [book.0])?;
    fts::refresh(&tx, BookId(new_book))?;
    tx.commit()?;
    Ok(BookId(new_book))
}

/// Positions 0..n in the current order.
fn renumber_tracks(conn: &Connection, book: BookId) -> Result<()> {
    let ids: Vec<i64> = {
        let mut stmt =
            conn.prepare("SELECT id FROM tracks WHERE book_id = ?1 ORDER BY position, id")?;
        stmt.query_map([book.0], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?
    };
    for (pos, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE tracks SET position = ?2 WHERE id = ?1",
            params![id, pos as i64],
        )?;
    }
    Ok(())
}

/// The library and folders of a book's tracks (for re-reading it).
pub fn book_folders(
    conn: &Connection,
    book: BookId,
) -> Result<(Option<i64>, Vec<std::path::PathBuf>)> {
    let library: Option<i64> = conn
        .query_row(
            "SELECT library_id FROM books WHERE id = ?1",
            [book.0],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    let mut dirs: Vec<std::path::PathBuf> = crate::read::tracks(conn, book)?
        .into_iter()
        .filter_map(|t| t.path.parent().map(std::path::Path::to_owned))
        .collect();
    dirs.sort();
    dirs.dedup();
    Ok((library, dirs))
}
