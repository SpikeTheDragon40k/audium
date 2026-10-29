//! Sidecar files next to a book: `metadata.json`, `desc.txt`, `reader.txt`,
//! `cover.*`, `folder.*`. Names match case-insensitively.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use audium_core::text::{clean, clean_multiline};
use serde_json::Value;

use crate::limits;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sidecars {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub narrators: Vec<String>,
    pub series: Option<String>,
    pub series_index: Option<String>,
    pub description: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub cover: Option<PathBuf>,
}

const COVER_STEMS: [&str; 2] = ["cover", "folder"];
const IMAGE_EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];

/// Reads the sidecars in `dir`. Unreadable or malformed files are skipped.
pub fn read_sidecars(dir: &Path) -> Sidecars {
    let mut out = Sidecars::default();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    let mut files: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| Some((e.file_name().to_str()?.to_lowercase(), e.path())))
        .collect();
    files.sort();

    for (name, path) in &files {
        match name.as_str() {
            "metadata.json" => {
                if let Some(json) =
                    read_small(path).and_then(|s| serde_json::from_str::<Value>(&s).ok())
                {
                    apply_metadata_json(&mut out, &json);
                }
            }
            "desc.txt" if out.description.is_none() => {
                out.description =
                    read_small(path).and_then(|s| clean_multiline(&s, limits::DESCRIPTION));
            }
            "reader.txt" if out.narrators.is_empty() => {
                out.narrators = read_small(path)
                    .map(|s| split_people(s.lines().next().unwrap_or("")))
                    .unwrap_or_default();
            }
            _ => {}
        }
    }
    // cover.* wins over folder.*; within a stem, the extension order decides.
    out.cover = COVER_STEMS.iter().find_map(|stem| {
        IMAGE_EXTENSIONS.iter().find_map(|ext| {
            let wanted = format!("{stem}.{ext}");
            files
                .iter()
                .find(|(n, _)| *n == wanted)
                .map(|(_, p)| p.clone())
        })
    });
    out
}

fn read_small(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > limits::SIDECAR_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(limits::SIDECAR_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Some(
        text.strip_prefix('\u{feff}')
            .map(str::to_owned)
            .unwrap_or(text),
    )
}

/// Audiobookshelf-style `metadata.json`. Unknown fields are ignored; fields
/// of the wrong type are skipped.
fn apply_metadata_json(out: &mut Sidecars, json: &Value) {
    let str_field = |key: &str| {
        json.get(key)
            .and_then(Value::as_str)
            .and_then(|s| clean(s, limits::NAME))
    };
    out.title = str_field("title");
    out.subtitle = str_field("subtitle");
    out.authors = string_list(json.get("authors"));
    out.narrators = string_list(json.get("narrators"));
    out.description = json
        .get("description")
        .and_then(Value::as_str)
        .and_then(|s| clean_multiline(s, limits::DESCRIPTION));
    out.year = json.get("publishedYear").and_then(|v| match v {
        Value::String(s) => s.trim().get(..4)?.parse().ok(),
        Value::Number(n) => n.as_i64().and_then(|n| i32::try_from(n).ok()),
        _ => None,
    });
    out.genre = string_list(json.get("genres")).into_iter().next();
    out.publisher = str_field("publisher");
    out.language = str_field("language");
    // "series": ["Name #3"] or [{"name": "...", "sequence": "3"}]
    if let Some(first) = json
        .get("series")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
    {
        match first {
            Value::String(s) => {
                let (name, index) = split_series(s);
                out.series = clean(name, limits::NAME);
                out.series_index = index.and_then(|i| clean(i, limits::NAME));
            }
            Value::Object(o) => {
                out.series = o
                    .get("name")
                    .and_then(Value::as_str)
                    .and_then(|s| clean(s, limits::NAME));
                out.series_index = o
                    .get("sequence")
                    .and_then(Value::as_str)
                    .and_then(|s| clean(s, limits::NAME));
            }
            _ => {}
        }
    }
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| match v {
                Value::String(s) => clean(s, limits::NAME),
                Value::Object(o) => o
                    .get("name")
                    .and_then(Value::as_str)
                    .and_then(|s| clean(s, limits::NAME)),
                _ => None,
            })
            .take(64)
            .collect(),
        Some(Value::String(s)) => split_people(s),
        _ => Vec::new(),
    }
}

