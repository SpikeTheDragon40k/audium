#include "playbackcontroller.h"

#include <QCollator>
#include <QFile>
#include <QFileInfo>
#include <QVariantMap>

#include <algorithm>
#include <cstdlib>

namespace
{
// Position updates closer together than this are not re-emitted.
constexpr qint64 PositionEmitThresholdMs = 50;
}

PlaybackController::PlaybackController(QObject *parent)
    : QObject(parent)
{
}

PlaybackController::~PlaybackController() = default;

void PlaybackController::load(const QList<TrackSource> &tracks, const QList<ChapterInfo> &chapters, int startTrack, qint64 startOffsetMs)
{
    m_tracks = tracks;
    m_embeddedChapters = QList<QList<ChapterInfo>>(tracks.size());
    m_explicitChapters = chapters;
    m_trackPosition = 0;
    if (!m_errorString.isEmpty()) {
        m_errorString.clear();
        Q_EMIT errorStringChanged();
    }
    Q_EMIT tracksChanged();
    Q_EMIT durationChanged();

    if (tracks.isEmpty()) {
        setTrackIndex(-1);
        rebuildChapters();
        backendStop();
        reportState(State::Stopped);
        return;
    }

    startTrack = std::clamp(startTrack, 0, int(tracks.size()) - 1);
    startOffsetMs = std::max<qint64>(0, startOffsetMs);
    setTrackIndex(startTrack);
    m_trackPosition = startOffsetMs;
    rebuildChapters();
    Q_EMIT positionChanged();
    reportState(State::Loading);
    backendLoad(m_tracks, startTrack, startOffsetMs);
}

void PlaybackController::openFiles(const QList<QUrl> &urls)
{
    QList<QUrl> local;
    for (const QUrl &url : urls) {
        if (url.isLocalFile()) {
            local << url;
        }
    }
    QCollator collator;
    collator.setNumericMode(true);
    std::sort(local.begin(), local.end(), [&collator](const QUrl &a, const QUrl &b) {
        return collator.compare(a.toLocalFile(), b.toLocalFile()) < 0;
    });

    QList<TrackSource> tracks;
    for (const QUrl &url : std::as_const(local)) {
        const QString file = url.toLocalFile();
        tracks << TrackSource{QFile::encodeName(file), QFileInfo(file).completeBaseName(), 0};
    }
    load(tracks);
}

void PlaybackController::play()
{
    if (hasMedia() && m_state != State::Error) {
        backendPlay();
    }
}

void PlaybackController::pause()
{
    if (hasMedia()) {
        backendPause();
    }
}

void PlaybackController::togglePlayPause()
{
    if (m_state == State::Playing) {
        pause();
    } else {
        play();
    }
}

void PlaybackController::stop()
{
    backendStop();
    m_tracks.clear();
    m_embeddedChapters.clear();
    m_explicitChapters.clear();
    m_trackPosition = 0;
    setTrackIndex(-1);
    rebuildChapters();
    Q_EMIT tracksChanged();
    Q_EMIT durationChanged();
    Q_EMIT positionChanged();
    reportState(State::Stopped);
}

void PlaybackController::seekBook(qint64 positionMs)
{
    if (!hasMedia()) {
        return;
    }
    positionMs = std::max<qint64>(0, positionMs);
    // A track of unknown length is open-ended: anything at or past its start
    // lands in it, because we can't know where the following tracks begin.
    int index = trackCount() - 1;
    for (int i = 0; i < trackCount(); ++i) {
        const qint64 length = m_tracks[i].durationMs;
        if (length <= 0 || positionMs < trackStart(i) + length) {
            index = i;
            break;
        }
    }
    // seekTrack() clamps the offset to the track's length when it's known.
    seekTrack(index, positionMs - trackStart(index));
}

void PlaybackController::seekTrack(int trackIndex, qint64 offsetMs)
{
    if (trackIndex < 0 || trackIndex >= trackCount() || m_state == State::Error) {
        return;
    }
    const qint64 length = m_tracks[trackIndex].durationMs;
    offsetMs = std::max<qint64>(0, offsetMs);
    if (length > 0) {
        offsetMs = std::min(offsetMs, length);
    }
    // Update immediately so the UI doesn't jump back while the backend seeks.
    setTrackIndex(trackIndex);
    m_trackPosition = offsetMs;
    Q_EMIT positionChanged();
    updateChapterIndex();
    backendSeek(trackIndex, offsetMs);
}

void PlaybackController::skip(qint64 deltaMs)
{
    seekBook(position() + deltaMs);
}

void PlaybackController::seekChapter(int chapterIndex)
{
    if (chapterIndex >= 0 && chapterIndex < m_chapters.size()) {
        seekBook(m_chapters[chapterIndex].startMs);
    }
}

void PlaybackController::nextChapter()
{
    if (m_chapterIndex + 1 < m_chapters.size()) {
        seekChapter(m_chapterIndex + 1);
    }
}

void PlaybackController::previousChapter()
{
    if (m_chapterIndex < 0) {
        return;
    }
    const qint64 intoChapter = position() - m_chapters[m_chapterIndex].startMs;
    if (intoChapter > PreviousChapterGraceMs || m_chapterIndex == 0) {
        seekChapter(m_chapterIndex);
    } else {
        seekChapter(m_chapterIndex - 1);
    }
}

void PlaybackController::setRate(double rate)
{
    rate = std::clamp(rate, MinRate, MaxRate);
    backendSetRate(rate);
    reportRate(rate);
}

void PlaybackController::setVolume(int volume)
{
    volume = std::clamp(volume, 0, MaxVolume);
    backendSetVolume(volume);
    reportVolume(volume);
}

