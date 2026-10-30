//! Grouping heuristics: which files of a folder form which book.
//!
//! - every `.m4b` is one book; an `.m4a` is one book when it has chapters or
//!   is the only audio file of its folder (otherwise it is a track);
//! - ...except that such container files sharing an album tag (or, without
//!   albums, all numbered "n of N" by track tags) are the parts of one book;
//! - the other files of a folder (disc folders merged in) form one book...
//! - ...unless the folder looks like an author folder of loose files: then
//!   files are split by album tag or, without albums, by filename prefix.
//!
//! Track order prefers disc/track tags when every file has them, and falls
//! back to natural filename order.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use audium_core::natural_cmp;
use audium_core::text::match_key;
use audium_metadata::FileInfo;

use crate::discover::{DirGroup, FoundFile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    /// One container file (m4b, or a chaptered m4a).
    Container,
    /// A whole folder.
    Folder,
    /// Part of a folder, split by album tag.
    SplitByAlbum,
    /// Part of a folder, split by filename prefix.
    SplitByFilename,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Indexes into the group's files, in playback order.
    pub files: Vec<usize>,
    pub kind: GroupKind,
    /// Path (relative to the root) used for the folder-structure heuristic.
    /// For a book that is only part of a folder, the folder plus the book's
    /// own name, so "Author/Title" still applies.
    pub folder_guess: PathBuf,
    /// True when the order came from disc/track tags.
    pub ordered_by_tags: bool,
    /// Track numbers are partly missing or duplicated.
    pub inconsistent_numbering: bool,
}

fn ext_is(path: &Path, ext: &str) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Groups one folder's readable files into book candidates. `infos[i]` is the
/// probe result for `group.files[i]`; unreadable files (`None`) are left out.
pub fn group_folder(group: &DirGroup, infos: &[Option<FileInfo>]) -> Vec<Candidate> {
    let readable: Vec<usize> = (0..group.files.len())
        .filter(|&i| infos.get(i).is_some_and(Option::is_some))
        .collect();
    let info = |i: usize| infos[i].as_ref();
    let mut candidates = Vec::new();

    let only_one = readable.len() == 1;
    let (containers, rest): (Vec<usize>, Vec<usize>) = readable.iter().partition(|&&i| {
        let p = &group.files[i].path;
        ext_is(p, "m4b")
            || (ext_is(p, "m4a") && (only_one || info(i).is_some_and(|f| !f.chapters.is_empty())))
    });
    let container_books = group_containers(infos, &containers);
    let shares_folder = container_books.len() + usize::from(!rest.is_empty()) > 1;
    for files in container_books {
        if let [i] = files[..] {
            candidates.push(Candidate {
                files: vec![i],
                kind: GroupKind::Container,
                folder_guess: own_name_guess(group, i, shares_folder),
                ordered_by_tags: false,
                inconsistent_numbering: false,
            });
        } else {
            // A book split into several container files ("Part 1 of 12").
            let (files, ordered_by_tags, inconsistent_numbering) = order(group, infos, files);
            let folder_guess = if shares_folder {
                book_name_guess(group, infos, &files)
            } else {
                group.rel_dir.clone()
            };
            candidates.push(Candidate {
                files,
                kind: GroupKind::Container,
                folder_guess,
                ordered_by_tags,
                inconsistent_numbering,
            });
        }
    }
    if rest.is_empty() {
        return candidates;
    }

    let album_of = |i: usize| {
        info(i)
            .and_then(|f| f.tags.album.as_deref())
            .map(match_key)
            .filter(|k| !k.is_empty())
    };
    let mut albums: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut no_album = Vec::new();
    for &i in &rest {
        match album_of(i) {
            Some(k) => albums.entry(k).or_default().push(i),
            None => no_album.push(i),
        }
    }

    let parts: Vec<(GroupKind, Option<String>, Vec<usize>)> = if albums.len() >= 2 {
        // Several albums in one folder: one book per album. Untagged files
        // join the album their filename prefix matches, or form their own.
        let mut by_album: Vec<(GroupKind, Option<String>, Vec<usize>)> = albums
            .into_iter()
            .map(|(k, v)| (GroupKind::SplitByAlbum, Some(k), v))
            .collect();
        for (prefix, files) in split_by_prefix(group, &no_album) {
            match by_album
                .iter_mut()
                .find(|(_, k, _)| k.as_deref() == Some(prefix.as_str()))
            {
                Some((_, _, v)) => v.extend(files),
                None => by_album.push((GroupKind::SplitByFilename, Some(prefix), files)),
            }
        }
        by_album
    } else if rest.len() == 1 || looks_like_one_book(group, infos, &rest, !albums.is_empty()) {
        vec![(GroupKind::Folder, None, rest.clone())]
    } else {
        let split = split_by_prefix(group, &rest);
        if split.len() == 1 {
            vec![(GroupKind::Folder, None, rest.clone())]
        } else {
            split
                .into_iter()
                .map(|(k, v)| (GroupKind::SplitByFilename, Some(k), v))
                .collect()
        }
    };

    let several = parts.len() > 1 || shares_folder;
    for (kind, _key, files) in parts {
        let (files, ordered_by_tags, inconsistent_numbering) = order(group, infos, files);
        let folder_guess = if several || (files.len() == 1 && group.rel_dir.as_os_str().is_empty())
        {
            book_name_guess(group, infos, &files)
        } else {
            group.rel_dir.clone()
        };
        candidates.push(Candidate {
            files,
            kind,
            folder_guess,
            ordered_by_tags,
            inconsistent_numbering,
        });
    }
    candidates
}

