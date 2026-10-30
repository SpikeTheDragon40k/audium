#pragma once

#include <QObject>

class PlaybackSession;

// Desktop notifications: a book was finished, the sleep timer paused
// playback. Uses the desktop's standard notification event, so it works
// without an installed notifyrc file.
class Notifier : public QObject
{
    Q_OBJECT

public:
    explicit Notifier(PlaybackSession *session, QObject *parent = nullptr);

Q_SIGNALS:
    // Emitted for each notification sent (for tests).
    void notified(const QString &title, const QString &text);

private:
    void notify(const QString &title, const QString &text);
};
