//! Scans one library root and reconciles it with the database.
//!
//! - Folders whose files all have the stored path, size and mtime are left
//!   as they are (no file is opened).
//! - Changed folders are re-read and regrouped. Each file keeps its stored
//!   track when the path matches, or when a missing track has the same size
//!   and partial hash (a move or rename), so progress and bookmarks follow.
//! - Files not found are marked missing; books are never deleted here.
//! - An unreachable root, or a root that suddenly has no audio files while
//!   the database knows some (an unmounted drive), marks the library
//!   unavailable and changes nothing else.
//!
//! All changes are applied in one transaction at the end, so a cancelled or
//! failed scan leaves the database as it was.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use audium_core::text::clean;
use audium_core::{BookId, BookUid, TrackId};
use audium_metadata::cover::read_cover_file;
use audium_metadata::heuristics::{PathGuess, guess_from_filename, guess_from_folders};
use audium_metadata::{
    BookInputs, Cover, FileInfo, Overrides, Sidecars, read_embedded_cover, read_sidecars, resolve,
};
use audium_storage::write::{
    self, ApplyReport, BookMetadata, BookTarget, BookWrite, ChapterWrite, ExistingTrack, Library,
    ScanChanges, Snapshot, TrackWrite,
};
use audium_storage::{Database, StorageError};

