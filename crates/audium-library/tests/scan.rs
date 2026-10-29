//! End-to-end scans of copies of the generated fixtures in a temp dir.

// Test helpers may panic on setup failures.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use audium_library::{ScanError, ScanOptions, ScanSummary, scan_library};
use audium_storage::read::{self, BookFilter, BookSummary};
use audium_storage::write::{self, Library};
use audium_storage::{Database, now_ms};

fn fixtures() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/generated");
    assert!(
        dir.is_dir(),
        "fixtures missing: run scripts/make-fixtures.sh first"
    );
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("mkdir");
    for e in fs::read_dir(from).expect("read fixtures").flatten() {
        let target = to.join(e.file_name());
        if e.file_type().expect("type").is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            fs::copy(e.path(), &target).expect("copy");
        }
    }
}

struct Env {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    cache: PathBuf,
    db: Database,
}

impl Env {
    /// A library whose root contains the given fixture folders' contents.
    fn new(fixture_dirs: &[&str]) -> Env {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("library");
        fs::create_dir_all(&root).expect("root");
        for d in fixture_dirs {
            copy_dir(&fixtures().join(d), &root);
        }
        let cache = tmp.path().join("cache");
        let db = Database::open(&tmp.path().join("audium.db")).expect("db");
        write::add_library(&db, &root).expect("library");
        Env {
            _tmp: tmp,
            root,
            cache,
            db,
        }
    }

    fn library(&self) -> Library {
        write::libraries(self.db.conn())
            .expect("libraries")
            .remove(0)
    }

    fn scan(&mut self) -> ScanSummary {
        self.try_scan(&AtomicBool::new(false)).expect("scan")
    }

    fn try_scan(&mut self, cancel: &AtomicBool) -> Result<ScanSummary, ScanError> {
        let lib = self.library();
        let opts = ScanOptions {
            cache_dir: self.cache.clone(),
            threads: 4,
            force: false,
        };
        scan_library(&mut self.db, &lib, &opts, cancel, &|_| {})
    }

    fn books(&self) -> Vec<BookSummary> {
        read::list_books(self.db.conn(), BookFilter::All).expect("books")
    }

    fn book(&self, title: &str) -> BookSummary {
        self.books()
            .into_iter()
            .find(|b| b.title == title)
            .unwrap_or_else(|| panic!("no book {title:?} in {:?}", self.titles()))
    }

    fn titles(&self) -> Vec<String> {
        self.books().into_iter().map(|b| b.title).collect()
    }

    fn track_names(&self, title: &str) -> Vec<String> {
        let id = self.book(title).id;
        read::tracks(self.db.conn(), id)
            .expect("tracks")
            .into_iter()
            .map(|t| {
                t.path
                    .strip_prefix(&self.root)
                    .expect("in root")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    fn add_progress(&self, title: &str) {
        let id = self.book(title).id;
        self.db
            .conn()
            .execute(
                "INSERT INTO progress (book_id, track_id, offset_ms, book_position_ms, started_at, updated_at)
                 VALUES (?1, (SELECT id FROM tracks WHERE book_id = ?1 ORDER BY position LIMIT 1), 1234, 1234, ?2, ?2)",
                rusqlite::params![id.0, now_ms()],
            )
            .expect("progress");
    }
}

/// Size, mtime and full content of every file under `dir`.
fn file_states(dir: &Path) -> BTreeMap<PathBuf, (u64, std::time::SystemTime, Vec<u8>)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_owned()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).expect("read").flatten() {
            let m = fs::symlink_metadata(e.path()).expect("meta");
            if m.is_dir() {
                stack.push(e.path());
            } else if m.is_file() {
                out.insert(
                    e.path(),
                    (
                        m.len(),
                        m.modified().expect("mtime"),
                        fs::read(e.path()).expect("read"),
                    ),
                );
            }
        }
    }
    out
}

