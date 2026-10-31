#include "mpvplaybackbackend.h"

#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QLoggingCategory>

#include <mpv/client.h>

#include <clocale>
#include <cmath>

Q_LOGGING_CATEGORY(lcMpv, "audium.playback.mpv")

namespace
{
constexpr qint64 EndOfTrackMarginMs = 500;
// See m_seekHoldMs.
constexpr qint64 SeekHoldToleranceMs = 50;
constexpr qint64 SeekHoldMaxMs = 1500;

enum ObservedProperty : quint64 {
    PropTimePos = 1,
    PropDuration,
    PropPause,
    PropPlaylistPos,
    PropEofReached,
    PropSpeed,
    PropChapterList,
};

QString mpvError(int code)
{
    return QString::fromUtf8(mpv_error_string(code));
}

QString nodeString(const mpv_node &node)
{
    return node.format == MPV_FORMAT_STRING ? QString::fromUtf8(node.u.string) : QString();
}

double nodeDouble(const mpv_node &node)
{
    switch (node.format) {
    case MPV_FORMAT_DOUBLE:
        return node.u.double_;
    case MPV_FORMAT_INT64:
        return double(node.u.int64);
    default:
        return 0.0;
    }
}

// Parses mpv's "chapter-list" (an array of {title, time}) into track-relative chapters.
QList<ChapterInfo> parseChapterList(const mpv_node &node)
{
    QList<ChapterInfo> chapters;
    if (node.format != MPV_FORMAT_NODE_ARRAY || !node.u.list) {
        return chapters;
    }
    for (int i = 0; i < node.u.list->num; ++i) {
        const mpv_node &entry = node.u.list->values[i];
        if (entry.format != MPV_FORMAT_NODE_MAP || !entry.u.list) {
            continue;
        }
        ChapterInfo chapter;
        for (int k = 0; k < entry.u.list->num; ++k) {
            const char *key = entry.u.list->keys[k];
            if (qstrcmp(key, "title") == 0) {
                chapter.title = nodeString(entry.u.list->values[k]);
            } else if (qstrcmp(key, "time") == 0) {
                const double seconds = nodeDouble(entry.u.list->values[k]);
                chapter.startMs = std::isfinite(seconds) && seconds > 0 ? qint64(std::llround(seconds * 1000.0)) : 0;
            }
        }
        chapters << chapter;
    }
    return chapters;
}

qint64 secondsToMs(double seconds)
{
    return std::isfinite(seconds) && seconds > 0 ? qint64(std::llround(seconds * 1000.0)) : 0;
}
} // namespace

