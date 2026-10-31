cmake_minimum_required(VERSION 3.24)

project(audium VERSION 1.1.0 LANGUAGES CXX)

set(QT_MIN_VERSION "6.8.0")
set(KF_MIN_VERSION "6.10.0")

find_package(ECM ${KF_MIN_VERSION} REQUIRED NO_MODULE)
list(APPEND CMAKE_MODULE_PATH ${ECM_MODULE_PATH})

include(KDEInstallDirs)
include(KDECMakeSettings)
include(KDECompilerSettings NO_POLICY_SCOPE)
include(FeatureSummary)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# KDECompilerSettings enables -Wmissing-include-dirs, which fires for the
# (legitimately empty) autogen include dirs of Qt's QML module targets.
if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
    add_compile_options(-Wno-missing-include-dirs)
endif()

find_package(Qt6 ${QT_MIN_VERSION} REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 Widgets Test)
find_package(KF6 ${KF_MIN_VERSION} REQUIRED COMPONENTS Kirigami I18n CoreAddons ColorScheme Config Notifications StatusNotifierItem)
if(UNIX AND NOT APPLE)
    # MPRIS (media keys and Plasma's media controls).
    find_package(Qt6 ${QT_MIN_VERSION} REQUIRED COMPONENTS DBus)
endif()

qt_policy(SET QTP0001 NEW)
qt_policy(SET QTP0004 NEW)

# CXX-Qt CMake integration (vendored, see cmake/third_party/cxx-qt-cmake/VENDORED.txt).
# It uses the system Corrosion package.
include(${CMAKE_CURRENT_SOURCE_DIR}/cmake/third_party/cxx-qt-cmake/CxxQt.cmake)

# Rust code is never built with the MSVC debug runtime (see CxxQt.cmake).
if(MSVC)
    set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreadedDLL")
endif()

cxx_qt_import_crate(
    MANIFEST_PATH ${CMAKE_CURRENT_SOURCE_DIR}/crates/audium-bridge/Cargo.toml
    CRATES audium_bridge
    LOCKED
    QT_MODULES Qt::Core Qt::Gui Qt::Qml Qt::Quick
)
cxx_qt_import_qml_module(audium_bridge_qml
    URI "org.audium.bridge"
    SOURCE_CRATE audium_bridge
)

add_subdirectory(app)

include(CTest)
if(BUILD_TESTING)
    add_subdirectory(app/tests)
endif()

feature_summary(WHAT ALL INCLUDE_QUIET_PACKAGES FATAL_ON_MISSING_REQUIRED_PACKAGES)
