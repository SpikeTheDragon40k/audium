// Takes the screenshots for the app stores (scripts/make-screenshots.sh):
// the real UI, offscreen, at 1280×800 in the default Breeze Light colours,
// with the demo library (scripts/make-demo-library.sh) after some listening.
// Not a test: it saves PNGs into AUDIUM_SCREENSHOT_DIR.

#include <QApplication>
#include <QDateTime>
#include <QGuiApplication>
#include <QIcon>
#include <QPalette>
#include <QQmlApplicationEngine>
#include <QQuickItem>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QtQml/QQmlExtensionPlugin>
#include <QTemporaryDir>
#include <QtTest>

#include <KColorScheme>
#include <KLocalizedQmlContext>
#include <KLocalizedString>
#include <KSharedConfig>

#include "audium_bridge/src/book_list_model.cxxqt.h"
#include "audium_bridge/src/library_controller.cxxqt.h"
#include "audium_bridge/src/playback_session.cxxqt.h"
#include "playback/mpvplaybackbackend.h"
#include "sessionwiring.h"
#include "uitest.h"

#include <memory>

Q_IMPORT_QML_PLUGIN(org_audium_appPlugin)

using namespace uitest;

class MakeScreenshots : public QObject
{
    Q_OBJECT

    QString m_out;
    QTemporaryDir m_tmp;
    std::unique_ptr<MpvPlaybackBackend> m_player;
    std::unique_ptr<LibraryController> m_library;
    std::unique_ptr<PlaybackSession> m_session;
    std::unique_ptr<SessionWiring> m_wiring;
    std::unique_ptr<QQmlApplicationEngine> m_engine;
    QQuickWindow *m_window = nullptr;
    QHash<QString, qint64> m_ids; // book id by title
    QHash<QString, qint64> m_durations;

    void save(const QString &name)
    {
        QTest::qWait(800); // transitions, covers loading
        const QImage shot = m_window->grabWindow();
        QVERIFY(shot.save(m_out + QLatin1Char('/') + name + QStringLiteral(".png")));
    }

    // Plays `title` from `positionMs` and pauses again: the position is saved.
    void listenTo(const QString &title, qint64 positionMs)
    {
        QVERIFY2(m_ids.contains(title), qPrintable(title));
        QVERIFY(m_session->playBookAt(m_ids[title], positionMs));
        QTRY_COMPARE_WITH_TIMEOUT(m_player->state(), PlaybackController::State::Playing, 5000);
        QTest::qWait(600);
        m_player->pause();
        QTRY_COMPARE(m_player->state(), PlaybackController::State::Paused);
    }

    // Plays the last seconds of `title`: it is marked finished.
    void finish(const QString &title)
    {
        QVERIFY2(m_ids.contains(title), qPrintable(title));
        QVERIFY(m_session->playBookAt(m_ids[title], m_durations[title] - 1500));
        QTRY_COMPARE_WITH_TIMEOUT(m_player->state(), PlaybackController::State::Playing, 5000);
        QTRY_VERIFY_WITH_TIMEOUT(m_player->state() != PlaybackController::State::Playing, 8000);
        QTest::qWait(500);
    }