MpvPlaybackBackend::MpvPlaybackBackend(const QMap<QByteArray, QByteArray> &extraOptions, QObject *parent)
    : PlaybackController(parent)
{
    // libmpv refuses to start unless LC_NUMERIC is "C". This is process-wide;
    // Qt formats numbers with QLocale, so nothing else depends on it.
    std::setlocale(LC_NUMERIC, "C");

    m_mpv = mpv_create();
    if (!m_mpv) {
        reportError(tr("The audio engine (libmpv) could not be started."));
        reportState(State::Error);
        return;
    }

    QMap<QByteArray, QByteArray> options{
        {"config", "no"},
        {"terminal", "no"},
        {"load-scripts", "no"},
        {"input-default-bindings", "no"},
        {"input-vo-keyboard", "no"},
        {"ytdl", "no"},
        {"resume-playback", "no"},
        {"save-position-on-quit", "no"},
        {"vo", "null"},
        {"vid", "no"},
        {"audio-display", "no"},
        {"af", "scaletempo2"},
        {"keep-open", "yes"},
        {"gapless-audio", "weak"},
        {"prefetch-playlist", "yes"},
        {"idle", "yes"},
        {"pause", "yes"},
        {"audio-client-name", "audium"},
    };
    for (auto it = extraOptions.cbegin(); it != extraOptions.cend(); ++it) {
        options.insert(it.key(), it.value());
    }
    for (auto it = options.cbegin(); it != options.cend(); ++it) {
        const int rc = mpv_set_option_string(m_mpv, it.key().constData(), it.value().constData());
        if (rc < 0) {
            qCWarning(lcMpv) << "option" << it.key() << "=" << it.value() << "rejected:" << mpvError(rc);
        }
    }

    mpv_request_log_messages(m_mpv, "warn");
    mpv_observe_property(m_mpv, PropTimePos, "time-pos", MPV_FORMAT_DOUBLE);
    mpv_observe_property(m_mpv, PropDuration, "duration", MPV_FORMAT_DOUBLE);
    mpv_observe_property(m_mpv, PropPause, "pause", MPV_FORMAT_FLAG);
    mpv_observe_property(m_mpv, PropPlaylistPos, "playlist-pos", MPV_FORMAT_INT64);
    mpv_observe_property(m_mpv, PropEofReached, "eof-reached", MPV_FORMAT_FLAG);
    mpv_observe_property(m_mpv, PropSpeed, "speed", MPV_FORMAT_DOUBLE);
    mpv_observe_property(m_mpv, PropChapterList, "chapter-list", MPV_FORMAT_NODE);
    mpv_set_wakeup_callback(m_mpv, &MpvPlaybackBackend::wakeup, this);

    const int rc = mpv_initialize(m_mpv);
    if (rc < 0) {
        mpv_terminate_destroy(m_mpv);
        m_mpv = nullptr;
        reportError(tr("The audio engine (libmpv) could not be started: %1").arg(mpvError(rc)));
        reportState(State::Error);
    }
}

MpvPlaybackBackend::~MpvPlaybackBackend()
{
    // Joins mpv's threads. A wakeup that fires meanwhile only posts a queued
    // call to this object, which Qt discards once the object is destroyed.
    if (m_mpv) {
        mpv_terminate_destroy(m_mpv);
    }
}

void MpvPlaybackBackend::wakeup(void *context)
{
    // Runs on an mpv thread: touch nothing but the atomic flag and the queue.
    auto *self = static_cast<MpvPlaybackBackend *>(context);
    if (!self->m_wakeupPending.exchange(true)) {
        QMetaObject::invokeMethod(self, &MpvPlaybackBackend::drainEvents, Qt::QueuedConnection);
    }
}

void MpvPlaybackBackend::drainEvents()
{
    m_wakeupPending = false;
    if (!m_mpv) {
        return;
    }
    while (true) {
        const mpv_event *event = mpv_wait_event(m_mpv, 0);
        if (!event || event->event_id == MPV_EVENT_NONE) {
            break;
        }
        handleEvent(*event);
    }
}

void MpvPlaybackBackend::handleEvent(const mpv_event &event)
{
    switch (event.event_id) {
    case MPV_EVENT_PROPERTY_CHANGE:
        handlePropertyChange(event);
        break;
    case MPV_EVENT_START_FILE:
        m_fileLoaded = false;
        break;
    case MPV_EVENT_FILE_LOADED:
        handleFileLoaded();
        break;
    case MPV_EVENT_END_FILE:
        handleEndFile(event);
        break;
    case MPV_EVENT_LOG_MESSAGE: {
        // mpv/ffmpeg "warn" messages are usually harmless file quirks.
        const auto *msg = static_cast<const mpv_event_log_message *>(event.data);
        const QString text = QString::fromUtf8(msg->text).trimmed();
        if (msg->log_level <= MPV_LOG_LEVEL_ERROR) {
            qCWarning(lcMpv).noquote() << msg->prefix << text;
        } else {
            qCInfo(lcMpv).noquote() << msg->prefix << text;
        }
        break;
    }
    case MPV_EVENT_SHUTDOWN:
        reportError(tr("The audio engine stopped unexpectedly."));
        reportState(State::Error);
        break;
    default:
        break;
    }
}

