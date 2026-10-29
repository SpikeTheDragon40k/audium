-- Audium schema v1.
-- Paths are BLOBs: raw OS bytes on Unix, UTF-16LE on Windows (lossless).
-- Times are milliseconds since the Unix epoch (UTC).

CREATE TABLE libraries (
    id            INTEGER PRIMARY KEY,
    path          BLOB    NOT NULL UNIQUE,
    display_path  TEXT    NOT NULL,
    available     INTEGER NOT NULL DEFAULT 1,
    added_at      INTEGER NOT NULL,
    last_scan_at  INTEGER
);

CREATE TABLE series (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    name_sort  TEXT NOT NULL
);

CREATE TABLE books (
    id               INTEGER PRIMARY KEY,
    uid              TEXT    NOT NULL UNIQUE,
    library_id       INTEGER REFERENCES libraries(id) ON DELETE SET NULL,
    title            TEXT    NOT NULL,
    title_sort       TEXT    NOT NULL,
    subtitle         TEXT,
    description      TEXT,
    year             INTEGER,
    genre            TEXT,
    publisher        TEXT,
    language         TEXT,
    series_id        INTEGER REFERENCES series(id) ON DELETE SET NULL,
    series_index     TEXT,
    duration_ms      INTEGER NOT NULL DEFAULT 0,
    cover_path       TEXT,
    confidence       INTEGER NOT NULL,
    available        INTEGER NOT NULL DEFAULT 1,
    -- Set when the user merged/split the book: rescans keep its tracks.
    grouping_locked  INTEGER NOT NULL DEFAULT 0,
    -- JSON object: field name -> metadata source.
    provenance       TEXT    NOT NULL DEFAULT '{}',
    added_at         INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL
);
CREATE INDEX books_title_sort ON books(title_sort);
CREATE INDEX books_added_at ON books(added_at DESC);
CREATE INDEX books_library ON books(library_id);
CREATE INDEX books_series ON books(series_id);

CREATE TABLE tracks (
    id            INTEGER PRIMARY KEY,
    book_id       INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    path          BLOB    NOT NULL UNIQUE,
    display_path  TEXT    NOT NULL,
    size          INTEGER NOT NULL,
    mtime_ms      INTEGER NOT NULL,
    -- blake3 over size + first and last 64 KiB; detects moves and renames.
    partial_hash  BLOB,
    duration_ms   INTEGER NOT NULL DEFAULT 0,
    disc          INTEGER,
    number        INTEGER,
    title         TEXT,
    missing       INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX tracks_book ON tracks(book_id, position);
CREATE INDEX tracks_size ON tracks(size);

CREATE TABLE chapters (
    id        INTEGER PRIMARY KEY,
    book_id   INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    position  INTEGER NOT NULL,
    title     TEXT    NOT NULL,
    -- Book-wide start.
    start_ms  INTEGER NOT NULL,
    -- 'embedded' or 'tracks'.
    source    TEXT    NOT NULL
);
CREATE INDEX chapters_book ON chapters(book_id, position);

CREATE TABLE contributors (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    name_sort  TEXT NOT NULL
);

CREATE TABLE book_contributors (
    book_id         INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    contributor_id  INTEGER NOT NULL REFERENCES contributors(id) ON DELETE CASCADE,
    role            TEXT    NOT NULL CHECK (role IN ('author', 'narrator')),
    position        INTEGER NOT NULL,
    PRIMARY KEY (book_id, contributor_id, role)
);
CREATE INDEX book_contributors_contributor ON book_contributors(contributor_id, role);

CREATE TABLE collections (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE,
    created_at  INTEGER NOT NULL
);

CREATE TABLE collection_books (
    collection_id  INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    book_id        INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    position       INTEGER NOT NULL,
    PRIMARY KEY (collection_id, book_id)
);

CREATE TABLE bookmarks (
    id                INTEGER PRIMARY KEY,
    book_id           INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    track_id          INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
    offset_ms         INTEGER NOT NULL,
    book_position_ms  INTEGER NOT NULL,
    title             TEXT,
    note              TEXT,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL
);
CREATE INDEX bookmarks_book ON bookmarks(book_id, book_position_ms);

CREATE TABLE progress (
    book_id           INTEGER PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
    track_id          INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
    offset_ms         INTEGER NOT NULL,
    book_position_ms  INTEGER NOT NULL,
    started_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    finished_at       INTEGER
);
CREATE INDEX progress_updated ON progress(updated_at DESC);

CREATE TABLE book_settings (
    book_id  INTEGER PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
    speed    REAL
);

CREATE TABLE sessions (
    id                 INTEGER PRIMARY KEY,
    book_id            INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    started_at         INTEGER NOT NULL,
    ended_at           INTEGER NOT NULL,
    start_position_ms  INTEGER NOT NULL,
    end_position_ms    INTEGER NOT NULL,
    listened_ms        INTEGER NOT NULL
);
CREATE INDEX sessions_started ON sessions(started_at);
CREATE INDEX sessions_book ON sessions(book_id, started_at);

CREATE TABLE overrides (
    book_id     INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    field       TEXT    NOT NULL,
    value       TEXT    NOT NULL,
    updated_at  INTEGER NOT NULL,
    PRIMARY KEY (book_id, field)
);

CREATE TABLE settings (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL
);

CREATE TABLE review_queue (
    book_id       INTEGER PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
    -- JSON array of reason codes.
    reasons       TEXT    NOT NULL,
    created_at    INTEGER NOT NULL,
    dismissed_at  INTEGER
);
