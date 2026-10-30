// Performance pass on a large library (scripts/perf.sh): scan and rescan
// times, how long the library views take to fill, scrolling frame times and
// memory. Prints the measurements; the limits only catch gross regressions.
// Skipped unless AUDIUM_PERF_LIBRARY names a generated library
// (scripts/make-big-library.sh).

#include <QElapsedTimer>
#include <QFile>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickItem>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QSignalSpy>
#include <QtQml/QQmlExtensionPlugin>
#include <QTemporaryDir>
#include <QtTest>

#include <KLocalizedQmlContext>
#include <KLocalizedString>

#include "audium_bridge/src/library_controller.cxxqt.h"
#include "audium_bridge/src/playback_session.cxxqt.h"
#include "playback/mpvplaybackbackend.h"
#include "sessionwiring.h"
#include "uitest.h"

#include <algorithm>
#include <memory>
#include <numeric>

Q_IMPORT_QML_PLUGIN(org_audium_appPlugin)

using namespace uitest;

namespace
{
// Resident memory of this process, in MiB (Linux).
double residentMiB()
{
    QFile status(QStringLiteral("/proc/self/status"));
    if (!status.open(QIODevice::ReadOnly)) {
        return -1;
    }
    for (const QByteArray &line : status.readAll().split('\n')) {
        if (line.startsWith("VmRSS:")) {
            return line.mid(6).trimmed().split(' ').constFirst().toDouble() / 1024.0;
        }
    }
    return -1;
}

struct Frames {
    double averageMs = 0;
    double worstMs = 0;
    int count = 0;
};

// Scrolls `view` from top to bottom in steps of a quarter page (about two
// mouse-wheel notches), waiting for each frame, and times the frames.
Frames scrollThrough(QQuickWindow *window, QQuickItem *view)
{
    Frames f;
    QList<double> times;
    view->setProperty("contentY", view->property("originY").toReal() - view->property("topMargin").toReal());
    QTest::qWait(200);
    const qreal step = view->height() * 0.25;
    for (int i = 0; i < 2000 && !view->property("atYEnd").toBool(); ++i) {
        QElapsedTimer t;
        t.start();
        view->setProperty("contentY", view->property("contentY").toReal() + step);
        QSignalSpy swapped(window, &QQuickWindow::frameSwapped);
        window->update();
        swapped.wait(1000);
        times << t.nsecsElapsed() / 1e6;
    }
    if (!times.isEmpty()) {
        f.count = times.size();
        f.worstMs = *std::max_element(times.cbegin(), times.cend());
        f.averageMs = std::accumulate(times.cbegin(), times.cend(), 0.0) / times.size();
    }
    return f;
}
} // namespace

class TestPerf : public QObject
{
    Q_OBJECT

    QString m_libraryDir;
    QTemporaryDir m_tmp;
    std::unique_ptr<MpvPlaybackBackend> m_player;
    std::unique_ptr<LibraryController> m_library;
    std::unique_ptr<PlaybackSession> m_session;
    std::unique_ptr<SessionWiring> m_wiring;
    std::unique_ptr<QQmlApplicationEngine> m_engine;
    QQuickWindow *m_window = nullptr;
    QQuickItem *m_libraryPage = nullptr;

    static void report(const char *what, const QString &value)
    {
        qInfo().noquote() << QStringLiteral("PERF %1: %2").arg(QString::fromLatin1(what), value);
    }

