find_package(PkgConfig REQUIRED)
pkg_check_modules(MPV REQUIRED IMPORTED_TARGET mpv>=2.0)

# Everything except main() lives in this static QML module, so the executable
# and the tests load exactly the same code.
add_library(audium_ui STATIC
    src/playback/playbackcontroller.cpp
    src/playback/playbackcontroller.h
    src/playback/mpvplaybackbackend.cpp
    src/playback/mpvplaybackbackend.h
    src/sessionwiring.cpp
    src/sessionwiring.h
)
qt_add_qml_module(audium_ui
    URI org.audium.app
    OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/qml/org/audium/app
    QML_FILES
        qml/Main.qml
        qml/pages/LibraryPage.qml
        qml/pages/NowPlayingPage.qml
        qml/pages/BookDetailsPage.qml
        qml/pages/NeedsReviewPage.qml
        qml/pages/SettingsPage.qml
        qml/pages/BooksPage.qml
        qml/pages/EditMetadataPage.qml
        qml/pages/SplitBookPage.qml
        qml/components/MiniPlayer.qml
        qml/components/BookCover.qml
        qml/components/BookTile.qml
        qml/components/BookRow.qml
        qml/components/BookGrid.qml
        qml/components/BookList.qml
        qml/components/GroupGrid.qml
        qml/components/Bookshelf.qml
        qml/components/SleepTimerButton.qml
        qml/components/TimeFormat.js
        qml/components/ReviewReasons.js
)
target_include_directories(audium_ui PUBLIC src)
# qmltyperegistrar includes registered headers by bare file name.
target_include_directories(audium_ui PRIVATE src/playback src)
target_link_libraries(audium_ui PUBLIC
    Qt::Quick
    Qt::QuickControls2
    KF6::I18n
    KF6::I18nQml
    PkgConfig::MPV
    audium_bridge_qml
)

add_executable(audium
    src/main.cpp
)
target_compile_definitions(audium PRIVATE AUDIUM_VERSION_STRING="${PROJECT_VERSION}")
target_link_libraries(audium PRIVATE
    Qt::Widgets
    KF6::CoreAddons
    audium_ui
    audium_uiplugin
)

install(TARGETS audium ${KDE_INSTALL_TARGETS_DEFAULT_ARGS})