/// Splits a folder's container files into books: those sharing an album tag
/// are one book; untagged ones are one book only when every file has a
/// unique track number and they agree on a track total above 1 (otherwise
/// untagged "Series 1.m4b", "Series 2.m4b" are separate volumes).
fn group_containers(infos: &[Option<FileInfo>], containers: &[usize]) -> Vec<Vec<usize>> {
    let tags = |i: usize| infos[i].as_ref().map(|f| &f.tags);
    let mut by_album: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut untagged = Vec::new();
    for &i in containers {
        match tags(i)
            .and_then(|t| t.album.as_deref())
            .map(match_key)
            .filter(|k| !k.is_empty())
        {
            Some(k) => by_album.entry(k).or_default().push(i),
            None => untagged.push(i),
        }
    }
    let mut books: Vec<Vec<usize>> = by_album.into_values().collect();

    let totals: Vec<Option<u32>> = untagged
        .iter()
        .map(|&i| tags(i).and_then(|t| t.track_total))
        .collect();
    let mut numbers: Vec<Option<u32>> = untagged
        .iter()
        .map(|&i| tags(i).and_then(|t| t.track))
        .collect();
    numbers.sort_unstable();
    let numbered = untagged.len() > 1
        && numbers.iter().all(Option::is_some)
        && numbers.windows(2).all(|w| w[0] != w[1])
        && totals
            .iter()
            .all(|t| t.is_some_and(|n| n > 1) && *t == totals[0]);
    if numbered {
        books.push(untagged);
    } else {
        books.extend(untagged.into_iter().map(|i| vec![i]));
    }
    // Keep the folder's file order between books.
    books.sort_by_key(|b| b.iter().min().copied());
    books
}

/// "Author/Book.m4b" sharing a folder with other books: guess from
/// "Author/Book". A container alone in "Author/Book/" uses the folder.
fn own_name_guess(group: &DirGroup, i: usize, shares_folder: bool) -> PathBuf {
    if shares_folder || group.rel_dir.as_os_str().is_empty() {
        group.rel_dir.join(stem(&group.files[i].path))
    } else {
        group.rel_dir.clone()
    }
}

