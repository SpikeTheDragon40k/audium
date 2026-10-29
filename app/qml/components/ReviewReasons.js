.pragma library

// Human-readable text for "Needs review" reason codes (see audium-library
// confidence.rs). `i18n` is passed in from a QML context.
function describe(code, i18n) {
    switch (code) {
    case "missing_title": return i18n("No title found");
    case "missing_author": return i18n("No author found");
    case "title_from_path": return i18n("Title guessed from the folder or file name");
    case "author_from_path": return i18n("Author guessed from the folder or file name");
    case "split_by_album": return i18n("Split from a folder with several albums");
    case "split_by_filename": return i18n("Split from a folder by file names");
    case "inconsistent_numbering": return i18n("Track numbers are missing or duplicated");
    case "unreadable_files": return i18n("Some files in the folder could not be read");
    default: return code;
    }
}
