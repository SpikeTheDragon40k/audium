# CLAUDE.md — Audium

Offline-first desktop audiobook player for Linux (primary) and Windows (secondary).

## 0. How to work

**First session: plan, don't code.**
1. Inspect the environment: Rust toolchain, Qt 6 version, KF6/Kirigami, CMake, CXX-Qt compatibility, libmpv, ffmpeg.
2. Read this whole file, then write `ARCHITECTURE.md` and `ROADMAP.md`. Flag any decision here you believe is wrong, with reasons and alternatives.
3. Stop and wait for my approval.

**After approval: work autonomously.**
- Don't ask about routine technical decisions. Implement one milestone at a time, smallest vertical slice first.
- After each milestone: build, run tests, fix failures, update docs, record what was done and what's still open in `ROADMAP.md`, then commit.
- Stop and ask only if a fixed decision (section 2) turns out not to work, or something would touch my audiobook files.
- Never claim something is tested if it wasn't. Never describe unfinished work as done. If you run out of context or time, leave the repo compiling and write exact continuation steps in `ROADMAP.md`.

## 1. Product requirements

- Native desktop app: no web views, no Electron, no server, no Docker, no Audiobookshelf integration.
- Fully offline: no accounts, no telemetry, no cloud, **no online metadata lookup**.
- Safe for existing libraries: **never modify, move, rename, or delete audio files.** Audium only reads them.
- Responsive with large, messy libraries (1000+ books, inconsistent tags and folder layouts).
- Platform priority: **Arch Linux + KDE Plasma 6 on Wayland is the primary target.** Windows 11 x64 must compile in CI and be architecturally supported. A polished Windows installer comes in the release milestone.

## 2. Technology decisions (fixed unless proven unworkable)

| Area | Choice |
|---|---|
| Core logic | **Rust** workspace: domain, library scanning, grouping, metadata resolution, SQLite, search, progress, stats, export/import |
| UI | **Qt 6 + QML + Kirigami (KF6)** on all platforms; Windows dependencies via **KDE Craft** |
| Bridge | **CXX-Qt**. Validate it in Milestone 0; if it blocks, document why and propose a fallback before switching |
| Playback | **libmpv** (audio only, `vo=null`) behind a `PlaybackController` abstraction |
| Storage | SQLite (`rusqlite`), versioned migrations, WAL, foreign keys on |
| Metadata | Evaluate Rust crates (e.g. `lofty`) for tags and artwork; chapters from libmpv/ffprobe where crates fall short. Verify M4B, MP3 ID3 CHAP, MP4 atoms and artwork support before relying on any one library |

### Playback notes
- The `PlaybackController` interface covers: load book (list of tracks + start position), play, pause, stop, seek (book-wide and in-track), rate, volume, next/previous chapter, position, duration, state, error. The interface itself must not depend on mpv.
- It lives on the Qt side. Forward mpv events with a wakeup callback and `QMetaObject::invokeMethod` onto the Qt thread. Never touch QObjects from Rust worker threads.
- Load multi-file books as an mpv playlist. Use `af=scaletempo2` for pitch-corrected speed from 0.5× to 3.0×.
- Never fake playback in place of a real backend.

## 3. Repository layout (adjust if the CXX-Qt build requires it; explain any deviation)

```
audium/
  CLAUDE.md  README.md  ARCHITECTURE.md  ROADMAP.md
  Cargo.toml  CMakeLists.txt
  crates/
    audium-core/      # domain types, no Qt, no I/O
    audium-metadata/  # tag/chapter/cover readers, resolution chain
    audium-library/   # scanner, grouping heuristics, reconciliation
    audium-storage/   # SQLite, migrations, queries, export/import
    audium-bridge/    # CXX-Qt objects and list models exposed to QML
  app/
    src/main.cpp
    src/playback/    # PlaybackController + mpv backend
    src/integration/ # MPRIS, tray, notifications, Windows SMTC (later)
    qml/  pages/  components/  themes/
  migrations/  tests/fixtures/  scripts/
  packaging/linux/ (PKGBUILD, .desktop, icon)  packaging/windows/
  .github/workflows/
```

The domain must not depend on Qt. Avoid circular crate dependencies.

## 4. Domain model

Entities: LibraryRoot, Audiobook, AudioTrack, Chapter, Contributor (author, narrator), Series, Collection, Bookmark (position + optional title + note), PlaybackProgress, ListeningSession, MetadataOverride, BookSettings (per-book speed), ApplicationSettings.

- A book has 1..n tracks. Chapters come from embedded data, or are inferred from tracks.
- Store a position as **track id + offset**, with a derived book-wide position.
- Keep **book identity** (stable id, survives rescans and moves) separate from **file identity** (path, size, mtime, partial hash).
- Use stable database ids.

## 5. Scanner: built for a messy library

- Multiple roots, recursive, extension filter, symlink-loop safe, bounded concurrency, cancellable, with progress reporting. Never runs on the UI thread.
- **Grouping heuristics:**
  - one m4b/m4a = one book;
  - a folder of audio files = one book;
  - subfolders named like `CD1`, `Disc 2` or `Part 03` merge into the parent book;
  - an author folder containing many single files is split by album tag or filename prefix.
- Natural sort (`2 < 10`). Prefer disc/track tags over filenames when they're present.
- **Confidence score per book.** Low-confidence groupings and books with missing title or author go into a **"Needs review" queue**. From there the user can:
  - merge books;
  - split a book;
  - edit metadata (stored as overrides in the DB, never written to files).
