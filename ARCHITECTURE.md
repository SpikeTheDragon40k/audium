# Audium — Architecture

Status: proposed and approved 2026-10-29, before Milestone 0. Sections marked *(decision)* clarify or adjust CLAUDE.md; none replaces a fixed §2 choice.

## 1. Environment baseline (checked 2026-10-29)

| Component | Version | Notes |
|---|---|---|
| OS / session | Arch Linux, kernel 7.2.11, Plasma 6.8.2, Wayland | primary target |
| Rust | 1.99.1 stable (clippy, rustfmt) | pinned via `rust-toolchain.toml` |
| C/C++ | GCC 16.2.1, Clang 23.1.4 | |
| Qt | 6.12.0 | minimum supported: Qt 6.8 |
| KF6 | Kirigami 6.31, kirigami-addons 1.16, KI18n, KConfig, KCoreAddons, KCrash, KDBusAddons, KNotifications, KStatusNotifierItem, KIconThemes, qqc2-desktop-style, Breeze | minimum supported: KF 6.10 |
| CMake / Ninja / ECM / Corrosion | not installed (repo: 4.4.5 / 1.13.2 / 6.31.0 / 0.6.1) | required for M0 |
| mpv / libmpv | not installed (repo: 0.41.0, libmpv.so.2) | required for M1 |
| ffmpeg / ffprobe | 9.0.2 | fixtures; optional chapter fallback |
| SQLite | 3.54.0 system | rusqlite `bundled` is used instead |
| Crates | cxx-qt 0.10.0, lofty 0.25.5, rusqlite 0.41.1 | CXX-Qt with Qt 6.12 not yet validated (risk R1) |

Install the missing pieces with:
`sudo pacman -S --needed cmake ninja extra-cmake-modules corrosion mpv`

## 2. Overview

```
 QML (Kirigami pages/components)  ── talks only to ──▶  bridge QObjects + list models
        │                                                    │
        ▼                                                    ▼
 app/src (C++)                                     crates/audium-bridge (CXX-Qt, staticlib)
  ├ main.cpp (wiring, QStandardPaths, KAboutData)         │ PlaybackSession, LibraryController,
  ├ playback/ PlaybackController ◀── signals/slots ──▶    │ BookListModel, ChapterModel, ReviewQueueModel,
  │           MpvPlaybackBackend (libmpv, vo=null)        │ BookmarkModel, StatsModel, SettingsController
  └ integration/ MPRIS (QtDBus), tray (KSNI), notify      ▼
                                          audium-library ─▶ audium-metadata ─▶ audium-core
                                          audium-storage ──────────────────▶ audium-core
```

Dependency direction: `core` ← `metadata` ← `library` → `storage` → `core`; `bridge` → all of them. There are no cycles. `audium-core` does no I/O and has no Qt dependency.

*(Changed in M2)* `audium-library` depends on `audium-storage`. It owns the scan orchestration (snapshot → discover → read → group → plan → apply), so the whole scan can be tested end to end without Qt. Storage still knows nothing about scanning: it defines plain write types (`ScanChanges`, `BookWrite`, `TrackWrite`) and applies them in one transaction.

## 3. Build *(decision)*

CMake is the top level, with Cargo underneath via **Corrosion** and CXX-Qt's `cxx_qt_import_crate`. `audium-bridge` is the only `staticlib`. The other crates are plain `rlib`s, so `cargo test` works with no Qt installed. ECM supplies KDE install dirs, compiler settings and packaging helpers.

Details settled in M0:
- `cxx-qt-cmake` 0.10.0 (one CMake file) is vendored in `cmake/third_party/cxx-qt-cmake/` instead of being fetched at configure time. Builds work offline and the version is pinned to the crates.
- **Deviation from the CLAUDE.md layout:** the bridge's Cargo package is named `audium_bridge` (underscore), while its directory stays `crates/audium-bridge/`. cxx-qt-build only exports generated headers when `CXX_QT_EXPORT_CRATE_<CARGO_PKG_NAME>` is set, and CMake sets that variable from the target name, which always uses underscores. A hyphenated package name therefore never exports anything.
- The app's QML is a static QML module (`audium_ui`, URI `org.audium.app`). The executable and the Qt tests link the same module, so the tests load the real `Main.qml`.
- The bridge's QML module URI is `org.audium.bridge`.
- Localization uses `KLocalization::setupLocalizedContext` (KF6::I18nQml); `KLocalizedContext` is deprecated since KF 6.8.

