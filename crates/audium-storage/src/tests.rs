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

mod queries {
    use super::*;
    use crate::read::{BookQuery, GroupKind, Scope, Sort};

    fn library(db: &mut Database) {
        let lib = write::add_library(db, Path::new("/books")).expect("lib");
        let mut hobbit = book(
            BookTarget::New(uid(1)),
            "The Hobbit",
            &["J. R. R. Tolkien"],
            vec![track("/books/h.m4b", 100)],
        );
        hobbit.metadata.series = Some("Middle-earth".into());
        hobbit.metadata.series_index = Some("1".into());
        let mut rings = book(
            BookTarget::New(uid(2)),
            "The Lord of the Rings",
            &["J. R. R. Tolkien"],
            vec![track("/books/r.m4b", 100)],
        );
        rings.metadata.series = Some("Middle-earth".into());
        rings.metadata.series_index = Some("2".into());
        let mut ete = book(
            BookTarget::New(uid(3)),
            "Livre d’été",
            &["Ünïcødé Àuthor"],
            vec![track("/books/e.m4b", 100)],
        );
        ete.metadata.series = None;
        ete.metadata.narrators = vec!["Zoé Reader".into()];
        write::apply(
            db,
            lib,
            &ScanChanges {
                books: vec![rings, ete, hobbit],
                ..Default::default()
            },
        )
        .expect("apply");
    }

    fn titles(db: &Database, q: BookQuery<'_>) -> Vec<String> {
        read::query_list(db.conn(), &q)
            .expect("query")
            .into_iter()
            .map(|b| b.title)
            .collect()
    }

    fn search(db: &Database, text: &str) -> Vec<String> {
        let mut t = titles(
            db,
            BookQuery {
                search: Some(text),
                ..BookQuery::default()
            },
        );
        t.sort();
        t
    }

    #[test]
    fn search_matches_prefixes_across_fields_and_ignores_accents() {
        let (_dir, mut db) = db();
        library(&mut db);
        assert_eq!(search(&db, "hob"), ["The Hobbit"]);
        assert_eq!(search(&db, "tolk"), ["The Hobbit", "The Lord of the Rings"]);
        assert_eq!(
            search(&db, "middle earth"),
            ["The Hobbit", "The Lord of the Rings"],
            "series"
        );
        assert_eq!(search(&db, "ete"), ["Livre d’été"], "accents folded");
        assert_eq!(search(&db, "zoe"), ["Livre d’été"], "narrator");
        assert_eq!(
            search(&db, "tolkien rings"),
            ["The Lord of the Rings"],
            "all words must match"
        );
        assert!(search(&db, "nothing like this").is_empty());
        assert_eq!(search(&db, "  ").len(), 3, "empty search lists everything");
        assert!(
            search(&db, "\"OR NEAR(").is_empty(),
            "operators are plain text"
        );
    }

    #[test]
    fn search_index_follows_rescans_and_removals() {
        let (_dir, mut db) = db();
        library(&mut db);
        let hobbit = book_id_by_title(&db, "The Hobbit");
        let tid = read::tracks(db.conn(), hobbit).expect("t")[0].id;
        let mut t = track("/books/h.m4b", 100);
        t.existing = Some(tid);
        let renamed = book(
            BookTarget::Existing(hobbit),
            "There and Back Again",
            &["Bilbo"],
            vec![t],
        );
        let lib = write::libraries(db.conn()).expect("libs")[0].id;
        write::apply(
            &mut db,
            lib,
            &ScanChanges {
                books: vec![renamed],
                ..Default::default()
            },
        )
        .expect("rescan");
        assert!(search(&db, "hobbit").is_empty());
        assert_eq!(search(&db, "bilbo"), ["There and Back Again"]);
        write::remove_library(&mut db, lib).expect("remove");
        let indexed: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM books_fts", [], |r| r.get(0))
            .expect("q");
        assert_eq!(indexed, 0);
    }

