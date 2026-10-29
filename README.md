# Audium

An offline-first desktop audiobook player for Linux (primary: Arch + KDE Plasma 6 on Wayland) and Windows.

- Native Qt 6 / Kirigami UI. No web views, no server, no accounts, no telemetry, no network access.
- Reads your library and never modifies, moves, renames or deletes audio files.
- Built to cope with large, messy libraries.

> **Status:** early development. See [ROADMAP.md](ROADMAP.md) for progress and [ARCHITECTURE.md](ARCHITECTURE.md) for the design.

## Building (Linux)

Dependencies (Arch):

```sh
sudo pacman -S --needed rustup cmake ninja extra-cmake-modules corrosion \
  qt6-base qt6-declarative kirigami kirigami-addons qqc2-desktop-style \
  ki18n kcoreaddons kconfig kcrash kdbusaddons knotifications \
  kstatusnotifieritem kiconthemes mpv ffmpeg
```

Build and run:

```sh
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build build
build/bin/audium
```

Run every check (formatting, clippy, Rust tests, the CMake build and Qt tests):

```sh
scripts/check.sh
```

Test audio is synthetic and generated locally with `scripts/make-fixtures.sh` (ffmpeg); no audio files are committed.

## AI-assisted development

This software is developed with AI assistance. All code is tested, reviewed and built by a human maintainer before it is committed or released.

## License

To be decided.
