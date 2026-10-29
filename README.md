# Audium

An offline-first desktop audiobook player for Linux (primary: Arch + KDE Plasma 6 on Wayland) and Windows.

- Native Qt 6 / Kirigami UI. No web views, no server, no accounts, no telemetry, no network access.
- Reads your library and never modifies, moves, renames or deletes audio files.
- Built to cope with large, messy libraries.

> **Status:** early development. See [ROADMAP.md](ROADMAP.md) for progress and [ARCHITECTURE.md](ARCHITECTURE.md) for the design.

## AI-assisted development

This software is developed with AI assistance. All code is tested, reviewed and built by a human maintainer before it is committed or released.

## Building (Linux)

Dependencies (Arch):

```sh
sudo pacman -S --needed rustup cmake ninja extra-cmake-modules corrosion \
  qt6-base qt6-declarative kirigami kirigami-addons qqc2-desktop-style \
  ki18n kcoreaddons kconfig kcrash kdbusaddons knotifications \
  kstatusnotifieritem kiconthemes mpv ffmpeg
```

Build instructions will be added once Milestone 0 is complete.

## License

To be decided.