/// "Name #3" -> ("Name", Some("3")).
pub fn split_series(s: &str) -> (&str, Option<&str>) {
    match s.rsplit_once('#') {
        Some((name, index)) if !name.trim().is_empty() && !index.trim().is_empty() => {
            (name.trim(), Some(index.trim()))
        }
        _ => (s.trim(), None),
    }
}

/// Splits "A; B / C" into names. Commas are kept ("Last, First").
pub fn split_people(s: &str) -> Vec<String> {
    s.split([';', '/'])
        .filter_map(|p| clean(p, limits::NAME))
        .take(64)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, content: &[u8]) {
        fs::write(dir.join(name), content).expect("write sidecar");
    }

    #[test]
    fn reads_all_sidecar_kinds() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "metadata.json",
            br#"{"title":" Json Title ","authors":["Ann A","Bob B"],"narrators":["Nia N"],
                 "series":["The Saga #2"],"publishedYear":"2011","genres":["Fantasy"],"description":"From json"}"#,
        );
        write(dir.path(), "desc.txt", "\u{feff}From desc.txt".as_bytes());
        write(dir.path(), "reader.txt", b"Other Reader\n");
        write(dir.path(), "Folder.JPG", &[0xFF, 0xD8, 0xFF, 0xE0]);
        write(dir.path(), "cover.png", b"\x89PNG\r\n\x1a\n");

        let s = read_sidecars(dir.path());
        assert_eq!(s.title.as_deref(), Some("Json Title"));
        assert_eq!(s.authors, vec!["Ann A", "Bob B"]);
        // metadata.json wins over desc.txt and reader.txt.
        assert_eq!(s.narrators, vec!["Nia N"]);
        assert_eq!(s.description.as_deref(), Some("From json"));
        assert_eq!(s.series.as_deref(), Some("The Saga"));
        assert_eq!(s.series_index.as_deref(), Some("2"));
        assert_eq!(s.year, Some(2011));
        assert_eq!(s.genre.as_deref(), Some("Fantasy"));
        assert_eq!(
            s.cover.as_deref().and_then(Path::file_name),
            Some("cover.png".as_ref())
        );
    }

    #[test]
    fn text_sidecars_without_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "DESC.TXT", b"Line one\n\n\nLine two");
        write(
            dir.path(),
            "reader.txt",
            b"Ann Reader; Bo Reader\nignored second line",
        );
        write(dir.path(), "folder.jpeg", &[0xFF, 0xD8, 0xFF]);
        let s = read_sidecars(dir.path());
        assert_eq!(s.description.as_deref(), Some("Line one\n\nLine two"));
        assert_eq!(s.narrators, vec!["Ann Reader", "Bo Reader"]);
        assert!(s.cover.is_some());
    }

    #[test]
    fn malformed_and_oversized_files_are_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "metadata.json", b"{ not json");
        write(
            dir.path(),
            "desc.txt",
            &vec![b'x'; (limits::SIDECAR_BYTES + 1) as usize],
        );
        let s = read_sidecars(dir.path());
        assert_eq!(s, Sidecars::default());
        assert_eq!(
            read_sidecars(&dir.path().join("missing")),
            Sidecars::default()
        );
    }

    #[test]
    fn wrong_json_types_are_skipped() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "metadata.json",
            br#"{"title":42,"authors":"Solo Author","series":[{"name":"S","sequence":"1.5"}]}"#,
        );
        let s = read_sidecars(dir.path());
        assert_eq!(s.title, None);
        assert_eq!(s.authors, vec!["Solo Author"]);
        assert_eq!(s.series.as_deref(), Some("S"));
        assert_eq!(s.series_index.as_deref(), Some("1.5"));
    }

    #[test]
    fn series_and_people_splitting() {
        assert_eq!(split_series("Saga #3"), ("Saga", Some("3")));
        assert_eq!(split_series("No Index"), ("No Index", None));
        assert_eq!(split_series("#5"), ("#5", None));
        assert_eq!(
            split_people("Doe, Jane; Roe, Rick / Kim"),
            vec!["Doe, Jane", "Roe, Rick", "Kim"]
        );
    }
}
