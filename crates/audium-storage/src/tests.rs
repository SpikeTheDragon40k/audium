use std::path::{Path, PathBuf};

use audium_core::{BookId, BookUid};

use crate::read::{self, BookFilter};
use crate::write::{
    self, BookMetadata, BookTarget, BookWrite, ChapterWrite, ScanChanges, TrackWrite,
};
use crate::{Database, now_ms};

fn db() -> (tempfile::TempDir, Database) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::open(&dir.path().join("audium.db")).expect("open");
    (dir, db)
}

fn uid(n: u8) -> BookUid {
    BookUid::from_random_bytes([n; 16])
}

fn track(path: &str, duration_ms: i64) -> TrackWrite {
    TrackWrite {
        existing: None,
        path: PathBuf::from(path),
        size: 1000,
        mtime_ms: 1,
        partial_hash: Some(vec![1, 2, 3]),
        duration_ms,
        disc: None,
        number: None,
        title: None,
    }
}

fn book(target: BookTarget, title: &str, authors: &[&str], tracks: Vec<TrackWrite>) -> BookWrite {
    BookWrite {
        target,
        metadata: BookMetadata {
            title: title.into(),
            authors: authors.iter().map(|s| s.to_string()).collect(),
            narrators: vec!["Narr".into()],
            series: Some("Saga".into()),
            series_index: Some("1".into()),
            provenance: vec![("title".into(), "embedded".into())],
            ..BookMetadata::default()
        },
        cover_path: None,
        confidence: 90,
        review_reasons: vec![],
        tracks,
        chapters: vec![
            ChapterWrite {
                title: "One".into(),
                start_ms: 0,
                embedded: true,
            },
            ChapterWrite {
                title: "Two".into(),
                start_ms: 500,
                embedded: true,
            },
        ],
    }
}

fn book_id_by_title(db: &Database, title: &str) -> BookId {
    read::list_books(db.conn(), BookFilter::All)
        .expect("list")
        .into_iter()
        .find(|b| b.title == title)
        .map(|b| b.id)
        .expect("book exists")
}

#[test]
fn applies_a_new_book_and_reads_it_back() {
    let (_dir, mut db) = db();
    let lib = write::add_library(&db, Path::new("/books")).expect("lib");
    assert_eq!(
        write::add_library(&db, Path::new("/books")).expect("same lib"),
        lib
    );

    let mut b = book(
        BookTarget::New(uid(1)),
        "The Book",
        &["Ann", "Bob"],
        vec![track("/books/a/1.mp3", 1000), track("/books/a/2.mp3", 2000)],
    );
    b.review_reasons = vec!["missing_author".into()];
    let report = write::apply(
        &mut db,
        lib,
        &ScanChanges {
            books: vec![b],
            ..Default::default()
        },
    )
    .expect("apply");
    assert_eq!((report.books_added, report.books_updated), (1, 0));

    let list = read::list_books(db.conn(), BookFilter::All).expect("list");
    assert_eq!(list.len(), 1);
    let s = &list[0];
    assert_eq!(
        (s.title.as_str(), s.duration_ms, s.track_count),
        ("The Book", 3000, 2)
    );
    assert_eq!(s.authors, vec!["Ann", "Bob"]);
    assert_eq!(s.narrators, vec!["Narr"]);
    assert_eq!(s.series.as_deref(), Some("Saga"));
    assert!(s.available);
    assert_eq!(s.review_reasons, vec!["missing_author"]);
    assert_eq!(
        read::list_books(db.conn(), BookFilter::NeedsReview)
            .expect("review")
            .len(),
        1
    );

    let detail = read::book(db.conn(), s.id)
        .expect("detail")
        .expect("exists");
    assert_eq!(
        detail.provenance,
        vec![("title".to_string(), "embedded".to_string())]
    );
    let tracks = read::tracks(db.conn(), s.id).expect("tracks");
    assert_eq!(
        tracks.iter().map(|t| t.path.clone()).collect::<Vec<_>>(),
        vec![
            PathBuf::from("/books/a/1.mp3"),
            PathBuf::from("/books/a/2.mp3")
        ]
    );
    assert_eq!(read::chapters(db.conn(), s.id).expect("chapters").len(), 2);
    let counts = read::counts(db.conn()).expect("counts");
    assert_eq!(
        (counts.books, counts.needs_review, counts.unavailable),
        (1, 1, 0)
    );
}

