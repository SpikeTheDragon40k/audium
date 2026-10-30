//! `LibraryController`: opens the database, manages library folders and runs
//! scans on a background thread.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use audium_core::{BookId, TrackId};
use audium_library::{Phase, ScanError, ScanOptions, ScanProgress, scan_library};
use audium_storage::{Database, read, write};
use audium_storage::{now_ms, organize};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QString, QStringList, QUrl, QVariant};

use crate::convert::{qs, string_list};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
        include!("cxx-qt-lib/qlist.h");
        type QList_QVariant = cxx_qt_lib::QList<cxx_qt_lib::QVariant>;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_uncreatable]
        #[qproperty(bool, ready, READ, NOTIFY)]
        #[qproperty(QString, error_string, cxx_name = "errorString", READ, NOTIFY)]
        #[qproperty(QString, database_path, cxx_name = "databasePath", READ, NOTIFY)]
        #[qproperty(QStringList, folders, READ, NOTIFY)]
        #[qproperty(bool, scanning, READ, NOTIFY)]
        #[qproperty(QString, scan_status, cxx_name = "scanStatus", READ, NOTIFY)]
        #[qproperty(i32, scan_done, cxx_name = "scanDone", READ, NOTIFY)]
        #[qproperty(i32, scan_total, cxx_name = "scanTotal", READ, NOTIFY)]
        /// Increases whenever the library content changed; models reload on it.
        #[qproperty(i32, revision, READ, NOTIFY)]
        #[qproperty(i32, book_count, cxx_name = "bookCount", READ, NOTIFY)]
        #[qproperty(i32, review_count, cxx_name = "reviewCount", READ, NOTIFY)]
        #[qproperty(i32, unavailable_count, cxx_name = "unavailableCount", READ, NOTIFY)]
        type LibraryController = super::LibraryControllerRust;

        /// Opens (and migrates) the database in `data_dir`. Returns false and
        /// sets `errorString` on failure; the database is never recreated.
        #[qinvokable]
        fn open(self: Pin<&mut LibraryController>, data_dir: &QString, cache_dir: &QString)
        -> bool;

        /// Adds a library folder and scans. Returns an error message, or an
        /// empty string on success.
        #[qinvokable]
        #[cxx_name = "addFolder"]
        fn add_folder(self: Pin<&mut LibraryController>, url: &QUrl) -> QString;

        /// Removes a folder from the library (never touches the files).
        #[qinvokable]
        #[cxx_name = "removeFolder"]
        fn remove_folder(self: Pin<&mut LibraryController>, index: i32);

        #[qinvokable]
        fn rescan(self: Pin<&mut LibraryController>);

        /// Re-reads every folder, including unchanged ones.
        #[qinvokable]
        #[cxx_name = "fullRescan"]
        fn full_rescan(self: Pin<&mut LibraryController>);

        #[qinvokable]
        #[cxx_name = "cancelScan"]
        fn cancel_scan(self: Pin<&mut LibraryController>);

        // Organizing. Each returns an error message, or "" on success.

        /// Sets metadata overrides (`fields[i]` = `values[i]`, an empty value
        /// removes one; lists one name per line) and re-reads the book.
        #[qinvokable]
        #[cxx_name = "editMetadata"]
        fn edit_metadata(
            self: Pin<&mut LibraryController>,
            book_id: i64,
            fields: &QStringList,
            values: &QStringList,
        ) -> QString;

        /// Merges `others` (book ids) into `target`.
        #[qinvokable]
        #[cxx_name = "mergeBooks"]
        fn merge_books(
            self: Pin<&mut LibraryController>,
            target: i64,
            others: &QList_QVariant,
        ) -> QString;

        /// Moves the given tracks of a book into a new book.
        #[qinvokable]
        #[cxx_name = "splitBook"]
        fn split_book(
            self: Pin<&mut LibraryController>,
            book_id: i64,
            track_ids: &QList_QVariant,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "dismissReview"]
        fn dismiss_review(self: Pin<&mut LibraryController>, book_id: i64) -> QString;

        /// Returns the new (or existing) collection's id, or 0 on error.
        #[qinvokable]
        #[cxx_name = "createCollection"]
        fn create_collection(self: Pin<&mut LibraryController>, name: &QString) -> i64;

        #[qinvokable]
        #[cxx_name = "renameCollection"]
        fn rename_collection(self: Pin<&mut LibraryController>, id: i64, name: &QString)
        -> QString;

        #[qinvokable]
        #[cxx_name = "deleteCollection"]
        fn delete_collection(self: Pin<&mut LibraryController>, id: i64) -> QString;

        #[qinvokable]
        #[cxx_name = "addToCollection"]
        fn add_to_collection(
            self: Pin<&mut LibraryController>,
            collection: i64,
            book_id: i64,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "removeFromCollection"]
        fn remove_from_collection(
            self: Pin<&mut LibraryController>,
            collection: i64,
            book_id: i64,
        ) -> QString;
    }

    impl cxx_qt::Threading for LibraryController {}
}

