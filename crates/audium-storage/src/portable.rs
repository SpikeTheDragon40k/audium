//! JSON export and import of everything the user added: progress, finished
//! state, bookmarks, listening sessions, metadata overrides, per-book speed,
//! collections and settings.
//!
//! Books are identified by a fingerprint (normalized title and author, total
//! duration, partial hashes of the tracks), never by path, so an export can be
//! imported after files were moved or on another computer. An import is
//! first planned (`plan_import`), shown to the user, then applied in one
//! transaction (`apply_import`) with exactly the planned decisions.

use std::collections::{BTreeMap, HashMap, HashSet};

use audium_core::text::match_key;
use audium_core::{BookId, TrackId};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{Database, Result, StorageError, organize, playback};

pub const FORMAT: &str = "audium-export";
pub const SCHEMA_VERSION: u32 = 1;
/// Files bigger than this are refused.
pub const MAX_IMPORT_BYTES: usize = 200 * 1024 * 1024;
/// Settings that describe this installation, never imported.
const LOCAL_SETTINGS: &[&str] = &["scan_rules_version"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportDoc {
    pub format: String,
    pub schema_version: u32,
    pub exported_at: i64,
    pub app_version: String,
    pub books: Vec<ExportBook>,
    #[serde(default)]
    pub collections: Vec<ExportCollection>,
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    /// `match_key` of the title.
    pub title: String,
    /// `match_key` of the first author ("" if none).
    pub author: String,
    pub duration_ms: i64,
    /// Hex partial hashes of the tracks, in order (missing ones skipped).
    #[serde(default)]
    pub hashes: Vec<String>,
}

/// A track position: index in the book, plus its hash when known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackRef {
    pub index: usize,
    #[serde(default)]
    pub hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportBook {
    pub fingerprint: Fingerprint,
    /// For display in the import preview.
    pub title: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub progress: Option<ExportProgress>,
    #[serde(default)]
    pub speed: Option<f64>,
    #[serde(default)]
    pub overrides: Vec<ExportOverride>,
    #[serde(default)]
    pub bookmarks: Vec<ExportBookmark>,
    #[serde(default)]
    pub sessions: Vec<ExportSession>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportProgress {
    pub track: Option<TrackRef>,
    pub offset_ms: i64,
    pub book_position_ms: i64,
    pub started_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportOverride {
    pub field: String,
    pub value: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBookmark {
    pub track: Option<TrackRef>,
    pub offset_ms: i64,
    pub book_position_ms: i64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportSession {
    pub started_at: i64,
    pub ended_at: i64,
    pub start_position_ms: i64,
    pub end_position_ms: i64,
    pub listened_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportCollection {
    pub name: String,
    pub created_at: i64,
    /// Indexes into `books`, in collection order.
    pub books: Vec<usize>,
}

// ---------------------------------------------------------------------------
// Library side

struct LibTrack {
    id: TrackId,
    hash: Option<String>,
    duration_ms: i64,
}

struct LibBook {
    id: BookId,
    title: String,
    fingerprint: Fingerprint,
    tracks: Vec<LibTrack>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn library_books(conn: &Connection) -> Result<Vec<LibBook>> {
    let mut books: Vec<LibBook> = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT b.id, b.title,
                (SELECT c.name FROM book_contributors bc JOIN contributors c ON c.id = bc.contributor_id
                  WHERE bc.book_id = b.id AND bc.role = 'author' ORDER BY bc.position LIMIT 1)
         FROM books b ORDER BY b.id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            BookId(r.get(0)?),
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    })?;
    for row in rows {
        let (id, title, author) = row?;
        let mut t = conn.prepare("SELECT id, partial_hash, duration_ms FROM tracks WHERE book_id = ?1 ORDER BY position, id")?;
        let tracks: Vec<LibTrack> = t
            .query_map([id.0], |r| {
                Ok(LibTrack {
                    id: TrackId(r.get(0)?),
                    hash: r.get::<_, Option<Vec<u8>>>(1)?.map(|h| hex(&h)),
                    duration_ms: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        books.push(LibBook {
            id,
            fingerprint: Fingerprint {
                title: match_key(&title),
                author: author.as_deref().map(match_key).unwrap_or_default(),
                duration_ms: tracks.iter().map(|t| t.duration_ms.max(0)).sum(),
                hashes: tracks.iter().filter_map(|t| t.hash.clone()).collect(),
            },
            title,
            tracks,
        });
    }
    Ok(books)
}

fn track_ref(book: &LibBook, track: Option<TrackId>) -> Option<TrackRef> {
    let track = track?;
    let index = book.tracks.iter().position(|t| t.id == track)?;
    Some(TrackRef {
        index,
        hash: book.tracks[index].hash.clone(),
    })
}

/// The library track a reference points at: by hash first, then by index.
fn resolve_track(book: &LibBook, r: Option<&TrackRef>) -> Option<TrackId> {
    let r = r?;
    if let Some(h) = &r.hash
        && let Some(t) = book.tracks.iter().find(|t| t.hash.as_ref() == Some(h))
    {
        return Some(t.id);
    }
    book.tracks.get(r.index).map(|t| t.id)
}

// ---------------------------------------------------------------------------
// Export

pub fn export(conn: &Connection, app_version: &str, now: i64) -> Result<ExportDoc> {
    let library = library_books(conn)?;
    let mut books = Vec::new();
    let mut index_of: HashMap<BookId, usize> = HashMap::new();
    for b in &library {
        let progress = conn
            .query_row(
                "SELECT track_id, offset_ms, book_position_ms, started_at, updated_at, finished_at FROM progress WHERE book_id = ?1",
                [b.id.0],
                |r| {
                    Ok((
                        r.get::<_, Option<i64>>(0)?.map(TrackId),
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .optional()?
            .map(|(track, offset_ms, book_position_ms, started_at, updated_at, finished_at)| ExportProgress {
                track: track_ref(b, track),
                offset_ms,
                book_position_ms,
                started_at,
                updated_at,
                finished_at,
            });
        let mut ov = conn.prepare(
            "SELECT field, value, updated_at FROM overrides WHERE book_id = ?1 ORDER BY field",
        )?;
        let overrides: Vec<ExportOverride> = ov
            .query_map([b.id.0], |r| {
                Ok(ExportOverride {
                    field: r.get(0)?,
                    value: r.get(1)?,
                    updated_at: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let bookmarks: Vec<ExportBookmark> = organize::bookmarks(conn, b.id)?
            .into_iter()
            .map(|m| ExportBookmark {
                track: track_ref(b, m.track),
                offset_ms: m.offset_ms,
                book_position_ms: m.book_position_ms,
                title: m.title,
                note: m.note,
                created_at: m.created_at,
                updated_at: m.updated_at,
            })
            .collect();
        let mut se = conn.prepare(
            "SELECT started_at, ended_at, start_position_ms, end_position_ms, listened_ms FROM sessions WHERE book_id = ?1 ORDER BY started_at",
        )?;
        let sessions: Vec<ExportSession> = se
            .query_map([b.id.0], |r| {
                Ok(ExportSession {
                    started_at: r.get(0)?,
                    ended_at: r.get(1)?,
                    start_position_ms: r.get(2)?,
                    end_position_ms: r.get(3)?,
                    listened_ms: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let speed = playback::book_speed(conn, b.id)?;
        let in_collection: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM collection_books WHERE book_id = ?1)",
            [b.id.0],
            |r| r.get(0),
        )?;
        if progress.is_none()
            && overrides.is_empty()
            && bookmarks.is_empty()
            && sessions.is_empty()
            && speed.is_none()
            && !in_collection
        {
            continue;
        }
        let authors = crate::read::book_links(conn, b.id)?
            .authors
            .into_iter()
            .map(|(_, n)| n)
            .collect();
        index_of.insert(b.id, books.len());
        books.push(ExportBook {
            fingerprint: b.fingerprint.clone(),
            title: b.title.clone(),
            authors,
            progress,
            speed,
            overrides,
            bookmarks,
            sessions,
        });
    }

    let mut collections = Vec::new();
    let mut cs =
        conn.prepare("SELECT id, name, created_at FROM collections ORDER BY lower(name)")?;
    let rows: Vec<(i64, String, i64)> = cs
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (id, name, created_at) in rows {
        let mut m = conn.prepare(
            "SELECT book_id FROM collection_books WHERE collection_id = ?1 ORDER BY position",
        )?;
        let members: Vec<usize> = m
            .query_map([id], |r| r.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .filter_map(|b| index_of.get(&BookId(b)).copied())
            .collect();
        collections.push(ExportCollection {
            name,
            created_at,
            books: members,
        });
    }

    let mut st = conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
    let settings: BTreeMap<String, String> = st
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .filter_map(|r| r.ok())
        .filter(|(k, _)| !LOCAL_SETTINGS.contains(&k.as_str()))
        .collect();

    Ok(ExportDoc {
        format: FORMAT.to_owned(),
        schema_version: SCHEMA_VERSION,
        exported_at: now,
        app_version: app_version.to_owned(),
        books,
        collections,
        settings,
    })
}

pub fn to_json(doc: &ExportDoc) -> Result<Vec<u8>> {
    serde_json::to_vec_pretty(doc).map_err(|e| StorageError::Invalid(e.to_string()))
}

/// Parses and checks an export file.
pub fn from_json(bytes: &[u8]) -> Result<ExportDoc> {
    if bytes.len() > MAX_IMPORT_BYTES {
        return Err(StorageError::Invalid(
            "the file is too big to be an Audium export".into(),
        ));
    }
    // Look at the header first, for a clear message on foreign or newer files.
    #[derive(Deserialize)]
    struct Header {
        format: Option<String>,
        schema_version: Option<u32>,
    }
    let header: Header = serde_json::from_slice(bytes)
        .map_err(|e| StorageError::Invalid(format!("not a valid export file: {e}")))?;
    if header.format.as_deref() != Some(FORMAT) {
        return Err(StorageError::Invalid(
            "this is not an Audium export file".into(),
        ));
    }
    match header.schema_version {
        Some(v) if v <= SCHEMA_VERSION && v >= 1 => {}
        Some(v) => {
            return Err(StorageError::Invalid(format!(
                "the file was made by a newer version of Audium (format {v}); this version reads format {SCHEMA_VERSION}"
            )));
        }
        None => {
            return Err(StorageError::Invalid(
                "the file has no format version".into(),
            ));
        }
    }
    serde_json::from_slice(bytes)
        .map_err(|e| StorageError::Invalid(format!("the export file is damaged: {e}")))
}

// ---------------------------------------------------------------------------
// Import

/// Durations of the same book differ slightly between encoders and probes.
fn durations_match(a: i64, b: i64) -> bool {
    let tolerance = (a.max(b) / 100).max(5_000);
    (a - b).abs() <= tolerance
}

/// The library book an exported book is: a shared track hash wins; else the
/// same title and author with a matching duration (closest wins).
fn find_match(
    fp: &Fingerprint,
    library: &[LibBook],
    by_hash: &HashMap<&str, usize>,
) -> Option<usize> {
    let mut counts: HashMap<usize, usize> = HashMap::new();
    for h in &fp.hashes {
        if let Some(&i) = by_hash.get(h.as_str()) {
            *counts.entry(i).or_default() += 1;
        }
    }
    if let Some((&i, _)) = counts
        .iter()
        .max_by_key(|(i, n)| (**n, std::cmp::Reverse(**i)))
    {
        return Some(i);
    }
    library
        .iter()
        .enumerate()
        .filter(|(_, b)| {
            b.fingerprint.title == fp.title
                && b.fingerprint.author == fp.author
                && !fp.title.is_empty()
        })
        .filter(|(_, b)| durations_match(b.fingerprint.duration_ms, fp.duration_ms))
        .min_by_key(|(_, b)| (b.fingerprint.duration_ms - fp.duration_ms).abs())
        .map(|(i, _)| i)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub books_in_file: usize,
    pub matched: usize,
    /// Titles of books in the file that are not in this library (skipped).
    pub unmatched: Vec<String>,
    pub progress_updated: usize,
    /// The library's progress is newer and is kept.
    pub progress_kept: usize,
    pub bookmarks_added: usize,
    pub sessions_added: usize,
    pub overrides_updated: usize,
    /// The library's edit is newer and is kept.
    pub overrides_kept: usize,
    pub speeds_set: usize,
    pub collections_created: usize,
    pub collection_entries_added: usize,
    pub settings_changed: usize,
}

impl ImportSummary {
    pub fn changes(&self) -> usize {
        self.progress_updated
            + self.bookmarks_added
            + self.sessions_added
            + self.overrides_updated
            + self.speeds_set
            + self.collections_created
            + self.collection_entries_added
            + self.settings_changed
    }
}

/// What an import will do, decided once and shown before applying.
#[derive(Debug, Clone)]
pub struct ImportPlan {
    pub summary: ImportSummary,
    writes: Vec<Write>,
    /// Books whose metadata overrides change (they need re-reading).
    pub books_with_new_overrides: Vec<BookId>,
}

#[derive(Debug, Clone)]
enum Write {
    Progress {
        book: BookId,
        track: Option<TrackId>,
        p: ExportProgress,
    },
    Bookmark {
        book: BookId,
        track: Option<TrackId>,
        b: ExportBookmark,
    },
    Session {
        book: BookId,
        s: ExportSession,
    },
    Override {
        book: BookId,
        o: ExportOverride,
    },
    Speed {
        book: BookId,
        speed: f64,
    },
    Collection {
        name: String,
        created_at: i64,
        books: Vec<BookId>,
    },
    Setting {
        key: String,
        value: String,
    },
}

pub fn plan_import(conn: &Connection, doc: &ExportDoc) -> Result<ImportPlan> {
    let library = library_books(conn)?;
    let mut by_hash: HashMap<&str, usize> = HashMap::new();
    for (i, b) in library.iter().enumerate() {
        for h in &b.fingerprint.hashes {
            by_hash.entry(h.as_str()).or_insert(i);
        }
    }
    let mut summary = ImportSummary {
        books_in_file: doc.books.len(),
        ..ImportSummary::default()
    };
    let mut writes = Vec::new();
    let mut overridden = Vec::new();
    let mut matched_ids: Vec<Option<BookId>> = Vec::with_capacity(doc.books.len());

    for eb in &doc.books {
        let Some(li) = find_match(&eb.fingerprint, &library, &by_hash) else {
            summary.unmatched.push(eb.title.clone());
            matched_ids.push(None);
            continue;
        };
        let lb = &library[li];
        summary.matched += 1;
        matched_ids.push(Some(lb.id));

        if let Some(p) = &eb.progress {
            let current = playback::progress(conn, lb.id)?;
            if current.is_none_or(|c| p.updated_at > c.updated_at) {
                summary.progress_updated += 1;
                writes.push(Write::Progress {
                    book: lb.id,
                    track: resolve_track(lb, p.track.as_ref()),
                    p: p.clone(),
                });
            } else if current.is_some_and(|c| p.updated_at < c.updated_at) {
                summary.progress_kept += 1;
            }
        }

        let existing = organize::bookmarks(conn, lb.id)?;
        for b in &eb.bookmarks {
            let dup = existing.iter().any(|x| {
                (x.book_position_ms - b.book_position_ms).abs() <= 1_000 && x.title == b.title
            });
            if !dup {
                summary.bookmarks_added += 1;
                writes.push(Write::Bookmark {
                    book: lb.id,
                    track: resolve_track(lb, b.track.as_ref()),
                    b: b.clone(),
                });
            }
        }

        let mut st = conn.prepare("SELECT started_at FROM sessions WHERE book_id = ?1")?;
        let known: HashSet<i64> = st
            .query_map([lb.id.0], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        for s in &eb.sessions {
            if !known.contains(&s.started_at) && s.listened_ms >= 0 && s.ended_at >= s.started_at {
                summary.sessions_added += 1;
                writes.push(Write::Session { book: lb.id, s: *s });
            }
        }

        let mut ov =
            conn.prepare("SELECT field, value, updated_at FROM overrides WHERE book_id = ?1")?;
        let current: HashMap<String, (String, i64)> = ov
            .query_map([lb.id.0], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    (r.get::<_, String>(1)?, r.get::<_, i64>(2)?),
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        let mut changed = false;
        for o in &eb.overrides {
            if !organize::OVERRIDE_FIELDS.contains(&o.field.as_str()) {
                continue;
            }
            match current.get(&o.field) {
                Some((value, _)) if *value == o.value => {}
                Some((_, at)) if *at >= o.updated_at => summary.overrides_kept += 1,
                _ => {
                    summary.overrides_updated += 1;
                    changed = true;
                    writes.push(Write::Override {
                        book: lb.id,
                        o: o.clone(),
                    });
                }
            }
        }
        if changed {
            overridden.push(lb.id);
        }

        if let Some(speed) = eb
            .speed
            .filter(|s| s.is_finite() && (0.5..=3.0).contains(s))
            && playback::book_speed(conn, lb.id)?.is_none()
        {
            summary.speeds_set += 1;
            writes.push(Write::Speed { book: lb.id, speed });
        }
    }

    for c in &doc.collections {
        let Some(name) = audium_core::text::clean(&c.name, 200) else {
            continue;
        };
        let id: Option<i64> = conn
            .query_row("SELECT id FROM collections WHERE name = ?1", [&name], |r| {
                r.get(0)
            })
            .optional()?;
        if id.is_none() {
            summary.collections_created += 1;
        }
        let books: Vec<BookId> = c
            .books
            .iter()
            .filter_map(|&i| matched_ids.get(i).copied().flatten())
            .collect();
        for b in &books {
            let present = match id {
                Some(id) => conn.query_row(
                    "SELECT EXISTS (SELECT 1 FROM collection_books WHERE collection_id = ?1 AND book_id = ?2)",
                    params![id, b.0],
                    |r| r.get::<_, bool>(0),
                )?,
                None => false,
            };
            if !present {
                summary.collection_entries_added += 1;
            }
        }
        writes.push(Write::Collection {
            name,
            created_at: c.created_at,
            books,
        });
    }

    for (key, value) in &doc.settings {
        if LOCAL_SETTINGS.contains(&key.as_str()) {
            continue;
        }
        if playback::setting(conn, key)?.as_deref() != Some(value.as_str()) {
            summary.settings_changed += 1;
            writes.push(Write::Setting {
                key: key.clone(),
                value: value.clone(),
            });
        }
    }

    Ok(ImportPlan {
        summary,
        writes,
        books_with_new_overrides: overridden,
    })
}

/// Applies a plan in one transaction. Nothing is written if anything fails.
pub fn apply_import(db: &mut Database, plan: &ImportPlan) -> Result<()> {
    let tx = db.conn_mut().transaction()?;
    for w in &plan.writes {
        match w {
            Write::Progress { book, track, p } => {
                tx.execute(
                    "INSERT INTO progress (book_id, track_id, offset_ms, book_position_ms, started_at, updated_at, finished_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT(book_id) DO UPDATE SET track_id = excluded.track_id, offset_ms = excluded.offset_ms,
                        book_position_ms = excluded.book_position_ms, updated_at = excluded.updated_at,
                        finished_at = excluded.finished_at,
                        started_at = min(progress.started_at, excluded.started_at)",
                    params![book.0, track.map(|t| t.0), p.offset_ms, p.book_position_ms, p.started_at, p.updated_at, p.finished_at],
                )?;
            }
            Write::Bookmark { book, track, b } => {
                let title = b
                    .title
                    .as_deref()
                    .and_then(|t| audium_core::text::clean(t, 500));
                let note = b
                    .note
                    .as_deref()
                    .and_then(|n| audium_core::text::clean_multiline(n, 20_000));
                tx.execute(
                    "INSERT INTO bookmarks (book_id, track_id, offset_ms, book_position_ms, title, note, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![book.0, track.map(|t| t.0), b.offset_ms, b.book_position_ms, title, note, b.created_at, b.updated_at],
                )?;
            }
            Write::Session { book, s } => {
                tx.execute(
                    "INSERT INTO sessions (book_id, started_at, ended_at, start_position_ms, end_position_ms, listened_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![book.0, s.started_at, s.ended_at, s.start_position_ms, s.end_position_ms, s.listened_ms],
                )?;
            }
            Write::Override { book, o } => {
                organize::set_override(&tx, *book, &o.field, &o.value, o.updated_at)?;
            }
            Write::Speed { book, speed } => playback::set_book_speed(&tx, *book, *speed)?,
            Write::Collection {
                name,
                created_at,
                books,
            } => {
                let id = organize::create_collection(&tx, name, *created_at)?;
                for b in books {
                    organize::add_to_collection(&tx, id, *b)?;
                }
            }
            Write::Setting { key, value } => playback::set_setting(&tx, key, value)?,
        }
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_tolerance() {
        assert!(
            durations_match(3_600_000, 3_600_000 + 30_000),
            "1% of an hour"
        );
        assert!(!durations_match(3_600_000, 3_600_000 + 40_000));
        assert!(durations_match(60_000, 64_000), "at least 5 s");
        assert!(!durations_match(60_000, 66_000));
    }

    #[test]
    fn rejects_foreign_newer_and_damaged_files() {
        assert!(from_json(b"not json").is_err());
        assert!(from_json(br#"{"format":"something-else","schema_version":1}"#).is_err());
        let newer = from_json(br#"{"format":"audium-export","schema_version":99,"books":[]}"#);
        assert!(matches!(newer, Err(StorageError::Invalid(m)) if m.contains("newer")));
        assert!(
            from_json(br#"{"format":"audium-export","schema_version":1,"books":"nope"}"#).is_err()
        );
        let ok = from_json(br#"{"format":"audium-export","schema_version":1,"exported_at":0,"app_version":"x","books":[]}"#);
        assert!(ok.is_ok());
    }
}
