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

*(Deviation in M2)* There is no dedicated writer thread yet. WAL plus `busy_timeout` (5 s) lets the scan thread and the rare UI-thread writes coexist, and each scan is a single transaction. M3 adds frequent progress writes; if those ever wait on a scan, they move to a writer thread.

## 10. Platform integration

Integrations sit behind small interfaces. Each one fails independently and only logs a warning when it fails; playback never depends on them.

| Linux | Windows |
|---|---|
| MPRIS2 via QtDBus (also serves media keys) | SMTC in an isolated WinRT file |
| KNotifications | |

`KStatusNotifierItem` provides the tray on both platforms; on Windows it falls back to `QSystemTrayIcon`.

Windows dependencies come through **KDE Craft**, using its prebuilt binary cache, on a local Windows machine. Until M7, a Windows build is best-effort: the code stays portable and platform code stays isolated. Cold Craft builds take hours, and whether mpv is available through Craft is still unverified.

## 11. Cross-cutting

- Logging: `tracing` + `tracing-subscriber` writing to stderr and a rotating file in the data directory. C++ logs through `qCInfo` categories.
- i18n: KI18n (`i18n()` in QML) from the start. English only for now.
- No outbound network access at runtime.

## 12. Risks

- **R1:** CXX-Qt 0.10 vs Qt 6.12, and how ECM and Corrosion work together. **Resolved on Linux in M0** (CMake + Corrosion 0.6.1 + ECM 6.31 build; a QML singleton with properties and an invokable works from C++ and QML). Windows is still unverified. If it blocks, the fallbacks are, in order:
  1. CXX-Qt from git main;
  2. an older Qt pinned only in CI;
  3. a thin hand-written `cxx` + C++ QObject layer.

  I'll stop and ask before switching.
- **R2:** gaps in lofty, id3 and mp4ameta on unusual files. Mitigated by the ffprobe fallback and by chapters inferred from tracks.
- **R3:** Craft build time, and whether mpv is available for Windows through Craft.
