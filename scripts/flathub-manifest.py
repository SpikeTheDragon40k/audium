#!/usr/bin/env python3
"""Writes the Flatpak manifest for Flathub from the local one.

The manifest in packaging/flatpak builds the working tree (`type: dir`),
which Flathub does not accept: there, the app's source must be a git tag
with its commit. This copies the manifest with that one source replaced,
plus cargo-sources.json, into dist/flathub/: the files that go into the
flathub/flathub pull request (and later the app's own Flathub repository).

Usage: scripts/flathub-manifest.py <tag>   (e.g. v1.1.0; the tag must exist)
"""

import shutil
import subprocess
import sys
from pathlib import Path

URL = "https://github.com/SpikeTheDragon40k/audium.git"
APP_ID = "io.github.SpikeTheDragon40k.Audium"

root = Path(__file__).resolve().parent.parent
if len(sys.argv) != 2:
    sys.exit(__doc__.strip().splitlines()[-1])
tag = sys.argv[1]
try:
    commit = subprocess.run(
        ["git", "-C", str(root), "rev-list", "-n", "1", f"refs/tags/{tag}"],
        check=True, capture_output=True, text=True,
    ).stdout.strip()
except subprocess.CalledProcessError:
    sys.exit(f"no tag {tag} in this repository")

source = root / "packaging/flatpak" / f"{APP_ID}.yml"
manifest = source.read_text()

local = """      - type: dir
        path: ../..
        skip:
          - .git
          - build
          - build-flatpak
          - .flatpak-builder
          - target
          - dist
"""
remote = f"""      - type: git
        url: {URL}
        tag: {tag}
        commit: {commit}
"""
if manifest.count(local) != 1:
    sys.exit(f"the app's `type: dir` source was not found exactly once in {source}")
manifest = manifest.replace(local, remote)
# The header's "build from the repository's root" instructions are for the
# local manifest.
header, sep, body = manifest.partition(f"app-id: {APP_ID}\n")
if not sep:
    sys.exit(f"no app-id line in {source}")
manifest = f"# Flatpak manifest for Audium {tag}, built from its git tag.\n" + sep + body

out = root / "dist/flathub"
out.mkdir(parents=True, exist_ok=True)
(out / f"{APP_ID}.yml").write_text(manifest)
shutil.copy(root / "packaging/flatpak/cargo-sources.json", out / "cargo-sources.json")
print(f"{out}/{APP_ID}.yml: {tag} at {commit}")
print(f"{out}/cargo-sources.json")
