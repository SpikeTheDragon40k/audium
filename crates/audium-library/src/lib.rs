//! Library scanning, grouping heuristics and reconciliation.
//!
//! This crate never modifies, moves, renames or deletes audio files. The only
//! files it writes are cover images in the cache directory.

pub mod confidence;
pub mod discover;
pub mod grouping;
pub mod hash;
pub mod scan;

pub use scan::{Phase, ScanError, ScanOptions, ScanProgress, ScanSummary, fresh_uid, scan_library};

/// Version of the grouping and metadata rules. Raise it whenever they change
/// so that existing libraries are re-read once (a forced scan), since
/// unchanged folders are otherwise never regrouped.
// History: 2 = container parts sharing an album form one book; 3 = the same,
// re-applied because version 2 was recorded without re-reading (a bug).
pub const RULES_VERSION: u32 = 3;

/// Settings key holding the rules version the library was last scanned with.
pub const RULES_VERSION_KEY: &str = "scan_rules_version";

/// Audio file extensions the scanner picks up (lowercase).
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "m4b", "m4a", "mp3", "aac", "ogg", "oga", "opus", "flac", "wav", "wma", "mka",
];
