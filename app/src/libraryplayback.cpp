#include "libraryplayback.h"

#include "playback/playbackcontroller.h"

LibraryPlayback::LibraryPlayback(PlaybackController *player, QObject *parent)
    : QObject(parent)
    , m_player(player)
{
}

bool LibraryPlayback::play(BookDetails *book)
{
    if (!book || !m_player) {
        return false;
    }
    QList<TrackSource> tracks;
    for (const PlaybackTrack &t : book->playbackTracks()) {
        tracks << TrackSource{QByteArray(reinterpret_cast<const char *>(t.path.data()), qsizetype(t.path.size())), t.title, t.duration_ms};
    }
    if (tracks.isEmpty()) {
        return false;
    }
    QList<ChapterInfo> chapters;
    for (const PlaybackChapter &c : book->playbackChapters()) {
        chapters << ChapterInfo{c.title, c.start_ms};
    }
    m_player->load(tracks, chapters);
    m_player->play();
    return true;
}