/// For a book that is part of a folder: the folder joined with the album,
/// or the common filename prefix.
fn book_name_guess(group: &DirGroup, infos: &[Option<FileInfo>], files: &[usize]) -> PathBuf {
    let album = files
        .iter()
        .find_map(|&i| infos[i].as_ref().and_then(|f| f.tags.album.clone()));
    let name = album.unwrap_or_else(|| {
        let first = stem(&group.files[files[0]].path);
        if files.len() == 1 {
            first
        } else {
            display_prefix(&first)
        }
    });
    group.rel_dir.join(name)
}

/// One folder, one book, when files are numbered: most of them have a
/// track tag, or a number right after the part all names share
/// ("01 - Intro", "Chapitre 02 – fin", "The Hobbit - 03 - ..."), or they
/// share an album.
fn looks_like_one_book(
    group: &DirGroup,
    infos: &[Option<FileInfo>],
    files: &[usize],
    has_album: bool,
) -> bool {
    if has_album {
        return true;
    }
    let stems: Vec<String> = files.iter().map(|&i| stem(&group.files[i].path)).collect();
    let common = common_prefix_len(&stems);
    let numbered = files
        .iter()
        .zip(&stems)
        .filter(|&(&i, stem)| {
            infos[i].as_ref().is_some_and(|f| f.tags.track.is_some())
                || stem[common..]
                    .trim_start_matches([' ', '_', '-', '.', '#', '–'])
                    .starts_with(|c: char| c.is_ascii_digit())
        })
        .count();
    numbered * 5 >= files.len() * 4
}

/// Length in bytes of the longest common prefix, backed off so it never
/// ends inside a number (so "Part 10"/"Part 11" keep their whole numbers).
fn common_prefix_len(stems: &[String]) -> usize {
    let Some(first) = stems.first() else {
        return 0;
    };
    let mut len = first.len();
    for s in &stems[1..] {
        len = first
            .char_indices()
            .zip(s.chars())
            .take_while(|((_, a), b)| a == b)
            .last()
            .map_or(0, |((i, a), _)| i + a.len_utf8())
            .min(len);
    }
    while len > 0 && first[..len].ends_with(|c: char| c.is_ascii_digit()) {
        len -= 1;
    }
    len
}

fn split_by_prefix(group: &DirGroup, files: &[usize]) -> Vec<(String, Vec<usize>)> {
    let mut by: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for &i in files {
        by.entry(prefix_key(&stem(&group.files[i].path)))
            .or_default()
            .push(i);
    }
    by.into_iter().collect()
}

const PART_WORDS: [&str; 10] = [
    "part", "pt", "chapter", "ch", "track", "cd", "disc", "kapitel", "teil", "chapitre",
];

/// The book-identifying part of a file name: "First Story - Part 2" ->
/// "first story", "Chapter 07" -> "".
pub fn prefix_key(stem: &str) -> String {
    match_key(&display_prefix(stem))
}

fn display_prefix(stem: &str) -> String {
    let mut s = stem.trim().to_owned();
    // Trailing number, optionally preceded by a part word: " - Part 2", "_03".
    let without_digits = s.trim_end_matches(|c: char| c.is_ascii_digit());
    if without_digits.len() < s.len() {
        let mut t = without_digits
            .trim_end_matches([' ', '_', '-', '.', '#'])
            .to_owned();
        let lower = t.to_lowercase();
        if let Some(word) = PART_WORDS.iter().find(|w| lower.ends_with(*w)) {
            let cut = t.len() - word.len();
            let boundary = t[..cut].chars().last().is_none_or(|c| !c.is_alphanumeric());
            if boundary {
                t.truncate(cut);
            }
        }
        s = t
            .trim_end_matches([' ', '_', '-', '.', '#', ','])
            .to_owned();
    }
    s
}