#[derive(Default)]
pub struct LibraryControllerRust {
    ready: bool,
    error_string: QString,
    database_path: QString,
    folders: QStringList,
    scanning: bool,
    scan_status: QString,
    scan_done: i32,
    scan_total: i32,
    revision: i32,
    book_count: i32,
    review_count: i32,
    unavailable_count: i32,

    db: Option<Database>,
    cache_dir: PathBuf,
    cancel: Arc<AtomicBool>,
    /// Folders to re-read once the running scan has finished.
    pending_dirs: Vec<PathBuf>,
}

/// Ids from a QML array (numbers arrive as doubles or integers).
fn ids(list: &QList<QVariant>) -> Vec<i64> {
    list.iter()
        .filter_map(|v| {
            v.value::<i64>()
                .or_else(|| v.value::<i32>().map(i64::from))
                .or_else(|| {
                    v.value::<f64>()
                        .filter(|f| f.fract() == 0.0)
                        .map(|f| f as i64)
                })
        })
        .filter(|&id| id > 0)
        .collect()
}

fn to_i32(n: impl TryInto<i32>) -> i32 {
    n.try_into().unwrap_or(i32::MAX)
}

impl qobject::LibraryController {
    fn open(mut self: Pin<&mut Self>, data_dir: &QString, cache_dir: &QString) -> bool {
        let _ = tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_target(false)
            .try_init();
        let data_dir = PathBuf::from(String::from(data_dir));
        let cache_dir = PathBuf::from(String::from(cache_dir));
        if let Err(e) = std::fs::create_dir_all(&data_dir) {
            self.as_mut()
                .set_error(format!("Cannot create {}: {e}", data_dir.display()));
            return false;
        }
        let path = data_dir.join("audium.db");
        match Database::open(&path) {
            Ok(db) => {
                self.as_mut().rust_mut().db = Some(db);
                self.as_mut().rust_mut().cache_dir = cache_dir;
                set_ro!(
                    self,
                    database_path,
                    database_path_changed,
                    qs(&path.to_string_lossy())
                );
                set_ro!(self, ready, ready_changed, true);
                self.as_mut().refresh();
                if !self.folders().is_empty() {
                    self.start_scan(false, Vec::new());
                }
                true
            }
            Err(e) => {
                tracing::error!(error = %e, "cannot open database");
                self.set_error(e.to_string());
                false
            }
        }
    }

    fn add_folder(mut self: Pin<&mut Self>, url: &QUrl) -> QString {
        let Some(local) = url.to_local_file() else {
            return qs("Only local folders can be added.");
        };
        let path = PathBuf::from(String::from(&local));
        if !path.is_dir() {
            return qs(&format!("{} is not a folder.", path.display()));
        }
        let Some(db) = self.rust().db.as_ref() else {
            return qs("The library database is not open.");
        };
        let existing = match write::libraries(db.conn()) {
            Ok(l) => l,
            Err(e) => return qs(&e.to_string()),
        };
        if let Some(clash) = existing
            .iter()
            .find(|l| path.starts_with(&l.path) || l.path.starts_with(&path))
        {
            return qs(&format!(
                "{} overlaps with the library folder {}.",
                path.display(),
                clash.display_path
            ));
        }
        if let Err(e) = write::add_library(db, &path) {
            return qs(&e.to_string());
        }
        self.as_mut().refresh();
        self.start_scan(false, Vec::new());
        QString::default()
    }

