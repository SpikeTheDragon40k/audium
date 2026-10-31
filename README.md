# Audium

**An offline audiobook player for the Linux desktop.** Audium plays the audiobooks you already have on disk, remembers where you are in every book, and stays out of your files.

- **Native and light:** Qt 6 and Kirigami, made for KDE Plasma 6 on Wayland. No web views, no server.
- **Private:** no accounts, no cloud, no online lookups, no telemetry. Audium makes no network connections at all.
- **Safe for your library:** it only reads your audio files. It never changes, moves, renames or deletes them; everything it learns, and everything you edit, is kept in its own database.
- **Made for real libraries:** single m4b files, folders of parts, CD subfolders and loose files in author folders, with or without good tags. Tested with 1000 books.

Packages are available for Arch Linux, Fedora, openSUSE Tumbleweed, Debian 13 and Ubuntu 26.10, plus a Flatpak file for any distribution (see [Installing](#installing)). Windows support is on hold.

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

Every release on the [Releases page](https://github.com/SpikeTheDragon40k/audium/releases) has packages for these distributions, plus a Flatpak file for any other:

| Distribution | File |
|---|---|
| Arch Linux and Arch-based (Manjaro, EndeavourOS, CachyOS…) | `audium-<version>-1-x86_64.pkg.tar.zst`, or the AUR package [`audium-audiobooks`](https://aur.archlinux.org/packages/audium-audiobooks) |
| Fedora 44 | `audium-<version>-1.fc44.x86_64.rpm` |
| openSUSE Tumbleweed | `audium-<version>-1.opensuse_tw.x86_64.rpm` |
| Debian 13 | `audium_<version>-1+debian13_amd64.deb` |
| Ubuntu 26.10 | `audium_<version>-1+ubuntu26.10_amd64.deb` |
| Any distribution with Flatpak | `audium-<version>.flatpak` |

Each package works only on the distribution it was built for, because it uses that distribution's Qt and KDE libraries. Ubuntu 24.04 and Debian 12 are too old (they don't have KDE Frameworks 6): use the Flatpak there. Each release also has the source code (`audium-<version>.tar.gz`), the Arch `PKGBUILD` and `SHA256SUMS`.

The commands below use the [GitHub CLI](https://cli.github.com/) to download the latest release; you can also download the files in your browser. Checking the download is optional, but recommended:

```sh
gh release download --repo SpikeTheDragon40k/audium --pattern SHA256SUMS
sha256sum --check --ignore-missing SHA256SUMS   # prints "<file>: OK"
```

Your package manager also installs the libraries Audium needs (Qt 6, the KDE Frameworks, mpv). Afterwards Audium is in your application launcher (under Multimedia), or run `audium` from a terminal.

### Arch Linux

From the AUR, with an AUR helper (it builds Audium from source, and updates come with your system updates):

```sh
yay -S audium-audiobooks
```

The AUR name `audium` belongs to an unrelated terminal music player, which also installs an `audium` command; the two can't be installed together.

Or the ready-made package from the release (update by installing the next release's package the same way):

```sh
gh release download --repo SpikeTheDragon40k/audium --pattern '*-x86_64.pkg.tar.zst'
sudo pacman -U audium-*-x86_64.pkg.tar.zst
```

To build the release's package yourself instead: download `audium-<version>.tar.gz` and `PKGBUILD` into an empty folder and run `makepkg -si`.

### Fedora

```sh
gh release download --repo SpikeTheDragon40k/audium --pattern '*.fc44.x86_64.rpm'
sudo dnf install ./audium-*.fc44.x86_64.rpm
```

### openSUSE Tumbleweed

```sh
gh release download --repo SpikeTheDragon40k/audium --pattern '*.opensuse_tw.x86_64.rpm'
sudo zypper install --allow-unsigned-rpm ./audium-*.opensuse_tw.x86_64.rpm
```

The packages aren't signed yet, hence `--allow-unsigned-rpm`; checking `SHA256SUMS` (above) confirms the file is the one released here.

### Debian and Ubuntu

```sh
# Debian 13
gh release download --repo SpikeTheDragon40k/audium --pattern '*+debian13_amd64.deb'
# Ubuntu 26.10
gh release download --repo SpikeTheDragon40k/audium --pattern '*+ubuntu26.10_amd64.deb'

sudo apt install ./audium_*_amd64.deb
```

### Flatpak (any distribution)

You need Flatpak with the [Flathub](https://flathub.org/setup) remote (it provides the KDE runtime the file uses). Audium itself is not on Flathub.

```sh
gh release download --repo SpikeTheDragon40k/audium --pattern '*.flatpak'
flatpak install --user ./audium-*.flatpak
```

Then start Audium from your application launcher, or with `flatpak run io.github.SpikeTheDragon40k.Audium`.

The Flatpak can read all your files (read-only, so your library can be anywhere) and can't change them. It keeps its own library and settings in `~/.var/app/io.github.SpikeTheDragon40k.Audium/`, separate from a native install; use **Export…** and **Import…** to move your progress between them. To build the Flatpak from source instead, see [Building from source](#building-from-source).

### Updating and uninstalling

**Updating:** install the newer release's file the same way (the AUR updates by itself). Your library, progress and settings are kept.

**Uninstalling:** `sudo pacman -R audium` (or `audium-audiobooks`), `sudo dnf remove audium`, `sudo zypper remove audium`, `sudo apt remove audium` or `flatpak uninstall io.github.SpikeTheDragon40k.Audium`. A native install keeps your data in `~/.local/share/audium` and `~/.cache/audium`; delete those folders too if you want it gone.

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

The Flatpak, from the source:

```sh
# Once: flatpak-builder from your distribution, then the KDE SDK and Rust (about 2 GB)
flatpak install flathub org.kde.Sdk//6.11 org.freedesktop.Sdk.Extension.rust-stable//25.08
# Build and install (the first build takes a while: it also builds mpv)
flatpak-builder --user --install --force-clean build-flatpak \
  packaging/flatpak/io.github.SpikeTheDragon40k.Audium.yml
```

Development scripts:

| Script | What it does |
|---|---|
| `scripts/check.sh` | Every check: formatting, clippy, Rust tests, the build, a staged install, the desktop metadata and the Qt tests |
| `scripts/make-fixtures.sh` | Generates the synthetic test audio (no audio files are committed) |
| `scripts/perf.sh` | Measures scanning, scrolling, search and memory on a generated 1000-book library |
| `scripts/release.sh <version>` | Builds a release (checks, source tarball, every package below, checksums, the AUR files) and publishes it on GitHub after asking |
| `scripts/build-packages.sh <version>` | Builds the Fedora, openSUSE, Debian and Ubuntu packages in containers of each distribution (Docker), then installs each one in a fresh container and starts it |
| `scripts/build-flatpak-bundle.sh <version>` | Builds the release's `.flatpak` file from its git tag |
| `scripts/make-screenshots.sh` | Takes the app-store screenshots from a generated demo library |

The design is described in [ARCHITECTURE.md](ARCHITECTURE.md), and progress and plans in [ROADMAP.md](ROADMAP.md).

## AI-assisted development

This software is developed with AI assistance. All code is tested, reviewed and built by a human maintainer before it is committed or released.

## License

Audium is free software, licensed under the [GNU General Public License, version 3](LICENSE) (GPL-3.0-only).
