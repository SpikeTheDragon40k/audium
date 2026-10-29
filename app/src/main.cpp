#include <QApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QStandardPaths>
#include <QtQml>
#include <QtQml/QQmlExtensionPlugin>

#include <KAboutData>
#include <KLocalizedQmlContext>
#include <KLocalizedString>

#include "audium_bridge/src/library_controller.cxxqt.h"
#include "libraryplayback.h"
#include "playback/mpvplaybackbackend.h"

Q_IMPORT_QML_PLUGIN(org_audium_appPlugin)

int main(int argc, char *argv[])
{
    QApplication app(argc, argv);

    KLocalizedString::setApplicationDomain("audium");
    QCoreApplication::setOrganizationDomain(QStringLiteral("audium.org"));

    KAboutData about(QStringLiteral("audium"),
                     i18nc("@title", "Audium"),
                     QStringLiteral(AUDIUM_VERSION_STRING),
                     i18n("Offline audiobook player"),
                     KAboutLicense::Unknown);
    about.setDesktopFileName(QStringLiteral("org.audium.Audium"));
    KAboutData::setApplicationData(about);
    QGuiApplication::setWindowIcon(QIcon::fromTheme(QStringLiteral("audiobook"), QIcon::fromTheme(QStringLiteral("media-playback-start"))));

    // Use the desktop (Breeze) style unless the user chose another one.
    if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE")) {
        QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));
    }

    // Created before the engine so they outlive every QML binding using them.
    MpvPlaybackBackend player;
    LibraryPlayback libraryPlayback(&player);
    LibraryController library;
    // Failure is shown in the UI (library.errorString); playback still works.
    library.open(QStandardPaths::writableLocation(QStandardPaths::AppDataLocation),
                 QStandardPaths::writableLocation(QStandardPaths::CacheLocation));

    QQmlApplicationEngine engine;
    KLocalization::setupLocalizedContext(&engine);
    engine.setInitialProperties({
        {QStringLiteral("player"), QVariant::fromValue(static_cast<PlaybackController *>(&player))},
        {QStringLiteral("library"), QVariant::fromValue(&library)},
        {QStringLiteral("libraryPlayback"), QVariant::fromValue(&libraryPlayback)},
    });
    engine.loadFromModule("org.audium.app", "Main");
    if (engine.rootObjects().isEmpty()) {
        return 1;
    }
    return app.exec();
}
