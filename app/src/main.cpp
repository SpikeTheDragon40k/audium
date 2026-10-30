#include <QApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QStandardPaths>
#include <QWindow>
#include <QtQml>
#include <QtQml/QQmlExtensionPlugin>

#include <KAboutData>
#include <KLocalizedQmlContext>
#include <KLocalizedString>

#include "audium_bridge/src/library_controller.cxxqt.h"
#include "audium_bridge/src/playback_session.cxxqt.h"
#include "integration/notifier.h"
#include "integration/tray.h"
#include "sessionwiring.h"
#include "theme.h"
#ifdef AUDIUM_HAVE_MPRIS
#include "integration/mpris.h"
#endif
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
    QGuiApplication::setWindowIcon(TrayIcon::appIcon());

    // Use the desktop (Breeze) style unless the user chose another one.
    if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE")) {
        QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));
    }

    // Created before the engine so they outlive every QML binding using them.
    MpvPlaybackBackend player;
    LibraryController library;
    PlaybackSession session;
    SessionWiring wiring(&player, &session);
    // Failure is shown in the UI (library.errorString); playback still works.
    if (library.open(QStandardPaths::writableLocation(QStandardPaths::AppDataLocation),
                     QStandardPaths::writableLocation(QStandardPaths::CacheLocation))) {
        session.setDatabasePath(library.getDatabasePath());
    }
    // The saved colour theme is applied before the window appears.
    Theme theme(&session);

    // Desktop integrations; each one may fail on its own (it then only logs).
    TrayIcon tray(&player, &session);
    Notifier notifier(&session);
#ifdef AUDIUM_HAVE_MPRIS
    Mpris mpris(&player, &session);
#endif

    QQmlApplicationEngine engine;
    KLocalization::setupLocalizedContext(&engine);
    engine.setInitialProperties({
        {QStringLiteral("player"), QVariant::fromValue(static_cast<PlaybackController *>(&player))},
        {QStringLiteral("library"), QVariant::fromValue(&library)},
        {QStringLiteral("session"), QVariant::fromValue(&session)},
        {QStringLiteral("tray"), QVariant::fromValue(&tray)},
    });
    engine.loadFromModule("org.audium.app", "Main");
    if (engine.rootObjects().isEmpty()) {
        return 1;
    }
    auto *window = qobject_cast<QWindow *>(engine.rootObjects().constFirst());
    tray.setWindow(window);
#ifdef AUDIUM_HAVE_MPRIS
    mpris.setWindow(window);
#endif
    return app.exec();
}