use crate::confidence::assess;
use crate::discover::FoundFile;
use crate::discover::{DirGroup, DiscoverError, discover};
use crate::grouping::{Candidate, group_folder, locked_candidate};
use crate::hash::partial_hash;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Covers are written to `<cache_dir>/covers/`.
    pub cache_dir: PathBuf,
    pub threads: usize,
    /// Re-read every folder, even unchanged ones.
    pub force: bool,
    /// Re-read these folders even if unchanged (e.g. after editing a book's
    /// metadata). A folder matches if it is a group's folder or holds files.
    pub force_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Discovering,
    Reading,
    Saving,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanProgress {
    pub phase: Phase,
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanSummary {
    pub available: bool,
    pub folders: usize,
    pub folders_unchanged: usize,
    pub files_read: usize,
    pub unreadable_files: usize,
    pub moved_files: usize,
    pub report: ApplyReport,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("scan cancelled")]
    Cancelled,
    #[error(transparent)]
    Storage(#[from] StorageError),
}

pub fn scan_library(
    db: &mut Database,
    library: &Library,
    opts: &ScanOptions,
    cancel: &AtomicBool,
    progress: &(dyn Fn(ScanProgress) + Sync),
) -> Result<ScanSummary, ScanError> {
    progress(ScanProgress {
        phase: Phase::Discovering,
        done: 0,
        total: 0,
    });
    let snapshot = write::snapshot(db.conn())?;
    let in_library = |t: &&ExistingTrack| t.library_id == Some(library.id);

    let groups = match discover(&library.path, cancel) {
        Ok(groups) => groups,
        Err(DiscoverError::Cancelled) => return Err(ScanError::Cancelled),
        Err(DiscoverError::Unavailable(path)) => {
            tracing::warn!(%path, "library root unavailable");
            write::set_library_available(db, library.id, false)?;
            return Ok(ScanSummary::default());
        }
    };
    if groups.is_empty()
        && snapshot
            .tracks
            .iter()
            .filter(in_library)
            .any(|t| !t.missing)
    {
        tracing::warn!(path = %library.path.display(), "library root has no audio files; treating it as unmounted");
        write::set_library_available(db, library.id, false)?;
        return Ok(ScanSummary::default());
    }

    let by_path: HashMap<&Path, &ExistingTrack> = snapshot
        .tracks
        .iter()
        .map(|t| (t.path.as_path(), t))
        .collect();
    let mut tracks_of_book: HashMap<BookId, Vec<&ExistingTrack>> = HashMap::new();
    for t in &snapshot.tracks {
        tracks_of_book.entry(t.book_id).or_default().push(t);
    }

    let mut summary = ScanSummary {
        available: true,
        folders: groups.len(),
        ..ScanSummary::default()
    };
    let mut changes = ScanChanges::default();
    let mut changed = Vec::new();
    for group in groups {
        let forced = opts.force
            || opts.force_dirs.iter().any(|d| {
                *d == group.dir
                    || group
                        .files
                        .iter()
                        .any(|f| f.path.parent() == Some(d.as_path()))
            });
        match unchanged_tracks(&group, &by_path, &tracks_of_book).filter(|_| !forced) {
            Some(ids) => {
                summary.folders_unchanged += 1;
                changes.present_tracks.extend(ids);
            }
            None => changed.push(group),
        }
    }

    // Read every file of the changed folders.
    let jobs: Vec<(PathBuf, bool)> = changed
        .iter()
        .flat_map(|g| g.files.iter())
        .map(|f| {
            let same = by_path.get(f.path.as_path()).is_some_and(|t| {
                t.size == f.size && t.mtime_ms == f.mtime_ms && t.partial_hash.is_some()
            });
            (f.path.clone(), !same)
        })
        .collect();
    let mut results = read_all(&jobs, opts.threads.max(1), cancel, progress)?.into_iter();
    summary.files_read = jobs.len();

    let scanned: HashSet<PathBuf> = changed
        .iter()
        .flat_map(|g| g.files.iter().map(|f| f.path.clone()))
        .collect();
    let present: HashSet<TrackId> = changes.present_tracks.iter().copied().collect();
    // Stored tracks whose file is gone: candidates for a move.
    let mut moved_from: HashMap<(i64, Vec<u8>), Vec<TrackId>> = HashMap::new();
    for t in &snapshot.tracks {
        let gone = if t.library_id == Some(library.id) {
            !scanned.contains(&t.path) && !present.contains(&t.id)
        } else {
            !t.path.exists()
        };
        if let (true, Some(hash)) = (gone, &t.partial_hash) {
            moved_from
                .entry((t.size, hash.clone()))
                .or_default()
                .push(t.id);
        }
    }
    let book_of: HashMap<TrackId, BookId> =
        snapshot.tracks.iter().map(|t| (t.id, t.book_id)).collect();

    let mut ctx = Ctx {
        snapshot: &snapshot,
        by_path: &by_path,
        book_of: &book_of,
        opts,
        moved_from,
        reused: HashSet::new(),
        claimed: HashSet::new(),
        uids: UidSource::new(&library.path),
        moved_files: 0,
    };
    let locked_of = |path: &Path| {
        by_path
            .get(path)
            .map(|t| t.book_id)
            .filter(|b| snapshot.books.get(b).is_some_and(|x| x.grouping_locked))
    };
    // Files of books the user merged or split, collected across folders.
    let mut locked: BTreeMap<BookId, (Vec<FoundFile>, Vec<FileRead>)> = BTreeMap::new();

    for group in &changed {
        let per_file: Vec<FileRead> = (0..group.files.len())
            .filter_map(|_| results.next())
            .collect();
        summary.unreadable_files += per_file.iter().filter(|(i, _)| i.is_none()).count();

        // Locked books keep their files; the rest is grouped as usual.
        let mut free = DirGroup {
            dir: group.dir.clone(),
            rel_dir: group.rel_dir.clone(),
            files: Vec::new(),
        };
        let mut free_reads = Vec::new();
        let mut has_locked = false;
        for (file, read) in group.files.iter().zip(per_file) {
            match locked_of(&file.path) {
                Some(book) => {
                    has_locked = true;
                    let entry = locked.entry(book).or_default();
                    entry.0.push(file.clone());
                    entry.1.push(read);
                }
                None => {
                    free.files.push(file.clone());
                    free_reads.push(read);
                }
            }
        }
        let infos: Vec<Option<FileInfo>> = free_reads.iter().map(|(i, _)| i.clone()).collect();
        let unreadable = infos.iter().any(Option::is_none);
        let candidates = group_folder(&free, &infos);
        // Sidecars describe a folder's only book.
        let sidecars = if candidates.len() == 1 && !has_locked {
            read_sidecars(&group.dir)
        } else {
            Sidecars::default()
        };
        for candidate in &candidates {
            changes.books.push(ctx.emit(
                &free,
                candidate,
                &free_reads,
                &sidecars,
                unreadable,
                None,
            ));
        }
    }

    for (book, (files, reads)) in locked {
        let dir = files[0]
            .path
            .parent()
            .map(Path::to_owned)
            .unwrap_or_default();
        let group = DirGroup {
            rel_dir: dir
                .strip_prefix(&library.path)
                .map(Path::to_owned)
                .unwrap_or_default(),
            dir,
            files,
        };
        let infos: Vec<Option<FileInfo>> = reads.iter().map(|(i, _)| i.clone()).collect();
        let Some(candidate) = locked_candidate(&group, &infos) else {
            continue; // nothing readable left: its tracks are kept as they are
        };
        let unreadable = infos.iter().any(Option::is_none);
        changes.books.push(ctx.emit(
            &group,
            &candidate,
            &reads,
            &Sidecars::default(),
            unreadable,
            Some(book),
        ));
    }
    summary.moved_files = ctx.moved_files;
    let reused = ctx.reused;

    changes.missing_tracks = snapshot
        .tracks
        .iter()
        .filter(in_library)
        .filter(|t| {
            !scanned.contains(&t.path) && !present.contains(&t.id) && !reused.contains(&t.id)
        })
        .map(|t| t.id)
        .collect();

    if cancel.load(Ordering::Relaxed) {
        return Err(ScanError::Cancelled);
    }
    progress(ScanProgress {
        phase: Phase::Saving,
        done: 0,
        total: changes.books.len(),
    });
    summary.report = write::apply(db, library.id, &changes)?;
    tracing::info!(?summary, "library scanned");
    Ok(summary)
}

/// State shared while turning candidates into book writes.
struct Ctx<'a> {
    snapshot: &'a Snapshot,
    by_path: &'a HashMap<&'a Path, &'a ExistingTrack>,
    book_of: &'a HashMap<TrackId, BookId>,
    opts: &'a ScanOptions,
    moved_from: HashMap<(i64, Vec<u8>), Vec<TrackId>>,
    reused: HashSet<TrackId>,
    claimed: HashSet<BookId>,
    uids: UidSource,
    moved_files: usize,
}

