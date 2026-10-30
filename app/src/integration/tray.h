#pragma once

#include <QIcon>
#include <QObject>
#include <QPointer>

class KStatusNotifierItem;
class PlaybackController;
class PlaybackSession;
class QAction;
class QMenu;
class QWindow;

// System tray icon: shows the current book, play/pause, and quit (added by
// KStatusNotifierItem). A left click shows or hides the window, a middle
// click plays or pauses.
class TrayIcon : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool available READ isAvailable CONSTANT)

public:
    TrayIcon(PlaybackController *player, PlaybackSession *session, QObject *parent = nullptr);
    ~TrayIcon() override;

    void setWindow(QWindow *window);

    // Audium's icon: from the icon theme when installed, otherwise the copy
    // built into the program. Also the window icon.
    static constexpr const char *AppIconName = "org.audium.Audium";
    static QIcon appIcon();

    // Whether the desktop shows tray icons (otherwise closing quits).
    bool isAvailable() const;

    // For tests.
    QMenu *menu() const { return m_menu; }
    QAction *playPauseAction() const { return m_playPause; }
    QString toolTipSubTitle() const;
    void secondaryActivate();

private:
    void update();

    PlaybackController *m_player;
    PlaybackSession *m_session;
    KStatusNotifierItem *m_item;
    QMenu *m_menu;
    QAction *m_playPause;
    QAction *m_nowPlaying;
};