    // Waits for the visible view called `name` to have `rows` items and a
    // tile called `tileName` on screen, returning how long that took.
    qint64 timeUntilFilled(const QString &name, int rows, const QString &tileName)
    {
        QElapsedTimer t;
        t.start();
        QQuickItem *view = nullptr;
        if (!QTest::qWaitFor([&] {
                view = visibleView(m_window, name);
                return view && view->property("count").toInt() >= rows && visibleItem(view, tileName);
            }, 30000)) {
            return -1;
        }
        return t.elapsed();
    }

private Q_SLOTS:
    void initTestCase()
    {
        m_libraryDir = qEnvironmentVariable("AUDIUM_PERF_LIBRARY");
        if (m_libraryDir.isEmpty()) {
            QSKIP("AUDIUM_PERF_LIBRARY is not set (see scripts/perf.sh)");
        }
        KLocalizedString::setApplicationDomain("audium");
        QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));
        QIcon::setThemeName(QStringLiteral("breeze"));
        QVERIFY(m_tmp.isValid());
        m_player = std::make_unique<MpvPlaybackBackend>(QMap<QByteArray, QByteArray>{{"ao", "null"}});
        m_library = std::make_unique<LibraryController>();
        m_session = std::make_unique<PlaybackSession>();
        m_wiring = std::make_unique<SessionWiring>(m_player.get(), m_session.get());
        QVERIFY(m_library->open(m_tmp.filePath(QStringLiteral("data")), m_tmp.filePath(QStringLiteral("cache"))));
        m_session->setDatabasePath(m_library->getDatabasePath());
        report("memory before loading anything", QStringLiteral("%1 MiB").arg(residentMiB(), 0, 'f', 0));
    }

    void scan()
    {
        QElapsedTimer t;
        t.start();
        QCOMPARE(m_library->addFolder(QUrl::fromLocalFile(m_libraryDir)), QString());
        QTRY_VERIFY_WITH_TIMEOUT(!m_library->getScanning(), 600000);
        const qint64 first = t.elapsed();
        report("first scan", QStringLiteral("%1 ms for %2 books (%3 need a review)")
                                 .arg(first).arg(m_library->getBookCount()).arg(m_library->getReviewCount()));
        QVERIFY(m_library->getBookCount() >= 900);

        t.restart();
        m_library->rescan();
        QTRY_VERIFY_WITH_TIMEOUT(!m_library->getScanning(), 600000);
        const qint64 again = t.elapsed();
        report("rescan, nothing changed", QStringLiteral("%1 ms").arg(again));

        // Gross-regression limits for a 1000-book library on a desktop.
        QVERIFY2(first < 120000, "first scan took over 2 minutes");
        QVERIFY2(again < 10000, "an unchanged rescan took over 10 s");
    }

    void views()
    {
        m_engine = std::make_unique<QQmlApplicationEngine>();
        KLocalization::setupLocalizedContext(m_engine.get());
        m_engine->setInitialProperties({
            {QStringLiteral("player"), QVariant::fromValue(static_cast<PlaybackController *>(m_player.get()))},
            {QStringLiteral("library"), QVariant::fromValue(m_library.get())},
            {QStringLiteral("session"), QVariant::fromValue(m_session.get())},
        });
        QElapsedTimer t;
        t.start();
        m_engine->loadFromModule("org.audium.app", "Main");
        QCOMPARE(m_engine->rootObjects().size(), 1);
        m_window = qobject_cast<QQuickWindow *>(m_engine->rootObjects().constFirst());
        QVERIFY(QTest::qWaitForWindowExposed(m_window));
        m_libraryPage = m_window->findChild<QQuickItem *>(QStringLiteral("libraryPage"));
        QVERIFY(m_libraryPage);
        QQuickItem *home = nullptr;
        QTRY_VERIFY((home = m_window->findChild<QQuickItem *>(QStringLiteral("homeView"))) && visibleTile(home));
        report("window shown with Home filled", QStringLiteral("%1 ms").arg(t.elapsed()));

        const int books = m_library->getBookCount();
        struct View {
            const char *name;
            int tab;
            int booksView;
            QString objectName;
            int rows;
            QString tileName;
        };
        const QString bookTile = QStringLiteral("bookTile");
        const View views[] = {
            {"Books grid", 1, 0, QStringLiteral("gridView"), books, bookTile},
            {"Books list", 1, 1, QStringLiteral("listView"), books, bookTile},
            {"Bookshelf", 1, 2, QStringLiteral("bookshelfView"), 1, bookTile},
            {"Authors", 2, 0, QStringLiteral("groupView"), 100, QStringLiteral("groupTile")},
        };
        for (const View &v : views) {
            QVERIFY(m_libraryPage->setProperty("booksView", v.booksView));
            QVERIFY(m_libraryPage->setProperty("tab", v.tab));
            const qint64 filled = timeUntilFilled(v.objectName, v.rows, v.tileName);
            QVERIFY2(filled >= 0, v.name);
            QQuickItem *view = visibleView(m_window, v.objectName);
            const Frames frames = scrollThrough(m_window, view);
            report(v.name, QStringLiteral("shown in %1 ms; scrolling: %2 frames, %3 ms average, %4 ms worst")
                               .arg(filled).arg(frames.count).arg(frames.averageMs, 0, 'f', 1).arg(frames.worstMs, 0, 'f', 1));
            QVERIFY2(filled < 3000, v.name);
            // Software rendering offscreen: generous, catches stalls only.
            QVERIFY2(frames.worstMs < 1000, v.name);
        }

        // Search: from pressing Enter to the grid showing the matches.
        QVERIFY(m_libraryPage->setProperty("booksView", 0));
        QVERIFY(m_libraryPage->setProperty("tab", 1));
        QQuickItem *search = nullptr;
        QTRY_VERIFY((search = m_window->findChild<QQuickItem *>(QStringLiteral("librarySearch"))));
        for (const QString &term : {QStringLiteral("garden"), QStringLiteral("Ada"), QStringLiteral("Saga")}) {
            QQuickItem *grid = visibleView(m_window, QStringLiteral("gridView"));
            QVERIFY(grid);
            t.restart();
            search->setProperty("text", term);
            QVERIFY(QMetaObject::invokeMethod(search, "accepted"));
            QTRY_VERIFY(grid->property("count").toInt() < books);
            report("search", QStringLiteral("“%1”: %2 books in %3 ms").arg(term).arg(grid->property("count").toInt()).arg(t.elapsed()));
            QVERIFY2(t.elapsed() < 1000, qPrintable(term));
        }
        search->setProperty("text", QString());
        QVERIFY(QMetaObject::invokeMethod(search, "accepted"));

        // Book details of the first book in the grid.
        QQuickItem *grid = visibleView(m_window, QStringLiteral("gridView"));
        QTRY_VERIFY(grid->property("count").toInt() == books);
        QQuickItem *tile = nullptr;
        QTRY_VERIFY((tile = visibleTile(grid)));
        t.restart();
        QVERIFY(clickOpensDetails(m_window, tile));
        report("book details opened", QStringLiteral("%1 ms (including the click)").arg(t.elapsed()));

        QTest::qWait(500);
        report("memory with the window open", QStringLiteral("%1 MiB").arg(residentMiB(), 0, 'f', 0));
    }

    void cleanupTestCase()
    {
        if (m_session) {
            m_session->shutdown();
        }
        m_engine.reset();
    }
};

QTEST_MAIN(TestPerf)
#include "test_perf.moc"
