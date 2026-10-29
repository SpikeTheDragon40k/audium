//! The metadata resolution chain:
//! user overrides → sidecar files → embedded tags → folder structure → filename.
//!
//! Each resolved field records where it came from.

use std::collections::HashMap;

use crate::heuristics::PathGuess;
use crate::probe::FileTags;
use crate::sidecar::{Sidecars, split_people};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    Override,
    Sidecar,
    Embedded,
    Folder,
    Filename,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Override => "override",
            Source::Sidecar => "sidecar",
            Source::Embedded => "embedded",
            Source::Folder => "folder",
            Source::Filename => "filename",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field<T> {
    pub value: T,
    pub source: Source,
}

/// User edits, stored in the database, never written to files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub authors: Option<Vec<String>>,
    pub narrators: Option<Vec<String>>,
    pub series: Option<String>,
    pub series_index: Option<String>,
    pub description: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
}

pub struct BookInputs<'a> {
    pub overrides: &'a Overrides,
    pub sidecars: &'a Sidecars,
    /// Tags of the book's files, in book order.
    pub tracks: &'a [FileTags],
    pub folder: PathGuess,
    pub filename: PathGuess,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedMetadata {
    pub title: Option<Field<String>>,
    pub subtitle: Option<Field<String>>,
    pub authors: Option<Field<Vec<String>>>,
    pub narrators: Option<Field<Vec<String>>>,
    pub series: Option<Field<String>>,
    pub series_index: Option<String>,
    pub description: Option<Field<String>>,
    pub year: Option<Field<i32>>,
    pub genre: Option<Field<String>>,
    pub publisher: Option<String>,
    pub language: Option<String>,
}

fn first<T>(candidates: impl IntoIterator<Item = (Source, Option<T>)>) -> Option<Field<T>> {
    candidates
        .into_iter()
        .find_map(|(source, value)| value.map(|value| Field { value, source }))
}

fn non_empty(v: Vec<String>) -> Option<Vec<String>> {
    (!v.is_empty()).then_some(v)
}

/// The most common value of a tag across tracks (ties: first seen).
fn majority<T: Clone + Eq + std::hash::Hash>(
    tracks: &[FileTags],
    get: impl Fn(&FileTags) -> Option<&T>,
) -> Option<T> {
    let mut counts: HashMap<&T, (usize, usize)> = HashMap::new();
    for (i, value) in tracks.iter().filter_map(&get).enumerate() {
        counts.entry(value).or_insert((0, i)).0 += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.0.cmp(&b.1.0).then(b.1.1.cmp(&a.1.1)))
        .map(|(v, _)| v.clone())
}

