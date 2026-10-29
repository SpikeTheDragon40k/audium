#pragma once

#include "playbackcontroller.h"

#include <QHash>
#include <QMap>

#include <atomic>

struct mpv_handle;
struct mpv_event;

// PlaybackController backed by libmpv, audio only.
//
// mpv calls the wakeup callback from its own threads; the callback only posts
// a queued call, and all events are drained and handled on the Qt thread.
class MpvPlaybackBackend final : public PlaybackController
{
    Q_OBJECT

public:
    // `extraOptions` are applied after the defaults (tests use ao=null).
    explicit MpvPlaybackBackend(const QMap<QByteArray, QByteArray> &extraOptions = {}, QObject *parent = nullptr);
    ~MpvPlaybackBackend() override;

    bool isAvailable() const { return m_mpv != nullptr; }

protected:
    void backendLoad(const QList<TrackSource> &tracks, int startTrack, qint64 startOffsetMs) override;
    void backendPlay() override;
    void backendPause() override;
    void backendStop() override;
    void backendSeek(int trackIndex, qint64 offsetMs) override;
    void backendSetRate(double rate) override;
    void backendSetVolume(int volume) override;

private:
    static void wakeup(void *context);
    void drainEvents();
    void handleEvent(const mpv_event &event);
    void handlePropertyChange(const mpv_event &event);
    void handleFileLoaded();
    void handleEndFile(const mpv_event &event);
    void updatePlayState();
    void applyPendingSeek();
    bool command(std::initializer_list<const char *> args);
    void setFlag(const char *name, bool value);
    void setDouble(const char *name, double value);

    mpv_handle *m_mpv = nullptr;
    std::atomic_bool m_wakeupPending = false;

    QHash<qint64, int> m_entryIndex; // mpv playlist entry id -> track index
    int m_playlistPos = -1;
    bool m_fileLoaded = false; // the file at m_playlistPos is loaded
    bool m_paused = true;
    bool m_wantPlaying = false;
    bool m_anyTrackLoaded = false;
    int m_pendingSeekTrack = -1;
    qint64 m_pendingSeekMs = 0;
};