    #[test]
    fn sorting_and_scopes() {
        let (_dir, mut db) = db();
        library(&mut db);
        assert_eq!(
            titles(&db, BookQuery::default()),
            ["The Hobbit", "Livre d’été", "The Lord of the Rings"],
            "title sort ignores 'The': hobbit < livre < lord"
        );
        assert_eq!(
            titles(
                &db,
                BookQuery {
                    sort: Sort::Author,
                    ..BookQuery::default()
                }
            ),
            ["The Hobbit", "The Lord of the Rings", "Livre d’été"],
            "by author, then series index"
        );
        let groups = read::list_groups(db.conn(), GroupKind::Authors).expect("authors");
        let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["J. R. R. Tolkien", "Ünïcødé Àuthor"]);
        assert_eq!(groups[0].book_count, 2);
        let tolkien = groups[0].id;
        assert_eq!(
            titles(
                &db,
                BookQuery {
                    scope: Scope::Author(tolkien),
                    ..BookQuery::default()
                }
            )
            .len(),
            2
        );

        let series = read::list_groups(db.conn(), GroupKind::Series).expect("series");
        assert_eq!(
            (series.len(), series[0].name.as_str(), series[0].book_count),
            (1, "Middle-earth", 2)
        );
        assert_eq!(
            titles(
                &db,
                BookQuery {
                    scope: Scope::Series(series[0].id),
                    ..BookQuery::default()
                }
            ),
            ["The Hobbit", "The Lord of the Rings"]
        );
        let narrators = read::list_groups(db.conn(), GroupKind::Narrators).expect("narrators");
        assert!(narrators.iter().any(|g| g.name == "Zoé Reader"));
        // Scope and search combine.
        assert_eq!(
            titles(
                &db,
                BookQuery {
                    scope: Scope::Author(tolkien),
                    search: Some("lord"),
                    ..BookQuery::default()
                }
            ),
            ["The Lord of the Rings"]
        );
    }

    #[test]
    fn migration_two_indexes_existing_books() {
        // A v1 database with books, opened by this version: the books are searchable.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("audium.db");
        {
            let mut db = Database::open_with(&path, &crate::MIGRATIONS[..1]).expect("v1");
            let lib = write::add_library(&db, Path::new("/books")).expect("lib");
            // Apply without the FTS refresh: the v1 schema has no books_fts.
            let tx = db.conn_mut().transaction().expect("tx");
            tx.execute("INSERT INTO books (uid, library_id, title, title_sort, confidence, added_at, updated_at) VALUES (?1, ?2, 'Old Book', 'old book', 100, 0, 0)", rusqlite::params![uid(9).as_str(), lib.0]).expect("insert");
            tx.commit().expect("commit");
        }
        let db = Database::open(&path).expect("migrate to v2");
        assert_eq!(search(&db, "old"), ["Old Book"]);
    }
}

mod organizing {
    use super::*;
    use crate::organize;
    use crate::read::{BookQuery, GroupKind, Scope};

    fn three_books(db: &mut Database) -> (BookId, BookId, BookId) {
        let lib = write::add_library(db, Path::new("/books")).expect("lib");
        write::apply(
            db,
            lib,
            &ScanChanges {
                books: vec![
                    book(
                        BookTarget::New(uid(1)),
                        "Part A",
                        &["A"],
                        vec![track("/books/s/1.mp3", 1000), track("/books/s/2.mp3", 1000)],
                    ),
                    book(
                        BookTarget::New(uid(2)),
                        "Part B",
                        &["A"],
                        vec![track("/books/s/3.mp3", 1000)],
                    ),
                    book(
                        BookTarget::New(uid(3)),
                        "Part C",
                        &["A"],
                        vec![track("/books/s/4.mp3", 1000)],
                    ),
                ],
                ..Default::default()
            },
        )
        .expect("apply");
        (
            book_id_by_title(db, "Part A"),
            book_id_by_title(db, "Part B"),
            book_id_by_title(db, "Part C"),
        )
    }

    fn track_paths(db: &Database, id: BookId) -> Vec<String> {
        read::tracks(db.conn(), id)
            .expect("t")
            .into_iter()
            .map(|t| t.display_path)
            .collect()
    }

