//! Confidence score per book and the reasons that send a book to the
//! "Needs review" queue.

use audium_metadata::{ResolvedMetadata, Source};

use crate::grouping::{Candidate, GroupKind};

/// Books scoring below this are queued for review.
pub const REVIEW_THRESHOLD: u8 = 70;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    MissingTitle,
    MissingAuthor,
    TitleFromPath,
    AuthorFromPath,
    SplitByAlbum,
    SplitByFilename,
    InconsistentNumbering,
    UnreadableFiles,
}

impl Reason {
    pub fn code(self) -> &'static str {
        match self {
            Reason::MissingTitle => "missing_title",
            Reason::MissingAuthor => "missing_author",
            Reason::TitleFromPath => "title_from_path",
            Reason::AuthorFromPath => "author_from_path",
            Reason::SplitByAlbum => "split_by_album",
            Reason::SplitByFilename => "split_by_filename",
            Reason::InconsistentNumbering => "inconsistent_numbering",
            Reason::UnreadableFiles => "unreadable_files",
        }
    }

    fn penalty(self) -> u8 {
        match self {
            Reason::MissingTitle => 40,
            Reason::MissingAuthor => 30,
            Reason::TitleFromPath | Reason::AuthorFromPath => 10,
            Reason::SplitByAlbum => 15,
            Reason::SplitByFilename => 30,
            Reason::InconsistentNumbering => 15,
            Reason::UnreadableFiles => 20,
        }
    }

    /// Reasons that queue a book regardless of its score.
    fn always_review(self) -> bool {
        matches!(self, Reason::MissingTitle | Reason::MissingAuthor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    pub score: u8,
    pub reasons: Vec<Reason>,
    pub needs_review: bool,
}

pub fn assess(
    candidate: &Candidate,
    meta: &ResolvedMetadata,
    unreadable_in_folder: bool,
) -> Assessment {
    let from_path = |s: Source| matches!(s, Source::Folder | Source::Filename);
    let mut reasons = Vec::new();
    match &meta.title {
        None => reasons.push(Reason::MissingTitle),
        Some(f) if from_path(f.source) => reasons.push(Reason::TitleFromPath),
        Some(_) => {}
    }
    match &meta.authors {
        None => reasons.push(Reason::MissingAuthor),
        Some(f) if from_path(f.source) => reasons.push(Reason::AuthorFromPath),
        Some(_) => {}
    }
    match candidate.kind {
        GroupKind::SplitByAlbum => reasons.push(Reason::SplitByAlbum),
        GroupKind::SplitByFilename => reasons.push(Reason::SplitByFilename),
        GroupKind::Container | GroupKind::Folder => {}
    }
    if candidate.inconsistent_numbering {
        reasons.push(Reason::InconsistentNumbering);
    }
    if unreadable_in_folder {
        reasons.push(Reason::UnreadableFiles);
    }
    let penalty: u32 = reasons.iter().map(|r| u32::from(r.penalty())).sum();
    let score = 100u32.saturating_sub(penalty) as u8;
    let needs_review = score < REVIEW_THRESHOLD || reasons.iter().any(|r| r.always_review());
    Assessment {
        score,
        reasons,
        needs_review,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audium_metadata::Field;
    use std::path::PathBuf;

    fn candidate(kind: GroupKind) -> Candidate {
        Candidate {
            files: vec![0],
            kind,
            folder_guess: PathBuf::new(),
            ordered_by_tags: true,
            inconsistent_numbering: false,
        }
    }

    fn meta(title: Option<Source>, author: Option<Source>) -> ResolvedMetadata {
        ResolvedMetadata {
            title: title.map(|source| Field {
                value: "T".into(),
                source,
            }),
            authors: author.map(|source| Field {
                value: vec!["A".into()],
                source,
            }),
            ..ResolvedMetadata::default()
        }
    }

    #[test]
    fn well_tagged_book_is_confident() {
        let a = assess(
            &candidate(GroupKind::Container),
            &meta(Some(Source::Embedded), Some(Source::Embedded)),
            false,
        );
        assert_eq!((a.score, a.needs_review), (100, false));
        assert!(a.reasons.is_empty());
    }

    #[test]
    fn path_derived_metadata_is_fine_but_noted() {
        let a = assess(
            &candidate(GroupKind::Folder),
            &meta(Some(Source::Folder), Some(Source::Folder)),
            false,
        );
        assert_eq!((a.score, a.needs_review), (80, false));
        assert_eq!(
            a.reasons,
            vec![Reason::TitleFromPath, Reason::AuthorFromPath]
        );
    }

    #[test]
    fn missing_author_always_needs_review() {
        let a = assess(
            &candidate(GroupKind::Container),
            &meta(Some(Source::Embedded), None),
            false,
        );
        assert!(a.needs_review);
        assert_eq!(a.score, 70);
    }

    #[test]
    fn heuristic_splits_lower_the_score() {
        let a = assess(
            &candidate(GroupKind::SplitByFilename),
            &meta(Some(Source::Folder), Some(Source::Folder)),
            false,
        );
        assert!(a.needs_review && a.score < REVIEW_THRESHOLD);
        let a = assess(
            &candidate(GroupKind::SplitByAlbum),
            &meta(Some(Source::Embedded), Some(Source::Embedded)),
            false,
        );
        assert!(!a.needs_review);
        let mut c = candidate(GroupKind::Folder);
        c.inconsistent_numbering = true;
        let a = assess(&c, &meta(Some(Source::Folder), Some(Source::Folder)), true);
        assert!(a.needs_review);
        assert_eq!(
            a.reasons.iter().map(|r| r.code()).collect::<Vec<_>>(),
            [
                "title_from_path",
                "author_from_path",
                "inconsistent_numbering",
                "unreadable_files"
            ]
        );
    }
}
