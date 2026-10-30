// Desktop integrations with the real playback stack: MPRIS (driven from a
// second D-Bus connection, like Plasma's media controls), the tray icon,
// notifications and closing to the tray. Runs on a private session bus
// (ctest starts it under dbus-run-session), never the user's.

#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusMessage>
#include <QDBusPendingCall>
#include <QDBusPendingReply>
#include <QDBusVariant>
#include <QMenu>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QtQml/QQmlExtensionPlugin>
#include <QtTest>

#include <KLocalizedQmlContext>

#include "audium_bridge/src/book_list_model.cxxqt.h"
#include "audium_bridge/src/library_controller.cxxqt.h"
#include "audium_bridge/src/playback_session.cxxqt.h"
#include "integration/mpris.h"
#include "integration/notifier.h"
#include "integration/tray.h"
#include "playback/mpvplaybackbackend.h"
#include "sessionwiring.h"

Q_IMPORT_QML_PLUGIN(org_audium_appPlugin)

using State = PlaybackController::State;

namespace
{
const QString Path = QStringLiteral("/org/mpris/MediaPlayer2");
const QString Player = QStringLiteral("org.mpris.MediaPlayer2.Player");
const QString Root = QStringLiteral("org.mpris.MediaPlayer2");
const QString Props = QStringLiteral("org.freedesktop.DBus.Properties");

bool near(qint64 actual, qint64 expected, qint64 tolerance = 300)
{
    return std::abs(actual - expected) <= tolerance;
}
} // namespace

// Stands in for TrayIcon in QML (an offscreen test has no tray to show).
class FakeTray : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool available MEMBER available CONSTANT)
public:
    bool available = true;
};

// Collects PropertiesChanged and Seeked signals seen on the bus.
class Listener : public QObject
{
    Q_OBJECT
public:
    QList<QVariantMap> changes;
    QList<qlonglong> seeks;

public Q_SLOTS:
    void propertiesChanged(const QString &interface, const QVariantMap &changed, const QStringList &)
    {
        if (interface == Player) {
            changes << changed;
        }
    }
    void seeked(qlonglong position)
    {
        seeks << position;
    }
};

class TestIntegration : public QObject
{
    Q_OBJECT

    QTemporaryDir m_dir;
    std::unique_ptr<LibraryController> m_library;
    qint64 m_m4b = 0; // the chaptered m4b: Opening 0-2 s, Middle 2-4 s, Ending 4-6 s
    QDBusConnection m_client{QStringLiteral("")};

    // An async call from the client connection, waited for with the event
    // loop running (the service lives in this same thread).
    QDBusMessage call(const QString &service, const QString &interface, const QString &method, const QVariantList &args = {})
    {
        QDBusMessage msg = QDBusMessage::createMethodCall(service, Path, interface, method);
        msg.setArguments(args);
        QDBusPendingCall pending = m_client.asyncCall(msg, 5000);
        if (!QTest::qWaitFor([&] { return pending.isFinished(); }, 6000)) {
            return {};
        }
        return pending.reply();
    }

    QVariant get(const QString &service, const QString &interface, const QString &name)
    {
        const QDBusMessage reply = call(service, Props, QStringLiteral("Get"), {interface, name});
        if (reply.type() != QDBusMessage::ReplyMessage || reply.arguments().isEmpty()) {
            return {};
        }
        return reply.arguments().constFirst().value<QDBusVariant>().variant();
    }

    QVariantMap metadata(const QString &service)
    {
        QVariant v = get(service, Player, QStringLiteral("Metadata"));
        // Over the bus a{sv} arrives as a QDBusArgument.
        return qdbus_cast<QVariantMap>(v.value<QDBusArgument>());
    }