    #[test]
    fn collections() {
        let (_dir, mut db) = db();
        let (a, b, _) = three_books(&mut db);
        let favs = organize::create_collection(db.conn(), " Favourites ", 1).expect("create");
        assert_eq!(
            organize::create_collection(db.conn(), "Favourites", 2).expect("same"),
            favs
        );
        assert!(organize::create_collection(db.conn(), "   ", 2).is_err());
        organize::add_to_collection(db.conn(), favs, b).expect("add");
        organize::add_to_collection(db.conn(), favs, a).expect("add");
        organize::add_to_collection(db.conn(), favs, a).expect("again: no-op");
        let titles: Vec<String> = read::query_list(
            db.conn(),
            &BookQuery {
                scope: Scope::Collection(favs),
                ..BookQuery::default()
            },
        )
        .expect("q")
        .into_iter()
        .map(|b| b.title)
        .collect();
        assert_eq!(titles, ["Part B", "Part A"], "in the order added");
        assert_eq!(
            organize::collections_of(db.conn(), a).expect("of"),
            [(favs, "Favourites".to_string())]
        );
        let other = organize::create_collection(db.conn(), "Other", 3).expect("create");
        assert!(
            organize::rename_collection(db.conn(), other, "Favourites").is_err(),
            "names are unique"
        );
        organize::rename_collection(db.conn(), favs, "Best").expect("rename");
        let groups = read::list_groups(db.conn(), GroupKind::Collections).expect("groups");
        assert_eq!(
            groups
                .iter()
                .map(|g| (g.name.as_str(), g.book_count))
                .collect::<Vec<_>>(),
            [("Best", 2), ("Other", 0)]
        );
        organize::remove_from_collection(db.conn(), favs, a).expect("remove");
        organize::delete_collection(db.conn(), other).expect("delete");
        assert_eq!(
            read::list_groups(db.conn(), GroupKind::Collections)
                .expect("g")
                .len(),
            1
        );
        assert_eq!(
            read::list_books(db.conn(), BookFilter::All)
                .expect("books")
                .len(),
            3,
            "books stay"
        );
    }

    #[test]
    fn bookmarks() {
        let (_dir, mut db) = db();
        let (a, _, _) = three_books(&mut db);
        let t = read::tracks(db.conn(), a).expect("t");
        let late = organize::add_bookmark(db.conn(), a, Some(t[1].id), 500, 1500, 10).expect("add");
        let early = organize::add_bookmark(db.conn(), a, Some(t[0].id), 200, 200, 11).expect("add");
        organize::update_bookmark(
            db.conn(),
            late,
            "  Important\u{0} ",
            "line one\n\nline two",
            12,
        )
        .expect("edit");
        let list = organize::bookmarks(db.conn(), a).expect("list");
        assert_eq!(
            list.iter().map(|b| b.id).collect::<Vec<_>>(),
            [early, late],
            "in book order"
        );
        assert_eq!(list[1].title.as_deref(), Some("Important"));
        assert_eq!(list[1].note.as_deref(), Some("line one\n\nline two"));
        organize::update_bookmark(db.conn(), late, "", "", 13).expect("clear");
        assert_eq!(
            organize::bookmarks(db.conn(), a).expect("list")[1].title,
            None
        );
        organize::delete_bookmark(db.conn(), early).expect("delete");
        assert_eq!(organize::bookmarks(db.conn(), a).expect("list").len(), 1);
    }

    #[test]
    fn overrides_and_dismissing_reviews() {
        let (_dir, mut db) = db();
        let (a, _, _) = three_books(&mut db);
        organize::set_override(db.conn(), a, "title", " My Title ", 1).expect("set");
        organize::set_override(db.conn(), a, "authors", "One\nTwo", 1).expect("set");
        assert!(
            organize::set_override(db.conn(), a, "path", "x", 1).is_err(),
            "unknown field"
        );
        let o = organize::overrides(db.conn(), a).expect("get");
        assert_eq!(o.get("title").map(String::as_str), Some("My Title"));
        organize::set_override(db.conn(), a, "title", "", 2).expect("clear");
        assert!(
            !organize::overrides(db.conn(), a)
                .expect("get")
                .contains_key("title")
        );
        // Overrides are user data.
        let has: bool = db
            .conn()
            .query_row(
                &format!("SELECT {} FROM books WHERE id = ?1", write::HAS_USER_DATA),
                [a.0],
                |r| r.get(0),
            )
            .expect("q");
        assert!(has);

        db.conn().execute("INSERT INTO review_queue (book_id, reasons, created_at) VALUES (?1, '[\"missing_author\"]', 0)", [a.0]).expect("queue");
        assert_eq!(
            read::list_books(db.conn(), BookFilter::NeedsReview)
                .expect("q")
                .len(),
            1
        );
        organize::dismiss_review(db.conn(), a, 5).expect("dismiss");
        assert!(
            read::list_books(db.conn(), BookFilter::NeedsReview)
                .expect("q")
                .is_empty()
        );
    }

