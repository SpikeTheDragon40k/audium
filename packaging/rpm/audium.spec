# RPM package for Audium, for Fedora and openSUSE Tumbleweed. Built from a
# release tarball in a container of each distribution by
# scripts/build-packages.sh, which also sets the version.

# The same reason as the PKGBUILD's '!lto': with LTO flags, the C++ compiled
# inside the Rust bridge's build script can't be linked into the app.
%global _lto_cflags %{nil}
# The Rust code is built by cargo without debug info.
%global debug_package %{nil}

Name:           audium
Version:        1.1.1
Release:        1%{?dist}
Summary:        Offline audiobook player for KDE Plasma
License:        GPL-3.0-only
URL:            https://github.com/SpikeTheDragon40k/audium
Source0:        %{name}-%{version}.tar.gz
ExclusiveArch:  x86_64

BuildRequires:  cmake
BuildRequires:  gcc-c++
BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  corrosion
BuildRequires:  pkgconfig(mpv)
BuildRequires:  cmake(Qt6Core)
BuildRequires:  cmake(Qt6DBus)
BuildRequires:  cmake(Qt6Gui)
BuildRequires:  cmake(Qt6Qml)
BuildRequires:  cmake(Qt6Quick)
BuildRequires:  cmake(Qt6QuickControls2)
BuildRequires:  cmake(Qt6Test)
BuildRequires:  cmake(Qt6Widgets)
BuildRequires:  cmake(KF6Kirigami)
BuildRequires:  cmake(KF6I18n)
BuildRequires:  cmake(KF6CoreAddons)
BuildRequires:  cmake(KF6ColorScheme)
BuildRequires:  cmake(KF6Config)
BuildRequires:  cmake(KF6Notifications)
BuildRequires:  cmake(KF6StatusNotifierItem)
%if 0%{?suse_version}
BuildRequires:  kf6-extra-cmake-modules
BuildRequires:  ninja
%else
BuildRequires:  extra-cmake-modules
BuildRequires:  ninja-build
%endif

# QML modules and the desktop style are loaded at run time, so they don't
# show up as library dependencies.
%if 0%{?suse_version}
Requires:       kf6-kirigami-imports
Requires:       kf6-qqc2-desktop-style
Requires:       qt6-declarative-imports
Requires:       libQt6Svg6
%else
Requires:       kf6-kirigami
Requires:       kf6-qqc2-desktop-style
Requires:       qt6-qtdeclarative
Requires:       qt6-qtsvg
%endif
Requires:       hicolor-icon-theme

%description
Audium is a desktop audiobook player for the books you already have on disk.
It works entirely offline: no accounts, no cloud, no online lookups, no
telemetry. It only reads your library and never changes, moves, renames or
deletes your audio files.

%prep
%autosetup

%build
export CXXFLAGS="%{optflags}"
export LDFLAGS="%{?build_ldflags}"
# The tests (scripts/check.sh) run before a release, not here.
cmake -B rpm-build -S . -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=%{_prefix} \
    -DBUILD_TESTING=OFF
cmake --build rpm-build

%install
DESTDIR=%{buildroot} cmake --install rpm-build
# Without a debug package, openSUSE doesn't strip binaries (Fedora does).
strip --strip-unneeded %{buildroot}%{_bindir}/audium

%files
%license LICENSE
%doc README.md
%{_bindir}/audium
%{_datadir}/applications/io.github.SpikeTheDragon40k.Audium.desktop
%{_datadir}/metainfo/io.github.SpikeTheDragon40k.Audium.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/io.github.SpikeTheDragon40k.Audium.svg
%{_datadir}/audium/

%changelog
* Sat Oct 31 2026 Adriano Inghingolo <68734231+SpikeTheDragon40k@users.noreply.github.com> - 1.1.1-1
- Packages for Fedora, openSUSE Tumbleweed, Debian and Ubuntu.
* Sat Oct 31 2026 Adriano Inghingolo <68734231+SpikeTheDragon40k@users.noreply.github.com> - 1.1.0-1
- See ROADMAP.md in the source for what each release contains.
