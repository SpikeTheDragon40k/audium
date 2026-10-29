//! Audium domain types and pure logic.
//!
//! This crate performs no I/O and has no Qt dependency. Everything here must be
//! deterministic and unit-testable.

/// Reverse-DNS application id, shared by the desktop file, D-Bus names and data dirs.
pub const APP_ID: &str = "org.audium.Audium";

/// The application version, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver_like() {
        assert_eq!(VERSION.split('.').count(), 3);
    }
}