    #[test]
    fn merging_moves_tracks_and_user_data() {
        let (_dir, mut db) = db();
        let (a, b, c) = three_books(&mut db);
        let tb = read::tracks(db.conn(), b).expect("t")[0].id;
        organize::add_bookmark(db.conn(), b, Some(tb), 1, 1, 1).expect("bookmark");
        playback_progress(&db, b, tb);
        let favs = organize::create_collection(db.conn(), "Favs", 1).expect("c");
        organize::add_to_collection(db.conn(), favs, c).expect("add");

        organize::merge_books(&mut db, a, &[b, c, a], 10).expect("merge");
        let books = read::list_books(db.conn(), BookFilter::All).expect("books");
        assert_eq!(books.len(), 1);
        assert_eq!(
            track_paths(&db, a),
            [
                "/books/s/1.mp3",
                "/books/s/2.mp3",
                "/books/s/3.mp3",
                "/books/s/4.mp3"
            ]
        );
        assert_eq!(
            organize::bookmarks(db.conn(), a).expect("b").len(),
            1,
            "bookmark moved"
        );
        assert!(
            crate::playback::progress(db.conn(), a)
                .expect("p")
                .is_some(),
            "progress moved (target had none)"
        );
        assert_eq!(
            organize::collections_of(db.conn(), a).expect("c").len(),
            1,
            "collection membership moved"
        );
        let locked: bool = db
            .conn()
            .query_row(
                "SELECT grouping_locked FROM books WHERE id = ?1",
                [a.0],
                |r| r.get(0),
            )
            .expect("q");
        assert!(locked);
        assert!(organize::merge_books(&mut db, BookId(999), &[a], 11).is_err());
    }

    fn playback_progress(db: &Database, book: BookId, track: audium_core::TrackId) {
        crate::playback::save_progress(db.conn(), book, Some(track), 10, 2010, 5)
            .expect("progress");
    }

    #[test]
    fn splitting_moves_selected_tracks_to_a_new_book() {
        let (_dir, mut db) = db();
        let (a, _, _) = three_books(&mut db);
        let t = read::tracks(db.conn(), a).expect("t");
        organize::add_bookmark(db.conn(), a, Some(t[1].id), 1, 1001, 1).expect("bookmark");
        assert!(
            organize::split_book(&mut db, a, &[t[0].id, t[1].id], &uid(9), 2).is_err(),
            "cannot move every track"
        );
        assert!(organize::split_book(&mut db, a, &[], &uid(9), 2).is_err());
        let new = organize::split_book(&mut db, a, &[t[1].id], &uid(9), 2).expect("split");
        assert_eq!(track_paths(&db, a), ["/books/s/1.mp3"]);
        assert_eq!(track_paths(&db, new), ["/books/s/2.mp3"]);
        assert_eq!(
            organize::bookmarks(db.conn(), new).expect("b").len(),
            1,
            "bookmark followed its track"
        );
        let new_summary = read::book_summary(db.conn(), new)
            .expect("q")
            .expect("exists");
        assert_eq!(new_summary.authors, ["A"]);
        let (lib, dirs) = organize::book_folders(db.conn(), new).expect("folders");
        assert!(lib.is_some());
        assert_eq!(dirs, [std::path::PathBuf::from("/books/s")]);
    }
}

mod statistics {
    use super::*;
    use crate::playback::{self, SessionRecord};
    use crate::stats;