impl Ctx<'_> {
    /// One candidate book -> a book write: matches its files to stored
    /// tracks (by path, or as moved files), picks the stored book it
    /// continues (or `locked_target`), resolves metadata and caches a cover.
    fn emit(
        &mut self,
        group: &DirGroup,
        candidate: &Candidate,
        reads: &[FileRead],
        sidecars: &Sidecars,
        unreadable: bool,
        locked_target: Option<BookId>,
    ) -> BookWrite {
        let infos: Vec<Option<FileInfo>> = reads.iter().map(|(i, _)| i.clone()).collect();
        let mut tracks = Vec::with_capacity(candidate.files.len());
        for &i in &candidate.files {
            let file = &group.files[i];
            let info = infos[i].as_ref();
            let stored = self.by_path.get(file.path.as_path()).copied();
            let hash = reads[i]
                .1
                .clone()
                .or_else(|| stored.and_then(|t| t.partial_hash.clone()));
            let existing = stored.map(|t| t.id).or_else(|| {
                let ids = self.moved_from.get_mut(&(file.size, hash.clone()?))?;
                let id = ids.pop()?;
                self.moved_files += 1;
                Some(id)
            });
            if let Some(id) = existing {
                self.reused.insert(id);
            }
            tracks.push(TrackWrite {
                existing,
                path: file.path.clone(),
                size: file.size,
                mtime_ms: file.mtime_ms,
                partial_hash: hash,
                duration_ms: info.map_or(0, |f| i64::try_from(f.duration_ms).unwrap_or(i64::MAX)),
                disc: info.and_then(|f| f.tags.disc).or(file.disc_hint),
                number: info.and_then(|f| f.tags.track),
                title: info.and_then(|f| f.tags.title.clone()),
            });
        }

        let target = match locked_target {
            Some(book) => {
                self.claimed.insert(book);
                Some(book)
            }
            None => {
                let snapshot = self.snapshot;
                // Locked books are never taken over by other candidates.
                let available =
                    |b: BookId| snapshot.books.get(&b).is_some_and(|x| !x.grouping_locked);
                let user_data = |b: BookId| snapshot.books.get(&b).is_some_and(|x| x.has_user_data);
                let book_of: HashMap<TrackId, BookId> = tracks
                    .iter()
                    .filter_map(|t| t.existing)
                    .filter_map(|id| self.book_of.get(&id).map(|b| (id, *b)))
                    .filter(|(_, b)| available(*b))
                    .collect();
                pick_target(&tracks, &book_of, &user_data, &mut self.claimed)
            }
        };
        let overrides = target
            .and_then(|id| self.snapshot.books.get(&id))
            .map(|b| parse_overrides(&b.overrides))
            .unwrap_or_default();
        let target = match target {
            Some(id) => BookTarget::Existing(id),
            None => BookTarget::New(self.uids.next()),
        };
        let uid = match &target {
            BookTarget::Existing(id) => self.snapshot.books.get(id).map(|b| b.uid.clone()),
            BookTarget::New(uid) => Some(uid.clone()),
        };
        let book = build_book(
            group, candidate, &infos, sidecars, &overrides, unreadable, target, tracks,
        );
        let cover_path = uid.and_then(|uid| {
            let cover = sidecars
                .cover
                .as_deref()
                .and_then(read_cover_file)
                .or_else(|| {
                    candidate
                        .files
                        .iter()
                        .take(3)
                        .find_map(|&i| read_embedded_cover(&group.files[i].path))
                })?;
            store_cover(&self.opts.cache_dir, &uid, &cover)
        });
        BookWrite { cover_path, ..book }
    }
}

