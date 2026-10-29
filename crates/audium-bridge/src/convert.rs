//! Small conversions between storage values and Qt types.

use std::path::Path;

use cxx_qt_lib::{QString, QStringList, QUrl};

pub fn qs(s: &str) -> QString {
    QString::from(s)
}

pub fn qs_opt(s: Option<&str>) -> QString {
    s.map(QString::from).unwrap_or_default()
}

pub fn string_list<S: AsRef<str>>(items: &[S]) -> QStringList {
    items.iter().map(|s| QString::from(s.as_ref())).collect()
}

/// A file URL for a cache file (cover images). Cache paths are UTF-8.
pub fn file_url(path: Option<&str>) -> QUrl {
    path.map(|p| QUrl::from_local_file(&QString::from(p)))
        .unwrap_or_default()
}

/// The path bytes the playback backend passes to libmpv: raw OS bytes on
/// Unix, UTF-8 on Windows (where mpv expects UTF-8).
pub fn playback_path_bytes(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        path.to_string_lossy().into_owned().into_bytes()
    }
}