/// Orders files: by (disc, track) tags when every file has a track number
/// and those are unique; otherwise naturally by disc folder and file name.
fn order(
    group: &DirGroup,
    infos: &[Option<FileInfo>],
    mut files: Vec<usize>,
) -> (Vec<usize>, bool, bool) {
    let file = |i: usize| -> &FoundFile { &group.files[i] };
    let tags = |i: usize| infos[i].as_ref().map(|f| &f.tags);
    let disc = |i: usize| {
        tags(i)
            .and_then(|t| t.disc)
            .or(file(i).disc_hint)
            .unwrap_or(0)
    };
    let track = |i: usize| tags(i).and_then(|t| t.track);

    let with_track = files.iter().filter(|&&i| track(i).is_some()).count();
    let mut keys: Vec<(u32, u32)> = files
        .iter()
        .filter_map(|&i| Some((disc(i), track(i)?)))
        .collect();
    keys.sort_unstable();
    let unique = keys.windows(2).all(|w| w[0] != w[1]);
    let by_tags = files.len() > 1 && with_track == files.len() && unique;
    let inconsistent = files.len() > 1 && ((with_track > 0 && with_track < files.len()) || !unique);

    let natural = |a: &usize, b: &usize| -> Ordering {
        file(*a)
            .disc_hint
            .unwrap_or(0)
            .cmp(&file(*b).disc_hint.unwrap_or(0))
            .then_with(|| {
                natural_cmp(
                    &file(*a).path.to_string_lossy(),
                    &file(*b).path.to_string_lossy(),
                )
            })
    };
    if by_tags {
        files.sort_by(|a, b| {
            (disc(*a), track(*a))
                .cmp(&(disc(*b), track(*b)))
                .then_with(|| natural(a, b))
        });
    } else {
        files.sort_by(natural);
    }
    (files, by_tags, inconsistent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use audium_metadata::{FileTags, RawChapter};

    struct Builder {
        rel: PathBuf,
        files: Vec<FoundFile>,
        infos: Vec<Option<FileInfo>>,
    }

    impl Builder {
        fn new(rel: &str) -> Self {
            Builder {
                rel: PathBuf::from(rel),
                files: vec![],
                infos: vec![],
            }
        }
        fn file(mut self, name: &str, tags: FileTags) -> Self {
            self.files.push(FoundFile {
                path: PathBuf::from("/root").join(&self.rel).join(name),
                size: 1,
                mtime_ms: 0,
                disc_hint: name
                    .split('/')
                    .next()
                    .filter(|_| name.contains('/'))
                    .and_then(crate::discover::disc_number),
            });
            self.infos.push(Some(FileInfo {
                tags,
                duration_ms: 1000,
                chapters: vec![],
            }));
            self
        }
        fn chaptered(mut self, name: &str) -> Self {
            self = self.file(name, FileTags::default());
            if let Some(Some(info)) = self.infos.last_mut() {
                info.chapters = vec![
                    RawChapter {
                        title: None,
                        start_ms: 0,
                    },
                    RawChapter {
                        title: None,
                        start_ms: 500,
                    },
                ];
            }
            self
        }
        fn broken(mut self, name: &str) -> Self {
            self = self.file(name, FileTags::default());
            *self.infos.last_mut().expect("file") = None;
            self
        }
        fn run(&self) -> Vec<Candidate> {
            let group = DirGroup {
                dir: PathBuf::from("/root").join(&self.rel),
                rel_dir: self.rel.clone(),
                files: self.files.clone(),
            };
            group_folder(&group, &self.infos)
        }
        fn names(&self, c: &Candidate) -> Vec<String> {
            c.files
                .iter()
                .map(|&i| {
                    self.files[i]
                        .path
                        .strip_prefix(Path::new("/root").join(&self.rel))
                        .expect("prefix")
                        .to_string_lossy()
                        .into_owned()
                })
                .collect()
        }
    }

    fn album(a: &str) -> FileTags {
        FileTags {
            album: Some(a.into()),
            ..FileTags::default()
        }
    }

    fn numbered(disc: Option<u32>, track: u32) -> FileTags {
        FileTags {
            disc,
            track: Some(track),
            ..FileTags::default()
        }
    }

    #[test]
    fn one_m4b_is_one_book_and_uses_its_folder() {
        let b = Builder::new("Author/Book").file("book.m4b", FileTags::default());
        let c = b.run();
        assert_eq!(c.len(), 1);
        assert_eq!(
            (c[0].kind, c[0].folder_guess.clone()),
            (GroupKind::Container, PathBuf::from("Author/Book"))
        );
    }

    #[test]
    fn many_m4b_in_an_author_folder_are_separate_books() {
        let b = Builder::new("Author")
            .file("First.m4b", FileTags::default())
            .file("Second.m4b", FileTags::default());
        let c = b.run();
        assert_eq!(c.len(), 2);
        assert!(c.iter().all(|c| c.kind == GroupKind::Container));
        assert_eq!(c[0].folder_guess, PathBuf::from("Author/First"));
    }

    #[test]
    fn m4b_parts_sharing_an_album_are_one_book() {
        // An audiobook split into numbered m4b parts (seen in a real library).
        let part = |n: u32| FileTags {
            album: Some("Il profeta dell'incubo: Il Ciclo del sogno".into()),
            title: Some(format!("{n} - Il profeta dell'incubo: Capitolo {n}")),
            track: Some(n),
            track_total: Some(12),
            ..FileTags::default()
        };
        let mut b = Builder::new("Il profeta dell_incubo");
        // Listed in the order a directory read returned them.
        for n in [6, 5, 4, 3, 12, 1, 7, 9, 2, 8, 11, 10] {
            b = b.file(&format!("Il profeta dell_incubo - Il Ciclo del sogno [B07KJGK3WY] - {n:02} - Capitolo {n}.m4b"), part(n));
        }
        let c = b.run();
        assert_eq!(c.len(), 1, "{c:?}");
        assert_eq!(c[0].kind, GroupKind::Container);
        assert!(c[0].ordered_by_tags && !c[0].inconsistent_numbering);
        let order: Vec<String> = b
            .names(&c[0])
            .iter()
            .map(|n| n[n.len() - 15..].to_string())
            .collect();
        assert!(
            order[0].ends_with("Capitolo 1.m4b")
                && order[1].ends_with("Capitolo 2.m4b")
                && order[11].ends_with("Capitolo 12.m4b"),
            "{order:?}"
        );
        assert_eq!(c[0].folder_guess, PathBuf::from("Il profeta dell_incubo"));
    }

    #[test]
    fn untagged_numbered_m4b_merge_only_with_consistent_track_tags() {
        let n = |track: u32, total: u32| FileTags {
            track: Some(track),
            track_total: Some(total),
            ..FileTags::default()
        };
        let b = Builder::new("A/B")
            .file("x 2.m4b", n(2, 2))
            .file("x 1.m4b", n(1, 2));
        assert_eq!(b.run().len(), 1);
        // No tags at all: separate volumes.
        let b = Builder::new("A")
            .file("Series 1.m4b", FileTags::default())
            .file("Series 2.m4b", FileTags::default());
        assert_eq!(b.run().len(), 2);
        // Disagreeing totals: separate.
        let b = Builder::new("A")
            .file("a.m4b", n(1, 3))
            .file("b.m4b", n(2, 5));
        assert_eq!(b.run().len(), 2);
        // Different albums: separate books.
        let b = Builder::new("Ann")
            .file("One.m4b", album("One"))
            .file("Two.m4b", album("Two"));
        assert_eq!(b.run().len(), 2);
    }

    #[test]
    fn m4a_tracks_without_chapters_form_a_folder_book() {
        let b = Builder::new("A/B")
            .file("01.m4a", numbered(None, 1))
            .file("02.m4a", numbered(None, 2));
        let c = b.run();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].kind, GroupKind::Folder);
        let b = Builder::new("A")
            .chaptered("Book.m4a")
            .file("Other.m4a", FileTags::default());
        assert_eq!(
            b.run()
                .iter()
                .filter(|c| c.kind == GroupKind::Container)
                .count(),
            1
        );
    }

    #[test]
    fn folder_of_tracks_is_one_book_in_natural_order() {
        let b = Builder::new("Bea/Book")
            .file("Track 10.mp3", FileTags::default())
            .file("Track 2.mp3", FileTags::default())
            .file("Track 1.mp3", FileTags::default());
        let c = b.run();
        assert_eq!(c.len(), 1);
        assert_eq!(
            b.names(&c[0]),
            ["Track 1.mp3", "Track 2.mp3", "Track 10.mp3"]
        );
        assert_eq!(
            (c[0].kind, c[0].folder_guess.clone()),
            (GroupKind::Folder, PathBuf::from("Bea/Book"))
        );
        assert!(!c[0].ordered_by_tags);
    }

    #[test]
    fn numbered_track_titles_are_not_split() {
        let b = Builder::new("A/B")
            .file("01 - Intro.mp3", FileTags::default())
            .file("02 - The Beginning.mp3", FileTags::default())
            .file("03 - End.mp3", FileTags::default());
        assert_eq!(b.run().len(), 1);
    }

    #[test]
    fn numbers_after_a_shared_prefix_are_one_book() {
        for names in [
            [
                "Chapitre 01 – début.mp3",
                "Chapitre 02 – fin.mp3",
                "Chapitre 10 – épilogue.mp3",
            ],
            [
                "The Hobbit - 01 - An Unexpected Party.mp3",
                "The Hobbit - 02 - Roast Mutton.mp3",
                "The Hobbit - 03 - A Short Rest.mp3",
            ],
            ["Part 10.mp3", "Part 11.mp3", "Part 9.mp3"],
        ] {
            let mut b = Builder::new("A/B");
            for n in names {
                b = b.file(n, FileTags::default());
            }
            let c = b.run();
            assert_eq!(c.len(), 1, "{names:?}");
        }
        assert_eq!(common_prefix_len(&["Part 10".into(), "Part 11".into()]), 5);
        assert_eq!(common_prefix_len(&["ab".into(), "xy".into()]), 0);
        assert_eq!(
            common_prefix_len(&["été 1".into(), "été 2".into()]),
            "été ".len()
        );
    }

    #[test]
    fn disc_folders_merge_and_tags_beat_filenames() {
        // Filenames restart per disc; tags put them in the right order.
        let b = Builder::new("Cy/Disc Book")
            .file("CD2/01.mp3", numbered(Some(2), 1))
            .file("CD1/02.mp3", numbered(Some(1), 2))
            .file("CD1/01.mp3", numbered(Some(1), 1))
            .file("CD2/02.mp3", numbered(Some(2), 2));
        let c = b.run();
        assert_eq!(c.len(), 1);
        assert_eq!(
            b.names(&c[0]),
            ["CD1/01.mp3", "CD1/02.mp3", "CD2/01.mp3", "CD2/02.mp3"]
        );
        assert!(c[0].ordered_by_tags && !c[0].inconsistent_numbering);
    }

    #[test]
    fn disc_folders_without_tags_use_folder_order() {
        let b = Builder::new("Cy/Disc Book")
            .file("Disc 10/01.mp3", FileTags::default())
            .file("Disc 2/02.mp3", FileTags::default())
            .file("Disc 2/01.mp3", FileTags::default());
        let c = b.run();
        assert_eq!(
            b.names(&c[0]),
            ["Disc 2/01.mp3", "Disc 2/02.mp3", "Disc 10/01.mp3"]
        );
    }

    #[test]
    fn track_tags_override_misleading_filenames() {
        let b = Builder::new("A/B")
            .file("a.mp3", numbered(None, 3))
            .file("b.mp3", numbered(None, 1))
            .file("c.mp3", numbered(None, 2));
        let c = b.run();
        assert_eq!(b.names(&c[0]), ["b.mp3", "c.mp3", "a.mp3"]);
    }

    #[test]
    fn partial_or_duplicate_track_numbers_fall_back_and_are_flagged() {
        let b = Builder::new("A/B")
            .file("2.mp3", numbered(None, 1))
            .file("1.mp3", FileTags::default());
        let c = b.run();
        assert_eq!(b.names(&c[0]), ["1.mp3", "2.mp3"]);
        assert!(c[0].inconsistent_numbering && !c[0].ordered_by_tags);

        let b = Builder::new("A/B")
            .file("x.mp3", numbered(None, 1))
            .file("y.mp3", numbered(None, 1));
        assert!(b.run()[0].inconsistent_numbering);
    }

    #[test]
    fn author_folder_splits_by_album_tag() {
        let b = Builder::new("Dee")
            .file("a1.mp3", album("First Story"))
            .file("b1.mp3", album("Second Story"))
            .file("a2.mp3", album("First Story"))
            .file("Second Story 3.mp3", FileTags::default());
        let c = b.run();
        assert_eq!(c.len(), 2, "{c:?}");
        assert_eq!(b.names(&c[0]), ["a1.mp3", "a2.mp3"]);
        assert_eq!(
            b.names(&c[1]),
            ["b1.mp3", "Second Story 3.mp3"],
            "untagged file joins by prefix"
        );
        assert!(c.iter().all(|c| c.kind == GroupKind::SplitByAlbum));
        assert_eq!(c[0].folder_guess, PathBuf::from("Dee/First Story"));
    }

    #[test]
    fn author_folder_splits_by_filename_prefix() {
        let b = Builder::new("Dee")
            .file("First Story - Part 1.mp3", FileTags::default())
            .file("First Story - Part 2.mp3", FileTags::default())
            .file("Second Story - Part 1.mp3", FileTags::default());
        let c = b.run();
        assert_eq!(c.len(), 2);
        assert_eq!(
            b.names(&c[0]),
            ["First Story - Part 1.mp3", "First Story - Part 2.mp3"]
        );
        assert_eq!(c[0].kind, GroupKind::SplitByFilename);
        assert_eq!(c[0].folder_guess, PathBuf::from("Dee/First Story"));
        assert_eq!(
            c[1].folder_guess,
            PathBuf::from("Dee/Second Story - Part 1")
        );
    }

    #[test]
    fn one_album_tag_keeps_the_folder_together() {
        let b = Builder::new("A/B")
            .file("intro.mp3", album("Book"))
            .file("middle.mp3", album("Book"))
            .file("end.mp3", FileTags::default());
        assert_eq!(b.run().len(), 1);
    }

    #[test]
    fn mixed_container_and_tracks() {
        let b = Builder::new("Ann")
            .file("Novel.m4b", FileTags::default())
            .file("Story 1.mp3", FileTags::default())
            .file("Story 2.mp3", FileTags::default());
        let c = b.run();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].kind, GroupKind::Container);
        assert_eq!(c[0].folder_guess, PathBuf::from("Ann/Novel"));
        assert_eq!(c[1].folder_guess, PathBuf::from("Ann/Story"));
    }

    #[test]
    fn unreadable_files_are_left_out() {
        let b = Builder::new("A/B")
            .file("1.mp3", FileTags::default())
            .broken("2.mp3")
            .file("3.mp3", FileTags::default());
        let c = b.run();
        assert_eq!(b.names(&c[0]), ["1.mp3", "3.mp3"]);
        assert!(Builder::new("A").broken("x.mp3").run().is_empty());
    }

    #[test]
    fn single_file_in_the_root_uses_its_own_name() {
        let b = Builder::new("").file("Ann Author - Book.mp3", FileTags::default());
        assert_eq!(b.run()[0].folder_guess, PathBuf::from("Ann Author - Book"));
    }

    #[test]
    fn prefixes() {
        assert_eq!(prefix_key("First Story - Part 2"), "first story");
        assert_eq!(prefix_key("Chapter 07"), "");
        assert_eq!(prefix_key("book_03"), "book");
        assert_eq!(prefix_key("Dune"), "dune");
        assert_eq!(prefix_key("Report 1984"), "report");
        assert_eq!(
            prefix_key("Department 2"),
            "department",
            "\"part\" inside a word is kept"
        );
    }
}
