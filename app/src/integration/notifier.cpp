#include "notifier.h"

#include <KLocalizedString>
#include <KNotification>

#include "audium_bridge/src/playback_session.cxxqt.h"

Notifier::Notifier(PlaybackSession *session, QObject *parent)
    : QObject(parent)
{
    connect(session, &PlaybackSession::bookFinished, this, [this](const QString &title) {
        notify(i18nc("@title notification", "Book finished"), i18nc("@info notification", "You have finished “%1”.", title));
    });
    connect(session, &PlaybackSession::sleepTimerExpired, this, [this] {
        notify(i18nc("@title notification", "Sleep timer"), i18nc("@info notification", "Playback was paused. Good night!"));
    });
}

void Notifier::notify(const QString &title, const QString &text)
{
    KNotification::event(KNotification::Notification, title, text);
    Q_EMIT notified(title, text);
}
