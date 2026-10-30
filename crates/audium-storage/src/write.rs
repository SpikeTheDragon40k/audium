//! Writes: library roots and applying scan results.
//!
//! Books that carry user data (progress, bookmarks, sessions, overrides,
//! settings, collections) are never deleted; they are marked unavailable.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use audium_core::text::sort_key;
use audium_core::{BookId, BookUid, LibraryId, TrackId};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::{Database, Result, StorageError, now_ms, paths};

/// SQL condition (on `books.id`) true when a book has user data.
pub(crate) const HAS_USER_DATA: &str = "(EXISTS (SELECT 1 FROM progress WHERE book_id = books.id)
     OR EXISTS (SELECT 1 FROM bookmarks WHERE book_id = books.id)
     OR EXISTS (SELECT 1 FROM sessions WHERE book_id = books.id)
     OR EXISTS (SELECT 1 FROM overrides WHERE book_id = books.id)
     OR EXISTS (SELECT 1 FROM book_settings WHERE book_id = books.id)
     OR EXISTS (SELECT 1 FROM collection_books WHERE book_id = books.id))";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library {
    pub id: LibraryId,
    pub path: PathBuf,
    pub display_path: String,
    pub available: bool,
    pub last_scan_at: Option<i64>,
}

pub fn add_library(db: &Database, path: &Path) -> Result<LibraryId> {
    let bytes = paths::to_bytes(path);
    db.conn().execute(
        "INSERT INTO libraries (path, display_path, added_at) VALUES (?1, ?2, ?3) ON CONFLICT(path) DO NOTHING",
        params![bytes, paths::display(path), now_ms()],
    )?;
    let id = db
        .conn()
        .query_row("SELECT id FROM libraries WHERE path = ?1", [&bytes], |r| {
            r.get(0)
        })?;
    Ok(LibraryId(id))
}

pub fn libraries(conn: &Connection) -> Result<Vec<Library>> {
    let mut stmt =
        conn.prepare("SELECT id, path, display_path, available, last_scan_at FROM libraries ORDER BY display_path")?;
    let rows = stmt.query_map([], |r| {
        Ok(Library {
            id: LibraryId(r.get(0)?),
            path: paths::from_bytes(&r.get::<_, Vec<u8>>(1)?),
            display_path: r.get(2)?,
            available: r.get(3)?,
            last_scan_at: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Removes a library root. Its books without user data are deleted; books
/// with user data are kept, detached and unavailable. Files are not touched.
pub fn remove_library(db: &mut Database, id: LibraryId) -> Result<()> {
    let tx = db.conn_mut().transaction()?;
    tx.execute(
        &format!("DELETE FROM books WHERE library_id = ?1 AND NOT {HAS_USER_DATA}"),
        [id.0],
    )?;
    tx.execute(
        "UPDATE books SET library_id = NULL, available = 0 WHERE library_id = ?1",
        [id.0],
    )?;
    tx.execute("DELETE FROM libraries WHERE id = ?1", [id.0])?;
    cleanup_orphans(&tx)?;
    tx.commit()?;
    Ok(())
}

/// A track as currently stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingTrack {
    pub id: TrackId,
    pub book_id: BookId,
    pub library_id: Option<LibraryId>,
    pub path: PathBuf,
    pub size: i64,
    pub mtime_ms: i64,
    pub partial_hash: Option<Vec<u8>>,
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingBook {
    pub uid: BookUid,
    pub grouping_locked: bool,
    /// Has progress, bookmarks, sessions, overrides, settings or collections.
    pub has_user_data: bool,
    /// Raw `overrides` rows: field -> value.
    pub overrides: HashMap<String, String>,
}

/// Everything the scanner needs to plan a reconciliation.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub tracks: Vec<ExistingTrack>,
    pub books: HashMap<BookId, ExistingBook>,
}

pub fn snapshot(conn: &Connection) -> Result<Snapshot> {
    let mut snap = Snapshot::default();
    let mut stmt = conn.prepare(
        "SELECT t.id, t.book_id, b.library_id, t.path, t.size, t.mtime_ms, t.partial_hash, t.missing
         FROM tracks t JOIN books b ON b.id = t.book_id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ExistingTrack {
            id: TrackId(r.get(0)?),
            book_id: BookId(r.get(1)?),
            library_id: r.get::<_, Option<i64>>(2)?.map(LibraryId),
            path: paths::from_bytes(&r.get::<_, Vec<u8>>(3)?),
            size: r.get(4)?,
            mtime_ms: r.get(5)?,
            partial_hash: r.get(6)?,
            missing: r.get(7)?,
        })
    })?;
    snap.tracks = rows.collect::<rusqlite::Result<_>>()?;

    let mut stmt = conn.prepare(&format!(
        "SELECT id, uid, grouping_locked, {HAS_USER_DATA} FROM books"
    ))?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, bool>(2)?,
            r.get::<_, bool>(3)?,
        ))
    })?;
    for row in rows {
        let (id, uid, locked, has_user_data) = row?;
        let uid = BookUid::parse(&uid)
            .ok_or_else(|| StorageError::Invalid(format!("book {id} has uid {uid:?}")))?;
        snap.books.insert(
            BookId(id),
            ExistingBook {
                uid,
                grouping_locked: locked,
                has_user_data,
                overrides: HashMap::new(),
            },
        );
    }
    let mut stmt = conn.prepare("SELECT book_id, field, value FROM overrides")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            BookId(r.get(0)?),
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (book, field, value) = row?;
        if let Some(b) = snap.books.get_mut(&book) {
            b.overrides.insert(field, value);
        }
    }
    Ok(snap)
}