    fn remove_folder(mut self: Pin<&mut Self>, index: i32) {
        if self.scanning {
            self.as_mut().cancel_scan();
        }
        let result = {
            let mut rust = self.as_mut().rust_mut();
            let Some(db) = rust.db.as_mut() else {
                return;
            };
            write::libraries(db.conn()).and_then(|libs| {
                match usize::try_from(index).ok().and_then(|i| libs.get(i)) {
                    Some(lib) => write::remove_library(db, lib.id),
                    None => Ok(()),
                }
            })
        };
        if let Err(e) = result {
            self.as_mut().set_error(e.to_string());
        }
        self.as_mut().refresh();
        self.bump_revision();
    }

    fn rescan(self: Pin<&mut Self>) {
        self.start_scan(false, Vec::new());
    }

    fn full_rescan(self: Pin<&mut Self>) {
        self.start_scan(true, Vec::new());
    }

    fn cancel_scan(self: Pin<&mut Self>) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Runs a small write on the UI thread's connection, then refreshes the
    /// counts and models. Refused while a scan runs (it holds the database).
    fn organize(
        mut self: Pin<&mut Self>,
        write: impl FnOnce(&mut Database) -> audium_storage::Result<()>,
    ) -> QString {
        if self.scanning {
            return qs("The library is being scanned. Try again when the scan has finished.");
        }
        let result = {
            let mut rust = self.as_mut().rust_mut();
            let Some(db) = rust.db.as_mut() else {
                return qs("The library database is not open.");
            };
            write(db)
        };
        self.as_mut().refresh();
        self.as_mut().bump_revision();
        match result {
            Ok(()) => QString::default(),
            Err(e) => {
                tracing::error!(error = %e, "library change failed");
                qs(&e.to_string())
            }
        }
    }

    /// Re-reads the folders of these books (e.g. after merging or editing).
    fn reread_books(self: Pin<&mut Self>, books: &[BookId]) {
        let Some(db) = self.rust().db.as_ref() else {
            return;
        };
        let mut dirs = Vec::new();
        for book in books {
            if let Ok((_, d)) = organize::book_folders(db.conn(), *book) {
                dirs.extend(d);
            }
        }
        if !dirs.is_empty() {
            self.start_scan(false, dirs);
        }
    }

    fn edit_metadata(
        mut self: Pin<&mut Self>,
        book_id: i64,
        fields: &QStringList,
        values: &QStringList,
    ) -> QString {
        let fields: Vec<String> = QList::<QString>::from(fields)
            .iter()
            .map(String::from)
            .collect();
        let values: Vec<String> = QList::<QString>::from(values)
            .iter()
            .map(String::from)
            .collect();
        if fields.len() != values.len() {
            return qs("fields and values do not match");
        }
        let book = BookId(book_id);
        let error = self.as_mut().organize(|db| {
            let now = now_ms();
            for (field, value) in fields.iter().zip(&values) {
                organize::set_override(db.conn(), book, field, value, now)?;
            }
            Ok(())
        });
        if error.is_empty() {
            self.reread_books(&[book]);
        }
        error
    }

    fn merge_books(mut self: Pin<&mut Self>, target: i64, others: &QList<QVariant>) -> QString {
        let others: Vec<BookId> = ids(others).into_iter().map(BookId).collect();
        let target = BookId(target);
        let error = self
            .as_mut()
            .organize(|db| organize::merge_books(db, target, &others, now_ms()));
        if error.is_empty() {
            self.reread_books(&[target]);
        }
        error
    }

