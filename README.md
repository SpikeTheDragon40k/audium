# Audium

An offline-first desktop audiobook player for Linux (primary: Arch + KDE Plasma 6 on Wayland). Windows support is planned (see [ROADMAP.md](ROADMAP.md), Milestone 8).

- Native Qt 6 / Kirigami UI. No web views, no server, no accounts, no telemetry, no network access.
- Reads your library and never modifies, moves, renames or deletes audio files. Everything Audium learns or you edit is kept in its own database.
- Built to cope with large, messy libraries (tested with 1000 books).

> **Status:** feature-complete for Linux, before its first release. See [ROADMAP.md](ROADMAP.md) for progress and [ARCHITECTURE.md](ARCHITECTURE.md) for the design.

## Features

- **Library:** add one or more folders. Audium finds books in single m4b/m4a files, folders of numbered parts, `CD1`/`Disc 2` subfolders, and author folders full of loose files. Formats: m4b, m4a, mp3, aac, ogg, oga, opus, flac, wav, wma, mka.
- **Metadata:** from `metadata.json`, `desc.txt`, `reader.txt` and `cover.*`/`folder.*` next to the files, then embedded tags, then folder and file names. Uncertain books go to a **Needs Review** list, where you can edit, merge or split them. Your edits are stored in Audium, never written to the files.
- **Listening:** resumes where you left off, per-book speed (0.5×–3×, pitch-corrected), chapters, bookmarks with notes, and a sleep timer (minutes or end of chapter) that fades out; moving the mouse or pressing a key over Audium during the fade gives you more time.
- **Browsing:** Home (continue listening, recently added, finished), books as a grid, a list or a bookshelf, authors, series, collections, and search.
- **Statistics:** time listened today, this week, this month and in total, streaks, books finished and a 30-day chart. Settings → Reset Statistics… clears the listening history (progress, bookmarks and finished books stay).
- **Your data:** export and import progress, bookmarks, statistics, edits and settings as JSON (books are recognised by their content, not their paths), and back up the database.
- **Desktop:** media keys, Plasma's media controls and lock screen (MPRIS), a tray icon, closing to the tray while playing, and notifications.
- **Appearance:** follows your Plasma colours (Breeze light or dark), or uses the Dracula or Nord theme (Settings → Appearance).

### Keyboard

| Key | Action |
|---|---|
| Space | Play / pause |
| ← / → | Skip back / forward (Settings → Playback) |
| `[` / `]` | Slower / faster |
| `B` | Add a bookmark |
| Ctrl+F | Search the library |
| Escape | Close the open page |

### Where Audium keeps its data

- Database (progress, bookmarks, statistics, edits, settings): `~/.local/share/audium/audium.db`. A backup is made automatically before every database upgrade.
- Cover thumbnails: `~/.cache/audium/`.

## Installing (Arch Linux)

Releases come with an Arch package and its PKGBUILD:

```sh
sudo pacman -U audium-<version>-1-x86_64.pkg.tar.zst
```

To build the package yourself from a release tarball, put `PKGBUILD` and `audium-<version>.tar.gz` in one folder and run `makepkg -si`.

## Building (Linux)

Dependencies (Arch):

```sh
sudo pacman -S --needed rust cmake ninja extra-cmake-modules corrosion \
  qt6-base qt6-declarative qt6-svg kirigami qqc2-desktop-style \
  ki18n kcoreaddons kconfig kcolorscheme knotifications kstatusnotifieritem \
  mpv ffmpeg
```

(`ffmpeg` is only needed for the tests' generated audio. `rustup` works instead of `rust`; the toolchain is pinned in `rust-toolchain.toml`.)

Build and run:

```sh
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build build
build/bin/audium
```

Run every check (formatting, clippy, Rust tests, the CMake build, a staged install, the desktop metadata and the Qt tests):

```sh
scripts/check.sh
```

Test audio is synthetic and generated locally with `scripts/make-fixtures.sh` (ffmpeg); no audio files are committed. `scripts/perf.sh` measures scanning, scrolling, search and memory on a generated 1000-book library. `scripts/release.sh <version>` builds a release (checks, source tarball, Arch package) and, after asking, publishes it on GitHub.

## Known limitations

- **Windows is not supported yet.** The code keeps platform parts separate, but it has not been built on Windows; the build, Windows media controls (SMTC) and an installer are Milestone 8.
- **Chapters** are read from MP3 (ID3 `CHAP`) and MP4/M4B files. Other formats, and files without chapter data, get one chapter per file.
- **Moving the mouse to extend the sleep timer** only works over Audium's window: Wayland does not let applications see the pointer elsewhere. Media keys and the tray's Play/Pause extend it too.
- **The bookshelf view** scrolls less smoothly than the grid and list in very large libraries with many small shelves (see ROADMAP, M7 performance).
- **English only** so far (the UI is ready for translations).
- **Logs** go to the terminal only; there is no log file yet.

## AI-assisted development

This software is developed with AI assistance. All code is tested, reviewed and built by a human maintainer before it is committed or released.

## License

Audium is free software, licensed under the [GNU General Public License, version 3](LICENSE) (GPL-3.0-only).