    /// Epoch ms of local midnight starting `day`, as SQLite computes it.
    fn local_midnight(db: &Database, day: &str) -> i64 {
        db.conn()
            .query_row(
                "SELECT CAST(strftime('%s', ?1, 'utc') AS INTEGER) * 1000",
                [day],
                |r| r.get(0),
            )
            .expect("q")
    }

    fn session(db: &Database, book: BookId, start: i64, end: i64) {
        let s = SessionRecord {
            book,
            started_at: start,
            ended_at: end,
            start_position_ms: 0,
            end_position_ms: end - start,
            listened_ms: end - start,
        };
        playback::record_session(db.conn(), &s).expect("session");
    }

    fn library(db: &mut Database) -> (BookId, BookId) {
        let lib = write::add_library(db, Path::new("/books")).expect("lib");
        write::apply(
            db,
            lib,
            &ScanChanges {
                books: vec![
                    book(
                        BookTarget::New(uid(1)),
                        "Long",
                        &["A"],
                        vec![track("/books/l.m4b", 100)],
                    ),
                    book(
                        BookTarget::New(uid(2)),
                        "Short",
                        &["A"],
                        vec![track("/books/s.m4b", 100)],
                    ),
                ],
                ..Default::default()
            },
        )
        .expect("apply");
        (book_id_by_title(db, "Long"), book_id_by_title(db, "Short"))
    }

    #[test]
    fn sessions_are_split_at_local_midnight() {
        let (_dir, mut db) = db();
        let (long, short) = library(&mut db);
        let midnight = local_midnight(&db, "2026-10-30");
        // 10 minutes before and 20 after local midnight.
        session(&db, long, midnight - 600_000, midnight + 1_200_000);
        // Same day, later.
        session(&db, short, midnight + 3_600_000, midnight + 3_900_000);
        let mut days = stats::listening_per_day(db.conn()).expect("days");
        days.sort();
        let d = |s| audium_core::stats::Date::parse(s).expect("date");
        assert_eq!(
            days,
            [(d("2026-10-29"), 600_000), (d("2026-10-30"), 1_500_000)]
        );
    }

    #[test]
    fn per_book_totals_and_finished_dates() {
        let (_dir, mut db) = db();
        let (long, short) = library(&mut db);
        let midnight = local_midnight(&db, "2026-10-30");
        session(&db, long, midnight + 1_000, midnight + 7_201_000);
        session(&db, long, midnight + 8_000_000, midnight + 8_600_000);
        session(&db, short, midnight + 9_000_000, midnight + 9_060_000);
        let totals = stats::per_book_totals(db.conn(), 10).expect("totals");
        assert_eq!(
            totals
                .iter()
                .map(|t| (t.title.as_str(), t.listened_ms, t.sessions))
                .collect::<Vec<_>>(),
            [("Long", 7_800_000, 2), ("Short", 60_000, 1)]
        );
        assert_eq!(stats::per_book_totals(db.conn(), 1).expect("top").len(), 1);

        playback::mark_finished(db.conn(), short, None, 100, 100, midnight + 9_100_000)
            .expect("finish");
        let finished = stats::finished_dates(db.conn()).expect("finished");
        assert_eq!(
            finished,
            [audium_core::stats::Date::parse("2026-10-30").expect("d")]
        );
        assert!(stats::today(db.conn()).expect("today").is_some());

        // Clearing removes the listening, not the finished books.
        assert_eq!(stats::clear_listening(db.conn()).expect("clear"), 3);
        assert!(
            stats::per_book_totals(db.conn(), 10)
                .expect("totals")
                .is_empty()
        );
        assert!(
            stats::listening_per_day(db.conn())
                .expect("days")
                .is_empty()
        );
        assert_eq!(stats::finished_dates(db.conn()).expect("finished").len(), 1);
    }
}

mod portability {
    use super::*;
    use crate::{organize, playback, portable};

    fn hashed(path: &str, hash: u8, ms: i64) -> TrackWrite {
        TrackWrite {
            partial_hash: Some(vec![hash; 16]),
            ..track(path, ms)
        }
    }

