#include <QApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QtQml>
#include <QtQml/QQmlExtensionPlugin>

#include <KAboutData>
#include <KLocalizedQmlContext>
#include <KLocalizedString>

#include "playback/mpvplaybackbackend.h"

Q_IMPORT_QML_PLUGIN(org_audium_appPlugin)

int main(int argc, char *argv[])
{
    QApplication app(argc, argv);

    KLocalizedString::setApplicationDomain("audium");
    QCoreApplication::setOrganizationName(QStringLiteral("Audium"));
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

    // Created before the engine so it outlives every QML binding that uses it.
    MpvPlaybackBackend player;

    QQmlApplicationEngine engine;
    KLocalization::setupLocalizedContext(&engine);
    engine.setInitialProperties({{QStringLiteral("player"), QVariant::fromValue(static_cast<PlaybackController *>(&player))}});
    engine.loadFromModule("org.audium.app", "Main");
    if (engine.rootObjects().isEmpty()) {
        return 1;
    }
    return app.exec();
}