void MpvPlaybackBackend::handlePropertyChange(const mpv_event &event)
{
    const auto *prop = static_cast<const mpv_event_property *>(event.data);
    const bool hasData = prop->data != nullptr;

    switch (event.reply_userdata) {
    case PropTimePos:
        if (hasData && m_fileLoaded && m_pendingSeekTrack < 0) {
            const qint64 ms = secondsToMs(*static_cast<double *>(prop->data));
            if (m_seekHoldMs >= 0 && m_seekHoldTrack != m_playlistPos) {
                m_seekHoldMs = -1; // playback moved to another track
            }
            if (m_seekHoldMs >= 0) {
                // Hold the seek target until playback has really moved on
                // from it (or, as a safety net, after a while of playing).
                const bool movedOn = !m_paused && ms >= m_seekHoldMs - SeekHoldToleranceMs;
                const bool tooLong = !m_paused && QDateTime::currentMSecsSinceEpoch() - m_seekHoldSince > SeekHoldMaxMs;
                if (!movedOn && !tooLong) {
                    break;
                }
                m_seekHoldMs = -1;
            }
            reportTrackPosition(m_playlistPos, ms);
        }
        break;
    case PropDuration:
        if (hasData && m_fileLoaded) {
            reportTrackDuration(m_playlistPos, secondsToMs(*static_cast<double *>(prop->data)));
        }
        break;
    case PropPause:
        if (hasData) {
            m_paused = *static_cast<int *>(prop->data) != 0;
            updatePlayState();
        }
        break;
    case PropPlaylistPos:
        if (hasData) {
            m_playlistPos = int(*static_cast<int64_t *>(prop->data));
            // This notification can arrive after the new file was loaded and
            // a seek into it applied: then the seek's position stands.
            const bool seekedHere = m_seekHoldMs >= 0 && m_seekHoldTrack == m_playlistPos;
            if (m_playlistPos >= 0 && m_pendingSeekTrack < 0 && !seekedHere) {
                reportTrackPosition(m_playlistPos, 0);
            }
        }
        break;
    case PropEofReached:
        // With keep-open=yes this only happens at the end of the last track.
        // Not while a new playlist loads: then it describes the old file.
        if (hasData && *static_cast<int *>(prop->data) != 0 && m_fileLoaded && m_playlistPos == trackCount() - 1) {
            m_wantPlaying = false;
            m_atEndOfBook = true;
            reportTrackPosition(m_playlistPos, std::max<qint64>(0, duration() - trackStart(m_playlistPos)));
            reportEndOfBook();
        }
        break;
    case PropSpeed:
        if (hasData) {
            reportRate(*static_cast<double *>(prop->data));
        }
        break;
    case PropChapterList:
        if (hasData && m_fileLoaded) {
            reportTrackChapters(m_playlistPos, parseChapterList(*static_cast<mpv_node *>(prop->data)));
        }
        break;
    default:
        break;
    }
}

void MpvPlaybackBackend::handleFileLoaded()
{
    int64_t pos = -1;
    mpv_get_property(m_mpv, "playlist-pos", MPV_FORMAT_INT64, &pos);
    m_playlistPos = int(pos);
    if (m_awaitTrack >= 0 && m_playlistPos != m_awaitTrack) {
        return; // a stray entry; mpv is switching to the requested one
    }
    m_awaitTrack = -1;
    m_fileLoaded = true;
    m_anyTrackLoaded = true;

    double seconds = 0;
    if (mpv_get_property(m_mpv, "duration", MPV_FORMAT_DOUBLE, &seconds) >= 0) {
        reportTrackDuration(m_playlistPos, secondsToMs(seconds));
    }
    mpv_node chapters;
    if (mpv_get_property(m_mpv, "chapter-list", MPV_FORMAT_NODE, &chapters) >= 0) {
        reportTrackChapters(m_playlistPos, parseChapterList(chapters));
        mpv_free_node_contents(&chapters);
    }

    // A file loaded without a pending seek starts at 0; with one, the seek
    // reports its own target.
    const bool seeking = m_pendingSeekTrack == m_playlistPos;
    applyPendingSeek();
    if (!seeking && m_pendingSeekTrack < 0) {
        m_seekHoldMs = -1;
        reportTrackPosition(m_playlistPos, 0);
    }
    updatePlayState();
}