    void set(const QString &service, const QString &name, const QVariant &value)
    {
        call(service, Props, QStringLiteral("Set"), {Player, name, QVariant::fromValue(QDBusVariant(value))});
    }

private Q_SLOTS:
    void initTestCase()
    {
        QVERIFY2(QDBusConnection::sessionBus().isConnected(), "no session bus: run under dbus-run-session (ctest does)");
        m_client = QDBusConnection::connectToBus(QDBusConnection::SessionBus, QStringLiteral("test-client"));
        QVERIFY(m_client.isConnected());
        QVERIFY(m_dir.isValid());
        m_library = std::make_unique<LibraryController>();
        QVERIFY(m_library->open(m_dir.filePath(QStringLiteral("data")), m_dir.filePath(QStringLiteral("cache"))));
        QCOMPARE(m_library->addFolder(QUrl::fromLocalFile(QStringLiteral(AUDIUM_FIXTURES_DIR))), QString());
        QTRY_VERIFY_WITH_TIMEOUT(!m_library->getScanning(), 30000);
        BookListModel books;
        books.setDatabasePath(m_library->getDatabasePath());
        const auto roles = books.roleNames();
        for (int row = 0; row < books.rowCount(QModelIndex()); ++row) {
            const QModelIndex i = books.index(row, 0);
            if (books.data(i, roles.key("title")).toString() == QStringLiteral("Fixture Book With Chapters")
                && !books.data(i, roles.key("coverUrl")).toUrl().isEmpty()) {
                m_m4b = books.data(i, roles.key("bookId")).toLongLong();
            }
        }
        QVERIFY(m_m4b > 0);
        QGuiApplication::setQuitOnLastWindowClosed(false);
    }

    void mprisControlsPlaybackOverTheBus()
    {
        MpvPlaybackBackend player({{"ao", "null"}});
        PlaybackSession session;
        SessionWiring wiring(&player, &session);
        session.setDatabasePath(m_library->getDatabasePath());
        Mpris mpris(&player, &session);
        QVERIFY(mpris.isRegistered());
        const QString service = mpris.serviceName();
        QCOMPARE(service, QStringLiteral("org.mpris.MediaPlayer2.audium"));

        Listener listener;
        QVERIFY(m_client.connect(service, Path, Props, QStringLiteral("PropertiesChanged"), &listener,
                                 SLOT(propertiesChanged(QString, QVariantMap, QStringList))));
        QVERIFY(m_client.connect(service, Path, Player, QStringLiteral("Seeked"), &listener, SLOT(seeked(qlonglong))));

        // Root interface.
        QCOMPARE(get(service, Root, QStringLiteral("Identity")).toString(), QStringLiteral("Audium"));
        QCOMPARE(get(service, Root, QStringLiteral("DesktopEntry")).toString(), QStringLiteral("io.github.SpikeTheDragon40k.Audium"));
        QCOMPARE(get(service, Root, QStringLiteral("CanQuit")).toBool(), true);
        QCOMPARE(get(service, Root, QStringLiteral("CanRaise")).toBool(), false); // no window yet
        QCOMPARE(get(service, Player, QStringLiteral("PlaybackStatus")).toString(), QStringLiteral("Stopped"));

        // Play the m4b: the "track" is the chapter.
        QVERIFY(session.playBook(m_m4b));
        QTRY_COMPARE_WITH_TIMEOUT(player.state(), State::Playing, 5000);
        QTRY_COMPARE(get(service, Player, QStringLiteral("PlaybackStatus")).toString(), QStringLiteral("Playing"));
        QVariantMap m;
        QTRY_VERIFY((m = metadata(service)).value(QStringLiteral("xesam:title")).toString() == QStringLiteral("Opening"));
        QCOMPARE(m.value(QStringLiteral("xesam:album")).toString(), QStringLiteral("Fixture Book With Chapters"));
        QCOMPARE(m.value(QStringLiteral("xesam:artist")).toStringList(), QStringList{QStringLiteral("Ada Example")});
        QVERIFY(near(m.value(QStringLiteral("mpris:length")).toLongLong() / 1000, 2000, 50));
        QVERIFY(m.value(QStringLiteral("mpris:artUrl")).toString().startsWith(QStringLiteral("file://")));
        QVERIFY(listener.changes.size() > 0);
        QVERIFY(std::any_of(listener.changes.begin(), listener.changes.end(), [](const QVariantMap &c) {
            return c.value(QStringLiteral("PlaybackStatus")).toString() == QStringLiteral("Playing");
        }));

        // Pause / PlayPause.
        call(service, Player, QStringLiteral("Pause"));
        QTRY_COMPARE(player.state(), State::Paused);
        QTRY_VERIFY(std::any_of(listener.changes.begin(), listener.changes.end(), [](const QVariantMap &c) {
            return c.value(QStringLiteral("PlaybackStatus")).toString() == QStringLiteral("Paused");
        }));
        call(service, Player, QStringLiteral("PlayPause"));
        QTRY_COMPARE(player.state(), State::Playing);
        call(service, Player, QStringLiteral("PlayPause"));
        QTRY_COMPARE(player.state(), State::Paused);

        // Next = next chapter; the position is relative to the chapter.
        call(service, Player, QStringLiteral("Next"));
        QTRY_VERIFY(near(player.position(), 2000));
        QTRY_COMPARE(metadata(service).value(QStringLiteral("xesam:title")).toString(), QStringLiteral("Middle"));
        QVERIFY(near(get(service, Player, QStringLiteral("Position")).toLongLong() / 1000, 0));

        // SetPosition within the current chapter (needs the current track id).
        const QVariant trackId = metadata(service).value(QStringLiteral("mpris:trackid"));
        call(service, Player, QStringLiteral("SetPosition"), {trackId, qlonglong(1000000)});
        QTRY_VERIFY(near(player.position(), 3000));
        // A stale track id is ignored.
        call(service, Player, QStringLiteral("SetPosition"),
             {QVariant::fromValue(QDBusObjectPath(QStringLiteral("/org/audium/Audium/book0/chapter9"))), qlonglong(0)});
        QTest::qWait(200);
        QVERIFY(near(player.position(), 3000));

        // Seek is relative and reported with a Seeked signal.
        listener.seeks.clear();
        call(service, Player, QStringLiteral("Seek"), {qlonglong(500000)});
        QTRY_VERIFY(near(player.position(), 3500));
        QTRY_VERIFY(!listener.seeks.isEmpty());
        QVERIFY(near(listener.seeks.constLast() / 1000, 1500));

        // Previous: 1.5 s into "Middle" is within the 3 s grace period, so it
        // goes back to the start of "Opening".
        call(service, Player, QStringLiteral("Previous"));
        QTRY_VERIFY(near(player.position(), 0));

        // Rate and volume are read/write.
        set(service, QStringLiteral("Rate"), 1.5);
        QTRY_COMPARE(player.rate(), 1.5);
        QTRY_COMPARE(get(service, Player, QStringLiteral("Rate")).toDouble(), 1.5);
        QCOMPARE(get(service, Player, QStringLiteral("MaximumRate")).toDouble(), 3.0);
        set(service, QStringLiteral("Volume"), 0.4);
        QTRY_COMPARE(player.volume(), 40);

        session.shutdown();
    }

