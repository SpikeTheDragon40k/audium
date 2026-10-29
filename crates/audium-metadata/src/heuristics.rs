//! Guesses from folder structure and file names, the last resorts of the
//! resolution chain.

use std::ffi::OsStr;
use std::path::Path;

use audium_core::text::clean;

use crate::limits;

/// What a book's location suggests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathGuess {
    pub title: Option<String>,
    pub author: Option<String>,
    pub series: Option<String>,
}

fn name(os: &OsStr) -> Option<String> {
    clean(&os.to_string_lossy(), limits::NAME)
}

/// `Author/Series/Book` or `Author/Book`, relative to the library root.
/// `book_dir` is the book's folder relative to the root (empty for books
/// directly in the root).
pub fn guess_from_folders(book_dir: &Path) -> PathGuess {
    let parts: Vec<String> = book_dir
        .components()
        .filter_map(|c| name(c.as_os_str()))
        .collect();
    let mut guess = PathGuess::default();
    match parts.as_slice() {
        [] => {}
        [book] => {
            // A single folder may be "Author - Title".
            match split_author_title(book) {
                Some((author, title)) => {
                    guess.author = Some(author);
                    guess.title = Some(title);
                }
                None => guess.title = Some(book.clone()),
            }
        }
        [.., author, book] if parts.len() == 2 => {
            guess.author = Some(author.clone());
            guess.title = Some(book.clone());
        }
        [.., author, series, book] => {
            guess.author = Some(author.clone());
            guess.series = Some(series.clone());
            guess.title = Some(book.clone());
        }
        _ => {}
    }
    guess
}

/// For a single-file book: "Author - Title.m4b", else the plain stem.
pub fn guess_from_filename(file: &Path) -> PathGuess {
    let Some(stem) = file.file_stem().and_then(name) else {
        return PathGuess::default();
    };
    match split_author_title(&stem) {
        Some((author, title)) => PathGuess {
            author: Some(author),
            title: Some(title),
            series: None,
        },
        None => PathGuess {
            title: Some(stem),
            ..PathGuess::default()
        },
    }
}

/// "Author - Title" -> (Author, Title) when there is exactly one " - ".
fn split_author_title(s: &str) -> Option<(String, String)> {
    let mut parts = s.split(" - ");
    let (a, b) = (parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let (a, b) = (clean(a, limits::NAME)?, clean(b, limits::NAME)?);
    // "01 - Intro" is a track name, not "Author - Title".
    if a.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_layouts() {
        let g = guess_from_folders(Path::new("Ann Author/The Saga/Book One"));
        assert_eq!(g.author.as_deref(), Some("Ann Author"));
        assert_eq!(g.series.as_deref(), Some("The Saga"));
        assert_eq!(g.title.as_deref(), Some("Book One"));

        let g = guess_from_folders(Path::new("Ann Author/Book One"));
        assert_eq!(
            (g.author.as_deref(), g.title.as_deref(), g.series),
            (Some("Ann Author"), Some("Book One"), None)
        );

        let g = guess_from_folders(Path::new("Ann Author - Book One"));
        assert_eq!(
            (g.author.as_deref(), g.title.as_deref()),
            (Some("Ann Author"), Some("Book One"))
        );

        let g = guess_from_folders(Path::new("Just A Title"));
        assert_eq!((g.author, g.title.as_deref()), (None, Some("Just A Title")));

        assert_eq!(guess_from_folders(Path::new("")), PathGuess::default());
        // Deeper trees use the last three levels.
        let g = guess_from_folders(Path::new("Audio/Books/Ann/Saga/One"));
        assert_eq!(g.author.as_deref(), Some("Ann"));
    }

    #[test]
    fn filenames() {
        let g = guess_from_filename(Path::new("x/Ann Author - Book One.m4b"));
        assert_eq!(
            (g.author.as_deref(), g.title.as_deref()),
            (Some("Ann Author"), Some("Book One"))
        );
        let g = guess_from_filename(Path::new("01 - Intro.mp3"));
        assert_eq!((g.author, g.title.as_deref()), (None, Some("01 - Intro")));
        let g = guess_from_filename(Path::new("A - B - C.mp3"));
        assert_eq!(g.title.as_deref(), Some("A - B - C"));
    }
}
