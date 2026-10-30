#include "mpris.h"

#include <QCoreApplication>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QLoggingCategory>
#include <QWindow>

#include <KLocalizedString>

#include <algorithm>
#include <cmath>

#include "audium_bridge/src/playback_session.cxxqt.h"
#include "playback/playbackcontroller.h"

Q_LOGGING_CATEGORY(lcMpris, "audium.integration.mpris")

namespace
{
const QString ObjectPath = QStringLiteral("/org/mpris/MediaPlayer2");
const QString ServiceBase = QStringLiteral("org.mpris.MediaPlayer2.audium");
const QString RootInterface = QStringLiteral("org.mpris.MediaPlayer2");
const QString PlayerInterface = QStringLiteral("org.mpris.MediaPlayer2.Player");
} // namespace

Mpris::Mpris(PlaybackController *player, PlaybackSession *session, QObject *parent)
    : QObject(parent)
    , m_player(player)
    , m_session(session)
{
    auto *playerAdaptor = new MprisPlayerAdaptor(this);
    new MprisRootAdaptor(this);
    connect(this, &Mpris::seeked, playerAdaptor, &MprisPlayerAdaptor::Seeked);

    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        qCInfo(lcMpris) << "no session bus; media controls are not available";
        return;
    }
    if (!bus.registerObject(ObjectPath, this)) {
        qCWarning(lcMpris) << "cannot register the MPRIS object:" << bus.lastError().message();
        return;
    }
    // A second Audium gets its own name, as the MPRIS spec suggests.
    const QStringList names{ServiceBase, ServiceBase + QStringLiteral(".instance") + QString::number(QCoreApplication::applicationPid())};
    for (const QString &name : names) {
        if (bus.registerService(name)) {
            m_serviceName = name;
            break;
        }
    }
    if (m_serviceName.isEmpty()) {
        qCWarning(lcMpris) << "cannot register an MPRIS service name:" << bus.lastError().message();
        bus.unregisterObject(ObjectPath);
        return;
    }

    connect(player, &PlaybackController::stateChanged, this, &Mpris::onStateChanged);
    connect(player, &PlaybackController::tracksChanged, this, &Mpris::onStateChanged);
    // Position itself is never signalled (clients poll it); seeks are.
    connect(player, &PlaybackController::seeked, this, [this] {
        // The chapter may have changed with the seek: metadata first.
        onMetadataChanged();
        Q_EMIT seeked(positionUs());
    });
    connect(player, &PlaybackController::chapterIndexChanged, this, &Mpris::onMetadataChanged);
    connect(player, &PlaybackController::chaptersChanged, this, &Mpris::onMetadataChanged);
    connect(player, &PlaybackController::durationChanged, this, &Mpris::onMetadataChanged);
    connect(player, &PlaybackController::trackIndexChanged, this, &Mpris::onMetadataChanged);
    connect(session, &PlaybackSession::bookTitleChanged, this, &Mpris::onMetadataChanged);
    connect(session, &PlaybackSession::bookAuthorsChanged, this, &Mpris::onMetadataChanged);
    connect(session, &PlaybackSession::coverUrlChanged, this, &Mpris::onMetadataChanged);
    connect(player, &PlaybackController::rateChanged, this, [this] {
        propertiesChanged(PlayerInterface, {{QStringLiteral("Rate"), m_player->rate()}});
    });
    connect(player, &PlaybackController::volumeChanged, this, [this] {
        propertiesChanged(PlayerInterface, {{QStringLiteral("Volume"), m_player->volume() / double(PlaybackController::MaxVolume)}});
    });
    m_lastMetadata = metadata();
    m_lastStatus = playbackStatus();
    qCInfo(lcMpris) << "media controls registered as" << m_serviceName;
}

Mpris::~Mpris()
{
    if (!m_serviceName.isEmpty()) {
        QDBusConnection::sessionBus().unregisterService(m_serviceName);
        QDBusConnection::sessionBus().unregisterObject(ObjectPath);
    }
}

void Mpris::setWindow(QWindow *window)
{
    m_window = window;
    propertiesChanged(RootInterface, {{QStringLiteral("CanRaise"), window != nullptr}});
}

