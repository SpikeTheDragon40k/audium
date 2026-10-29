//! Tag, chapter, sidecar and cover readers, plus the metadata resolution chain.
//!
//! Everything here only reads files. Values from files are untrusted: they are
//! cleaned and length-limited, and bad input yields errors, never panics.

pub mod cover;
pub mod heuristics;
pub mod probe;
pub mod resolve;
pub mod sidecar;

pub use cover::{Cover, read_embedded_cover};
pub use probe::{FileInfo, FileTags, RawChapter, probe};
pub use resolve::{BookInputs, Field, Overrides, ResolvedMetadata, Source, resolve};
pub use sidecar::{Sidecars, read_sidecars};

/// Limits for untrusted text.
pub mod limits {
    pub const NAME: usize = 512;
    pub const DESCRIPTION: usize = 32 * 1024;
    pub const MAX_CHAPTERS: usize = 10_000;
    /// Sidecar files bigger than this are ignored.
    pub const SIDECAR_BYTES: u64 = 1024 * 1024;
    /// Covers bigger than this are ignored.
    pub const COVER_BYTES: u64 = 20 * 1024 * 1024;
}

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("unsupported or corrupt audio file {path}: {message}")]
    Parse { path: String, message: String },
}