#[test]
fn scans_every_fixture_layout() {
    let mut env = Env::new(&["single-m4b", "folder-book", "multi-disc", "loose-author"]);
    let before = file_states(&env.root);
    let summary = env.scan();
    assert!(summary.available);
    assert_eq!(summary.unreadable_files, 0);

    let mut titles = env.titles();
    titles.sort();
    assert_eq!(
        titles,
        [
            "Disc Book",
            "First Story",
            "Fixture Book With Chapters",
            "Second Story",
            "The Folder Book"
        ]
    );

    // One m4b with chapters, tags and an embedded cover.
    let m4b = env.book("Fixture Book With Chapters");
    assert_eq!(
        (m4b.authors.clone(), m4b.narrators.clone()),
        (
            vec!["Ada Example".to_string()],
            vec!["Nora Narrator".to_string()]
        )
    );
    let chapters = read::chapters(env.db.conn(), m4b.id).expect("chapters");
    assert_eq!(
        chapters
            .iter()
            .map(|c| c.title.as_str())
            .collect::<Vec<_>>(),
        ["Opening", "Middle", "Ending"]
    );
    let cover = m4b.cover_path.clone().expect("cover cached");
    assert!(
        Path::new(&cover).starts_with(&env.cache),
        "covers go to the cache, never the library"
    );

    // A folder book: natural order, sidecars, one chapter per track.
    let folder = env.book("The Folder Book");
    assert_eq!(
        env.track_names("The Folder Book"),
        [
            "Bea Writer/The Folder Book/Track 1.mp3",
            "Bea Writer/The Folder Book/Track 2.mp3",
            "Bea Writer/The Folder Book/Track 10.mp3"
        ]
    );
    assert_eq!(folder.narrators, ["Nora Narrator"]);
    let detail = read::book(env.db.conn(), folder.id)
        .expect("detail")
        .expect("exists");
    assert_eq!(
        detail.description.as_deref(),
        Some("A synthetic description.")
    );
    assert!(folder.cover_path.is_some());
    assert_eq!(
        read::chapters(env.db.conn(), folder.id)
            .expect("chapters")
            .len(),
        3
    );
    assert!(folder.duration_ms > 5500);

    // CD1/CD2 merge into one book, ordered by disc/track tags.
    assert_eq!(
        env.track_names("Disc Book"),
        [
            "Cy Author/Disc Book/CD1/01.mp3",
            "Cy Author/Disc Book/CD1/02.mp3",
            "Cy Author/Disc Book/CD2/01.mp3",
            "Cy Author/Disc Book/CD2/02.mp3"
        ]
    );

    // An author folder of loose files splits by album.
    assert_eq!(env.track_names("First Story").len(), 2);
    assert_eq!(env.book("Second Story").authors, ["Dee Novelist"]);

    assert_eq!(
        file_states(&env.root),
        before,
        "scanning must not modify any file"
    );
}

#[test]
fn untagged_and_corrupt_files() {
    let mut env = Env::new(&["unicode", "corrupt"]);
    let summary = env.scan();
    assert_eq!(summary.unreadable_files, 1, "garbage.m4b");

    // No tags: Author/Book from the folders.
    let book = env.book("Livre d’été 本");
    assert_eq!(book.authors, ["Ünïcødé Àuthor"]);
    assert_eq!(book.track_count, 2);
    assert!(book.review_reasons.is_empty(), "{:?}", book.review_reasons);

    // The truncated mp3 still forms a book, without an author: needs review.
    let truncated = env.book("truncated");
    assert!(
        truncated
            .review_reasons
            .contains(&"missing_author".to_string()),
        "{:?}",
        truncated.review_reasons
    );
    assert!(
        truncated
            .review_reasons
            .contains(&"unreadable_files".to_string())
    );
    assert_eq!(
        read::list_books(env.db.conn(), BookFilter::NeedsReview)
            .expect("review")
            .len(),
        1
    );
}

#[test]
fn unchanged_rescan_reads_nothing_and_keeps_ids() {
    let mut env = Env::new(&["folder-book", "multi-disc"]);
    let first = env.scan();
    let ids: Vec<_> = env.books().iter().map(|b| b.id).collect();
    let second = env.scan();
    assert_eq!(second.folders_unchanged, first.folders);
    assert_eq!(second.files_read, 0);
    assert_eq!(env.books().iter().map(|b| b.id).collect::<Vec<_>>(), ids);
    assert_eq!(
        (second.report.books_added, second.report.books_updated),
        (0, 0)
    );
}

#[test]
fn moved_and_renamed_book_keeps_identity_and_progress() {
    let mut env = Env::new(&["folder-book"]);
    env.scan();
    let before = env.book("The Folder Book");
    let tracks_before: Vec<_> = read::tracks(env.db.conn(), before.id)
        .expect("tracks")
        .iter()
        .map(|t| t.id)
        .collect();
    env.add_progress("The Folder Book");

    // Rename the folder, move it under another author, rename one file.
    let new_dir = env.root.join("Someone Else/Renamed Folder");
    fs::create_dir_all(new_dir.parent().expect("parent")).expect("mkdir");
    fs::rename(env.root.join("Bea Writer/The Folder Book"), &new_dir).expect("move");
    fs::rename(
        new_dir.join("Track 2.mp3"),
        new_dir.join("Track 02 renamed.mp3"),
    )
    .expect("rename");

    let summary = env.scan();
    assert_eq!(summary.moved_files, 3);
    let after = env.book("The Folder Book"); // title comes from the album tag
    assert_eq!(after.id, before.id, "same book row");
    let detail_before = before.id;
    let tracks_after: Vec<_> = read::tracks(env.db.conn(), detail_before)
        .expect("tracks")
        .iter()
        .map(|t| t.id)
        .collect();
    let mut a = tracks_after.clone();
    let mut b = tracks_before.clone();
    a.sort();
    b.sort();
    assert_eq!(a, b, "same track rows");
    assert!(after.available);
    let progress: i64 = env
        .db
        .conn()
        .query_row(
            "SELECT book_position_ms FROM progress WHERE book_id = ?1",
            [after.id.0],
            |r| r.get(0),
        )
        .expect("progress kept");
    assert_eq!(progress, 1234);
    assert_eq!(env.books().len(), 1);
}

