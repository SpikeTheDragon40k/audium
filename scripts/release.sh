#!/usr/bin/env bash
# Builds a release locally and, after asking, publishes it on GitHub.
#
# Usage: scripts/release.sh <version>      e.g. scripts/release.sh 0.1.0
#
# Needs: a clean checkout of `main` with the tag v<version> on HEAD, the
# version in CMakeLists.txt, packaging/linux/PKGBUILD, packaging/aur/PKGBUILD
# and packaging/rpm/audium.spec, makepkg, Docker (scripts/build-packages.sh),
# flatpak-builder (scripts/build-flatpak-bundle.sh), and gh (logged in) for
# publishing. Output goes to dist/<version>/; the AUR files, with this
# release's checksum, to dist/<version>/aur/ (pushed to the AUR by hand).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
version="${1:?usage: scripts/release.sh <version>}"
tag="v$version"
dist="$root/dist/$version"

fail() { echo "release: $*" >&2; exit 1; }

[[ -z "$(git status --porcelain)" ]] || fail "the working tree has changes; commit them first"
[[ "$(git rev-parse HEAD)" == "$(git rev-parse "$tag^{commit}" 2>/dev/null)" ]] \
    || fail "tag $tag is not on HEAD (git tag -a $tag -m 'Audium $version')"
grep -q "project(audium VERSION $version " CMakeLists.txt || fail "CMakeLists.txt is not at version $version"
grep -q "^pkgver=$version$" packaging/linux/PKGBUILD || fail "PKGBUILD is not at version $version"
grep -q "^pkgver=$version$" packaging/aur/PKGBUILD || fail "packaging/aur/PKGBUILD is not at version $version"
grep -q "^Version: *$version$" packaging/rpm/audium.spec || fail "packaging/rpm/audium.spec is not at version $version"

echo "==> Checks"
scripts/check.sh

echo "==> Source tarball"
rm -rf -- "$dist"
mkdir -p "$dist/arch"
git archive --format=tar.gz --prefix="audium-$version/" -o "$dist/arch/audium-$version.tar.gz" "$tag"

echo "==> Arch package"
cp packaging/linux/PKGBUILD "$dist/arch/"
sum="$(sha256sum "$dist/arch/audium-$version.tar.gz" | cut -d' ' -f1)"
sed -i "s/^sha256sums=.*/sha256sums=('$sum')/" "$dist/arch/PKGBUILD"
(cd "$dist/arch" && makepkg --force --cleanbuild)
cp "$dist/arch/audium-$version.tar.gz" "$dist/arch/PKGBUILD" "$dist/"
cp "$dist"/arch/audium-"$version"-*.pkg.tar.zst "$dist/"

echo "==> Fedora, openSUSE Tumbleweed, Debian and Ubuntu packages"
scripts/build-packages.sh "$version"

echo "==> Flatpak bundle"
scripts/build-flatpak-bundle.sh "$version"

echo "==> AUR files"
mkdir -p "$dist/aur"
sed "s/^sha256sums=.*/sha256sums=('$sum')/" packaging/aur/PKGBUILD > "$dist/aur/PKGBUILD"
(cd "$dist/aur" && makepkg --printsrcinfo > .SRCINFO)

(cd "$dist" && sha256sum audium-* audium_* PKGBUILD > SHA256SUMS)

echo
echo "Built in $dist:"
ls -1 "$dist" | grep -vE '^(arch|aur)$'
echo
read -r -p "Publish $tag on GitHub with these files? [y/N] " answer
if [[ "$answer" == [yY] ]]; then
    git push origin "$tag"
    gh release create "$tag" --title "Audium $version" --notes "See ROADMAP.md for what this release contains." \
        "$dist"/audium-* "$dist"/audium_* "$dist/PKGBUILD" "$dist/SHA256SUMS"
else
    echo "Not published. Install locally with: sudo pacman -U $dist/audium-$version-*.pkg.tar.zst"
fi
