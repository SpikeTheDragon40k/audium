#!/usr/bin/env bash
# Builds Audium's .rpm and .deb packages from a release tarball, each in a
# container of its distribution (Docker), then installs every package in a
# fresh container of the same distribution and starts the app there.
#
# Usage: scripts/build-packages.sh <version> [target...]
#   targets: fedora debian ubuntu tumbleweed (default: all)
#
# Needs dist/<version>/audium-<version>.tar.gz (scripts/release.sh makes it;
# otherwise it is made here from the tag v<version>). The packages go to
# dist/<version>/. The packaging files are packaging/rpm/audium.spec and
# packaging/debian/; their version must match.
#
# To try unreleased source: AUDIUM_TARBALL=<file> (an audium-<version>/
# tree) and AUDIUM_OUT=<dir> (instead of dist/<version>).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
version="${1:?usage: scripts/build-packages.sh <version> [target...]}"
shift
targets=("$@")
[[ ${#targets[@]} -gt 0 ]] || targets=(fedora debian ubuntu tumbleweed)
dist="${AUDIUM_OUT:-$root/dist/$version}"
tarball="${AUDIUM_TARBALL:-$dist/audium-$version.tar.gz}"

fail() { echo "build-packages: $*" >&2; exit 1; }

command -v docker >/dev/null || fail "docker not found"
grep -q "^Version: *$version$" packaging/rpm/audium.spec || fail "packaging/rpm/audium.spec is not at version $version"
if [[ ! -f "$tarball" ]]; then
    git rev-parse -q --verify "refs/tags/v$version" >/dev/null || fail "no $tarball and no tag v$version"
    mkdir -p "$dist"
    git archive --format=tar.gz --prefix="audium-$version/" -o "$tarball" "v$version"
fi

# image | package family | release suffix (the distribution in the file name)
target_info() {
    case "$1" in
        fedora) echo "fedora:44 rpm fc44" ;;
        tumbleweed) echo "opensuse/tumbleweed:latest rpm opensuse_tw" ;;
        debian) echo "debian:trixie deb debian13" ;;
        ubuntu) echo "ubuntu:26.10 deb ubuntu26.10" ;;
        *) fail "unknown target: $1" ;;
    esac
}

# Runs inside the build container. /work holds the tarball and the
# packaging files; packages go to /work/out.
read -r -d '' build_inside <<'INSIDE' || true
set -euo pipefail
# Hand everything back to the user running the script, even after a failure.
trap 'chown -R "$HOST_UID:$HOST_GID" /work' EXIT
cd /work
case "$FAMILY" in
rpm)
    mkdir -p rpmbuild/SOURCES rpmbuild/SPECS
    cp "audium-$VERSION.tar.gz" rpmbuild/SOURCES/
    cp audium.spec rpmbuild/SPECS/
    if command -v zypper >/dev/null; then
        zypper -n -q install rpm-build >/dev/null
        zypper -n -q install $(rpmspec -q --buildrequires rpmbuild/SPECS/audium.spec) >/dev/null
    else
        dnf -y -q install rpm-build 'dnf-command(builddep)' >/dev/null
        dnf -y -q builddep rpmbuild/SPECS/audium.spec >/dev/null
    fi
    rpmbuild -bb --define "_topdir /work/rpmbuild" --define "dist .$SUFFIX" rpmbuild/SPECS/audium.spec
    cp rpmbuild/RPMS/x86_64/audium-*.rpm out/
    ;;
deb)
    export DEBIAN_FRONTEND=noninteractive
    apt-get -qq update
    apt-get -qq install -y --no-install-recommends build-essential debhelper curl ca-certificates git >/dev/null
    tar xzf "audium-$VERSION.tar.gz"
    cp "audium-$VERSION.tar.gz" "audium_$VERSION.orig.tar.gz"
    cp -r debian "audium-$VERSION/debian"
    cd "audium-$VERSION"
    printf 'audium (%s-1+%s) %s; urgency=medium\n\n  * Release %s.\n\n -- %s  %s\n' \
        "$VERSION" "$SUFFIX" "$(. /etc/os-release && echo "$VERSION_CODENAME")" "$VERSION" \
        "$(sed -n 's/^Maintainer: //p' debian/control)" "$(date -R)" > debian/changelog
    apt-get -qq build-dep -y ./ >/dev/null
    # The distribution's Rust is too old: the version pinned in
    # rust-toolchain.toml, from rustup.
    curl -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal --default-toolchain none
    export PATH="$HOME/.cargo/bin:$PATH"
    rustup -q show active-toolchain || rustup -q toolchain install
    dpkg-buildpackage -b -us -uc
    cp ../audium_*_amd64.deb /work/out/
    ;;
esac
INSIDE

# Runs inside a fresh container: installs the package with the
# distribution's package manager (which pulls the dependencies) and starts
# the app offscreen for a few seconds with throwaway data folders.
read -r -d '' test_inside <<'INSIDE' || true
set -euo pipefail
case "$FAMILY" in
rpm)
    if command -v zypper >/dev/null; then
        zypper -n -q --no-gpg-checks install --allow-unsigned-rpm "/work/$PACKAGE" >/dev/null
    else
        dnf -y -q install "/work/$PACKAGE" >/dev/null
    fi
    ;;
deb)
    export DEBIAN_FRONTEND=noninteractive
    apt-get -qq update
    apt-get -qq install -y "/work/$PACKAGE" >/dev/null
    ;;
esac
missing="$(ldd /usr/bin/audium | grep 'not found' || true)"
[[ -z "$missing" ]] || { echo "missing libraries:"; echo "$missing"; exit 1; }
tmp="$(mktemp -d)"
export XDG_DATA_HOME="$tmp/data" XDG_CACHE_HOME="$tmp/cache" XDG_CONFIG_HOME="$tmp/config" XDG_RUNTIME_DIR="$tmp/run"
mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"
rc=0
QT_QPA_PLATFORM=offscreen timeout 10 audium > "$tmp/log" 2>&1 || rc=$?
# Audium's own QML errors and missing modules fail; warnings from other
# packages' QML (e.g. a distribution's older KDE style) don't.
if [[ $rc -ne 124 ]] || grep -qiE 'org/audium/|is not installed|module .* not found|failed to load|cannot load' "$tmp/log"; then
    echo "Audium did not keep running (exit $rc), or reported QML errors:"
    cat "$tmp/log"
    exit 1
fi
echo "installed and started: $PACKAGE"
INSIDE

for target in "${targets[@]}"; do
    read -r image family suffix <<<"$(target_info "$target")"
    echo "==> $target ($image)"
    work="$(mktemp -d)"
    mkdir -p "$work/out"
    cp "$tarball" "$work/audium-$version.tar.gz"
    case "$family" in
        rpm) cp packaging/rpm/audium.spec "$work/" ;;
        deb) cp -r packaging/debian "$work/debian" ;;
    esac
    docker run --rm -v "$work:/work" \
        -e FAMILY="$family" -e VERSION="$version" -e SUFFIX="$suffix" \
        -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
        "$image" bash -c "$build_inside"
    for package in "$work"/out/*; do
        name="$(basename "$package")"
        docker run --rm -v "$work/out:/work:ro" -e FAMILY="$family" -e PACKAGE="$name" \
            "$image" bash -c "$test_inside"
        cp "$package" "$dist/"
    done
    rm -rf -- "$work"
done

echo
echo "Packages in $dist:"
ls -1 "$dist" | grep -E '\.(rpm|deb)$'
