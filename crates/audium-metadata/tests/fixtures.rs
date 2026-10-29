//! Reads the ffmpeg-generated fixtures (scripts/make-fixtures.sh).

// Test helpers may panic on setup failures.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;

use audium_metadata::{probe, read_embedded_cover, read_sidecars};

fn fixtures() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/generated");
    assert!(
        dir.is_dir(),
        "fixtures missing: run scripts/make-fixtures.sh first"
    );
    dir
}

#[test]
fn m4b_tags_duration_chapters_and_cover() {
    let path = fixtures().join("single-m4b/Ada Example - Fixture Book With Chapters.m4b");
    let info = probe(&path).expect("probe m4b");
    assert_eq!(
        info.tags.title.as_deref(),
        Some("Fixture Book With Chapters")
    );
    assert_eq!(info.tags.artist.as_deref(), Some("Ada Example"));
    assert_eq!(info.tags.composer.as_deref(), Some("Nora Narrator"));
    assert!(
        (5900..=6200).contains(&info.duration_ms),
        "duration {}",
        info.duration_ms
    );
    let titles: Vec<_> = info.chapters.iter().map(|c| c.title.as_deref()).collect();
    assert_eq!(titles, [Some("Opening"), Some("Middle"), Some("Ending")]);
    assert_eq!(info.chapters[1].start_ms, 2000);
    let cover = read_embedded_cover(&path).expect("embedded cover");
    assert_eq!(cover.extension, "png");
}

#[test]
fn mp3_id3_chapters() {
    let info = probe(&fixtures().join("single-mp3-chapters/book.mp3")).expect("probe mp3");
    let starts: Vec<_> = info.chapters.iter().map(|c| c.start_ms).collect();
    assert_eq!(starts, [0, 2000, 4000]);
    assert_eq!(info.chapters[2].title.as_deref(), Some("Ending"));
    assert!(read_embedded_cover(&fixtures().join("single-mp3-chapters/book.mp3")).is_none());
}

#[test]
fn disc_and_track_tags() {
    let info = probe(&fixtures().join("multi-disc/Cy Author/Disc Book/CD2/01.mp3")).expect("probe");
    assert_eq!((info.tags.disc, info.tags.track), (Some(2), Some(1)));
    assert_eq!(
        (info.tags.disc_total, info.tags.track_total),
        (Some(2), Some(2))
    );
    assert_eq!(info.tags.album.as_deref(), Some("Disc Book"));
    assert!(info.chapters.is_empty());
}

#[test]
fn untagged_unicode_file() {
    let path = fixtures().join("unicode/Ünïcødé Àuthor/Livre d’été 本/Chapitre 01 – début.mp3");
    let info = probe(&path).expect("probe unicode");
    assert_eq!(info.tags, Default::default());
    assert!(info.duration_ms > 1500);
}

#[test]
fn corrupt_files_are_errors_not_panics() {
    assert!(probe(&fixtures().join("corrupt/garbage.m4b")).is_err());
    // A truncated mp3 still parses (duration is an estimate).
    assert!(probe(&fixtures().join("corrupt/truncated.mp3")).is_ok());
    assert!(probe(&fixtures().join("does-not-exist.mp3")).is_err());
    assert!(read_embedded_cover(&fixtures().join("corrupt/garbage.m4b")).is_none());
}

#[test]
fn folder_book_sidecars() {
    let s = read_sidecars(&fixtures().join("folder-book/Bea Writer/The Folder Book"));
    assert_eq!(s.description.as_deref(), Some("A synthetic description."));
    assert_eq!(s.narrators, vec!["Nora Narrator"]);
    assert!(s.cover.is_some());
}