QString Mpris::playbackStatus() const
{
    if (!m_player->hasMedia()) {
        return QStringLiteral("Stopped");
    }
    switch (m_player->state()) {
    case PlaybackController::State::Playing:
        return QStringLiteral("Playing");
    case PlaybackController::State::Paused:
    case PlaybackController::State::Loading:
        return QStringLiteral("Paused");
    default:
        return QStringLiteral("Stopped");
    }
}

qint64 Mpris::chapterStartMs() const
{
    const int index = m_player->chapterIndex();
    const auto chapters = m_player->chapters();
    return index >= 0 && index < chapters.size() ? chapters[index].startMs : 0;
}

qint64 Mpris::chapterEndMs() const
{
    const int index = m_player->chapterIndex();
    const auto chapters = m_player->chapters();
    if (index >= 0 && index + 1 < chapters.size()) {
        return chapters[index + 1].startMs;
    }
    return m_player->duration();
}

QDBusObjectPath Mpris::trackId() const
{
    if (!m_player->hasMedia()) {
        return QDBusObjectPath(QStringLiteral("/org/mpris/MediaPlayer2/TrackList/NoTrack"));
    }
    // A new id per book and chapter, so clients notice chapter changes.
    return QDBusObjectPath(QStringLiteral("/org/audium/Audium/book%1/chapter%2")
                               .arg(qMax<qint64>(0, m_session->getBookId()))
                               .arg(qMax(0, m_player->chapterIndex())));
}

QVariantMap Mpris::metadata() const
{
    QVariantMap m;
    m.insert(QStringLiteral("mpris:trackid"), QVariant::fromValue(trackId()));
    if (!m_player->hasMedia()) {
        return m;
    }
    const QString book = m_session->getBookTitle();
    QString title = m_player->chapterTitle();
    if (title.isEmpty()) {
        title = m_player->trackTitle();
    }
    if (title.isEmpty()) {
        title = book;
    }
    m.insert(QStringLiteral("xesam:title"), title);
    if (!book.isEmpty()) {
        m.insert(QStringLiteral("xesam:album"), book);
    }
    const QString authors = m_session->getBookAuthors();
    if (!authors.isEmpty()) {
        m.insert(QStringLiteral("xesam:artist"), authors.split(QStringLiteral(", "), Qt::SkipEmptyParts));
    }
    const qint64 length = chapterEndMs() - chapterStartMs();
    if (length > 0) {
        m.insert(QStringLiteral("mpris:length"), length * 1000);
    }
    const QUrl cover = m_session->getCoverUrl();
    if (!cover.isEmpty()) {
        m.insert(QStringLiteral("mpris:artUrl"), cover.toString());
    }
    return m;
}

qint64 Mpris::positionUs() const
{
    return qMax<qint64>(0, m_player->position() - chapterStartMs()) * 1000;
}

bool Mpris::canGoNext() const
{
    return m_player->hasMedia() && m_player->chapterIndex() + 1 < m_player->chapters().size();
}

bool Mpris::canGoPrevious() const
{
    return m_player->hasMedia();
}

void Mpris::propertiesChanged(const QString &interface, const QVariantMap &changed)
{
    if (m_serviceName.isEmpty() || changed.isEmpty()) {
        return;
    }
    QDBusMessage signal = QDBusMessage::createSignal(ObjectPath, QStringLiteral("org.freedesktop.DBus.Properties"), QStringLiteral("PropertiesChanged"));
    signal << interface << changed << QStringList();
    QDBusConnection::sessionBus().send(signal);
}

void Mpris::onStateChanged()
{
    const QString status = playbackStatus();
    QVariantMap changed;
    if (status != m_lastStatus) {
        m_lastStatus = status;
        changed.insert(QStringLiteral("PlaybackStatus"), status);
    }
    const bool media = m_player->hasMedia();
    changed.insert(QStringLiteral("CanPlay"), media);
    changed.insert(QStringLiteral("CanPause"), media);
    changed.insert(QStringLiteral("CanSeek"), media);
    changed.insert(QStringLiteral("CanGoNext"), canGoNext());
    changed.insert(QStringLiteral("CanGoPrevious"), canGoPrevious());
    propertiesChanged(PlayerInterface, changed);
    onMetadataChanged();
}

