#include "sessionwiring.h"

#include <QCoreApplication>
#include <QEvent>

#include "audium_bridge/src/playback_session.cxxqt.h"
#include "playback/playbackcontroller.h"

SessionWiring::SessionWiring(PlaybackController *player, PlaybackSession *session, QObject *parent)
    : QObject(parent)
    , m_player(player)
    , m_session(session)
{
    connect(session, &PlaybackSession::loadRequested, this, &SessionWiring::loadPendingBook);
    connect(session, &PlaybackSession::pauseRequested, player, &PlaybackController::pause);
    connect(session, &PlaybackSession::fadeRequested, player, &PlaybackController::setVolumeFactor);
    // "Play from here" (a chapter or bookmark of the loaded book): seek, and
    // play if it was paused.
    connect(session, &PlaybackSession::seekRequested, player, [player](qint64 positionMs) {
        player->seekBook(positionMs);
        player->play();
    });

    // Anything loaded behind the session's back (e.g. "Open Files") is not a
    // library book: stop tracking.
    connect(player, &PlaybackController::tracksChanged, this, [this] {
        if (!m_loadingFromSession) {
            m_session->detach();
        }
    });
    connect(player, &PlaybackController::stateChanged, this, [this] {
        m_session->onState(int(m_player->state()));
    });
    connect(player, &PlaybackController::positionChanged, this, [this] {
        m_session->onPosition(m_player->trackIndex(), m_player->trackPosition(), m_player->position());
    });
    connect(player, &PlaybackController::rateChanged, this, [this] {
        m_session->onRate(m_player->rate());
    });
    connect(player, &PlaybackController::endOfBook, this, [this] {
        m_session->onEndOfBook();
    });

    if (QCoreApplication *app = QCoreApplication::instance()) {
        connect(app, &QCoreApplication::aboutToQuit, this, [this] {
            m_session->shutdown();
        });
        app->installEventFilter(this);
    }
}

void SessionWiring::loadPendingBook()
{
    QList<TrackSource> tracks;
    for (const SessionTrack &t : m_session->pendingTracks()) {
        tracks << TrackSource{QByteArray(reinterpret_cast<const char *>(t.path.data()), qsizetype(t.path.size())), t.title, t.duration_ms};
    }
    QList<ChapterInfo> chapters;
    for (const SessionChapter &c : m_session->pendingChapters()) {
        chapters << ChapterInfo{c.title, c.start_ms};
    }
    m_loadingFromSession = true;
    m_player->setVolumeFactor(1.0);
    m_player->load(tracks, chapters, m_session->pendingStartTrack(), m_session->pendingStartOffset());
    m_loadingFromSession = false;
    m_player->setRate(m_session->pendingRate());
    m_player->play();
}

bool SessionWiring::eventFilter(QObject *watched, QEvent *event)
{
    if (m_session->getSleepFading()) {
        switch (event->type()) {
        case QEvent::MouseMove:
        case QEvent::MouseButtonPress:
        case QEvent::KeyPress:
        case QEvent::Wheel:
        case QEvent::TouchBegin:
            m_session->userActivity();
            break;
        default:
            break;
        }
    }
    return QObject::eventFilter(watched, event);
}
