#pragma once

#include <QObject>

class PlaybackController;
class PlaybackSession;

// Connects the Rust PlaybackSession (policy: what to load, progress, sleep
// timer) to the PlaybackController (audio). Everything happens on the Qt
// thread. Also watches for user activity to extend a fading sleep timer:
// mouse movement, key presses, clicks and wheel events over Audium's windows
// (Wayland does not report the pointer anywhere else).
class SessionWiring : public QObject
{
    Q_OBJECT

public:
    SessionWiring(PlaybackController *player, PlaybackSession *session, QObject *parent = nullptr);

protected:
    bool eventFilter(QObject *watched, QEvent *event) override;

private:
    void loadPendingBook();

    PlaybackController *m_player;
    PlaybackSession *m_session;
    bool m_loadingFromSession = false;
};