#[test]
fn rescan_updates_in_place_and_cleans_orphans() {
    let (_dir, mut db) = db();
    let lib = write::add_library(&db, Path::new("/books")).expect("lib");
    write::apply(
        &mut db,
        lib,
        &ScanChanges {
            books: vec![book(
                BookTarget::New(uid(1)),
                "Old",
                &["Gone Author"],
                vec![track("/books/x.m4b", 100)],
            )],
            ..Default::default()
        },
    )
    .expect("first");
    let id = book_id_by_title(&db, "Old");
    let track_id = read::tracks(db.conn(), id).expect("tracks")[0].id;

    let mut t = track("/books/x.m4b", 100);
    t.existing = Some(track_id);
    let mut b = book(BookTarget::Existing(id), "New", &["New Author"], vec![t]);
    b.metadata.series = None;
    let report = write::apply(
        &mut db,
        lib,
        &ScanChanges {
            books: vec![b],
            ..Default::default()
        },
    )
    .expect("second");
    assert_eq!((report.books_added, report.books_updated), (0, 1));

    let list = read::list_books(db.conn(), BookFilter::All).expect("list");
    assert_eq!(
        (list.len(), list[0].id, list[0].title.as_str()),
        (1, id, "New")
    );
    assert_eq!(list[0].authors, vec!["New Author"]);
    let orphans: i64 = db
        .conn()
        .query_row(
            "SELECT count(*) FROM contributors WHERE name = 'Gone Author'",
            [],
            |r| r.get(0),
        )
        .expect("q");
    let series: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM series", [], |r| r.get(0))
        .expect("q");
    assert_eq!((orphans, series), (0, 0));
    assert_eq!(
        read::tracks(db.conn(), id).expect("tracks")[0].id,
        track_id,
        "track id is stable"
    );
}

#[test]
fn missing_tracks_make_books_unavailable_and_come_back() {
    let (_dir, mut db) = db();
    let lib = write::add_library(&db, Path::new("/books")).expect("lib");
    write::apply(
        &mut db,
        lib,
        &ScanChanges {
            books: vec![book(
                BookTarget::New(uid(1)),
                "B",
                &["A"],
                vec![track("/books/b.m4b", 100)],
            )],
            ..Default::default()
        },
    )
    .expect("apply");
    let id = book_id_by_title(&db, "B");
    let tid = read::tracks(db.conn(), id).expect("tracks")[0].id;

    let r = write::apply(
        &mut db,
        lib,
        &ScanChanges {
            missing_tracks: vec![tid],
            ..Default::default()
        },
    )
    .expect("missing");
    assert_eq!(r.tracks_missing, 1);
    assert!(!read::list_books(db.conn(), BookFilter::All).expect("list")[0].available);
    assert!(read::tracks(db.conn(), id).expect("tracks")[0].missing);

    write::apply(
        &mut db,
        lib,
        &ScanChanges {
            present_tracks: vec![tid],
            ..Default::default()
        },
    )
    .expect("present");
    assert!(read::list_books(db.conn(), BookFilter::All).expect("list")[0].available);

    write::set_library_available(&db, lib, false).expect("unmount");
    assert!(!read::list_books(db.conn(), BookFilter::All).expect("list")[0].available);
}

#[test]
fn removing_a_library_keeps_books_with_user_data() {
    let (_dir, mut db) = db();
    let lib = write::add_library(&db, Path::new("/books")).expect("lib");
    write::apply(
        &mut db,
        lib,
        &ScanChanges {
            books: vec![
                book(
                    BookTarget::New(uid(1)),
                    "Listened",
                    &["A"],
                    vec![track("/books/l.m4b", 100)],
                ),
                book(
                    BookTarget::New(uid(2)),
                    "Untouched",
                    &["A"],
                    vec![track("/books/u.m4b", 100)],
                ),
            ],
            ..Default::default()
        },
    )
    .expect("apply");
    let listened = book_id_by_title(&db, "Listened");
    db.conn()
        .execute(
            "INSERT INTO progress (book_id, offset_ms, book_position_ms, started_at, updated_at) VALUES (?1, 10, 10, ?2, ?2)",
            rusqlite::params![listened.0, now_ms()],
        )
        .expect("progress");

    write::remove_library(&mut db, lib).expect("remove");
    let list = read::list_books(db.conn(), BookFilter::All).expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!((list[0].id, list[0].available), (listened, false));
    let progress: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM progress", [], |r| r.get(0))
        .expect("q");
    assert_eq!(progress, 1);
    assert!(write::libraries(db.conn()).expect("libs").is_empty());
}