    void closeColumns()
    {
        QObject *pageStack = m_window->property("pageStack").value<QObject *>();
        QObject *layers = pageStack->property("layers").value<QObject *>();
        while (layers->property("depth").toInt() > 1 || pageStack->property("depth").toInt() > 1) {
            QTest::keyClick(m_window, Qt::Key_Escape);
            QTest::qWait(400);
        }
    }

private Q_SLOTS:
    void initTestCase()
    {
        m_out = qEnvironmentVariable("AUDIUM_SCREENSHOT_DIR");
        // Absolute: playback refuses relative track paths.
        const QString demo = QFileInfo(qEnvironmentVariable("AUDIUM_DEMO_LIBRARY")).absoluteFilePath();
        if (m_out.isEmpty() || qEnvironmentVariableIsEmpty("AUDIUM_DEMO_LIBRARY")) {
            QSKIP("set AUDIUM_DEMO_LIBRARY and AUDIUM_SCREENSHOT_DIR (see scripts/make-screenshots.sh)");
        }
        KLocalizedString::setApplicationDomain("audium");
        QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));
        QIcon::setThemeName(QStringLiteral("breeze"));
        QVERIFY2(QApplication::setStyle(QStringLiteral("Breeze")), "Breeze widget style not installed");
        // Breeze Light, the desktop's default, whatever this machine uses.
        const QString light = QStringLiteral("/usr/share/color-schemes/BreezeLight.colors");
        QVERIFY2(QFile::exists(light), "Breeze colour schemes not installed");
        qApp->setProperty("KDE_COLOR_SCHEME_PATH", light);
        QGuiApplication::setPalette(KColorScheme::createApplicationPalette(KSharedConfig::openConfig(light, KConfig::SimpleConfig)));

        QVERIFY(m_tmp.isValid());
        m_player = std::make_unique<MpvPlaybackBackend>(QMap<QByteArray, QByteArray>{{"ao", "null"}});
        m_library = std::make_unique<LibraryController>();
        m_session = std::make_unique<PlaybackSession>();
        m_wiring = std::make_unique<SessionWiring>(m_player.get(), m_session.get());
        QVERIFY(m_library->open(m_tmp.filePath(QStringLiteral("data")), m_tmp.filePath(QStringLiteral("cache"))));
        m_session->setDatabasePath(m_library->getDatabasePath());
        QCOMPARE(m_library->addFolder(QUrl::fromLocalFile(demo)), QString());
        QTRY_VERIFY_WITH_TIMEOUT(!m_library->getScanning(), 60000);
        QCOMPARE(m_library->getBookCount(), 16);

        BookListModel books;
        books.setDatabasePath(m_library->getDatabasePath());
        const auto roles = books.roleNames();
        for (int row = 0; row < books.rowCount(QModelIndex()); ++row) {
            const QModelIndex i = books.index(row, 0);
            const QString title = books.data(i, roles.key("title")).toString();
            m_ids[title] = books.data(i, roles.key("bookId")).toLongLong();
            m_durations[title] = books.data(i, roles.key("durationMs")).toLongLong();
        }
    }

    void listening()
    {
        // Two finished books, three under way, and the one playing now.
        finish(QStringLiteral("Salt and Starlight"));
        finish(QStringLiteral("The Lantern Keepers"));
        listenTo(QStringLiteral("The Glass Orchard"), m_durations[QStringLiteral("The Glass Orchard")] * 15 / 100);
        listenTo(QStringLiteral("Nine Winters North"), m_durations[QStringLiteral("Nine Winters North")] * 70 / 100);
        listenTo(QStringLiteral("The Road to Kessaly"), m_durations[QStringLiteral("The Road to Kessaly")] * 40 / 100);
        listenTo(QStringLiteral("The Quiet Harbour"), m_durations[QStringLiteral("The Quiet Harbour")] * 38 / 100);
        QVERIFY(!QTest::currentTestFailed());
        // Saved by the writer thread, so wait for it.
        const QString db = m_library->getDatabasePath();
        QTRY_COMPARE(sqliteQuery(db, QStringLiteral("SELECT count(*) FROM progress WHERE finished_at IS NOT NULL;")), QStringLiteral("2"));
        QTRY_COMPARE(sqliteQuery(db, QStringLiteral("SELECT count(*) FROM progress WHERE finished_at IS NULL AND book_position_ms > 0;")),
                     QStringLiteral("4"));

        // A month of listening history, most days, a current streak of 7.
        const QList<QString> titles = {QStringLiteral("The Quiet Harbour"), QStringLiteral("The Road to Kessaly"),
                                       QStringLiteral("Nine Winters North"), QStringLiteral("Salt and Starlight"),
                                       QStringLiteral("The Lantern Keepers"), QStringLiteral("The Glass Orchard")};
        const int minutes[30] = {48, 62, 35, 0, 81, 54, 40, 95, 66, 0, 0, 72, 58, 44, 110, 63, 37, 0, 52, 75,
                                 41, 88, 0, 60, 46, 92, 57, 39, 70, 64};
        QStringList inserts;
        const QDateTime today = QDateTime::currentDateTime();
        for (int day = 0; day < 30; ++day) {
            if (minutes[day] == 0) {
                continue;
            }
            const qint64 start = QDateTime(today.date().addDays(-29 + day), QTime(20, 30)).toMSecsSinceEpoch();
            const qint64 ms = minutes[day] * 60000LL;
            const qint64 book = m_ids[titles[day % titles.size()]];
            inserts << QStringLiteral("INSERT INTO sessions (book_id, started_at, ended_at, start_position_ms, end_position_ms, listened_ms) "
                                      "VALUES (%1, %2, %3, 0, %4, %4);")
                           .arg(book).arg(start).arg(start + ms).arg(ms);
        }
        QVERIFY(sqliteQuery(db, inserts.join(QLatin1Char(' ')) + QStringLiteral(" SELECT 'ok';")) == QStringLiteral("ok"));
    }

    void screenshots()
    {
        m_engine = std::make_unique<QQmlApplicationEngine>();
        KLocalization::setupLocalizedContext(m_engine.get());
        m_engine->setInitialProperties({
            {QStringLiteral("player"), QVariant::fromValue(static_cast<PlaybackController *>(m_player.get()))},
            {QStringLiteral("library"), QVariant::fromValue(m_library.get())},
            {QStringLiteral("session"), QVariant::fromValue(m_session.get())},
        });
        m_engine->loadFromModule("org.audium.app", "Main");
        QCOMPARE(m_engine->rootObjects().size(), 1);
        m_window = qobject_cast<QQuickWindow *>(m_engine->rootObjects().constFirst());
        m_window->resize(1280, 800);
        QVERIFY(QTest::qWaitForWindowExposed(m_window));
        auto *libraryPage = m_window->findChild<QQuickItem *>(QStringLiteral("libraryPage"));
        QVERIFY(libraryPage);

        // 1. Home.
        QTRY_VERIFY(m_window->findChild<QQuickItem *>(QStringLiteral("homeView")));
        save(QStringLiteral("1-home"));

        // 2. The books, with one book's details beside them.
        QVERIFY(libraryPage->setProperty("tab", 1));
        QVERIFY(QMetaObject::invokeMethod(m_window, "showBook", Q_ARG(QVariant, m_ids[QStringLiteral("The Road to Kessaly")]),
                                          Q_ARG(QVariant, QVariant())));
        save(QStringLiteral("2-book"));
        closeColumns();

        // 3. Now Playing.
        QVERIFY(QMetaObject::invokeMethod(m_window, "showNowPlaying"));
        save(QStringLiteral("3-now-playing"));
        closeColumns();

        // 4. The bookshelf, by author (the demo library has only one series).
        auto *shelfGrouping = libraryPage->findChild<QObject *>(QStringLiteral("shelfGrouping"));
        QVERIFY(shelfGrouping);
        QVERIFY(shelfGrouping->setProperty("value", 1));
        QVERIFY(libraryPage->setProperty("booksView", 2));
        save(QStringLiteral("4-bookshelf"));
        QVERIFY(libraryPage->setProperty("booksView", 0));
        QVERIFY(libraryPage->setProperty("tab", 0));

        // 5. Statistics.
        QVERIFY(QMetaObject::invokeMethod(m_window, "showStats"));
        save(QStringLiteral("5-statistics"));
        closeColumns();
    }

    void cleanupTestCase()
    {
        if (m_session) {
            m_player->stop();
            m_session->shutdown();
        }
        m_engine.reset();
    }
};

QTEST_MAIN(MakeScreenshots)
#include "make_screenshots.moc"