void Mpris::onMetadataChanged()
{
    const QVariantMap m = metadata();
    if (m == m_lastMetadata) {
        return;
    }
    m_lastMetadata = m;
    propertiesChanged(PlayerInterface, {{QStringLiteral("Metadata"), m}, {QStringLiteral("CanGoNext"), canGoNext()}});
}

// --- org.mpris.MediaPlayer2 ------------------------------------------------

MprisRootAdaptor::MprisRootAdaptor(Mpris *mpris)
    : QDBusAbstractAdaptor(mpris)
    , m_mpris(mpris)
{
}

bool MprisRootAdaptor::canRaise() const
{
    return m_mpris->window() != nullptr;
}

QString MprisRootAdaptor::identity() const
{
    return i18nc("@title application name", "Audium");
}

QString MprisRootAdaptor::desktopEntry() const
{
    return QStringLiteral("org.audium.Audium");
}

void MprisRootAdaptor::Raise()
{
    if (QWindow *w = m_mpris->window()) {
        w->show();
        w->raise();
        w->requestActivate();
    }
}

void MprisRootAdaptor::Quit()
{
    QCoreApplication::quit();
}

// --- org.mpris.MediaPlayer2.Player ----------------------------------------

MprisPlayerAdaptor::MprisPlayerAdaptor(Mpris *mpris)
    : QDBusAbstractAdaptor(mpris)
    , m_mpris(mpris)
{
}

double MprisPlayerAdaptor::rate() const
{
    return m_mpris->player()->rate();
}

void MprisPlayerAdaptor::setRate(double rate)
{
    // The spec: a rate of 0 means pause.
    if (rate <= 0.0) {
        m_mpris->player()->pause();
    } else {
        m_mpris->player()->setRate(rate);
    }
}

double MprisPlayerAdaptor::volume() const
{
    return m_mpris->player()->volume() / double(PlaybackController::MaxVolume);
}

void MprisPlayerAdaptor::setVolume(double volume)
{
    m_mpris->player()->setVolume(int(std::lround(std::clamp(volume, 0.0, 1.0) * PlaybackController::MaxVolume)));
}

double MprisPlayerAdaptor::minimumRate() const
{
    return PlaybackController::MinRate;
}

double MprisPlayerAdaptor::maximumRate() const
{
    return PlaybackController::MaxRate;
}

bool MprisPlayerAdaptor::canPlay() const
{
    return m_mpris->player()->hasMedia();
}

bool MprisPlayerAdaptor::canPause() const
{
    return m_mpris->player()->hasMedia();
}

bool MprisPlayerAdaptor::canSeek() const
{
    return m_mpris->player()->hasMedia();
}

void MprisPlayerAdaptor::Next()
{
    m_mpris->player()->nextChapter();
}

void MprisPlayerAdaptor::Previous()
{
    m_mpris->player()->previousChapter();
}

void MprisPlayerAdaptor::Pause()
{
    m_mpris->player()->pause();
}

void MprisPlayerAdaptor::PlayPause()
{
    // A media key during the sleep timer's fade means "keep listening".
    if (m_mpris->session()->getSleepFading()) {
        m_mpris->session()->userActivity();
        return;
    }
    m_mpris->player()->togglePlayPause();
}

void MprisPlayerAdaptor::Stop()
{
    m_mpris->player()->pause();
}

void MprisPlayerAdaptor::Play()
{
    m_mpris->session()->userActivity();
    m_mpris->player()->play();
}

void MprisPlayerAdaptor::Seek(qlonglong offsetUs)
{
    m_mpris->player()->seekBook(m_mpris->player()->position() + offsetUs / 1000);
}

void MprisPlayerAdaptor::SetPosition(const QDBusObjectPath &trackId, qlonglong positionUs)
{
    // Ignored unless it is about the current track, as the spec requires.
    if (trackId != m_mpris->trackId() || positionUs < 0) {
        return;
    }
    const qint64 target = m_mpris->chapterStartMs() + positionUs / 1000;
    if (target > m_mpris->chapterEndMs()) {
        return;
    }
    m_mpris->player()->seekBook(target);
}

void MprisPlayerAdaptor::OpenUri(const QString &uri)
{
    Q_UNUSED(uri)
    // Not supported: Audium plays books from its library.
}
