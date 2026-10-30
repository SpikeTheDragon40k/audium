#pragma once

#include <QByteArray>
#include <QList>
#include <QObject>
#include <QString>
#include <QUrl>
#include <QVariantList>
#include <QtQmlIntegration/qqmlintegration.h>

// One audio file of a book. The path is in the native 8-bit encoding and is
// never round-tripped through QString, so non-UTF-8 paths survive.
struct TrackSource {
    QByteArray path;
    QString title;         // display title, may be empty
    qint64 durationMs = 0; // 0 = unknown; learned once the track is loaded
};

// A chapter with a book-wide start position.
struct ChapterInfo {
    QString title;
    qint64 startMs = 0;
};

// Backend-independent playback interface. It owns the book-level logic
// (track offsets, chapters, skipping); a backend only plays individual tracks
// and reports what happens through the protected report*() functions.
// All positions are in milliseconds. Everything runs on the Qt thread.
class PlaybackController : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by the application")

    Q_PROPERTY(State state READ state NOTIFY stateChanged)
    Q_PROPERTY(bool hasMedia READ hasMedia NOTIFY tracksChanged)
    Q_PROPERTY(qint64 position READ position NOTIFY positionChanged)
    Q_PROPERTY(qint64 duration READ duration NOTIFY durationChanged)
    Q_PROPERTY(int trackIndex READ trackIndex NOTIFY trackIndexChanged)
    Q_PROPERTY(int trackCount READ trackCount NOTIFY tracksChanged)
    Q_PROPERTY(qint64 trackPosition READ trackPosition NOTIFY positionChanged)
    Q_PROPERTY(QString trackTitle READ trackTitle NOTIFY trackIndexChanged)
    Q_PROPERTY(QVariantList chapters READ chaptersForQml NOTIFY chaptersChanged)
    Q_PROPERTY(int chapterIndex READ chapterIndex NOTIFY chapterIndexChanged)
    Q_PROPERTY(QString chapterTitle READ chapterTitle NOTIFY chapterIndexChanged)
    Q_PROPERTY(double rate READ rate WRITE setRate NOTIFY rateChanged)
    Q_PROPERTY(int volume READ volume WRITE setVolume NOTIFY volumeChanged)
    Q_PROPERTY(QString errorString READ errorString NOTIFY errorStringChanged)
    Q_PROPERTY(double minRate READ minRate CONSTANT)
    Q_PROPERTY(double maxRate READ maxRate CONSTANT)
    Q_PROPERTY(int maxVolume READ maxVolume CONSTANT)

public:
    enum class State {
        Stopped, // nothing loaded
        Loading, // load() called, first track not ready yet
        Paused,
        Playing,
        Error,   // nothing playable; see errorString
    };
    Q_ENUM(State)

    static constexpr double MinRate = 0.5;
    static constexpr double MaxRate = 3.0;
    static constexpr int MaxVolume = 100;
    // previousChapter() restarts the current chapter when further in than this.
    static constexpr qint64 PreviousChapterGraceMs = 3000;

    explicit PlaybackController(QObject *parent = nullptr);
    ~PlaybackController() override;

    // Replaces the current book. If `chapters` is empty, chapters come from
    // the tracks' embedded chapters, or one chapter per track.
    void load(const QList<TrackSource> &tracks,
              const QList<ChapterInfo> &chapters = {},
              int startTrack = 0,
              qint64 startOffsetMs = 0);

    // Development helper until the library exists (Milestone 2): loads local
    // files as one book, in natural filename order.
    Q_INVOKABLE void openFiles(const QList<QUrl> &urls);

    Q_INVOKABLE void play();
    Q_INVOKABLE void pause();
    Q_INVOKABLE void togglePlayPause();
    Q_INVOKABLE void stop();
    Q_INVOKABLE void seekBook(qint64 positionMs);
    Q_INVOKABLE void seekTrack(int trackIndex, qint64 offsetMs);
    Q_INVOKABLE void skip(qint64 deltaMs);
    Q_INVOKABLE void seekChapter(int chapterIndex);
    Q_INVOKABLE void nextChapter();
    Q_INVOKABLE void previousChapter();

    void setRate(double rate);
    void setVolume(int volume);
    // Output multiplier on top of the user's volume (sleep-timer fade).
    // Does not change `volume`.
    void setVolumeFactor(double factor);

    State state() const { return m_state; }
    bool hasMedia() const { return !m_tracks.isEmpty(); }
    qint64 position() const;
    qint64 duration() const;
    int trackIndex() const { return m_trackIndex; }
    int trackCount() const { return int(m_tracks.size()); }
    qint64 trackPosition() const { return m_trackPosition; }
    QString trackTitle() const;
    QList<ChapterInfo> chapters() const { return m_chapters; }
    QVariantList chaptersForQml() const;
    int chapterIndex() const { return m_chapterIndex; }
    QString chapterTitle() const;
    double rate() const { return m_rate; }
    int volume() const { return m_volume; }
    QString errorString() const { return m_errorString; }
    static double minRate() { return MinRate; }
    static double maxRate() { return MaxRate; }
    static int maxVolume() { return MaxVolume; }

    // Book-wide start of a track (sum of the known durations before it).
    qint64 trackStart(int trackIndex) const;
    // 0 when not known yet.
    qint64 trackDuration(int trackIndex) const;

Q_SIGNALS:
    void stateChanged();
    void tracksChanged();
    void positionChanged();
    void durationChanged();
    void trackIndexChanged();
    void chaptersChanged();
    void chapterIndexChanged();
    void rateChanged();
    void volumeChanged();
    void errorStringChanged();
    // The last track played to its end.
    void endOfBook();
    // A track could not be played; playback continues with the next one if possible.
    void trackFailed(int trackIndex, const QString &message);

protected:
    // Backend primitives. Track indexes are always valid when these are called.
    virtual void backendLoad(const QList<TrackSource> &tracks, int startTrack, qint64 startOffsetMs) = 0;
    virtual void backendPlay() = 0;
    virtual void backendPause() = 0;
    virtual void backendStop() = 0;
    virtual void backendSeek(int trackIndex, qint64 offsetMs) = 0;
    virtual void backendSetRate(double rate) = 0;
    // Effective output volume, 0..MaxVolume (user volume x fade factor).
    virtual void backendSetVolume(double volume) = 0;

    // Called by the backend.
    void reportState(State state);
    void reportTrackPosition(int trackIndex, qint64 offsetMs);
    void reportTrackDuration(int trackIndex, qint64 durationMs);
    // Embedded chapters of one track, with track-relative start positions.
    void reportTrackChapters(int trackIndex, const QList<ChapterInfo> &chapters);
    void reportRate(double rate);
    void reportVolume(int volume);
    void reportError(const QString &message);
    void reportTrackFailed(int trackIndex, const QString &message);
    void reportEndOfBook();

private:
    void rebuildChapters();
    void updateChapterIndex(bool chaptersRebuilt = false);
    void setTrackIndex(int index);

    QList<TrackSource> m_tracks;
    QList<QList<ChapterInfo>> m_embeddedChapters; // per track, track-relative
    QList<ChapterInfo> m_explicitChapters;
    QList<ChapterInfo> m_chapters; // effective, book-wide
    State m_state = State::Stopped;
    int m_trackIndex = -1;
    qint64 m_trackPosition = 0;
    int m_chapterIndex = -1;
    double m_rate = 1.0;
    int m_volume = MaxVolume;
    double m_volumeFactor = 1.0;
    QString m_errorString;
};

Q_DECLARE_METATYPE(TrackSource)
Q_DECLARE_METATYPE(ChapterInfo)
