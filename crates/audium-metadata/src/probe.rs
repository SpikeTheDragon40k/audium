//! Reads tags, duration and chapters of one audio file.

use std::path::Path;
use std::time::Duration;

use audium_core::text::{clean, clean_multiline};
use lofty::config::ParseOptions;
use lofty::file::{AudioFile, FileType, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, Tag};

use crate::{MetadataError, limits};

/// Book-relevant tags of one file, already cleaned.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileTags {
    pub title: Option<String>,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub composer: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub track: Option<u32>,
    pub track_total: Option<u32>,
    pub disc: Option<u32>,
    pub disc_total: Option<u32>,
    /// "Movement"/"content group": what audiobook taggers use for series.
    pub series: Option<String>,
    pub series_index: Option<String>,
    pub description: Option<String>,
    pub publisher: Option<String>,
    pub language: Option<String>,
}

/// A chapter with a start position relative to its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawChapter {
    pub title: Option<String>,
    pub start_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileInfo {
    pub tags: FileTags,
    pub duration_ms: u64,
    pub chapters: Vec<RawChapter>,
}

/// Probes one audio file. Never modifies it.
pub fn probe(path: &Path) -> Result<FileInfo, MetadataError> {
    let display = path.display().to_string();
    let parse_error = |message: String| MetadataError::Parse {
        path: display.clone(),
        message,
    };

    let probe = Probe::open(path)
        .map_err(|e| parse_error(e.to_string()))?
        .options(ParseOptions::new().read_cover_art(false))
        .guess_file_type()
        .map_err(|e| MetadataError::Io {
            path: display.clone(),
            source: e,
        })?;
    let file_type = probe.file_type();
    let tagged = probe.read().map_err(|e| parse_error(e.to_string()))?;

    let duration_ms = duration_to_ms(tagged.properties().duration());
    let tags = tagged
        .primary_tag()
        .or_else(|| tagged.first_tag())
        .map(read_tags)
        .unwrap_or_default();

    let chapters = match file_type {
        Some(FileType::Mpeg) => id3_chapters(path),
        Some(FileType::Mp4) => mp4_chapters(path),
        _ => Vec::new(),
    };

    Ok(FileInfo {
        tags,
        duration_ms,
        chapters: sanitize_chapters(chapters, duration_ms),
    })
}

fn duration_to_ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

fn text(tag: &Tag, key: ItemKey) -> Option<String> {
    tag.get_string(key).and_then(|s| clean(s, limits::NAME))
}

fn read_tags(tag: &Tag) -> FileTags {
    let description = tag
        .get_string(ItemKey::Description)
        .or_else(|| tag.get_string(ItemKey::Comment))
        .and_then(|s| clean_multiline(s, limits::DESCRIPTION));
    let year = tag
        .get_string(ItemKey::Year)
        .or_else(|| tag.get_string(ItemKey::RecordingDate))
        .or_else(|| tag.get_string(ItemKey::OriginalReleaseDate))
        .and_then(parse_year);

    FileTags {
        title: tag.title().and_then(|s| clean(&s, limits::NAME)),
        album: tag.album().and_then(|s| clean(&s, limits::NAME)),
        artist: tag.artist().and_then(|s| clean(&s, limits::NAME)),
        album_artist: text(tag, ItemKey::AlbumArtist),
        composer: text(tag, ItemKey::Composer),
        genre: tag.genre().and_then(|s| clean(&s, limits::NAME)),
        year,
        track: tag.track().filter(|&n| n > 0),
        track_total: tag.track_total().filter(|&n| n > 0),
        disc: tag.disk().filter(|&n| n > 0),
        disc_total: tag.disk_total().filter(|&n| n > 0),
        series: text(tag, ItemKey::Movement).or_else(|| text(tag, ItemKey::ContentGroup)),
        series_index: text(tag, ItemKey::MovementNumber),
        description,
        publisher: text(tag, ItemKey::Publisher).or_else(|| text(tag, ItemKey::Label)),
        language: text(tag, ItemKey::Language),
    }
}

/// Extracts a plausible year from "2004", "2004-05-06", "2004 ..." etc.
fn parse_year(s: &str) -> Option<i32> {
    let digits: String = s.trim().chars().take_while(char::is_ascii_digit).collect();
    let year: i32 = digits.get(..4)?.parse().ok()?;
    (1000..=9999).contains(&year).then_some(year)
}

fn id3_chapters(path: &Path) -> Vec<RawChapter> {
    let Ok(tag) = id3::Tag::read_from_path(path) else {
        return Vec::new();
    };
    tag.chapters()
        .map(|c| RawChapter {
            title: c
                .frames
                .iter()
                .find(|f| f.id() == "TIT2")
                .and_then(|f| f.content().text())
                .and_then(|t| clean(t, limits::NAME)),
            start_ms: u64::from(c.start_time),
        })
        .collect()
}

fn mp4_chapters(path: &Path) -> Vec<RawChapter> {
    let Ok(tag) = mp4ameta::Tag::read_from_path(path) else {
        return Vec::new();
    };
    // Prefer the Nero chapter list; fall back to a QuickTime chapter track.
    let list = if tag.chapter_list().is_empty() {
        tag.chapter_track()
    } else {
        tag.chapter_list()
    };
    list.iter()
        .map(|c| RawChapter {
            title: clean(&c.title, limits::NAME),
            start_ms: duration_to_ms(c.start),
        })
        .collect()
}

/// Sorts chapters, drops duplicates and chapters outside the file, and caps
/// the count. A single chapter at 0 carries no information and is dropped.
fn sanitize_chapters(mut chapters: Vec<RawChapter>, duration_ms: u64) -> Vec<RawChapter> {
    chapters.retain(|c| duration_ms == 0 || c.start_ms < duration_ms);
    chapters.sort_by_key(|c| c.start_ms);
    chapters.dedup_by_key(|c| c.start_ms);
    chapters.truncate(limits::MAX_CHAPTERS);
    if chapters.len() == 1 && chapters[0].start_ms == 0 {
        chapters.clear();
    }
    chapters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn years() {
        assert_eq!(parse_year("2004"), Some(2004));
        assert_eq!(parse_year("2004-05-06T00:00:00Z"), Some(2004));
        assert_eq!(parse_year(" 1999 remaster"), Some(1999));
        assert_eq!(parse_year("04"), None);
        assert_eq!(parse_year("abcd"), None);
        assert_eq!(parse_year("0000"), None);
    }

    #[test]
    fn chapter_sanitizing() {
        let c = |start_ms, t: &str| RawChapter {
            title: Some(t.into()),
            start_ms,
        };
        let out = sanitize_chapters(
            vec![c(5000, "b"), c(0, "a"), c(5000, "dup"), c(99_000, "out")],
            10_000,
        );
        assert_eq!(out, vec![c(0, "a"), c(5000, "b")]);
        assert!(sanitize_chapters(vec![c(0, "only")], 10_000).is_empty());
    }
}
