# Audium

**An offline audiobook player for the Linux desktop.** Audium plays the audiobooks you already have on disk, remembers where you are in every book, and stays out of your files.

- **Native and light:** Qt 6 and Kirigami, made for KDE Plasma 6 on Wayland. No web views, no server.
- **Private:** no accounts, no cloud, no online lookups, no telemetry. Audium makes no network connections at all.
- **Safe for your library:** it only reads your audio files. It never changes, moves, renames or deletes them; everything it learns, and everything you edit, is kept in its own database.
- **Made for real libraries:** single m4b files, folders of parts, CD subfolders and loose files in author folders, with or without good tags. Tested with 1000 books.

Version 1.0.0 is packaged for Arch Linux and Arch-based distributions (Manjaro, EndeavourOS, CachyOS…). On other distributions you can build the Flatpak yourself; publishing it on Flathub is planned for 1.1.0. Windows support is on hold.

## Contents

- [Installing](#installing)
- [Getting started](#getting-started)
- [Features](#features)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Your data](#your-data)
- [Known limitations](#known-limitations)
- [Building from source](#building-from-source)
- [AI-assisted development](#ai-assisted-development)
- [License](#license)

## Installing

### From a release (recommended)

Each release on the [Releases page](https://github.com/SpikeTheDragon40k/audium/releases) has these files:

| File | What it is |
|---|---|
| `audium-<version>-1-x86_64.pkg.tar.zst` | The ready-to-install Arch package |
| `audium-<version>.tar.gz` | The source code of that release |
| `PKGBUILD` | The recipe to build the package yourself |
| `SHA256SUMS` | Checksums, to check the downloads |

**1. Download** the package and `SHA256SUMS` from the release, in your browser or with the GitHub CLI:

```sh
gh release download v1.0.0 --repo SpikeTheDragon40k/audium \
  --pattern 'audium-1.0.0-1-x86_64.pkg.tar.zst' --pattern 'SHA256SUMS'
```

**2. Check** that the download is intact (optional, recommended). It should print `audium-1.0.0-1-x86_64.pkg.tar.zst: OK`:

```sh
sha256sum --check --ignore-missing SHA256SUMS
```

**3. Install.** pacman also installs the libraries Audium needs (Qt 6, the KDE Frameworks, mpv) if they are missing:

```sh
sudo pacman -U audium-1.0.0-1-x86_64.pkg.tar.zst
```

Audium is now in your application launcher (under Multimedia), or run `audium` from a terminal.

**Updating:** download the newer package and install it the same way with `sudo pacman -U`. Your library, progress and settings are kept.

**Uninstalling:** `sudo pacman -R audium`. Your data stays in `~/.local/share/audium` and `~/.cache/audium`; delete those folders too if you want it gone.

### Building the package yourself

If you prefer to build the package from the release's source:

```sh
mkdir audium-build && cd audium-build
gh release download v1.0.0 --repo SpikeTheDragon40k/audium \
  --pattern 'audium-1.0.0.tar.gz' --pattern 'PKGBUILD' --pattern 'SHA256SUMS'
makepkg -si
```

`makepkg` checks the source against the checksum in the PKGBUILD, installs the build tools it needs (Rust, CMake, …), builds Audium and installs the package. The build takes a few minutes.

### Flatpak (any distribution)

Audium is not on Flathub yet. Until it is, you can build the Flatpak from the source and install it for your user. You need Flatpak with the [Flathub](https://flathub.org/setup) remote, and then:

```sh
# Once: flatpak-builder from your distribution (e.g. sudo pacman -S flatpak-builder,
# sudo apt install flatpak-builder, sudo dnf install flatpak-builder),
# then the KDE SDK and Rust (about 2 GB)
flatpak install flathub org.kde.Sdk//6.11 org.freedesktop.Sdk.Extension.rust-stable//25.08

# Build and install (the first build takes a while: it also builds mpv)
git clone https://github.com/SpikeTheDragon40k/audium.git && cd audium
flatpak-builder --user --install --force-clean build-flatpak \
  packaging/flatpak/io.github.SpikeTheDragon40k.Audium.yml
```

Then start Audium from your application launcher, or with `flatpak run io.github.SpikeTheDragon40k.Audium`.

The Flatpak can read all your files (read-only, so your library can be anywhere) and can't change them. It keeps its own library and settings in `~/.var/app/io.github.SpikeTheDragon40k.Audium/`, separate from an Arch-package install; use **Export…** and **Import…** to move your progress between them.

## Getting started

1. **Add your library.** Click **Add Library Folder…** and choose the folder with your audiobooks. You can add several folders, and remove them later in **Settings**. Audium scans them in the background, and you can keep using it meanwhile.
2. **Check the review list.** Books Audium is unsure about (no author, a guessed title, an unusual folder layout) appear in **Needs Review** (a banner at the top of the library opens it). There you can fix their details, merge books that belong together, or split one that doesn't. You can also leave them as they are.
3. **Listen.** Click a book, then **Play**. Audium remembers your position, so **Resume** picks up where you stopped, even after a restart. The player bar at the bottom of the window is always there; click it for **Now Playing**, with chapters, speed, volume and the sleep timer.

Audium rescans your folders each time it starts, so new books appear by themselves. To rescan now, use **Rescan**. If a drive with your books isn't connected, those books show as unavailable; nothing is lost, and they come back when the drive does.

## Features

- **Library:** grid, list or bookshelf; Home with Continue Listening, Recently Added and Finished; authors, series and your own collections; search by title, author, narrator or series.
- **Formats:** m4b, m4a, mp3, aac, ogg, oga, opus, flac, wav, wma, mka. Books made of many files play as one, without gaps.
- **Metadata:** read from `metadata.json`, `desc.txt`, `reader.txt` and `cover.*`/`folder.*` next to the files, then from the files' tags, then from folder and file names. Your edits are stored in Audium, never written into the files.
- **Listening:** resume anywhere, speed from 0.5× to 3× without chipmunk voices (remembered per book), configurable skip back and forward, chapters, and bookmarks with notes.
- **Sleep timer:** after a number of minutes or at the end of the chapter, with a gentle fade-out. Moving the mouse or pressing a key over Audium during the fade gives you more time.
- **Statistics:** time listened today, this week, this month and in total, your listening streak, books finished, a 30-day chart and your most listened books.
- **Desktop integration:** media keys, Plasma's media controls and lock screen, a tray icon (closing the window keeps playing in the tray; this can be turned off), and notifications when you finish a book or the sleep timer stops playback.
- **Appearance:** follows your Plasma colours, light or dark, or uses the Dracula or Nord theme (**Settings → Appearance**).

## Keyboard shortcuts

| Key | Action |
|---|---|
| Space | Play / pause |
| ← / → | Skip back / forward (lengths in **Settings → Playback**) |
| `[` / `]` | Slower / faster |
| `B` | Add a bookmark |
| Ctrl+F | Search the library |
| Escape | Close the open page |

## Your data

- **Where it is:** the database with your progress, bookmarks, statistics, edits and settings is `~/.local/share/audium/audium.db`. Cover thumbnails are cached in `~/.cache/audium/`.
- **Backups:** **Settings → Your Data → Back Up Database…** saves a complete copy. Audium also makes a backup by itself before every database upgrade.
- **Moving to another computer:** **Export…** saves your progress, bookmarks, statistics, edits, collections and settings to a JSON file; **Import…** on the other computer shows what will change before applying it. Books are recognised by their content, so the folders can be organised differently there.
- **Resetting statistics:** **Reset Statistics…** deletes the listening history after asking. Progress, bookmarks and finished books stay.

## Known limitations

- **Linux only for now.** There is a ready-made package for Arch; other distributions build the Flatpak themselves until it is on Flathub (planned for 1.1.0). Windows is on hold.
- **Chapters** are read from MP3 (ID3 `CHAP`) and MP4/M4B files. Other formats, and files without chapter data, get one chapter per file.
- **Extending the sleep timer by moving the mouse** works only over Audium's window: Wayland does not let applications see the pointer elsewhere. The media keys and the tray's Play/Pause extend it too.
- **The bookshelf view** scrolls less smoothly than the grid and list in very large libraries with many small shelves.
- **English only** so far; the interface is ready for translations.
- **Logs** go to the terminal only (run `audium` from a terminal to see them); there is no log file yet.

## Building from source

Dependencies on Arch:

```sh
sudo pacman -S --needed rust cmake ninja extra-cmake-modules corrosion \
  qt6-base qt6-declarative qt6-svg kirigami qqc2-desktop-style \
  ki18n kcoreaddons kconfig kcolorscheme knotifications kstatusnotifieritem \
  mpv ffmpeg
```

`ffmpeg` is needed only for the tests' generated audio. `rustup` works instead of `rust`; the toolchain version is pinned in `rust-toolchain.toml`.

Build and run:

```sh
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build build
build/bin/audium
```

Development scripts:

| Script | What it does |
|---|---|
| `scripts/check.sh` | Every check: formatting, clippy, Rust tests, the build, a staged install, the desktop metadata and the Qt tests |
| `scripts/make-fixtures.sh` | Generates the synthetic test audio (no audio files are committed) |
| `scripts/perf.sh` | Measures scanning, scrolling, search and memory on a generated 1000-book library |
| `scripts/release.sh <version>` | Builds a release (checks, source tarball, Arch package, checksums) and publishes it on GitHub after asking |

The design is described in [ARCHITECTURE.md](ARCHITECTURE.md), and progress and plans in [ROADMAP.md](ROADMAP.md).

## AI-assisted development

This software is developed with AI assistance. All code is tested, reviewed and built by a human maintainer before it is committed or released.

## License

Audium is free software, licensed under the [GNU General Public License, version 3](LICENSE) (GPL-3.0-only).