void MpvPlaybackBackend::handleEndFile(const mpv_event &event)
{
    const auto *end = static_cast<const mpv_event_end_file *>(event.data);
    // Ignore entries of a previous playlist (their ids are no longer mapped).
    const int index = m_entryIndex.value(end->playlist_entry_id, -1);
    if (index < 0) {
        return;
    }
    m_fileLoaded = false;
    if (end->reason != MPV_END_FILE_REASON_ERROR) {
        return;
    }
    const QString message = tr("Could not play track %1: %2").arg(index + 1).arg(mpvError(end->error));
    qCWarning(lcMpv).noquote() << message;
    reportTrackFailed(index, message);
    // A pending seek into a failed track can never be applied, and if the
    // start track fails, whatever mpv plays next is the start.
    if (index == m_pendingSeekTrack) {
        m_pendingSeekTrack = -1;
    }
    if (index == m_awaitTrack) {
        m_awaitTrack = -1;
    }
    // mpv moves on to the next entry by itself; after the last one it stops.
    if (index == trackCount() - 1) {
        m_wantPlaying = false;
        if (!m_anyTrackLoaded) {
            reportError(trackCount() == 1 ? message : tr("None of the files could be played."));
            reportState(State::Error);
        } else {
            setFlag("pause", true);
        }
    }
}

void MpvPlaybackBackend::updatePlayState()
{
    if (!hasMedia() || state() == State::Error || state() == State::Stopped) {
        return;
    }
    if (!m_anyTrackLoaded) {
        reportState(State::Loading);
        return;
    }
    reportState(m_paused ? State::Paused : State::Playing);
}

void MpvPlaybackBackend::applyPendingSeek()
{
    if (m_pendingSeekTrack < 0 || m_pendingSeekTrack != m_playlistPos || !m_fileLoaded) {
        return;
    }
    // The target may have been computed before this track's length was known.
    // Seeking past the end would make mpv skip to the next track, so stop
    // just short of it instead.
    qint64 offsetMs = m_pendingSeekMs;
    const qint64 length = trackDuration(m_playlistPos);
    if (length > 0 && offsetMs > length - EndOfTrackMarginMs) {
        offsetMs = std::max<qint64>(0, length - EndOfTrackMarginMs);
    }
    m_pendingSeekTrack = -1;
    seekInCurrentFile(offsetMs);
    reportTrackPosition(m_playlistPos, offsetMs);
    if (m_wantPlaying) {
        setFlag("pause", false);
    }
}

void MpvPlaybackBackend::backendLoad(const QList<TrackSource> &tracks, int startTrack, qint64 startOffsetMs)
{
    if (!m_mpv) {
        reportState(State::Error);
        return;
    }
    for (const TrackSource &track : tracks) {
        // Only absolute local paths: anything else could be taken for a URL,
        // a playlist, stdin ("-") or be resolved against the working directory.
        if (track.path.isEmpty() || track.path.contains('\0') || !QDir::isAbsolutePath(QFile::decodeName(track.path))) {
            reportError(tr("Invalid file path: %1").arg(QFile::decodeName(track.path)));
            reportState(State::Error);
            return;
        }
    }

    m_wantPlaying = false;
    m_atEndOfBook = false;
    m_anyTrackLoaded = false;
    m_fileLoaded = false;
    m_entryIndex.clear();
    setFlag("pause", true);
    command({"stop"});
    // Tell mpv where to start before the playlist exists: otherwise it may
    // begin with the first entry before `playlist-play-index` takes effect.
    const QByteArray start = QByteArray::number(startTrack);
    mpv_set_property_string(m_mpv, "playlist-start", start.constData());
    m_awaitTrack = startTrack;
    for (const TrackSource &track : tracks) {
        command({"loadfile", track.path.constData(), "append"});
    }
    for (int i = 0; i < tracks.size(); ++i) {
        const QByteArray name = "playlist/" + QByteArray::number(i) + "/id";
        int64_t id = -1;
        if (mpv_get_property(m_mpv, name.constData(), MPV_FORMAT_INT64, &id) >= 0) {
            m_entryIndex.insert(id, i);
        }
    }
    m_pendingSeekTrack = startOffsetMs > 0 ? startTrack : -1;
    m_pendingSeekMs = startOffsetMs;
    command({"playlist-play-index", start.constData()});
}