#[test]
fn books_emptied_by_regrouping_are_removed_unless_they_have_user_data() {
    let (_dir, mut db) = db();
    let lib = write::add_library(&db, Path::new("/books")).expect("lib");
    write::apply(
        &mut db,
        lib,
        &ScanChanges {
            books: vec![
                book(
                    BookTarget::New(uid(1)),
                    "Part A",
                    &["A"],
                    vec![track("/books/s/1.mp3", 100)],
                ),
                book(
                    BookTarget::New(uid(2)),
                    "Part B",
                    &["A"],
                    vec![track("/books/s/2.mp3", 100)],
                ),
                book(
                    BookTarget::New(uid(3)),
                    "Part C",
                    &["A"],
                    vec![track("/books/s/3.mp3", 100)],
                ),
            ],
            ..Default::default()
        },
    )
    .expect("apply");
    let (a, b, c) = (
        book_id_by_title(&db, "Part A"),
        book_id_by_title(&db, "Part B"),
        book_id_by_title(&db, "Part C"),
    );
    db.conn()
        .execute("INSERT INTO bookmarks (book_id, offset_ms, book_position_ms, created_at, updated_at) VALUES (?1, 0, 0, 0, 0)", [c.0])
        .expect("bookmark");
    let tid = |id| read::tracks(db.conn(), id).expect("tracks")[0].id;
    let (ta, tb, tc) = (tid(a), tid(b), tid(c));

    // A rescan merges all three files into book A.
    let mut tracks = vec![
        track("/books/s/1.mp3", 100),
        track("/books/s/2.mp3", 100),
        track("/books/s/3.mp3", 100),
    ];
    for (t, id) in tracks.iter_mut().zip([ta, tb, tc]) {
        t.existing = Some(id);
    }
    let report = write::apply(
        &mut db,
        lib,
        &ScanChanges {
            books: vec![book(BookTarget::Existing(a), "Merged", &["A"], tracks)],
            ..Default::default()
        },
    )
    .expect("merge");
    assert_eq!(report.books_removed, 1);
    let titles: Vec<String> = read::list_books(db.conn(), BookFilter::All)
        .expect("list")
        .into_iter()
        .map(|b| b.title)
        .collect();
    // B (no user data) is gone; C keeps its bookmark, without tracks and unavailable.
    assert_eq!(titles, vec!["Merged", "Part C"]);
    assert!(
        !read::book(db.conn(), c)
            .expect("c")
            .expect("exists")
            .summary
            .available
    );
}

#[test]
fn unicode_and_non_utf8_paths_round_trip() {
    let (_dir, mut db) = db();
    let lib = write::add_library(&db, Path::new("/books")).expect("lib");
    let mut paths = vec![PathBuf::from("/books/Ünïcødé/本 – d’été.mp3")];
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        paths.push(PathBuf::from(std::ffi::OsStr::from_bytes(
            b"/books/latin1-\xe9.mp3",
        )));
    }
    let tracks = paths
        .iter()
        .map(|p| TrackWrite {
            path: p.clone(),
            ..track("/unused", 100)
        })
        .collect();
    write::apply(
        &mut db,
        lib,
        &ScanChanges {
            books: vec![book(BookTarget::New(uid(1)), "U", &["A"], tracks)],
            ..Default::default()
        },
    )
    .expect("apply");
    let id = book_id_by_title(&db, "U");
    let stored: Vec<PathBuf> = read::tracks(db.conn(), id)
        .expect("tracks")
        .into_iter()
        .map(|t| t.path)
        .collect();
    assert_eq!(stored, paths);
    let snap = write::snapshot(db.conn()).expect("snapshot");
    assert!(
        paths
            .iter()
            .all(|p| snap.tracks.iter().any(|t| &t.path == p))
    );
}

mod playback_state {
    use super::*;
    use crate::playback::{self, SessionRecord};

    fn two_books(db: &mut Database) -> (BookId, BookId) {
        let lib = write::add_library(db, Path::new("/books")).expect("lib");
        write::apply(
            db,
            lib,
            &ScanChanges {
                books: vec![
                    book(
                        BookTarget::New(uid(1)),
                        "Alpha",
                        &["A"],
                        vec![track("/books/a.m4b", 10_000)],
                    ),
                    book(
                        BookTarget::New(uid(2)),
                        "Beta",
                        &["B"],
                        vec![track("/books/b.m4b", 10_000)],
                    ),
                ],
                ..Default::default()
            },
        )
        .expect("apply");
        (book_id_by_title(db, "Alpha"), book_id_by_title(db, "Beta"))
    }

    fn titles(db: &Database, filter: BookFilter) -> Vec<String> {
        read::list_books(db.conn(), filter)
            .expect("list")
            .into_iter()
            .map(|b| b.title)
            .collect()
    }