/// The stored tracks of a folder if nothing in it changed: same files with
/// the same size and mtime, and its books have no tracks elsewhere.
fn unchanged_tracks(
    group: &DirGroup,
    by_path: &HashMap<&Path, &ExistingTrack>,
    tracks_of_book: &HashMap<BookId, Vec<&ExistingTrack>>,
) -> Option<Vec<TrackId>> {
    let mut ids = Vec::with_capacity(group.files.len());
    let mut books = HashSet::new();
    for f in &group.files {
        let t = by_path.get(f.path.as_path())?;
        if t.size != f.size || t.mtime_ms != f.mtime_ms {
            return None;
        }
        ids.push(t.id);
        books.insert(t.book_id);
    }
    let paths: HashSet<&Path> = group.files.iter().map(|f| f.path.as_path()).collect();
    let complete = books.iter().all(|b| {
        tracks_of_book
            .get(b)
            .is_some_and(|ts| ts.iter().all(|t| paths.contains(t.path.as_path())))
    });
    complete.then_some(ids)
}

/// Probe result and (when requested) partial hash of one file.
type FileRead = (Option<FileInfo>, Option<Vec<u8>>);

/// Probes files (and hashes those flagged) on `threads` worker threads.
fn read_all(
    jobs: &[(PathBuf, bool)],
    threads: usize,
    cancel: &AtomicBool,
    progress: &(dyn Fn(ScanProgress) + Sync),
) -> Result<Vec<FileRead>, ScanError> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let total = jobs.len();
    progress(ScanProgress {
        phase: Phase::Reading,
        done: 0,
        total,
    });

    let mut results: Vec<Option<FileRead>> = vec![None; total];
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads.min(total.max(1)))
            .map(|_| {
                scope.spawn(|| {
                    let mut local = Vec::new();
                    while !cancel.load(Ordering::Relaxed) {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some((path, need_hash)) = jobs.get(i) else {
                            break;
                        };
                        let info = audium_metadata::probe(path)
                            .inspect_err(|e| tracing::warn!(error = %e, "unreadable audio file"))
                            .ok();
                        let hash = if *need_hash {
                            partial_hash(path).ok()
                        } else {
                            None
                        };
                        local.push((i, (info, hash)));
                        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                        if n.is_multiple_of(16) || n == total {
                            progress(ScanProgress {
                                phase: Phase::Reading,
                                done: n,
                                total,
                            });
                        }
                    }
                    local
                })
            })
            .collect();
        for worker in workers {
            // A panicking worker (a bug in a parser) only loses its files.
            for (i, r) in worker.join().unwrap_or_default() {
                results[i] = Some(r);
            }
        }
    });
    if cancel.load(Ordering::Relaxed) {
        return Err(ScanError::Cancelled);
    }
    Ok(results
        .into_iter()
        .map(|r| r.unwrap_or((None, None)))
        .collect())
}