- Metadata resolution order: user overrides → sidecar files (`metadata.json`, `desc.txt`, `reader.txt`, `cover.*`, `folder.*`) → embedded tags → folder structure (`Author/Series/Book`) → filename heuristics.
- Incremental rescans use cheap identity (path, size, mtime), with a partial hash (first and last N MB) only for detecting moves. Never hash whole files on every scan.
- **Reconciliation:** a moved or renamed book keeps its progress, bookmarks and stats. A missing file marks the book unavailable (for example when `/mnt/Storage` isn't mounted). **Never delete records that have progress.**

## 6. Features

### MVP (through Milestone 3)
- Library import, grid and list views, book details, playback with real audio.
- Controls: play/pause, seek, configurable skip back/forward, speed, volume, chapter navigation.
- Seamless track transitions.
- Resume after restart. Save progress periodically (debounced), on pause, on track change and on shutdown.
- Detect end of book and mark it finished.
- **Per-book remembered speed.**
- **Sleep timer:** by minutes or end of chapter, with a fade-out. Shaking the mouse or pressing a key during the fade extends it.

### Later milestones
- **Bookmarks** with optional notes: one-key add, a list per book showing the chapter name, jump/edit/delete.
- Search, sorting, author/series/collection views, a chapter browser, metadata overrides.
- **Bookshelf view** inspired by Audiobookshelf: covers standing on shelf rows grouped by series, author or collection, with a progress bar under each cover. This is an alternative to the plain grid, and the user can toggle between them.
- **Listening stats page:** time listened today/week/month/all time, current streak, books finished, per-book totals, and a simple chart. Built from ListeningSession records.
- **JSON export/import:** progress, bookmarks, stats, overrides and settings in a versioned schema. Books are matched by fingerprint (normalized title + author + total duration + partial hash), not by path. Import previews conflicts before applying.
- **Tray icon:** play/pause, current book, quit. Closing the window minimizes to the tray while playing (configurable).
- Themes: follow the system Breeze light/dark by default, with optional **Dracula** and **Nord** palettes via centralized theme tokens.

## 7. UI

- Pages: Library (Continue Listening, Recently Added, Finished, grid/list/bookshelf), Book Details, Now Playing, Needs Review, Stats, Settings (library folders, playback, theme, scan, database backup and export/import, about).
- A persistent mini-player on every page.
- Follow the Kirigami HIG: responsive layouts, no hard-coded absolute sizes, no blocking modals for routine actions, minimal animation.
- Full keyboard support:
  - Space: play/pause
  - ←/→: skip back/forward
  - `B`: add bookmark
  - `[` / `]`: change speed
  - Ctrl+F: search
- Never put database queries in QML. QML only talks to bridge models and controllers.

## 8. Platform integration (optional at runtime)

The app must play audio even if every integration fails.

- **Linux:** MPRIS2 (title = chapter, album = book, artist = author, art, position, rate, next/previous = chapter), media keys, notifications, tray, `.desktop` file, icon, XDG directories.
- **Windows:** SMTC media controls, tray, app metadata. The installer comes in the release milestone.
- No hard-coded paths. Don't assume paths are UTF-8; handle long paths on Windows.

## 9. Database

- Migrations cover: libraries, books, tracks, chapters, contributors, book_contributors, series, collections, bookmarks, progress, book_settings, sessions, overrides, settings, review_queue.
- Use indexes for library listing, search, continue-listening and stats.
- Use transactions for multi-row updates.
- Back up automatically before every migration. If a migration fails, preserve the original database and show the error. **Never silently recreate the database.**

## 10. Testing

- Generate fixtures with ffmpeg via `scripts/make-fixtures.sh`. Never commit copyrighted audio.
- Unit tests cover:
  - grouping heuristics (including the messy layouts in section 5);
  - natural sort;
  - the metadata fallback chain;
  - migrations;
  - progress persistence and resume near track boundaries;
  - bookmarks;
  - reconciliation after a move or rename;
  - missing and corrupt files;
  - Unicode and long paths;
  - export/import round trip;
  - sleep-timer logic;
  - stats aggregation.
- Integration tests for the playback state machine using the real mpv backend.
- Run `cargo fmt`, `cargo clippy -D warnings`, `cargo test` and the Qt tests at every milestone.

## 11. Milestones

0. **Foundation:** environment check, workspace, a Kirigami window, a working CXX-Qt bridge, Linux CI. Windows CI compiles via Craft (it may be marked "allowed to fail" until Milestone 7, with the reason documented).
1. **Real playback:** mpv-backed controller, play one file and a multi-file book, seek, speed, volume, errors, state reflected in QML.
2. **Library:** SQLite, folder picker, scanner with grouping heuristics, metadata and covers, grid/list, book details, Needs Review queue.
3. **Persistence (MVP):** progress, resume, track transitions, per-book speed, sleep timer, end-of-book handling.
4. **Organization:** search, sort, authors, series, collections, bookmarks with notes, chapter browser, overrides, merge/split, bookshelf view.
5. **Insights and portability:** listening stats page and JSON export/import.
6. **Desktop integration:** MPRIS, media keys, tray, notifications, and Windows SMTC.
7. **Release:** PKGBUILD + `.desktop` file, Windows installer, theme polish (Dracula/Nord), performance pass on a 1000-book synthetic library, docs, known limitations.

## 12. Engineering rules

- Prefer simple solutions and few dependencies. Keep platform code isolated.
- No `unsafe` without a written justification. No panics on bad input. Validate all file metadata.
- No global mutable state. Use structured logging (`tracing`).
- Builds must be reproducible: commit `Cargo.lock` and pin CI toolchains.
- No cloud services, no analytics, no outbound network requests at all.
