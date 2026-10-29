//! Lossless path <-> BLOB encoding.
//!
//! Unix: the raw bytes of the OS string. Windows: UTF-16LE code units, which
//! preserve unpaired surrogates.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[cfg(unix)]
pub fn to_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(unix)]
pub fn from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(OsString::from_vec(bytes.to_vec()))
}

#[cfg(windows)]
pub fn to_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(windows)]
pub fn from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::windows::ffi::OsStringExt;
    let wide: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    PathBuf::from(OsString::from_wide(&wide))
}

/// For display only; lossy.
pub fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_unicode() {
        let p = Path::new("/books/Ünïcødé/本 – d’été.mp3");
        assert_eq!(from_bytes(&to_bytes(p)), p);
    }

    #[cfg(unix)]
    #[test]
    fn round_trips_non_utf8_on_unix() {
        use std::os::unix::ffi::OsStrExt;
        let raw = b"/books/latin1-\xe9t\xe9.mp3";
        let p = Path::new(std::ffi::OsStr::from_bytes(raw));
        assert_eq!(to_bytes(p), raw);
        assert_eq!(from_bytes(raw), p);
        assert!(display(p).contains('\u{FFFD}'));
    }
}