    void aSecondAudiumGetsItsOwnName()
    {
        // Another process (the client connection) already owns the name.
        QVERIFY(m_client.registerService(QStringLiteral("org.mpris.MediaPlayer2.audium")));
        MpvPlaybackBackend player({{"ao", "null"}});
        PlaybackSession session;
        Mpris mpris(&player, &session);
        QVERIFY(mpris.isRegistered());
        QCOMPARE(mpris.serviceName(), QStringLiteral("org.mpris.MediaPlayer2.audium.instance%1").arg(QCoreApplication::applicationPid()));
        QCOMPARE(get(mpris.serviceName(), Root, QStringLiteral("Identity")).toString(), QStringLiteral("Audium"));
        QVERIFY(m_client.unregisterService(QStringLiteral("org.mpris.MediaPlayer2.audium")));
    }

    void mprisPlayPauseExtendsAFadingSleepTimer()
    {
        MpvPlaybackBackend player({{"ao", "null"}});
        PlaybackSession session;
        SessionWiring wiring(&player, &session);
        session.setDatabasePath(m_library->getDatabasePath());
        Mpris mpris(&player, &session);
        QVERIFY(session.playBook(m_m4b));
        QTRY_COMPARE_WITH_TIMEOUT(player.state(), State::Playing, 5000);
        session.setFadeMs(1000);
        session.startSleepTimer(1300);
        QTRY_VERIFY_WITH_TIMEOUT(session.getSleepFading(), 2000);
        call(mpris.serviceName(), Player, QStringLiteral("PlayPause"));
        QTRY_VERIFY(!session.getSleepFading());
        QCOMPARE(player.state(), State::Playing);
        session.cancelSleepTimer();
        session.shutdown();
    }