pub fn resolve(input: &BookInputs<'_>) -> ResolvedMetadata {
    let o = input.overrides;
    let s = input.sidecars;
    let t = input.tracks;
    let single = t.len() == 1;

    // A multi-file book's title is its album; a single file may only have a title.
    let embedded_title = majority(t, |x| x.album.as_ref())
        .or_else(|| if single { t[0].title.clone() } else { None });
    let embedded_authors = majority(t, |x| x.album_artist.as_ref())
        .or_else(|| majority(t, |x| x.artist.as_ref()))
        .map(|a| split_people(&a))
        .and_then(non_empty);
    let embedded_narrators = majority(t, |x| x.composer.as_ref())
        .map(|a| split_people(&a))
        .and_then(non_empty);

    let series = first([
        (Source::Override, o.series.clone()),
        (Source::Sidecar, s.series.clone()),
        (Source::Embedded, majority(t, |x| x.series.as_ref())),
        (Source::Folder, input.folder.series.clone()),
    ]);
    let series_index = match series.as_ref().map(|f| f.source) {
        Some(Source::Override) => o.series_index.clone(),
        Some(Source::Sidecar) => s.series_index.clone(),
        Some(Source::Embedded) => majority(t, |x| x.series_index.as_ref()),
        _ => None,
    };

    ResolvedMetadata {
        title: first([
            (Source::Override, o.title.clone()),
            (Source::Sidecar, s.title.clone()),
            (Source::Embedded, embedded_title),
            (Source::Folder, input.folder.title.clone()),
            (Source::Filename, input.filename.title.clone()),
        ]),
        subtitle: first([
            (Source::Override, o.subtitle.clone()),
            (Source::Sidecar, s.subtitle.clone()),
        ]),
        authors: first([
            (Source::Override, o.authors.clone().and_then(non_empty)),
            (Source::Sidecar, non_empty(s.authors.clone())),
            (Source::Embedded, embedded_authors),
            (Source::Folder, input.folder.author.clone().map(|a| vec![a])),
            (
                Source::Filename,
                input.filename.author.clone().map(|a| vec![a]),
            ),
        ]),
        narrators: first([
            (Source::Override, o.narrators.clone().and_then(non_empty)),
            (Source::Sidecar, non_empty(s.narrators.clone())),
            (Source::Embedded, embedded_narrators),
        ]),
        series,
        series_index,
        description: first([
            (Source::Override, o.description.clone()),
            (Source::Sidecar, s.description.clone()),
            (
                Source::Embedded,
                t.iter().find_map(|x| x.description.clone()),
            ),
        ]),
        year: first([
            (Source::Override, o.year),
            (Source::Sidecar, s.year),
            (Source::Embedded, majority(t, |x| x.year.as_ref())),
        ]),
        genre: first([
            (Source::Override, o.genre.clone()),
            (Source::Sidecar, s.genre.clone()),
            (Source::Embedded, majority(t, |x| x.genre.as_ref())),
        ]),
        publisher: s
            .publisher
            .clone()
            .or_else(|| majority(t, |x| x.publisher.as_ref())),
        language: s
            .language
            .clone()
            .or_else(|| majority(t, |x| x.language.as_ref())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(album: &str, artist: &str) -> FileTags {
        FileTags {
            album: Some(album.into()),
            artist: Some(artist.into()),
            ..FileTags::default()
        }
    }

    fn folder(author: &str, title: &str) -> PathGuess {
        PathGuess {
            author: Some(author.into()),
            title: Some(title.into()),
            series: None,
        }
    }

    #[test]
    fn chain_order_is_respected_field_by_field() {
        let overrides = Overrides {
            title: Some("Override Title".into()),
            ..Overrides::default()
        };
        let sidecars = Sidecars {
            authors: vec!["Sidecar Author".into()],
            ..Sidecars::default()
        };
        let tracks = [tags("Embedded Album", "Embedded Artist")];
        let r = resolve(&BookInputs {
            overrides: &overrides,
            sidecars: &sidecars,
            tracks: &tracks,
            folder: folder("Folder Author", "Folder Title"),
            filename: PathGuess::default(),
        });
        assert_eq!(
            r.title,
            Some(Field {
                value: "Override Title".into(),
                source: Source::Override
            })
        );
        assert_eq!(
            r.authors.map(|f| (f.value, f.source)),
            Some((vec!["Sidecar Author".into()], Source::Sidecar))
        );
    }

    #[test]
    fn each_level_falls_through_to_the_next() {
        let none = Overrides::default();
        let empty = Sidecars::default();
        let tracks = [tags("Album", "Artist")];
        let base = |tracks: &[FileTags], folder_guess: PathGuess, filename: PathGuess| {
            resolve(&BookInputs {
                overrides: &none,
                sidecars: &empty,
                tracks,
                folder: folder_guess,
                filename,
            })
        };

        let r = base(&tracks, folder("F Author", "F Title"), PathGuess::default());
        assert_eq!(r.title.map(|f| f.source), Some(Source::Embedded));
        assert_eq!(r.authors.map(|f| f.source), Some(Source::Embedded));

        let untagged = [FileTags::default()];
        let r = base(
            &untagged,
            folder("F Author", "F Title"),
            PathGuess::default(),
        );
        assert_eq!(
            r.title.map(|f| (f.value, f.source)),
            Some(("F Title".into(), Source::Folder))
        );
        assert_eq!(r.authors.map(|f| f.source), Some(Source::Folder));

        let r = base(
            &untagged,
            PathGuess::default(),
            PathGuess {
                title: Some("File Title".into()),
                author: Some("File Author".into()),
                series: None,
            },
        );
        assert_eq!(r.title.map(|f| f.source), Some(Source::Filename));
        assert_eq!(r.authors.map(|f| f.source), Some(Source::Filename));

        let r = base(&untagged, PathGuess::default(), PathGuess::default());
        assert_eq!((r.title, r.authors), (None, None));
    }

    #[test]
    fn embedded_values_use_the_majority_and_album_artist() {
        let none = Overrides::default();
        let empty = Sidecars::default();
        let mut odd = tags("Wrong Album", "Narrator X");
        odd.album_artist = Some("Real Author".into());
        let tracks = [tags("Book", "A"), odd, tags("Book", "A")];
        let r = resolve(&BookInputs {
            overrides: &none,
            sidecars: &empty,
            tracks: &tracks,
            folder: PathGuess::default(),
            filename: PathGuess::default(),
        });
        assert_eq!(r.title.map(|f| f.value), Some("Book".into()));
        assert_eq!(r.authors.map(|f| f.value), Some(vec!["Real Author".into()]));
    }

    #[test]
    fn single_file_title_tag_and_composer_narrator() {
        let none = Overrides::default();
        let empty = Sidecars::default();
        let tracks = [FileTags {
            title: Some("Only Title".into()),
            composer: Some("N One; N Two".into()),
            series: Some("Saga".into()),
            series_index: Some("4".into()),
            ..FileTags::default()
        }];
        let r = resolve(&BookInputs {
            overrides: &none,
            sidecars: &empty,
            tracks: &tracks,
            folder: PathGuess::default(),
            filename: PathGuess::default(),
        });
        assert_eq!(r.title.map(|f| f.value), Some("Only Title".into()));
        assert_eq!(
            r.narrators.map(|f| f.value),
            Some(vec!["N One".into(), "N Two".into()])
        );
        assert_eq!(r.series.map(|f| f.value), Some("Saga".into()));
        assert_eq!(r.series_index.as_deref(), Some("4"));
    }

    #[test]
    fn empty_override_lists_do_not_hide_lower_levels() {
        let overrides = Overrides {
            authors: Some(vec![]),
            ..Overrides::default()
        };
        let empty = Sidecars::default();
        let tracks = [tags("B", "Tag Author")];
        let r = resolve(&BookInputs {
            overrides: &overrides,
            sidecars: &empty,
            tracks: &tracks,
            folder: PathGuess::default(),
            filename: PathGuess::default(),
        });
        assert_eq!(r.authors.map(|f| f.source), Some(Source::Embedded));
    }
}