/// Resolved, display-ready metadata of a book.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookMetadata {
    pub title: String,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub narrators: Vec<String>,
    pub series: Option<String>,
    pub series_index: Option<String>,
    pub description: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    /// Field name -> metadata source ("embedded", "folder", ...).
    pub provenance: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackWrite {
    /// The stored track this file is (matched by path or detected as moved).
    pub existing: Option<TrackId>,
    pub path: PathBuf,
    pub size: i64,
    pub mtime_ms: i64,
    pub partial_hash: Option<Vec<u8>>,
    pub duration_ms: i64,
    pub disc: Option<u32>,
    pub number: Option<u32>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterWrite {
    pub title: String,
    /// Book-wide start.
    pub start_ms: i64,
    pub embedded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookTarget {
    Existing(BookId),
    New(BookUid),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookWrite {
    pub target: BookTarget,
    pub metadata: BookMetadata,
    pub cover_path: Option<String>,
    pub confidence: u8,
    /// Reason codes for the "Needs review" queue; empty = not queued.
    pub review_reasons: Vec<String>,
    pub tracks: Vec<TrackWrite>,
    pub chapters: Vec<ChapterWrite>,
}

/// The result of scanning one library root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanChanges {
    pub books: Vec<BookWrite>,
    /// Tracks whose files were not found.
    pub missing_tracks: Vec<TrackId>,
    /// Unchanged tracks that were seen again (clears `missing`).
    pub present_tracks: Vec<TrackId>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApplyReport {
    pub books_added: usize,
    pub books_updated: usize,
    pub books_removed: usize,
    pub tracks_missing: usize,
}

/// Marks a library root reachable or not (e.g. an unmounted drive).
pub fn set_library_available(db: &Database, id: LibraryId, available: bool) -> Result<()> {
    db.conn().execute(
        "UPDATE libraries SET available = ?2 WHERE id = ?1",
        params![id.0, available],
    )?;
    refresh_availability(db.conn())?;
    Ok(())
}

/// Applies one root's scan in a single transaction.
pub fn apply(db: &mut Database, library: LibraryId, changes: &ScanChanges) -> Result<ApplyReport> {
    let now = now_ms();
    let mut report = ApplyReport::default();
    let tx = db.conn_mut().transaction()?;

    for id in &changes.present_tracks {
        tx.execute("UPDATE tracks SET missing = 0 WHERE id = ?1", [id.0])?;
    }
    for id in &changes.missing_tracks {
        report.tracks_missing += tx.execute(
            "UPDATE tracks SET missing = 1 WHERE id = ?1 AND missing = 0",
            [id.0],
        )?;
    }

    let mut touched: HashSet<i64> = HashSet::new();
    for book in &changes.books {
        let book_id = match &book.target {
            BookTarget::Existing(id) => {
                report.books_updated += 1;
                id.0
            }
            BookTarget::New(uid) => {
                report.books_added += 1;
                tx.execute(
                    "INSERT INTO books (uid, library_id, title, title_sort, confidence, added_at, updated_at)
                     VALUES (?1, ?2, '', '', 0, ?3, ?3)",
                    params![uid.as_str(), library.0, now],
                )?;
                tx.last_insert_rowid()
            }
        };
        touched.insert(book_id);
        write_book(&tx, book_id, library, book, now)?;
    }

    // Books left without tracks (their files now belong to other books).
    let emptied: Vec<i64> = {
        let mut stmt = tx.prepare(
            "SELECT id FROM books WHERE NOT EXISTS (SELECT 1 FROM tracks WHERE book_id = books.id)",
        )?;
        stmt.query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?
    };
    for id in emptied {
        report.books_removed += tx.execute(
            &format!("DELETE FROM books WHERE id = ?1 AND NOT {HAS_USER_DATA}"),
            [id],
        )?;
    }

    tx.execute(
        "UPDATE libraries SET last_scan_at = ?2, available = 1 WHERE id = ?1",
        params![library.0, now],
    )?;
    cleanup_orphans(&tx)?;
    refresh_availability(&tx)?;
    tx.commit()?;
    Ok(report)
}

fn write_book(
    tx: &Transaction<'_>,
    book_id: i64,
    library: LibraryId,
    book: &BookWrite,
    now: i64,
) -> Result<()> {
    let m = &book.metadata;
    let series_id: Option<i64> = match &m.series {
        Some(name) => {
            tx.execute(
                "INSERT INTO series (name, name_sort) VALUES (?1, ?2) ON CONFLICT(name) DO NOTHING",
                params![name, sort_key(name)],
            )?;
            Some(
                tx.query_row("SELECT id FROM series WHERE name = ?1", [name], |r| {
                    r.get(0)
                })?,
            )
        }
        None => None,
    };
    let duration: i64 = book.tracks.iter().map(|t| t.duration_ms.max(0)).sum();
    let provenance = serde_json::Value::Object(
        m.provenance
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect(),
    )
    .to_string();
    tx.execute(
        "UPDATE books SET library_id = ?2, title = ?3, title_sort = ?4, subtitle = ?5, description = ?6,
            year = ?7, genre = ?8, publisher = ?9, language = ?10, series_id = ?11, series_index = ?12,
            duration_ms = ?13, cover_path = ?14, confidence = ?15, provenance = ?16, updated_at = ?17
         WHERE id = ?1",
        params![
            book_id,
            library.0,
            m.title,
            sort_key(&m.title),
            m.subtitle,
            m.description,
            m.year,
            m.genre,
            m.publisher,
            m.language,
            series_id,
            m.series_index,
            duration,
            book.cover_path,
            book.confidence,
            provenance,
            now
        ],
    )?;

    tx.execute(
        "DELETE FROM book_contributors WHERE book_id = ?1",
        [book_id],
    )?;
    for (role, names) in [("author", &m.authors), ("narrator", &m.narrators)] {
        for (pos, name) in names.iter().enumerate() {
            tx.execute(
                "INSERT INTO contributors (name, name_sort) VALUES (?1, ?2) ON CONFLICT(name) DO NOTHING",
                params![name, sort_key(name)],
            )?;
            let cid: i64 =
                tx.query_row("SELECT id FROM contributors WHERE name = ?1", [name], |r| {
                    r.get(0)
                })?;
            tx.execute(
                "INSERT OR IGNORE INTO book_contributors (book_id, contributor_id, role, position) VALUES (?1, ?2, ?3, ?4)",
                params![book_id, cid, role, pos as i64],
            )?;
        }
    }

    for (pos, t) in book.tracks.iter().enumerate() {
        let bytes = paths::to_bytes(&t.path);
        let display = paths::display(&t.path);
        let pos = pos as i64;
        match t.existing {
            Some(id) => {
                tx.execute(
                    "UPDATE tracks SET book_id = ?2, position = ?3, path = ?4, display_path = ?5, size = ?6,
                        mtime_ms = ?7, partial_hash = COALESCE(?8, partial_hash), duration_ms = ?9, disc = ?10,
                        number = ?11, title = ?12, missing = 0
                     WHERE id = ?1",
                    params![
                        id.0, book_id, pos, bytes, display, t.size, t.mtime_ms, t.partial_hash, t.duration_ms,
                        t.disc, t.number, t.title
                    ],
                )?;
            }
            None => {
                tx.execute(
                    "INSERT INTO tracks (book_id, position, path, display_path, size, mtime_ms, partial_hash,
                                         duration_ms, disc, number, title)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                     ON CONFLICT(path) DO UPDATE SET book_id = excluded.book_id, position = excluded.position,
                        size = excluded.size, mtime_ms = excluded.mtime_ms, partial_hash = excluded.partial_hash,
                        duration_ms = excluded.duration_ms, disc = excluded.disc, number = excluded.number,
                        title = excluded.title, missing = 0",
                    params![
                        book_id, pos, bytes, display, t.size, t.mtime_ms, t.partial_hash, t.duration_ms, t.disc,
                        t.number, t.title
                    ],
                )?;
            }
        }
    }

    tx.execute("DELETE FROM chapters WHERE book_id = ?1", [book_id])?;
    for (pos, c) in book.chapters.iter().enumerate() {
        tx.execute(
            "INSERT INTO chapters (book_id, position, title, start_ms, source) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![book_id, pos as i64, c.title, c.start_ms, if c.embedded { "embedded" } else { "tracks" }],
        )?;
    }

    if book.review_reasons.is_empty() {
        tx.execute("DELETE FROM review_queue WHERE book_id = ?1", [book_id])?;
    } else {
        let reasons = serde_json::to_string(&book.review_reasons)
            .map_err(|e| StorageError::Invalid(e.to_string()))?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT reasons FROM review_queue WHERE book_id = ?1",
                [book_id],
                |r| r.get(0),
            )
            .optional()?;
        match existing {
            // Unchanged reasons keep the user's "dismissed" state.
            Some(old) if old == reasons => {}
            Some(_) => {
                tx.execute(
                    "UPDATE review_queue SET reasons = ?2, dismissed_at = NULL WHERE book_id = ?1",
                    params![book_id, reasons],
                )?;
            }
            None => {
                tx.execute(
                    "INSERT INTO review_queue (book_id, reasons, created_at) VALUES (?1, ?2, ?3)",
                    params![book_id, reasons, now],
                )?;
            }
        }
    }
    Ok(())
}

fn cleanup_orphans(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM contributors WHERE NOT EXISTS (SELECT 1 FROM book_contributors WHERE contributor_id = contributors.id)",
        [],
    )?;
    conn.execute(
        "DELETE FROM series WHERE NOT EXISTS (SELECT 1 FROM books WHERE series_id = series.id)",
        [],
    )?;
    Ok(())
}

/// A book is available when its library is reachable and it has at least one
/// track whose file was found.
fn refresh_availability(conn: &Connection) -> Result<()> {
    conn.execute(
        "UPDATE books SET available = CASE
            WHEN library_id IS NOT NULL
             AND (SELECT available FROM libraries WHERE id = books.library_id) = 1
             AND EXISTS (SELECT 1 FROM tracks WHERE book_id = books.id AND missing = 0)
            THEN 1 ELSE 0 END",
        [],
    )?;
    Ok(())
}