Rejected alternative: a Cargo-only build that compiles the C++ through `cxx-qt-build`. It loses the ECM tooling and the Craft packaging paths.

Reproducibility: `rust-toolchain.toml` (1.99.1) and a committed `Cargo.lock`.

**Builds are local** *(decision, overrides CLAUDE.md's CI items)*: all builds, tests and release artifacts are produced on the developer's machines, not on GitHub Actions. GitHub hosts the code and the Releases only.
- `scripts/check.sh` runs fmt, clippy, `cargo test`, the CMake build and `ctest`. It's the per-milestone gate.
- `scripts/release.sh` builds the packages and uploads them with `gh release create`.
- Windows builds run locally through Craft on a Windows machine.

## 4. Crates

- **audium-core**:
  - ids: `BookId` and `TrackId` (newtypes over i64), plus `BookUid` (a UUID for stable identity);
  - the §4 entities;
  - `Position { track_id, offset_ms }` ↔ book-wide ms conversion;
  - natural sort and title/author normalization;
  - the fingerprint struct;
  - the sleep-timer state machine and stats aggregation over sessions;
  - confidence-scoring types.

  Everything here is pure functions.
- **audium-metadata**:
  - `TagReader` (lofty);
  - chapter readers (see §6);
  - sidecar readers (`metadata.json`, `desc.txt`, `reader.txt`, `cover.*`, `folder.*`);
  - the resolution chain *overrides → sidecar → embedded → folder structure → filename*. Each step returns `Option<field>` together with its provenance.

  All input is validated: lengths capped, control characters stripped, no panics.
- **audium-library**:
  - root walking with `walkdir` and `follow_links`; a set of visited inodes/file ids breaks symlink loops; extension filter;
  - grouping heuristics (CLAUDE.md §5) and confidence scoring;
  - cheap identity (path, size, mtime);
  - a partial hash (`blake3` over the first and last 1 MiB), computed only for unmatched candidates when detecting moves;
  - reconciliation planning and cancellable progress events.
- **audium-storage**:
  - rusqlite (`bundled`), opened with WAL, `foreign_keys=ON` and `busy_timeout`;
  - embedded migrations `migrations/NNNN_*.sql`, tracked with `PRAGMA user_version`;
  - the writer thread, the read connection and query functions;
  - versioned JSON export/import (serde).
- **audium-bridge**: CXX-Qt QObjects and `QAbstractListModel`s. Contains no SQL; it calls storage functions. Uses `CxxQtThread::queue` to post results to the Qt thread.

## 5. Playback *(decision)*

`PlaybackController` (abstract QObject, C++) has:
- methods: `load(QList<TrackSource>, startTrack, startOffset)`, play/pause/stop, `seekBook`, `seekTrack`, `rate`, `volume`, `nextChapter`/`previousChapter`;
- properties: `position`, `duration`, `state`, `error`, `currentTrackIndex`.

It contains no mpv types.

`MpvPlaybackBackend` configuration and behaviour:
- options: `vo=null`, `video=no`, `af=scaletempo2`, `keep-open=yes`, `gapless-audio=weak`;
- a multi-file book loads as one playlist, and properties are observed;
- events: `mpv_set_wakeup_callback` → `QMetaObject::invokeMethod(Qt::QueuedConnection)` → drain `mpv_wait_event(0)` on the Qt thread;
- rate is clamped to 0.5–3.0.

Implementation notes (M1):
- **QML access:** `Main.qml` receives the controller as a required property (`player`), set from `main.cpp`. QML never constructs it.
- **Error detection:** errors are detected only from `END_FILE` events for entries of the current playlist, matched by mpv playlist entry ids. The idle state isn't used, because its notifications can arrive after the next `load()` has started.
- **Book-level logic in the base class:** `PlaybackController` handles chapter navigation and book-wide seeking, so another backend gets them for free. A track of unknown length is treated as open-ended. A seek whose target is computed before the length is known is clamped to just before that track's end when it loads.
- **Logging:** mpv/ffmpeg warn-level logs go to `qCInfo` (category `audium.playback.mpv`); only error-level logs become warnings.

**Policy lives in Rust.** The `PlaybackSession` bridge object owns:
- which book and track to load;
- debounced progress saves (every 5 s, and on pause, track change and quit);
- session records;
- end-of-book handling;
- per-book speed;
- the sleep timer.

`main.cpp` connects controller signals to `PlaybackSession` invokables, and `PlaybackSession` signals to controller slots. Both objects live on the Qt thread. The sleep-timer state machine lives in `audium-core` and is ticked by a QTimer, so it is unit-testable.

**Sleep fade extension** *(decision)*: Wayland gives no global pointer access. During a fade, the timer is extended by:
- mouse movement or a key press over the Audium window;
- media-key or MPRIS Play/Pause;
- tray interaction.

## 6. Metadata and chapters *(decision)*

lofty covers tags, duration and artwork, but not ID3 `CHAP`/`CTOC` or MP4 chapters. Chapter sources, in order:
1. the `id3` crate for MP3 CHAP/CTOC;
2. `mp4ameta` for MP4/M4B chapters (Nero `chpl` and QuickTime chapter tracks);
3. `ffprobe` as an optional subprocess fallback, used only if it's on PATH;
4. chapters inferred from tracks.

libmpv is not used for scanning. Milestone 2 starts with a spike that verifies each format against generated fixtures.

## 7. Paths *(decision)*

Paths never pass through QString or QML.
- **Storage:** SQLite BLOBs. On Unix that's the raw `OsStr` bytes; on Windows, UTF-16 encoded losslessly (WTF-8).
- **QML:** sees only ids and display strings; display uses a lossy conversion.
- **mpv:** C++ receives a `QByteArray` from the bridge.
- **Windows:** long paths get `\\?\` handling, covered by a test.

Data, cache and config directories come from `QStandardPaths` in C++ and are passed to Rust at startup.

## 8. Data model

Tables:
- `libraries`
- `books`: id, uid, availability, confidence, …
- `tracks`: book_id, path BLOB, size, mtime, partial_hash, disc, number, duration_ms
- `chapters`, `contributors`, `book_contributors` (role)
- `series`, `collections`, `collection_books`
- `bookmarks`
- `progress`: track_id + offset_ms, book_position_ms, finished_at
- `book_settings`, `sessions`
- `overrides`: book_id, field, value
- `settings`
- `review_queue`: book_id, reason

Indexes:
- library listing: title_sort, added_at
- continue listening: progress.updated_at
- search: an FTS5 table over title, author, narrator and series
- stats: sessions.started_at

**Identity:** `books.uid` is stable across rescans and is never derived from paths. Tracks are matched first by path, then by (size + partial hash) to detect moves. A book whose files are all gone is marked `unavailable`. Records with progress, bookmarks or sessions are never deleted.

**Migrations:**
1. Before each migration, a `VACUUM INTO` backup is written to `audium.db.bak-<ver>-<timestamp>`.
2. A failed migration rolls back, the original file is left untouched, and the UI shows an error state.
3. The database is never recreated silently.

**Covers** *(decision, changed in M2)*: the chosen cover (a sidecar `cover.*`/`folder.*` first, otherwise the image embedded in one of the first three files) is copied unchanged to `<cache>/audium/covers/<book-uid>-<content-hash>.<ext>`. QML `Image` scales it asynchronously (`sourceSize`). There is no thumbnail step, so no image library is needed; M7's performance pass decides whether thumbnails are worth it. Nothing is ever written into the library.

## 9. Threading *(decision)*

| Thread | Work |
|---|---|
| Qt thread | UI, models, playback, integrations |
| Scan thread | a scan's writes, with its own connection, in one transaction per library root |
| Qt thread (writes) | small writes: adding and removing library folders |
| Read-only connection | list-model queries (WAL allows reads during writes) |
| Scanner pool | N = min(4, cores) `std::thread`s, cancelled through an `AtomicBool` |

There's no async runtime. Rust worker threads never touch QObjects.

*(Deviation in M2, settled in M3)* There is no single writer for everything. Instead:
- each scan writes from its own thread, in one transaction per root;
- adding and removing library folders writes from the UI thread (rare);
- everything playback writes goes through `audium_storage::Writer`. That's a thread with its own connection that runs queued writes in order: progress, finished marks, speed, sessions, settings. If a scan holds the write lock, a progress save waits on that thread (up to the 5 s busy timeout), not on the UI. Dropping the writer, or calling `flush()`, finishes every queued write.

## 10. Scanning and reconciliation (M2, refined in M3)
- **Discovery:** recursive, following symlinks. walkdir detects loops; the same file reached twice (by device and inode) counts once. Hidden entries are skipped. Disc folders (`CD1`, `Disc 2`, `Part 03`, `Vol. 4`, `CD 1 of 3`) merge into their parent.
- **Grouping** (`audium-library/src/grouping.rs`, pure and unit-tested):
  - **Container files:** `.m4b`, and `.m4a` with chapters or alone in its folder, are each a book.
    - *Except:* containers in one folder sharing an album tag are the parts of one book, ordered by track tags. Untagged containers merge only when all have unique track numbers with the same total above 1.
    - *(This refines CLAUDE.md §5 "one m4b = one book". A real library had one book split into 12 numbered m4b parts.)*
  - **Other files:** the rest of a folder forms one book, unless it has several album tags (split by album) or un-numbered names with different prefixes (split by name).
  - **"Numbered" names:** the text after their common prefix starts with a number (`Chapitre 01 – début`, `The Hobbit - 02 - …`).
  - **Order:** (disc, track) tags only when every file has a unique one; otherwise natural filename order, disc folders first.
- **Confidence:** starts at 100 and loses points for each reason (missing or path-derived title or author, heuristic split, inconsistent numbering, unreadable files). Below 70, or without a title or author, the book goes to the review queue.
- **Incremental rescans:** a folder whose files all match the stored path, size and mtime isn't opened. Sidecar edits are therefore only picked up by "Re-read Everything" (force).
- **Rules version:** `audium_library::RULES_VERSION`, stored in `settings` after a scan that reached every library folder. When grouping or metadata rules change, raise it: the next scan is then forced once, because unchanged folders are otherwise never regrouped.
- **Moves:** a file without a path match takes over a missing track with the same size and partial hash (blake3 of the size plus the first and last 64 KiB), so the book's id, progress and bookmarks follow. Hashes are computed once, for new or changed files.
- **Regrouping:** each candidate book reuses the stored book most of its tracks belonged to, preferring one with user data. Books emptied by regrouping are deleted only without user data.
- **Missing files:** tracks are marked missing and the book becomes unavailable. A scan never deletes a book for missing files.
- **Unmounted roots:** a root that's unreachable, or that suddenly has no audio files while the database knows some, marks the library unavailable and changes nothing else.
- **Failure safety:** all changes for a root are applied in one transaction at the end; a cancelled or failed scan leaves the database untouched.

## 11. Bridge and UI (M2–M3)
- **`LibraryController`:** created in `main.cpp`, `open(dataDir, cacheDir)` with `QStandardPaths` locations, and passed to QML as an initial property. It holds the read-write connection for folder changes, runs scans on a thread, and publishes progress through `CxxQtThread::queue`. `revision` tells models to reload.
- **`BookListModel` and `BookDetails`:** created in QML. They bind `databasePath` and `revision` and reload through `cxx_qt::Initialize` signal connections; each reload opens a short-lived read-only connection.
- **Read-only Rust properties:** set with the crate's `set_ro!` macro (assign plus emit if changed).
- **Kirigami 6.31 rule:** page actions must not be hidden when the page is created, neither by `DisplayHint.AlwaysHide` nor by `visible: false`. Either makes `ActionToolBar` destroy delegates during incubation (Qt warnings). Disable actions instead, or use an `InlineMessage` with an action, like the "Needs review" banner.
- **Layers:** Settings, Edit Metadata and Split Book open as `pageStack.layers` over the window, with a back button; Escape closes them. As columns, a wide window gave them no way to close. Kirigami 6.31 warns "StackView has detected conflicting anchors" on every layer push, even for a plain page in a minimal standalone file; it's harmless, and that one message is exempted in `test_bridge`.
- **Closing columns:** book details, group pages, Needs Review and Now Playing are columns beside the library and carry a Close action (`Main.closePage`). In a wide window Kirigami's back button is disabled for columns that are all visible. Escape closes the topmost layer, otherwise the rightmost column.
- **Pages:** created with `pageStack` as parent and destroyed shortly after removal. Destroying them immediately triggers the same incubation warning when navigating quickly. Now Playing is one persistent instance.
- **Views fill the page and scroll themselves.** A `Loader` inside a `ScrollView` gets no size, which made the library views blank and unclickable (found in M3).
- **Tests:** any Qt/QML warning fails `test_bridge`. It loads the UI with an empty and with a full library, checks view sizes, and clicks tiles with real mouse events.

## 12. Playback session (M3)
- **`PlaybackSession` (Rust bridge object):** created in `main.cpp`. It owns the policy:
  - **what to load:** a book's playable tracks, chapters, resume point and speed. `core::position::resume_point` resumes by track id, falls back to the book-wide position, and moves a position within 0.5 s of a track's end to the next track.
  - **when to save:** `core::progress`: every 5 s while playing, and immediately on pause, track change, seek (a jump of more than 3 s) and quit;
  - **listening sessions:** `core::listening`, stretches of at least 5 s;
  - **end of book:** marks the book finished;
  - **sleep timer:** `core::sleep`.
- **`SessionWiring` (C++):** connects session and controller on the Qt thread.
  - `loadRequested` → it reads `pending*()`, loads, sets the speed and plays.
  - Controller state, position, rate and end of book go to the session; `pauseRequested` and `fadeRequested` go back.
  - Loads from elsewhere ("Open Files") detach the session.
  - `aboutToQuit` → `shutdown()`, which saves and flushes.
- **Finished:** `progress.finished_at` is kept as history. A book counts as finished while `finished_at >= updated_at`, so any later save means it was restarted. After finishing, the final position is never saved again; replay starts at 0.
- **Sleep timer:** counts only while playing, ticked by position updates. The last 10 s fade the output through `PlaybackController::setVolumeFactor`; the user's volume never changes. Mouse, key, wheel and touch events anywhere in Audium (an application event filter) extend a fading timer: minutes restart, "end of chapter" moves to the next chapter's end. Only for library books.
- **View refresh:** `revision` bumps only after the session's writes commit (a writer job queues it to the Qt thread), on pause, stop, finish and book change, not on the 5 s saves. Views bind `library.revision + session.revision`; dynamic pages get it through `Qt.binding`.
- **mpv backend details:**
  - mpv's volume isn't mirrored back into `volume`, so a fade never looks like a user change.
  - Before building a playlist the backend sets `playlist-start` and ignores `FILE_LOADED` of other entries until the start track has loaded. Otherwise "resume at the start of track N" played track 1.

## 12a. Organization (M4)
- **Search:** an FTS5 table `books_fts` (migration 0002; `rowid` = book id; `unicode61 remove_diacritics 2`) over title, authors, narrators and series. `audium-storage::fts` re-indexes a book whenever `apply` writes it and prunes deleted books. User text becomes prefix terms (`"har"* "pot"*`), and operators are plain text.
- **Lists:** one `read::BookQuery` combines a filter (all, review, continue, recent, finished), a scope (library, author, narrator, series ordered by number, collection ordered by position, no series), search and a sort (title, author, recently played, recently added). `read::list_groups` lists authors, narrators, series and collections with counts and a cover.
- **User organization (`audium-storage::organize`):** collections, bookmarks (title and note, in book order, with the chapter they fall in), metadata overrides (validated field names; lists one name per line), review dismissal, merge and split.
- **Locked books:** merge and split set `books.grouping_locked`.
  - The scanner takes a locked book's files out of normal grouping and rebuilds exactly that book, across folders if needed. Its metadata and chapters are still re-resolved, so overrides and tag edits apply.
  - New files next to them are grouped as usual, and a locked book is never taken over by another candidate.
- **Re-reading one book:** an override edit stores the overrides, then re-reads only that book's folders (`ScanOptions::force_dirs`). A request made while a scan runs is queued until it ends.
- **Writes:** organizing writes from the UI are refused, with a message, while a scan runs instead of blocking. Bookmark writes go through the session's writer.
- **QML structure:** the library has tabs Home, Books (search, sort, grid, list or bookshelf), Authors, Series and Collections. Reusable views are `BookGrid`, `BookList`, `GroupGrid` and `Bookshelf`. Group pages are `BooksPage`; `EditMetadataPage`, `SplitBookPage` and `NeedsReviewPage` (merge) complete it.
- **Model rule:** a list model's `rowCount()` must return its real rows, never a `count` property updated after `endResetModel()`. Views read the row count *during* the reset, so they would keep the old number of items. `test_bridge` checks the row count seen at `modelReset`.

## 12b. Statistics and portability (M5)
- **Statistics:** computed from `sessions`.
  - SQLite works out each session's *local* day (`'localtime'`, so the system time zone and daylight saving time apply) and splits a session that crosses local midnight between the two days.
  - The pure `audium_core::stats` does the calendar arithmetic and sums: today, the week from Monday, the calendar month, all time, the current streak (today, or yesterday if nothing has been played yet today), the best streak, and the last N days.
  - Books finished come from `progress.finished_at`.
- **Export (`audium-storage::portable`):** a versioned JSON document (`format: "audium-export"`, `schema_version: 1`) with every book that has user data, plus collections and settings. Installation-specific settings (`scan_rules_version`) are left out.
  - Books are identified by a fingerprint: normalized title, first author, total duration, and the partial hashes of their tracks. Paths are never used.
  - Positions refer to tracks by index plus hash.
- **Import:**
  - **Matching:** a shared track hash first; otherwise the same title and author with the length within 1% (at least 5 s), closest length winning.
  - **`plan_import`:** decides everything without writing. Newer progress wins; bookmarks are added unless one exists within 1 s with the same title; sessions are added unless one has the same start; overrides take the newer one per field; speed only where none is set; collections are created or extended; settings are overwritten.
  - **Preview and apply:** the UI shows the plan's summary, and `apply_import` executes exactly that plan in one transaction. Books with new overrides are then re-read.
  - **Refused files:** foreign, damaged and newer-format files are refused with a clear message, as are files over 200 MB.
- **Database backup:** `VACUUM INTO` a temporary file next to the target, then a rename. Exports are also written to a temporary file and renamed, so a failure never leaves half a file.

## 13. Platform integration

Integrations sit behind small interfaces. Each one fails independently and only logs a warning when it fails; playback never depends on them.

| Linux | Windows |
|---|---|
| MPRIS2 via QtDBus (also serves media keys) | SMTC in an isolated WinRT file |
| KNotifications | |

`KStatusNotifierItem` provides the tray on both platforms; on Windows it falls back to `QSystemTrayIcon`.

Windows dependencies come through **KDE Craft**, using its prebuilt binary cache, on a local Windows machine. Until M7, a Windows build is best-effort: the code stays portable and platform code stays isolated. Cold Craft builds take hours, and whether mpv is available through Craft is still unverified.

## 14. Cross-cutting

- Logging: `tracing` + `tracing-subscriber` writing to stderr (a log file in the data directory is still open). C++ logs through `qCInfo` categories.
- i18n: KI18n (`i18n()` in QML) from the start. English only for now.
- No outbound network access at runtime.

## 15. Risks

- **R1:** CXX-Qt 0.10 vs Qt 6.12, and how ECM and Corrosion work together. **Resolved on Linux in M0** (CMake + Corrosion 0.6.1 + ECM 6.31 build; a QML singleton with properties and an invokable works from C++ and QML). Windows is still unverified. If it blocks, the fallbacks are, in order:
  1. CXX-Qt from git main;
  2. an older Qt pinned only in CI;
  3. a thin hand-written `cxx` + C++ QObject layer.

  I'll stop and ask before switching.
- **R2:** gaps in lofty, id3 and mp4ameta on unusual files. Mitigated by the ffprobe fallback and by chapters inferred from tracks.
- **R3:** Craft build time, and whether mpv is available for Windows through Craft.