    #[test]
    fn progress_round_trips_and_feeds_continue_listening() {
        let (_dir, mut db) = db();
        let (a, b) = two_books(&mut db);
        assert_eq!(playback::progress(db.conn(), a).expect("q"), None);
        let ta = read::tracks(db.conn(), a).expect("t")[0].id;

        playback::save_progress(db.conn(), a, Some(ta), 1234, 1234, 1_000).expect("save");
        playback::save_progress(db.conn(), b, None, 50, 50, 2_000).expect("save");
        let p = playback::progress(db.conn(), a).expect("q").expect("some");
        assert_eq!(
            (p.track_id, p.offset_ms, p.book_position_ms, p.finished),
            (Some(ta), 1234, 1234, false)
        );
        assert_eq!(
            titles(&db, BookFilter::ContinueListening),
            ["Beta", "Alpha"],
            "most recent first"
        );

        playback::save_progress(db.conn(), a, Some(ta), 2000, 2000, 3_000).expect("save");
        assert_eq!(
            titles(&db, BookFilter::ContinueListening),
            ["Alpha", "Beta"]
        );
        let s = read::book_summary(db.conn(), a).expect("q").expect("some");
        assert_eq!(
            (s.position_ms, s.finished, s.last_played_at),
            (2000, false, Some(3_000))
        );
    }

    #[test]
    fn finishing_and_restarting() {
        let (_dir, mut db) = db();
        let (a, _) = two_books(&mut db);
        playback::save_progress(db.conn(), a, None, 9_000, 9_000, 1_000).expect("save");
        playback::mark_finished(db.conn(), a, None, 10_000, 10_000, 2_000).expect("finish");
        assert!(
            playback::progress(db.conn(), a)
                .expect("q")
                .expect("p")
                .finished
        );
        assert_eq!(titles(&db, BookFilter::Finished), ["Alpha"]);
        assert!(titles(&db, BookFilter::ContinueListening).is_empty());

        // Any later save, even in the same millisecond, means it was restarted.
        playback::save_progress(db.conn(), a, None, 0, 0, 2_000).expect("restart");
        assert!(
            !playback::progress(db.conn(), a)
                .expect("q")
                .expect("p")
                .finished
        );
        assert_eq!(titles(&db, BookFilter::ContinueListening), ["Alpha"]);
        assert!(titles(&db, BookFilter::Finished).is_empty());
        let finished_at: Option<i64> = db
            .conn()
            .query_row("SELECT finished_at FROM progress", [], |r| r.get(0))
            .expect("q");
        assert_eq!(finished_at, Some(2_000), "history kept");
    }

    #[test]
    fn speed_sessions_settings_and_recent() {
        let (_dir, mut db) = db();
        let (a, _) = two_books(&mut db);
        assert_eq!(playback::book_speed(db.conn(), a).expect("q"), None);
        playback::set_book_speed(db.conn(), a, 1.5).expect("speed");
        playback::set_book_speed(db.conn(), a, 1.75).expect("speed");
        assert_eq!(playback::book_speed(db.conn(), a).expect("q"), Some(1.75));

        let s = SessionRecord {
            book: a,
            started_at: 1,
            ended_at: 9_001,
            start_position_ms: 0,
            end_position_ms: 9_000,
            listened_ms: 9_000,
        };
        playback::record_session(db.conn(), &s).expect("session");
        let n: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM sessions", [], |r| r.get(0))
            .expect("q");
        assert_eq!(n, 1);

        assert_eq!(
            playback::setting(db.conn(), "skip_back_ms").expect("q"),
            None
        );
        playback::set_setting(db.conn(), "skip_back_ms", "10000").expect("set");
        playback::set_setting(db.conn(), "skip_back_ms", "20000").expect("set");
        assert_eq!(
            playback::setting(db.conn(), "skip_back_ms")
                .expect("q")
                .as_deref(),
            Some("20000")
        );

        assert_eq!(
            read::list_books(db.conn(), BookFilter::RecentlyAdded)
                .expect("recent")
                .len(),
            2
        );
        // Speed and sessions are user data: the book survives removal of its library.
        let lib = write::libraries(db.conn()).expect("libs")[0].id;
        write::remove_library(&mut db, lib).expect("remove");
        assert_eq!(titles(&db, BookFilter::All), ["Alpha"]);
    }

    #[test]
    fn writer_runs_jobs_in_order_and_flushes_on_drop() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("audium.db");
        drop(Database::open(&path).expect("create"));
        let writer = crate::Writer::open(&path).expect("writer");
        for i in 0..100 {
            writer.submit(move |db| {
                playback::set_setting(db.conn(), "counter", &i.to_string()).expect("write");
            });
        }
        writer.flush();
        let ro = crate::ReadOnly::open(&path).expect("ro");
        assert_eq!(
            playback::setting(ro.conn(), "counter")
                .expect("q")
                .as_deref(),
            Some("99")
        );
        writer.submit(|db| playback::set_setting(db.conn(), "last", "yes").expect("write"));
        drop(writer);
        assert_eq!(
            playback::setting(ro.conn(), "last").expect("q").as_deref(),
            Some("yes")
        );
    }
}
