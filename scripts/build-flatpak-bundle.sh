#!/usr/bin/env bash
# Builds the Flatpak of a release from its git tag and packs it into a single
# file, dist/<version>/audium-<version>.flatpak, for the GitHub release.
# Installing it fetches the KDE runtime from Flathub; updating means
# installing the next release's file.
#
# Usage: scripts/build-flatpak-bundle.sh <version>
#
# Needs: the tag v<version>, flatpak-builder, and the SDKs listed in
# packaging/flatpak/io.github.SpikeTheDragon40k.Audium.yml.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
version="${1:?usage: scripts/build-flatpak-bundle.sh <version>}"
app_id=io.github.SpikeTheDragon40k.Audium
dist="$root/dist/$version"
work="$dist/flatpak"

# The manifest with the app's source pinned to the tag (dist/flathub/).
scripts/flathub-manifest.py "v$version"
rm -rf -- "$work"
mkdir -p "$work"
# From this repository rather than GitHub (same tag and commit): the release
# script pushes the tag only after everything is built.
sed "s|url: https://github.com/SpikeTheDragon40k/audium.git|url: file://$root|" \
    "dist/flathub/$app_id.yml" > "$work/$app_id.yml"
grep -q "url: file://$root" "$work/$app_id.yml" || { echo "build-flatpak-bundle: source URL not replaced" >&2; exit 1; }
cp dist/flathub/cargo-sources.json "$work/"

cd "$work"
flatpak-builder --force-clean --sandbox --disable-rofiles-fuse \
    --state-dir="$root/.flatpak-builder" --repo=repo builddir "$app_id.yml"
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
    repo "$dist/audium-$version.flatpak" "$app_id"
cd "$root"
rm -rf -- "$work"
echo "$dist/audium-$version.flatpak"
