#!/usr/bin/env python3
"""Writes the Flatpak sources for Audium's Rust dependencies.

Flatpak builds run without network access, so every crate in Cargo.lock is
listed as a download (with its checksum from Cargo.lock) that flatpak-builder
fetches beforehand and unpacks into cargo/vendor, plus a cargo config that
makes cargo use that directory instead of crates.io. This does the same job
as flatpak-builder-tools' cargo generator for a lock file with only
crates.io dependencies, which is what Audium has (the script refuses others).

Usage: scripts/flatpak-cargo-sources.py [Cargo.lock] [output.json]
       (default: Cargo.lock, packaging/flatpak/cargo-sources.json)
Re-run it whenever Cargo.lock changes.
"""

import json
import sys
import tomllib
from pathlib import Path

CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"
VENDOR = "cargo/vendor"

root = Path(__file__).resolve().parent.parent
lock_path = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "Cargo.lock"
out_path = Path(sys.argv[2]) if len(sys.argv) > 2 else root / "packaging/flatpak/cargo-sources.json"

lock = tomllib.loads(lock_path.read_text())
sources = []
for package in lock["package"]:
    source = package.get("source")
    if source is None:
        continue  # a crate of this workspace
    if source != CRATES_IO:
        sys.exit(f"{package['name']} {package['version']}: unsupported source {source}")
    name, version, checksum = package["name"], package["version"], package["checksum"]
    dest = f"{VENDOR}/{name}-{version}"
    sources.append({
        "type": "archive",
        "archive-type": "tar-gzip",
        "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
        "sha256": checksum,
        "dest": dest,
    })
    # Cargo checks vendored crates against this file; an empty file list
    # skips the per-file checks (the archive's checksum was checked above).
    sources.append({
        "type": "inline",
        "contents": json.dumps({"package": checksum, "files": {}}),
        "dest": dest,
        "dest-filename": ".cargo-checksum.json",
    })

sources.append({
    "type": "inline",
    "contents": (
        '[source.vendored-sources]\n'
        f'directory = "{VENDOR}"\n'
        '\n'
        '[source.crates-io]\n'
        'replace-with = "vendored-sources"\n'
    ),
    "dest": "cargo",
    "dest-filename": "config.toml",
})

out_path.parent.mkdir(parents=True, exist_ok=True)
out_path.write_text(json.dumps(sources, indent=4) + "\n")
print(f"{out_path}: {len(sources) // 2} crates")
