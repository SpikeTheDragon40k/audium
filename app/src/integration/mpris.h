#pragma once

#include <QDBusAbstractAdaptor>
#include <QDBusObjectPath>
#include <QObject>
#include <QPointer>
#include <QStringList>
#include <QVariantMap>
#include <QWindow>

class PlaybackController;
class PlaybackSession;

// MPRIS2 (org.mpris.MediaPlayer2) on the session bus, so Plasma's media
// controls, media keys and lock screen control Audium.
//
// The MPRIS "track" is the current chapter: title = chapter, album = book,
// artist = authors; position, length, Seek and SetPosition are relative to
// the chapter; Next/Previous go to the next/previous chapter.
//
// Optional at runtime: if there is no session bus, nothing happens.
class Mpris : public QObject
{
    Q_OBJECT

public:
    Mpris(PlaybackController *player, PlaybackSession *session, QObject *parent = nullptr);
    ~Mpris() override;

    // The window that Raise() shows.
    void setWindow(QWindow *window);

    bool isRegistered() const { return !m_serviceName.isEmpty(); }
    QString serviceName() const { return m_serviceName; }

    // Used by the adaptors.
    PlaybackController *player() const { return m_player; }
    PlaybackSession *session() const { return m_session; }
    QWindow *window() const { return m_window; }
    QString playbackStatus() const;
    QVariantMap metadata() const;
    qint64 positionUs() const;
    bool canGoNext() const;
    bool canGoPrevious() const;
    // Book-wide start and end of the current chapter (or the whole book).
    qint64 chapterStartMs() const;
    qint64 chapterEndMs() const;
    QDBusObjectPath trackId() const;

Q_SIGNALS:
    void seeked(qint64 positionUs);

private:
    void propertiesChanged(const QString &interface, const QVariantMap &changed);
    void onStateChanged();
    void onMetadataChanged();

    PlaybackController *m_player;
    PlaybackSession *m_session;
    QPointer<QWindow> m_window;
    QString m_serviceName;
    QVariantMap m_lastMetadata;
    QString m_lastStatus;
};

class MprisRootAdaptor : public QDBusAbstractAdaptor
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.mpris.MediaPlayer2")
    Q_PROPERTY(bool CanQuit READ canQuit)
    Q_PROPERTY(bool CanRaise READ canRaise)
    Q_PROPERTY(bool HasTrackList READ hasTrackList)
    Q_PROPERTY(QString Identity READ identity)
    Q_PROPERTY(QString DesktopEntry READ desktopEntry)
    Q_PROPERTY(QStringList SupportedUriSchemes READ supportedUriSchemes)
    Q_PROPERTY(QStringList SupportedMimeTypes READ supportedMimeTypes)

public:
    explicit MprisRootAdaptor(Mpris *mpris);

    bool canQuit() const { return true; }
    bool canRaise() const;
    bool hasTrackList() const { return false; }
    QString identity() const;
    QString desktopEntry() const;
    QStringList supportedUriSchemes() const { return {}; }
    QStringList supportedMimeTypes() const { return {}; }

public Q_SLOTS:
    void Raise();
    void Quit();

private:
    Mpris *m_mpris;
};

class MprisPlayerAdaptor : public QDBusAbstractAdaptor
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.mpris.MediaPlayer2.Player")
    Q_PROPERTY(QString PlaybackStatus READ playbackStatus)
    Q_PROPERTY(double Rate READ rate WRITE setRate)
    Q_PROPERTY(QVariantMap Metadata READ metadata)
    Q_PROPERTY(double Volume READ volume WRITE setVolume)
    Q_PROPERTY(qlonglong Position READ position)
    Q_PROPERTY(double MinimumRate READ minimumRate)
    Q_PROPERTY(double MaximumRate READ maximumRate)
    Q_PROPERTY(bool CanGoNext READ canGoNext)
    Q_PROPERTY(bool CanGoPrevious READ canGoPrevious)
    Q_PROPERTY(bool CanPlay READ canPlay)
    Q_PROPERTY(bool CanPause READ canPause)
    Q_PROPERTY(bool CanSeek READ canSeek)
    Q_PROPERTY(bool CanControl READ canControl)

public:
    explicit MprisPlayerAdaptor(Mpris *mpris);

    QString playbackStatus() const { return m_mpris->playbackStatus(); }
    double rate() const;
    void setRate(double rate);
    QVariantMap metadata() const { return m_mpris->metadata(); }
    double volume() const;
    void setVolume(double volume);
    qlonglong position() const { return m_mpris->positionUs(); }
    double minimumRate() const;
    double maximumRate() const;
    bool canGoNext() const { return m_mpris->canGoNext(); }
    bool canGoPrevious() const { return m_mpris->canGoPrevious(); }
    bool canPlay() const;
    bool canPause() const;
    bool canSeek() const;
    bool canControl() const { return true; }

public Q_SLOTS:
    void Next();
    void Previous();
    void Pause();
    void PlayPause();
    void Stop();
    void Play();
    void Seek(qlonglong offsetUs);
    void SetPosition(const QDBusObjectPath &trackId, qlonglong positionUs);
    void OpenUri(const QString &uri);

Q_SIGNALS:
    void Seeked(qlonglong positionUs);

private:
    Mpris *m_mpris;
};