    fn split_book(mut self: Pin<&mut Self>, book_id: i64, track_ids: &QList<QVariant>) -> QString {
        let tracks: Vec<TrackId> = ids(track_ids).into_iter().map(TrackId).collect();
        let book = BookId(book_id);
        let mut new_book = None;
        let error = self.as_mut().organize(|db| {
            let uid = audium_library::fresh_uid(db.path());
            new_book = Some(organize::split_book(db, book, &tracks, &uid, now_ms())?);
            Ok(())
        });
        if let Some(new_book) = new_book {
            self.reread_books(&[book, new_book]);
        }
        error
    }

    fn dismiss_review(self: Pin<&mut Self>, book_id: i64) -> QString {
        self.organize(|db| organize::dismiss_review(db.conn(), BookId(book_id), now_ms()))
    }

    fn create_collection(self: Pin<&mut Self>, name: &QString) -> i64 {
        let name = String::from(name);
        let mut id = 0;
        let error = self.organize(|db| {
            id = organize::create_collection(db.conn(), &name, now_ms())?;
            Ok(())
        });
        if error.is_empty() { id } else { 0 }
    }

    fn rename_collection(self: Pin<&mut Self>, id: i64, name: &QString) -> QString {
        let name = String::from(name);
        self.organize(|db| organize::rename_collection(db.conn(), id, &name))
    }

    fn delete_collection(self: Pin<&mut Self>, id: i64) -> QString {
        self.organize(|db| organize::delete_collection(db.conn(), id))
    }

    fn add_to_collection(self: Pin<&mut Self>, collection: i64, book_id: i64) -> QString {
        self.organize(|db| organize::add_to_collection(db.conn(), collection, BookId(book_id)))
    }

    fn remove_from_collection(self: Pin<&mut Self>, collection: i64, book_id: i64) -> QString {
        self.organize(|db| organize::remove_from_collection(db.conn(), collection, BookId(book_id)))
    }

    fn set_error(mut self: Pin<&mut Self>, message: String) {
        set_ro!(self, error_string, error_string_changed, qs(&message));
    }

    fn bump_revision(mut self: Pin<&mut Self>) {
        let next = self.revision.wrapping_add(1);
        set_ro!(self, revision, revision_changed, next);
    }

    /// Re-reads the folder list and counts.
    fn refresh(mut self: Pin<&mut Self>) {
        let Some(db) = self.rust().db.as_ref() else {
            return;
        };
        let (folders, counts) = match (write::libraries(db.conn()), read::counts(db.conn())) {
            (Ok(l), Ok(c)) => (l, c),
            (Err(e), _) | (_, Err(e)) => {
                self.set_error(e.to_string());
                return;
            }
        };
        let names: Vec<String> = folders
            .iter()
            .map(|l| {
                if l.available {
                    l.display_path.clone()
                } else {
                    format!("{} (unavailable)", l.display_path)
                }
            })
            .collect();
        set_ro!(self, folders, folders_changed, string_list(&names));
        set_ro!(self, book_count, book_count_changed, to_i32(counts.books));
        set_ro!(
            self,
            review_count,
            review_count_changed,
            to_i32(counts.needs_review)
        );
        set_ro!(
            self,
            unavailable_count,
            unavailable_count_changed,
            to_i32(counts.unavailable)
        );
    }

    /// Starts a scan; `force_dirs` are re-read even if unchanged. A request
    /// for specific folders made while a scan runs is queued until it ends.
    fn start_scan(mut self: Pin<&mut Self>, force: bool, force_dirs: Vec<PathBuf>) {
        if self.scanning {
            self.as_mut().rust_mut().pending_dirs.extend(force_dirs);
            return;
        }
        let Some(db) = self.rust().db.as_ref() else {
            return;
        };
        let db_path = db.path().to_owned();
        let cache_dir = self.cache_dir.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.as_mut().rust_mut().cancel = Arc::clone(&cancel);
        set_ro!(self, scanning, scanning_changed, true);
        set_ro!(self, scan_done, scan_done_changed, 0);
        set_ro!(self, scan_total, scan_total_changed, 0);
        set_ro!(
            self,
            scan_status,
            scan_status_changed,
            qs("Looking for audiobooks…")
        );
        let thread = self.qt_thread();

        std::thread::Builder::new()
            .name("audium-scan".into())
            .spawn(move || {
                let outcome = run_scan(&db_path, &cache_dir, force, force_dirs, &cancel, &thread);
                let _ = thread.queue(move |mut obj| {
                    set_ro!(obj, scanning, scanning_changed, false);
                    set_ro!(obj, scan_status, scan_status_changed, qs(&outcome));
                    obj.as_mut().refresh();
                    obj.as_mut().bump_revision();
                    let pending = std::mem::take(&mut obj.as_mut().rust_mut().pending_dirs);
                    if !pending.is_empty() {
                        obj.start_scan(false, pending);
                    }
                });
            })
            .map_err(|e| tracing::error!(error = %e, "cannot start scan thread"))
            .ok();
    }
}

