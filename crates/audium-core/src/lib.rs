//! Audium domain types and pure logic.
//!
//! This crate performs no I/O and has no Qt dependency. Everything here must be
//! deterministic and unit-testable.

pub mod ids;
pub mod listening;
pub mod natural;
pub mod position;
pub mod progress;
pub mod sleep;
pub mod text;

pub use ids::{BookId, BookUid, LibraryId, TrackId};
pub use natural::natural_cmp;

/// Reverse-DNS application id, shared by the desktop file, D-Bus names and data dirs.
pub const APP_ID: &str = "org.audium.Audium";

/// The application version, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Contributor roles stored for a book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Author,
    Narrator,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Author => "author",
            Role::Narrator => "narrator",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver_like() {
        assert_eq!(VERSION.split('.').count(), 3);
    }
}