void MpvPlaybackBackend::backendPlay()
{
    m_wantPlaying = true;
    // Replaying a finished book starts from the beginning.
    if (m_atEndOfBook) {
        seekTrack(0, 0);
    }
    setFlag("pause", false);
}

void MpvPlaybackBackend::backendPause()
{
    m_wantPlaying = false;
    setFlag("pause", true);
}

void MpvPlaybackBackend::backendStop()
{
    m_wantPlaying = false;
    m_atEndOfBook = false;
    m_pendingSeekTrack = -1;
    m_awaitTrack = -1;
    m_seekHoldMs = -1;
    m_entryIndex.clear();
    m_anyTrackLoaded = false;
    m_fileLoaded = false;
    if (m_mpv) {
        setFlag("pause", true);
        command({"stop"});
    }
}

void MpvPlaybackBackend::backendSeek(int trackIndex, qint64 offsetMs)
{
    if (!m_mpv) {
        return;
    }
    m_atEndOfBook = false;
    if (trackIndex == m_playlistPos && m_fileLoaded) {
        seekInCurrentFile(offsetMs);
        return;
    }
    m_pendingSeekTrack = trackIndex;
    m_pendingSeekMs = offsetMs;
    const QByteArray index = QByteArray::number(trackIndex);
    command({"playlist-play-index", index.constData()});
}

void MpvPlaybackBackend::backendSetRate(double rate)
{
    setDouble("speed", rate);
}

void MpvPlaybackBackend::backendSetVolume(double volume)
{
    // mpv's volume is only ever set from here; it is not mirrored back, so a
    // fade never shows up as a change of the user's volume.
    setDouble("volume", volume);
}

void MpvPlaybackBackend::seekInCurrentFile(qint64 offsetMs)
{
    const QByteArray seconds = QByteArray::number(double(offsetMs) / 1000.0, 'f', 3);
    if (command({"seek", seconds.constData(), "absolute+exact"})) {
        m_seekHoldMs = offsetMs;
        m_seekHoldTrack = m_playlistPos;
        m_seekHoldSince = QDateTime::currentMSecsSinceEpoch();
    }
}

bool MpvPlaybackBackend::command(std::initializer_list<const char *> args)
{
    if (!m_mpv) {
        return false;
    }
    QList<const char *> argv(args.begin(), args.end());
    argv << nullptr;
    const int rc = mpv_command(m_mpv, argv.data());
    if (rc < 0) {
        qCWarning(lcMpv) << "command" << *args.begin() << "failed:" << mpvError(rc);
    }
    return rc >= 0;
}

void MpvPlaybackBackend::setFlag(const char *name, bool value)
{
    if (m_mpv) {
        int flag = value ? 1 : 0;
        mpv_set_property(m_mpv, name, MPV_FORMAT_FLAG, &flag);
    }
}

void MpvPlaybackBackend::setDouble(const char *name, double value)
{
    if (m_mpv) {
        mpv_set_property(m_mpv, name, MPV_FORMAT_DOUBLE, &value);
    }
}
