//! Walks a library root and groups audio files by folder. Disc folders
//! ("CD1", "Disc 2", "Part 03", ...) are merged into their parent.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use walkdir::WalkDir;

use crate::AUDIO_EXTENSIONS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundFile {
    pub path: PathBuf,
    pub size: i64,
    pub mtime_ms: i64,
    /// Disc number taken from a disc folder name.
    pub disc_hint: Option<u32>,
}

/// Audio files of one folder (with its disc folders merged in).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirGroup {
    pub dir: PathBuf,
    /// `dir` relative to the library root.
    pub rel_dir: PathBuf,
    pub files: Vec<FoundFile>,
}

#[derive(Debug, thiserror::Error)]
pub enum DiscoverError {
    #[error("library folder {0} is not available")]
    Unavailable(String),
    #[error("scan cancelled")]
    Cancelled,
}

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

fn hidden(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|n| n.starts_with('.'))
}

/// Recursively finds audio files under `root`. Follows symlinks; loops and
/// unreadable entries are skipped (and logged). Hidden files and folders
/// are ignored. The same file reached twice (e.g. via a symlink) is listed once.
pub fn discover(root: &Path, cancel: &AtomicBool) -> Result<Vec<DirGroup>, DiscoverError> {
    if !root.is_dir() || std::fs::read_dir(root).is_err() {
        return Err(DiscoverError::Unavailable(root.display().to_string()));
    }
    let mut seen = HashSet::new();
    let mut by_dir: BTreeMap<PathBuf, Vec<FoundFile>> = BTreeMap::new();

    let walker = WalkDir::new(root)
        .follow_links(true)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !hidden(e.file_name()));
    for entry in walker {
        if cancel.load(Ordering::Relaxed) {
            return Err(DiscoverError::Cancelled);
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                // Includes symlink loops, which walkdir detects.
                tracing::warn!(error = %e, "skipping unreadable entry");
                continue;
            }
        };
        if !entry.file_type().is_file() || !is_audio(entry.path()) {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !seen.insert(file_identity(entry.path(), &meta)) {
            continue;
        }
        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        let Some(parent) = entry.path().parent() else {
            continue;
        };
        by_dir
            .entry(parent.to_owned())
            .or_default()
            .push(FoundFile {
                path: entry.path().to_owned(),
                size: i64::try_from(meta.len()).unwrap_or(i64::MAX),
                mtime_ms,
                disc_hint: None,
            });
    }

    // Merge disc folders into their parent folder.
    let mut groups: BTreeMap<PathBuf, Vec<FoundFile>> = BTreeMap::new();
    for (dir, mut files) in by_dir {
        let disc = (dir != root)
            .then(|| {
                dir.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(disc_number)
            })
            .flatten();
        let target = match (disc, dir.parent()) {
            (Some(n), Some(parent)) if parent.starts_with(root) => {
                for f in &mut files {
                    f.disc_hint = Some(n);
                }
                parent.to_owned()
            }
            _ => dir,
        };
        groups.entry(target).or_default().extend(files);
    }

    Ok(groups
        .into_iter()
        .map(|(dir, files)| DirGroup {
            rel_dir: dir
                .strip_prefix(root)
                .map(Path::to_owned)
                .unwrap_or_default(),
            dir,
            files,
        })
        .collect())
}

#[cfg(unix)]
fn file_identity(_path: &Path, meta: &std::fs::Metadata) -> (u64, u64, PathBuf) {
    use std::os::unix::fs::MetadataExt;
    (meta.dev(), meta.ino(), PathBuf::new())
}

#[cfg(not(unix))]
fn file_identity(path: &Path, _meta: &std::fs::Metadata) -> (u64, u64, PathBuf) {
    (0, 0, path.to_owned())
}

/// The disc number in a disc folder name: "CD1", "Disc 2", "disk_03",
/// "Part 03", "Vol. 2", "CD 1 of 3". Anything else returns `None`.
pub fn disc_number(name: &str) -> Option<u32> {
    const PREFIXES: [&str; 8] = ["disc", "disk", "cd", "part", "pt", "volume", "vol", "teil"];
    let lower = name.trim().to_lowercase();
    let rest = PREFIXES.iter().find_map(|p| lower.strip_prefix(p))?;
    let rest = rest.trim_start_matches([' ', '_', '-', '.', '#']);
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    let tail = rest[digits.len()..].trim();
    // Allow "of N" / "/N" after the number, nothing else.
    let tail_ok = tail.is_empty() || {
        let t = tail
            .strip_prefix("of")
            .or_else(|| tail.strip_prefix('/'))
            .map(str::trim);
        t.is_some_and(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()))
    };
    if !tail_ok {
        return None;
    }
    digits.parse().ok().filter(|&n: &u32| n < 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disc_folder_names() {
        for (name, n) in [
            ("CD1", 1),
            ("cd 2", 2),
            ("Disc 02", 2),
            ("disk_3", 3),
            ("Part 03", 3),
            ("Vol. 4", 4),
            ("CD 1 of 3", 1),
            ("Disc 2/3", 2),
        ] {
            assert_eq!(disc_number(name), Some(n), "{name}");
        }
        for name in [
            "CD",
            "Discography",
            "Part One",
            "Part 1 - The Beginning",
            "Book 1",
            "cd1x",
            "Partition 2",
            "01",
        ] {
            assert_eq!(disc_number(name), None, "{name}");
        }
    }

    #[test]
    fn audio_extensions() {
        assert!(is_audio(Path::new("a/B.M4B")));
        assert!(is_audio(Path::new("x.opus")));
        assert!(!is_audio(Path::new("cover.jpg")));
        assert!(!is_audio(Path::new("noext")));
    }
}
