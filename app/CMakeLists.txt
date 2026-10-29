# The QML UI is a static QML module so the executable and the tests load the same thing.
add_library(audium_ui STATIC)
qt_add_qml_module(audium_ui
    URI org.audium.app
    OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/qml/org/audium/app
    QML_FILES
        qml/Main.qml
)
target_link_libraries(audium_ui PUBLIC
    Qt::Quick
    Qt::QuickControls2
    KF6::I18n
    KF6::I18nQml
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