    /// The same three books; `prefix` changes every path. `hashes`: whether
    /// the third book's tracks keep their hashes (false = match by title).
    fn library(db: &mut Database, prefix: &str, third_hashed: bool, with_fourth: bool) {
        let lib = write::add_library(db, Path::new(prefix)).expect("lib");
        let mut third = book(
            BookTarget::New(uid(3)),
            "Third Book",
            &["Cara C"],
            vec![track(&format!("{prefix}/c.m4b"), 600_000)],
        );
        if third_hashed {
            third.tracks[0].partial_hash = Some(vec![3; 16]);
        }
        let mut books = vec![
            book(
                BookTarget::New(uid(1)),
                "First Book",
                &["Ann A"],
                vec![
                    hashed(&format!("{prefix}/a1.mp3"), 11, 1000),
                    hashed(&format!("{prefix}/a2.mp3"), 12, 2000),
                ],
            ),
            book(
                BookTarget::New(uid(2)),
                "Second Book",
                &["Bob B"],
                vec![hashed(&format!("{prefix}/b.m4b"), 2, 5000)],
            ),
            third,
        ];
        if with_fourth {
            books.push(book(
                BookTarget::New(uid(4)),
                "Only In A",
                &["Dee D"],
                vec![hashed(&format!("{prefix}/d.m4b"), 4, 100)],
            ));
        }
        write::apply(
            db,
            lib,
            &ScanChanges {
                books,
                ..Default::default()
            },
        )
        .expect("apply");
    }

    /// (progress (position, finished), bookmarks, sessions, speed, overrides)
    type UserData = (
        Option<(i64, bool)>,
        usize,
        usize,
        Option<f64>,
        Vec<(String, String)>,
    );

    fn user_data(db: &Database, title: &str) -> UserData {
        let id = book_id_by_title(db, title);
        let p = playback::progress(db.conn(), id)
            .expect("p")
            .map(|p| (p.book_position_ms, p.finished));
        let bookmarks = organize::bookmarks(db.conn(), id).expect("b").len();
        let sessions: i64 = db
            .conn()
            .query_row(
                "SELECT count(*) FROM sessions WHERE book_id = ?1",
                [id.0],
                |r| r.get(0),
            )
            .expect("s");
        let mut ov: Vec<(String, String)> = organize::overrides(db.conn(), id)
            .expect("o")
            .into_iter()
            .collect();
        ov.sort();
        (
            p,
            bookmarks,
            sessions as usize,
            playback::book_speed(db.conn(), id).expect("speed"),
            ov,
        )
    }