/// The stored book most of these tracks belong to, unless another candidate
/// of this scan already took it.
/// Among those, a book with user data (progress, bookmarks...) wins, so that
/// regrouping several books into one keeps the listening history attached.
fn pick_target(
    tracks: &[TrackWrite],
    book_of: &HashMap<TrackId, BookId>,
    has_user_data: &dyn Fn(BookId) -> bool,
    claimed: &mut HashSet<BookId>,
) -> Option<BookId> {
    let mut counts: HashMap<BookId, usize> = HashMap::new();
    for id in tracks.iter().filter_map(|t| t.existing) {
        if let Some(b) = book_of.get(&id) {
            *counts.entry(*b).or_default() += 1;
        }
    }
    let mut ranked: Vec<(BookId, usize)> = counts
        .into_iter()
        .filter(|(b, _)| !claimed.contains(b))
        .collect();
    ranked.sort_by(|a, b| {
        has_user_data(b.0)
            .cmp(&has_user_data(a.0))
            .then(b.1.cmp(&a.1))
            .then(a.0.cmp(&b.0))
    });
    let (book, _) = ranked.first().copied()?;
    claimed.insert(book);
    Some(book)
}

/// Overrides are stored one field per row; lists are newline-separated.
fn parse_overrides(raw: &HashMap<String, String>) -> Overrides {
    let text = |k: &str| {
        raw.get(k)
            .and_then(|v| clean(v, audium_metadata::limits::NAME))
    };
    let list = |k: &str| {
        raw.get(k).map(|v| {
            v.lines()
                .filter_map(|l| clean(l, audium_metadata::limits::NAME))
                .collect()
        })
    };
    Overrides {
        title: text("title"),
        subtitle: text("subtitle"),
        authors: list("authors"),
        narrators: list("narrators"),
        series: text("series"),
        series_index: text("series_index"),
        description: raw.get("description").and_then(|v| {
            audium_core::text::clean_multiline(v, audium_metadata::limits::DESCRIPTION)
        }),
        year: raw.get("year").and_then(|v| v.trim().parse().ok()),
        genre: text("genre"),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_book(
    group: &DirGroup,
    candidate: &Candidate,
    infos: &[Option<FileInfo>],
    sidecars: &Sidecars,
    overrides: &Overrides,
    unreadable: bool,
    target: BookTarget,
    tracks: Vec<TrackWrite>,
) -> BookWrite {
    let tags: Vec<_> = candidate
        .files
        .iter()
        .filter_map(|&i| infos[i].as_ref().map(|f| f.tags.clone()))
        .collect();
    let first_path = &group.files[candidate.files[0]].path;
    let filename = if candidate.files.len() == 1 {
        guess_from_filename(first_path)
    } else {
        PathGuess::default()
    };
    let meta = resolve(&BookInputs {
        overrides,
        sidecars,
        tracks: &tags,
        folder: guess_from_folders(&candidate.folder_guess),
        filename,
    });
    let assessment = assess(candidate, &meta, unreadable);

    let mut provenance = Vec::new();
    let mut note = |field: &str, source: Option<audium_metadata::Source>| {
        if let Some(s) = source {
            provenance.push((field.to_owned(), s.as_str().to_owned()));
        }
    };
    note("title", meta.title.as_ref().map(|f| f.source));
    note("authors", meta.authors.as_ref().map(|f| f.source));
    note("narrators", meta.narrators.as_ref().map(|f| f.source));
    note("series", meta.series.as_ref().map(|f| f.source));
    note("description", meta.description.as_ref().map(|f| f.source));

    let fallback_title = || {
        first_path
            .file_stem()
            .and_then(|s| clean(&s.to_string_lossy(), audium_metadata::limits::NAME))
            .unwrap_or_else(|| "Untitled".to_owned())
    };
    let metadata = BookMetadata {
        title: meta.title.map(|f| f.value).unwrap_or_else(fallback_title),
        subtitle: meta.subtitle.map(|f| f.value),
        authors: meta.authors.map(|f| f.value).unwrap_or_default(),
        narrators: meta.narrators.map(|f| f.value).unwrap_or_default(),
        series: meta.series.map(|f| f.value),
        series_index: meta.series_index,
        description: meta.description.map(|f| f.value),
        year: meta.year.map(|f| f.value),
        genre: meta.genre.map(|f| f.value),
        publisher: meta.publisher,
        language: meta.language,
        provenance,
    };

    let mut chapters = Vec::new();
    let mut start = 0i64;
    for (n, (&i, track)) in candidate.files.iter().zip(&tracks).enumerate() {
        let info = infos[i].as_ref();
        match info
            .map(|f| f.chapters.as_slice())
            .filter(|c| !c.is_empty())
        {
            Some(embedded) => {
                for (k, c) in embedded.iter().enumerate() {
                    chapters.push(ChapterWrite {
                        title: c
                            .title
                            .clone()
                            .unwrap_or_else(|| format!("Chapter {}", k + 1)),
                        start_ms: start + i64::try_from(c.start_ms).unwrap_or(0),
                        embedded: true,
                    });
                }
            }
            None => chapters.push(ChapterWrite {
                title: track
                    .title
                    .clone()
                    .or_else(|| {
                        group.files[i]
                            .path
                            .file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                    })
                    .unwrap_or_else(|| format!("Track {}", n + 1)),
                start_ms: start,
                embedded: false,
            }),
        }
        start += track.duration_ms.max(0);
    }

    BookWrite {
        target,
        metadata,
        cover_path: None,
        confidence: assessment.score,
        review_reasons: if assessment.needs_review {
            assessment
                .reasons
                .iter()
                .map(|r| r.code().to_owned())
                .collect()
        } else {
            vec![]
        },
        tracks,
        chapters,
    }
}

/// Writes a cover into the cache, named by book uid and content, and removes
/// the book's older cover files. Returns the cached file's path.
fn store_cover(cache_dir: &Path, uid: &BookUid, cover: &Cover) -> Option<String> {
    let dir = cache_dir.join("covers");
    std::fs::create_dir_all(&dir).ok()?;
    let digest = blake3::hash(&cover.data).to_hex();
    let name = format!("{uid}-{}.{}", &digest[..16], cover.extension);
    let path = dir.join(&name);
    if !path.exists() {
        let tmp = dir.join(format!(".{name}.tmp"));
        std::fs::write(&tmp, &cover.data).ok()?;
        std::fs::rename(&tmp, &path).ok()?;
    }
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let prefix = format!("{uid}-");
        for e in entries.flatten() {
            let n = e.file_name();
            let n = n.to_string_lossy();
            if n.starts_with(&prefix) && n != name {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    path.to_str().map(str::to_owned)
}

/// A new, unique book uid (for books the user creates, e.g. by splitting).
pub fn fresh_uid(context: &Path) -> BookUid {
    UidSource::new(context).next()
}

/// Book uids from the clock, the process id, the root and a counter.
struct UidSource {
    seed: blake3::Hasher,
    counter: u64,
}

impl UidSource {
    fn new(root: &Path) -> Self {
        let mut seed = blake3::Hasher::new();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        seed.update(&nanos.to_le_bytes());
        seed.update(&std::process::id().to_le_bytes());
        seed.update(root.as_os_str().as_encoded_bytes());
        UidSource { seed, counter: 0 }
    }

    fn next(&mut self) -> BookUid {
        self.counter += 1;
        let mut h = self.seed.clone();
        h.update(&self.counter.to_le_bytes());
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&h.finalize().as_bytes()[..16]);
        BookUid::from_random_bytes(bytes)
    }
}