/// Scans every library folder; runs on the scan thread with its own
/// connection. Returns a status line for the UI.
fn run_scan(
    db_path: &Path,
    cache_dir: &Path,
    force: bool,
    force_dirs: Vec<PathBuf>,
    cancel: &AtomicBool,
    thread: &cxx_qt::CxxQtThread<qobject::LibraryController>,
) -> String {
    let mut db = match Database::open(db_path) {
        Ok(db) => db,
        Err(e) => return format!("Scan failed: {e}"),
    };
    let libraries = match write::libraries(db.conn()) {
        Ok(l) => l,
        Err(e) => return format!("Scan failed: {e}"),
    };
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get().min(4));
    // Grouping rules changed since the last scan: re-read everything once.
    let rules = audium_storage::playback::setting(db.conn(), audium_library::RULES_VERSION_KEY)
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let force = force || rules < audium_library::RULES_VERSION;
    if force {
        tracing::info!(
            rules,
            current = audium_library::RULES_VERSION,
            "re-reading every folder"
        );
    }
    let opts = ScanOptions {
        cache_dir: cache_dir.to_owned(),
        threads,
        force,
        force_dirs,
    };

    let last_report = Mutex::new(Instant::now() - Duration::from_secs(1));
    let progress = |p: ScanProgress| {
        // At most ~10 UI updates per second, but always the final one.
        let Ok(mut last) = last_report.lock() else {
            return;
        };
        if p.done != p.total && last.elapsed() < Duration::from_millis(100) {
            return;
        }
        *last = Instant::now();
        let status = match p.phase {
            Phase::Discovering => "Looking for audiobooks…".to_owned(),
            Phase::Reading => format!("Reading files ({} of {})…", p.done, p.total),
            Phase::Saving => "Saving…".to_owned(),
        };
        let _ = thread.queue(move |mut obj| {
            set_ro!(obj, scan_status, scan_status_changed, qs(&status));
            set_ro!(obj, scan_done, scan_done_changed, to_i32(p.done));
            set_ro!(obj, scan_total, scan_total_changed, to_i32(p.total));
        });
    };

    let mut added = 0;
    let mut unavailable = Vec::new();
    for library in &libraries {
        match scan_library(&mut db, library, &opts, cancel, &progress) {
            Ok(summary) if !summary.available => unavailable.push(library.display_path.clone()),
            Ok(summary) => added += summary.report.books_added,
            Err(ScanError::Cancelled) => return "Scan cancelled.".to_owned(),
            Err(e) => {
                tracing::error!(error = %e, library = %library.display_path, "scan failed");
                return format!("Scan of {} failed: {e}", library.display_path);
            }
        }
    }
    // Only once every folder was actually re-read: an unmounted drive must be
    // regrouped when it comes back.
    if unavailable.is_empty()
        && let Err(e) = audium_storage::playback::set_setting(
            db.conn(),
            audium_library::RULES_VERSION_KEY,
            &audium_library::RULES_VERSION.to_string(),
        )
    {
        tracing::error!(error = %e, "cannot save the rules version");
    }
    let mut status = match added {
        0 => "Library is up to date.".to_owned(),
        1 => "1 new book.".to_owned(),
        n => format!("{n} new books."),
    };
    if !unavailable.is_empty() {
        status.push_str(&format!(" Unavailable: {}.", unavailable.join(", ")));
    }
    status
}
