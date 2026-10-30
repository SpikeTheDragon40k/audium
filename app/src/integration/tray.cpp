#include "tray.h"

#include <QAction>
#include <QIcon>
#include <QMenu>
#include <QSystemTrayIcon>
#include <QWindow>

#include <KLocalizedString>
#include <KStatusNotifierItem>

#include "audium_bridge/src/playback_session.cxxqt.h"
#include "playback/playbackcontroller.h"

TrayIcon::TrayIcon(PlaybackController *player, PlaybackSession *session, QObject *parent)
    : QObject(parent)
    , m_player(player)
    , m_session(session)
    , m_item(new KStatusNotifierItem(QStringLiteral("org.audium.Audium"), this))
    , m_menu(new QMenu)
{
    m_item->setCategory(KStatusNotifierItem::ApplicationStatus);
    m_item->setStatus(KStatusNotifierItem::Active);
    // By name when the icon theme has it (installed), so the tray host can
    // pick its size; otherwise the built-in image.
    if (QIcon::hasThemeIcon(QString::fromLatin1(AppIconName))) {
        m_item->setIconByName(QString::fromLatin1(AppIconName));
        m_item->setToolTipIconByName(QString::fromLatin1(AppIconName));
    } else {
        m_item->setIconByPixmap(appIcon());
        m_item->setToolTipIconByPixmap(appIcon());
    }
    m_item->setTitle(i18nc("@title application name", "Audium"));
    m_item->setToolTipTitle(i18nc("@title application name", "Audium"));

    m_nowPlaying = m_menu->addAction(QString());
    m_nowPlaying->setEnabled(false);
    m_playPause = m_menu->addAction(QString());
    connect(m_playPause, &QAction::triggered, player, &PlaybackController::togglePlayPause);
    m_menu->addSeparator();
    // KStatusNotifierItem appends "Show/Hide" (with a window) and "Quit".
    m_item->setContextMenu(m_menu);
    connect(m_item, &KStatusNotifierItem::secondaryActivateRequested, this, &TrayIcon::secondaryActivate);

    connect(player, &PlaybackController::stateChanged, this, &TrayIcon::update);
    connect(player, &PlaybackController::tracksChanged, this, &TrayIcon::update);
    connect(player, &PlaybackController::chapterIndexChanged, this, &TrayIcon::update);
    connect(player, &PlaybackController::trackIndexChanged, this, &TrayIcon::update);
    connect(session, &PlaybackSession::bookTitleChanged, this, &TrayIcon::update);
    update();
}

QIcon TrayIcon::appIcon()
{
    return QIcon::fromTheme(QString::fromLatin1(AppIconName), QIcon(QStringLiteral(":/icons/org.audium.Audium.svg")));
}

// The menu belongs to the KStatusNotifierItem, which deletes it.
TrayIcon::~TrayIcon() = default;

void TrayIcon::setWindow(QWindow *window)
{
    m_item->setAssociatedWindow(window);
}

bool TrayIcon::isAvailable() const
{
    return QSystemTrayIcon::isSystemTrayAvailable();
}

QString TrayIcon::toolTipSubTitle() const
{
    return m_item->toolTipSubTitle();
}

void TrayIcon::secondaryActivate()
{
    m_player->togglePlayPause();
}

void TrayIcon::update()
{
    const bool playing = m_player->state() == PlaybackController::State::Playing;
    m_playPause->setText(playing ? i18nc("@action:inmenu", "Pause") : i18nc("@action:inmenu", "Play"));
    m_playPause->setIcon(QIcon::fromTheme(playing ? QStringLiteral("media-playback-pause") : QStringLiteral("media-playback-start")));
    m_playPause->setEnabled(m_player->hasMedia());

    QString book = m_session->getBookTitle();
    if (book.isEmpty()) {
        book = m_player->trackTitle();
    }
    QString part = m_player->chapterTitle();
    if (part == book) {
        part.clear();
    }
    QString now;
    if (!m_player->hasMedia()) {
        now = i18nc("@info tray tooltip", "Nothing playing");
    } else if (part.isEmpty()) {
        now = book;
    } else {
        now = i18nc("@info book title — chapter", "%1 — %2", book, part);
    }
    m_nowPlaying->setText(now);
    m_item->setToolTipSubTitle(m_player->hasMedia() && !playing ? i18nc("@info tray tooltip", "%1 (paused)", now) : now);
}