#[test]
fn missing_files_never_delete_books() {
    let mut env = Env::new(&["folder-book"]);
    env.scan();
    env.add_progress("The Folder Book");
    let dir = env.root.join("Bea Writer/The Folder Book");

    fs::remove_file(dir.join("Track 10.mp3")).expect("remove one");
    let s = env.scan();
    assert_eq!(s.report.tracks_missing, 1);
    let book = env.book("The Folder Book");
    assert!(book.available, "other tracks remain");

    fs::remove_dir_all(&dir).expect("remove all");
    fs::create_dir_all(env.root.join("keep-root-non-empty")).expect("mkdir");
    fs::copy(
        fixtures().join("single-m4b/Ada Example - Fixture Book With Chapters.m4b"),
        env.root.join("keep-root-non-empty/x.m4b"),
    )
    .expect("copy");
    env.scan();
    let book = env.book("The Folder Book");
    assert!(!book.available);
    assert_eq!(book.track_count, 3, "tracks are kept, marked missing");
    assert!(
        read::tracks(env.db.conn(), book.id)
            .expect("tracks")
            .iter()
            .all(|t| t.missing)
    );
}

#[test]
fn unmounted_or_emptied_root_marks_the_library_unavailable() {
    let mut env = Env::new(&["single-m4b"]);
    env.scan();
    let hidden = env.root.with_file_name("library-unmounted");

    // An empty mount point: nothing is marked missing.
    fs::rename(&env.root, &hidden).expect("hide");
    fs::create_dir_all(&env.root).expect("empty mount point");
    let s = env.scan();
    assert!(!s.available);
    assert!(!env.library().available);
    let book = &env.books()[0];
    assert!(!book.available);
    assert!(
        read::tracks(env.db.conn(), book.id)
            .expect("tracks")
            .iter()
            .all(|t| !t.missing)
    );

    // The root missing entirely.
    fs::remove_dir(&env.root).expect("rmdir");
    assert!(!env.scan().available);

    // Mounted again: everything is back, nothing was re-created.
    fs::rename(&hidden, &env.root).expect("restore");
    let s = env.scan();
    assert!(s.available && env.library().available);
    assert_eq!(s.report.books_added, 0);
    assert!(env.books()[0].available);
}

#[cfg(unix)]
#[test]
fn symlink_loops_and_duplicate_links_are_safe() {
    let mut env = Env::new(&["single-m4b"]);
    std::os::unix::fs::symlink(&env.root, env.root.join("loop")).expect("loop link");
    std::os::unix::fs::symlink(
        env.root
            .join("Ada Example - Fixture Book With Chapters.m4b"),
        env.root.join("same-file.m4b"),
    )
    .expect("file link");
    env.scan();
    assert_eq!(env.books().len(), 1, "{:?}", env.titles());
}

#[cfg(unix)]
#[test]
fn non_utf8_and_long_paths() {
    use std::os::unix::ffi::OsStrExt;
    let mut env = Env::new(&[]);
    let src = fixtures().join("single-mp3-chapters/book.mp3");

    let latin1 = env
        .root
        .join(std::ffi::OsStr::from_bytes(b"Auteur/Le livre \xe9t\xe9"));
    fs::create_dir_all(&latin1).expect("non-utf8 dir");
    fs::copy(&src, latin1.join("book.mp3")).expect("copy");

    let mut deep = env.root.join("Deep Author");
    for i in 0..12 {
        deep = deep.join(format!("{i:02}-{}", "x".repeat(80)));
    }
    fs::create_dir_all(&deep).expect("deep dirs");
    fs::copy(&src, deep.join("book.mp3")).expect("copy deep");
    assert!(deep.as_os_str().len() > 1000);

    env.scan();
    let books = env.books();
    assert_eq!(books.len(), 2);
    let paths: Vec<PathBuf> = books
        .iter()
        .flat_map(|b| read::tracks(env.db.conn(), b.id).expect("tracks"))
        .map(|t| t.path)
        .collect();
    assert!(
        paths.contains(&latin1.join("book.mp3")),
        "non-UTF-8 path stored losslessly"
    );
    assert!(paths.contains(&deep.join("book.mp3")));
}

#[test]
fn cancelled_scan_changes_nothing() {
    let mut env = Env::new(&["folder-book"]);
    let cancel = AtomicBool::new(true);
    assert!(matches!(env.try_scan(&cancel), Err(ScanError::Cancelled)));
    assert!(env.books().is_empty());
    assert!(env.library().last_scan_at.is_none());
}
