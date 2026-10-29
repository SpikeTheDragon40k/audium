//! Library scanning, grouping heuristics and reconciliation.
//!
//! This crate never modifies, moves, renames or deletes audio files. The only
//! files it writes are cover images in the cache directory.

pub mod confidence;
pub mod discover;
pub mod grouping;
pub mod hash;
pub mod scan;

pub use scan::{Phase, ScanError, ScanOptions, ScanProgress, ScanSummary, scan_library};

/// Audio file extensions the scanner picks up (lowercase).
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "m4b", "m4a", "mp3", "aac", "ogg", "oga", "opus", "flac", "wav", "wma", "mka",
];
