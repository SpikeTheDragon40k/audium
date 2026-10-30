#include "theme.h"

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QLoggingCategory>
#include <QPalette>
#include <QStandardPaths>

#include <KColorScheme>
#include <KSharedConfig>

#include "audium_bridge/src/playback_session.cxxqt.h"

Q_LOGGING_CATEGORY(lcTheme, "audium.theme")

namespace
{
// Read by KColorScheme (and through it by the Kirigami desktop theme) for
// the colours a palette has no role for, e.g. positive and negative text.
constexpr const char *SchemePathProperty = "KDE_COLOR_SCHEME_PATH";
}

Theme::Theme(PlaybackSession *session, QObject *parent)
    : QObject(parent)
    , m_session(session)
{
    connect(m_session, &PlaybackSession::themeChanged, this, &Theme::apply);
    apply();
}

QString Theme::schemeFile(int scheme)
{
    QString name;
    switch (scheme) {
    case Dracula:
        name = QStringLiteral("Dracula.colors");
        break;
    case Nord:
        name = QStringLiteral("Nord.colors");
        break;
    default:
        return {};
    }
    // Installed: <data dir>/audium/themes. Built but not installed: next to
    // the build's bin directory (CMake copies them there).
    const QString relative = QStringLiteral("themes/") + name;
    QString file = QStandardPaths::locate(QStandardPaths::AppDataLocation, relative);
    if (file.isEmpty()) {
        const QString beside = QDir(QCoreApplication::applicationDirPath()).filePath(QStringLiteral("../share/audium/") + relative);
        if (QFileInfo::exists(beside)) {
            file = QFileInfo(beside).canonicalFilePath();
        }
    }
    if (file.isEmpty()) {
        qCWarning(lcTheme) << "colour scheme not found:" << name;
    }
    return file;
}

void Theme::apply()
{
    const int scheme = m_session->getTheme();
    if (scheme == m_applied) {
        return;
    }
    m_applied = scheme;
    const QString file = schemeFile(scheme);
    if (file.isEmpty()) {
        // Back to the desktop's colours.
        qApp->setProperty(SchemePathProperty, QVariant());
        QGuiApplication::setPalette(QPalette());
    } else {
        qApp->setProperty(SchemePathProperty, file);
        QGuiApplication::setPalette(KColorScheme::createApplicationPalette(KSharedConfig::openConfig(file, KConfig::SimpleConfig)));
    }
}