qint64 PlaybackController::position() const
{
    return m_trackIndex < 0 ? 0 : trackStart(m_trackIndex) + m_trackPosition;
}

qint64 PlaybackController::duration() const
{
    qint64 total = 0;
    for (const TrackSource &t : m_tracks) {
        total += t.durationMs;
    }
    return total;
}

QString PlaybackController::trackTitle() const
{
    if (m_trackIndex < 0 || m_trackIndex >= trackCount()) {
        return {};
    }
    return m_tracks[m_trackIndex].title;
}

QVariantList PlaybackController::chaptersForQml() const
{
    QVariantList list;
    list.reserve(m_chapters.size());
    for (const ChapterInfo &c : m_chapters) {
        list << QVariantMap{{QStringLiteral("title"), c.title}, {QStringLiteral("startMs"), c.startMs}};
    }
    return list;
}

QString PlaybackController::chapterTitle() const
{
    return m_chapterIndex < 0 ? QString() : m_chapters[m_chapterIndex].title;
}

qint64 PlaybackController::trackStart(int trackIndex) const
{
    qint64 start = 0;
    for (int i = 0; i < trackIndex && i < trackCount(); ++i) {
        start += m_tracks[i].durationMs;
    }
    return start;
}

qint64 PlaybackController::trackDuration(int trackIndex) const
{
    return trackIndex >= 0 && trackIndex < trackCount() ? m_tracks[trackIndex].durationMs : 0;
}

void PlaybackController::reportState(State state)
{
    if (m_state != state) {
        m_state = state;
        Q_EMIT stateChanged();
    }
}

void PlaybackController::reportTrackPosition(int trackIndex, qint64 offsetMs)
{
    if (trackIndex < 0 || trackIndex >= trackCount()) {
        return;
    }
    const bool trackChanged = trackIndex != m_trackIndex;
    setTrackIndex(trackIndex);
    if (trackChanged || std::abs(offsetMs - m_trackPosition) >= PositionEmitThresholdMs) {
        m_trackPosition = std::max<qint64>(0, offsetMs);
        Q_EMIT positionChanged();
        updateChapterIndex();
    }
}

void PlaybackController::reportTrackDuration(int trackIndex, qint64 durationMs)
{
    if (trackIndex < 0 || trackIndex >= trackCount() || durationMs <= 0) {
        return;
    }
    // Only fill in or correct by more than rounding noise.
    if (std::abs(m_tracks[trackIndex].durationMs - durationMs) > 1) {
        m_tracks[trackIndex].durationMs = durationMs;
        Q_EMIT durationChanged();
        Q_EMIT positionChanged();
        rebuildChapters();
    }
}

void PlaybackController::reportTrackChapters(int trackIndex, const QList<ChapterInfo> &chapters)
{
    if (trackIndex < 0 || trackIndex >= trackCount()) {
        return;
    }
    m_embeddedChapters[trackIndex] = chapters;
    rebuildChapters();
}

void PlaybackController::reportRate(double rate)
{
    if (!qFuzzyCompare(m_rate, rate)) {
        m_rate = rate;
        Q_EMIT rateChanged();
    }
}

void PlaybackController::reportVolume(int volume)
{
    if (m_volume != volume) {
        m_volume = volume;
        Q_EMIT volumeChanged();
    }
}

void PlaybackController::reportError(const QString &message)
{
    if (m_errorString != message) {
        m_errorString = message;
        Q_EMIT errorStringChanged();
    }
}

void PlaybackController::reportTrackFailed(int trackIndex, const QString &message)
{
    reportError(message);
    Q_EMIT trackFailed(trackIndex, message);
}

void PlaybackController::reportEndOfBook()
{
    Q_EMIT endOfBook();
}

void PlaybackController::rebuildChapters()
{
    QList<ChapterInfo> chapters;
    if (!m_explicitChapters.isEmpty()) {
        chapters = m_explicitChapters;
    } else {
        for (int i = 0; i < trackCount(); ++i) {
            const qint64 start = trackStart(i);
            if (m_embeddedChapters.value(i).isEmpty()) {
                chapters << ChapterInfo{m_tracks[i].title, start};
            } else {
                for (const ChapterInfo &c : m_embeddedChapters[i]) {
                    chapters << ChapterInfo{c.title, start + c.startMs};
                }
            }
        }
    }
    std::stable_sort(chapters.begin(), chapters.end(), [](const ChapterInfo &a, const ChapterInfo &b) {
        return a.startMs < b.startMs;
    });

    const bool changed = chapters.size() != m_chapters.size()
        || !std::equal(chapters.cbegin(), chapters.cend(), m_chapters.cbegin(), [](const ChapterInfo &a, const ChapterInfo &b) {
               return a.title == b.title && a.startMs == b.startMs;
           });
    if (changed) {
        m_chapters = chapters;
        Q_EMIT chaptersChanged();
    }
    updateChapterIndex(changed);
}

void PlaybackController::updateChapterIndex(bool chaptersRebuilt)
{
    int index = -1;
    const qint64 pos = position();
    for (int i = 0; i < m_chapters.size(); ++i) {
        if (m_chapters[i].startMs <= pos) {
            index = i;
        }
    }
    if (m_chapters.isEmpty() || m_trackIndex < 0) {
        index = -1;
    }
    // After a rebuild the title can change while the index stays the same.
    if (index != m_chapterIndex || chaptersRebuilt) {
        m_chapterIndex = index;
        Q_EMIT chapterIndexChanged();
    }
}

void PlaybackController::setTrackIndex(int index)
{
    if (m_trackIndex != index) {
        m_trackIndex = index;
        Q_EMIT trackIndexChanged();
    }
}