    #[test]
    fn export_import_round_trip_with_conflicts() {
        // Library A, with user data.
        let (_a_dir, mut a) = db();
        library(&mut a, "/home/me/books", true, true);
        let first = book_id_by_title(&a, "First Book");
        let second = book_id_by_title(&a, "Second Book");
        let third = book_id_by_title(&a, "Third Book");
        let fourth = book_id_by_title(&a, "Only In A");
        let t1 = read::tracks(a.conn(), first).expect("t");
        playback::save_progress(a.conn(), first, Some(t1[1].id), 500, 1500, 1_000).expect("p");
        playback::mark_finished(a.conn(), second, None, 5000, 5000, 2_000).expect("finished");
        playback::save_progress(a.conn(), third, None, 100, 100, 10_000).expect("p");
        playback::save_progress(a.conn(), fourth, None, 50, 50, 1_000).expect("p");
        organize::add_bookmark(a.conn(), first, Some(t1[1].id), 400, 1400, 3).expect("bm");
        organize::update_bookmark(a.conn(), 1, "Twist", "a note", 4).expect("bm");
        playback::record_session(
            a.conn(),
            &playback::SessionRecord {
                book: first,
                started_at: 100,
                ended_at: 9_100,
                start_position_ms: 0,
                end_position_ms: 1500,
                listened_ms: 9_000,
            },
        )
        .expect("s");
        organize::set_override(a.conn(), first, "title", "First Book (Edited)", 5).expect("ov");
        playback::set_book_speed(a.conn(), first, 1.25).expect("speed");
        let favs = organize::create_collection(a.conn(), "Favourites", 6).expect("c");
        organize::add_to_collection(a.conn(), favs, second).expect("c");
        organize::add_to_collection(a.conn(), favs, first).expect("c");
        playback::set_setting(a.conn(), "skip_back_ms", "10000").expect("setting");
        playback::set_setting(a.conn(), "scan_rules_version", "3").expect("setting");

        let doc = portable::export(a.conn(), "test", 99).expect("export");
        assert_eq!(doc.books.len(), 4);
        assert!(
            !doc.settings.contains_key("scan_rules_version"),
            "installation-specific"
        );
        let json = portable::to_json(&doc).expect("json");
        let parsed = portable::from_json(&json).expect("parse");
        assert_eq!(parsed, doc, "the JSON round-trips");

        // Library B: same books at other paths; the third book has no hashes;
        // "Only In A" is missing; B has newer progress for the third book.
        let (_b_dir, mut b) = db();
        library(&mut b, "/mnt/other", false, false);
        let b_third = book_id_by_title(&b, "Third Book");
        playback::save_progress(b.conn(), b_third, None, 300, 300, 20_000).expect("newer progress");

        let plan = portable::plan_import(b.conn(), &parsed).expect("plan");
        let s = plan.summary.clone();
        assert_eq!((s.books_in_file, s.matched), (4, 3));
        assert_eq!(s.unmatched, ["Only In A"]);
        assert_eq!(
            (s.progress_updated, s.progress_kept),
            (2, 1),
            "B's newer progress on Third Book is kept"
        );
        assert_eq!(
            (
                s.bookmarks_added,
                s.sessions_added,
                s.overrides_updated,
                s.speeds_set
            ),
            (1, 1, 1, 1)
        );
        assert_eq!(
            (
                s.collections_created,
                s.collection_entries_added,
                s.settings_changed
            ),
            (1, 2, 1)
        );
        assert_eq!(
            plan.books_with_new_overrides,
            [book_id_by_title(&b, "First Book")]
        );

        // Previewing changes nothing.
        assert_eq!(user_data(&b, "First Book").0, None);
        portable::apply_import(&mut b, &plan).expect("apply");

        let first_b = user_data(&b, "First Book");
        assert_eq!(
            first_b,
            user_data(&a, "First Book"),
            "progress, bookmark, session, speed, override"
        );
        assert_eq!(
            user_data(&b, "Second Book").0,
            Some((5000, true)),
            "finished state"
        );
        assert_eq!(
            user_data(&b, "Third Book").0,
            Some((300, false)),
            "newer local progress kept"
        );
        // Progress points at the matching track (found by hash, not index).
        let p = playback::progress(b.conn(), book_id_by_title(&b, "First Book"))
            .expect("p")
            .expect("some");
        let b_tracks = read::tracks(b.conn(), book_id_by_title(&b, "First Book")).expect("t");
        assert_eq!(p.track_id, Some(b_tracks[1].id));
        let coll = read::list_groups(b.conn(), read::GroupKind::Collections).expect("groups");
        assert_eq!(
            (coll[0].name.as_str(), coll[0].book_count),
            ("Favourites", 2)
        );
        assert_eq!(
            playback::setting(b.conn(), "skip_back_ms")
                .expect("s")
                .as_deref(),
            Some("10000")
        );
        assert_eq!(
            playback::setting(b.conn(), "scan_rules_version").expect("s"),
            None
        );

        // Importing the same file again changes nothing.
        let again = portable::plan_import(b.conn(), &parsed).expect("plan again");
        assert_eq!(again.summary.changes(), 0, "{:?}", again.summary);
    }

    #[test]
    fn empty_library_exports_settings_only() {
        let (_dir, db) = db();
        playback::set_setting(db.conn(), "skip_forward_ms", "45000").expect("setting");
        let doc = portable::export(db.conn(), "test", 1).expect("export");
        assert!(doc.books.is_empty());
        assert_eq!(
            doc.settings.get("skip_forward_ms").map(String::as_str),
            Some("45000")
        );
        assert_eq!(
            (doc.format.as_str(), doc.schema_version),
            ("audium-export", portable::SCHEMA_VERSION)
        );
    }
}