    void trayShowsTheBookAndPlaysOrPauses()
    {
        MpvPlaybackBackend player({{"ao", "null"}});
        PlaybackSession session;
        SessionWiring wiring(&player, &session);
        session.setDatabasePath(m_library->getDatabasePath());
        TrayIcon tray(&player, &session);
        QVERIFY(!tray.playPauseAction()->isEnabled());
        QVERIFY(session.playBook(m_m4b));
        QTRY_COMPARE_WITH_TIMEOUT(player.state(), State::Playing, 5000);
        QTRY_COMPARE(tray.toolTipSubTitle(), QStringLiteral("Fixture Book With Chapters — Opening"));
        QCOMPARE(tray.playPauseAction()->text(), QStringLiteral("Pause"));
        tray.playPauseAction()->trigger();
        QTRY_COMPARE(player.state(), State::Paused);
        QTRY_VERIFY(tray.toolTipSubTitle().endsWith(QStringLiteral("(paused)")));
        QCOMPARE(tray.playPauseAction()->text(), QStringLiteral("Play"));
        tray.secondaryActivate(); // middle click
        QTRY_COMPARE(player.state(), State::Playing);
        session.shutdown();
    }

    void notifiesBookFinishedAndSleepTimer()
    {
        MpvPlaybackBackend player({{"ao", "null"}});
        PlaybackSession session;
        SessionWiring wiring(&player, &session);
        session.setDatabasePath(m_library->getDatabasePath());
        Notifier notifier(&session);
        QSignalSpy notified(&notifier, &Notifier::notified);

        QVERIFY(session.playBookAt(m_m4b, 5500));
        QTRY_COMPARE_WITH_TIMEOUT(notified.count(), 1, 5000);
        QCOMPARE(notified.first().at(0).toString(), QStringLiteral("Book finished"));
        QVERIFY(notified.first().at(1).toString().contains(QStringLiteral("Fixture Book With Chapters")));

        QVERIFY(session.playBookAt(m_m4b, 0));
        QTRY_COMPARE_WITH_TIMEOUT(player.state(), State::Playing, 5000);
        session.setFadeMs(300);
        session.startSleepTimer(600);
        QTRY_COMPARE_WITH_TIMEOUT(notified.count(), 2, 3000);
        QCOMPARE(notified.last().at(0).toString(), QStringLiteral("Sleep timer"));
        session.shutdown();
    }

    void closingWhilePlayingHidesToTheTray()
    {
        MpvPlaybackBackend player({{"ao", "null"}});
        PlaybackSession session;
        SessionWiring wiring(&player, &session);
        session.setDatabasePath(m_library->getDatabasePath());
        FakeTray tray;
        QQmlApplicationEngine engine;
        KLocalization::setupLocalizedContext(&engine);
        engine.setInitialProperties({
            {QStringLiteral("player"), QVariant::fromValue(static_cast<PlaybackController *>(&player))},
            {QStringLiteral("library"), QVariant::fromValue(m_library.get())},
            {QStringLiteral("session"), QVariant::fromValue(&session)},
            {QStringLiteral("tray"), QVariant::fromValue(static_cast<QObject *>(&tray))},
        });
        engine.loadFromModule("org.audium.app", "Main");
        auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().constFirst());
        QVERIFY(window);
        window->show();
        QVERIFY(QTest::qWaitForWindowExposed(window));

        // Paused: closing closes.
        QVERIFY(session.playBook(m_m4b));
        QTRY_COMPARE_WITH_TIMEOUT(player.state(), State::Playing, 5000);
        player.pause();
        QTRY_COMPARE(player.state(), State::Paused);
        QVERIFY(window->close());
        window->show();
        QVERIFY(QTest::qWaitForWindowExposed(window));

        // Playing: the window hides, playback goes on.
        player.play();
        QTRY_COMPARE(player.state(), State::Playing);
        QVERIFY(!window->close());
        QVERIFY(!window->isVisible());
        QVERIFY(window->property("hiddenToTray").toBool());
        QTest::qWait(300);
        QCOMPARE(player.state(), State::Playing);

        // With the setting off, closing closes even while playing.
        window->show();
        QVERIFY(QTest::qWaitForWindowExposed(window));
        QVERIFY(!window->property("hiddenToTray").toBool());
        session.setCloseToTray(false);
        QVERIFY(window->close());
        session.shutdown();
    }

    void cleanupTestCase()
    {
        QDBusConnection::disconnectFromBus(QStringLiteral("test-client"));
    }
};

QTEST_MAIN(TestIntegration)
#include "test_integration.moc"
