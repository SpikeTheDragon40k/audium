// PlaybackSession + SessionWiring + the real mpv backend: progress, resume,
// per-book speed, end of book, listening sessions and the sleep timer.
// The library is the generated fixtures folder (only read); the database and
// cover cache live in a temporary directory.

#include <QSignalSpy>
#include <QSqlDatabase>
#include <QSqlQuery>
#include <QTemporaryDir>
#include <QtTest>

#include "audium_bridge/src/book_list_model.cxxqt.h"
#include "audium_bridge/src/library_controller.cxxqt.h"
#include "audium_bridge/src/playback_session.cxxqt.h"
#include "playback/mpvplaybackbackend.h"
#include "sessionwiring.h"

using State = PlaybackController::State;

namespace
{
constexpr qint64 ToleranceMs = 250;

bool near(qint64 actual, qint64 expected)
{
    return std::abs(actual - expected) <= ToleranceMs;
}
} // namespace

// The playback objects of one "application run".
struct Run {
    explicit Run(const QString &databasePath)
        : player({{"ao", "null"}})
        , wiring(&player, &session)
    {
        session.setDatabasePath(databasePath);
    }
    ~Run()
    {
        // What the application does on quit.
        session.shutdown();
    }
    MpvPlaybackBackend player;
    PlaybackSession session;
    SessionWiring wiring;
};

class TestSession : public QObject
{
    Q_OBJECT

    QTemporaryDir m_dir;
    std::unique_ptr<LibraryController> m_library;
    QString m_dbPath;
    QHash<QString, qint64> m_books; // title -> id (the m4b for the shared title)

    QVariant query(const QString &sql, qint64 bookId)
    {
        QSqlDatabase db = QSqlDatabase::database(QStringLiteral("check"));
        QSqlQuery q(db);
        q.prepare(sql);
        q.addBindValue(bookId);
        if (!q.exec() || !q.next()) {
            return {};
        }
        return q.value(0);
    }

    qint64 savedPosition(qint64 bookId)
    {
        return query(QStringLiteral("SELECT book_position_ms FROM progress WHERE book_id = ?"), bookId).toLongLong();
    }

    bool finished(qint64 bookId)
    {
        return query(QStringLiteral("SELECT finished_at IS NOT NULL AND finished_at >= updated_at FROM progress WHERE book_id = ?"), bookId).toBool();
    }

    void play(Run &run, const QString &title)
    {
        QVERIFY2(m_books.contains(title), qPrintable(title));
        QVERIFY(run.session.playBook(m_books.value(title)));
        QTRY_COMPARE_WITH_TIMEOUT(run.player.state(), State::Playing, 5000);
    }

    void clearProgress()
    {
        QSqlQuery q(QSqlDatabase::database(QStringLiteral("check")));
        QVERIFY(q.exec(QStringLiteral("DELETE FROM progress")));
        QVERIFY(q.exec(QStringLiteral("DELETE FROM book_settings")));
        QVERIFY(q.exec(QStringLiteral("DELETE FROM sessions")));
    }

private Q_SLOTS:
    void initTestCase()
    {
        QVERIFY(m_dir.isValid());
        m_library = std::make_unique<LibraryController>();
        QVERIFY(m_library->open(m_dir.filePath(QStringLiteral("data")), m_dir.filePath(QStringLiteral("cache"))));
        QCOMPARE(m_library->addFolder(QUrl::fromLocalFile(QStringLiteral(AUDIUM_FIXTURES_DIR))), QString());
        QTRY_VERIFY_WITH_TIMEOUT(!m_library->getScanning(), 30000);
        m_dbPath = m_library->getDatabasePath();

        BookListModel books;
        books.setDatabasePath(m_dbPath);
        const auto roles = books.roleNames();
        for (int row = 0; row < books.rowCount(QModelIndex()); ++row) {
            const QModelIndex i = books.index(row, 0);
            const QString title = books.data(i, roles.key("title")).toString();
            // Two books share the m4b's title; keep the one with a cover (the m4b).
            if (!m_books.contains(title) || !books.data(i, roles.key("coverUrl")).toUrl().isEmpty()) {
                m_books.insert(title, books.data(i, roles.key("bookId")).toLongLong());
            }
        }
        QVERIFY(m_books.contains(QStringLiteral("The Folder Book")));
        QVERIFY(m_books.contains(QStringLiteral("Disc Book")));

        QSqlDatabase db = QSqlDatabase::addDatabase(QStringLiteral("QSQLITE"), QStringLiteral("check"));
        db.setDatabaseName(m_dbPath);
        QVERIFY(db.open());
    }

    void init()
    {
        clearProgress();
    }

    void cleanupTestCase()
    {
        QSqlDatabase::database(QStringLiteral("check")).close();
        QSqlDatabase::removeDatabase(QStringLiteral("check"));
    }

    void savesOnPauseAndResumesAfterRestart()
    {
        const qint64 book = m_books.value(QStringLiteral("Fixture Book With Chapters"));
        {
            Run run(m_dbPath);
            play(run, QStringLiteral("Fixture Book With Chapters"));
            run.player.seekBook(3000);
            QTRY_VERIFY(near(run.player.position(), 3000));
            run.player.pause();
            QTRY_COMPARE(run.player.state(), State::Paused);
            QTRY_VERIFY2(near(savedPosition(book), run.player.position()), qPrintable(QString::number(savedPosition(book))));
        }
        // A new "application run" resumes where it stopped.
        Run run(m_dbPath);
        play(run, QStringLiteral("Fixture Book With Chapters"));
        QVERIFY2(run.player.position() >= 2900 && run.player.position() < 4000, qPrintable(QString::number(run.player.position())));
        QCOMPARE(run.player.chapterIndex(), 1);
    }

    void savesOnTrackChangeAndQuit()
    {
        const qint64 book = m_books.value(QStringLiteral("The Folder Book"));
        Run run(m_dbPath);
        play(run, QStringLiteral("The Folder Book"));
        run.player.seekTrack(0, 1700);
        // Crossing into track 2 saves immediately, while still playing.
        QTRY_COMPARE_WITH_TIMEOUT(run.player.trackIndex(), 1, 3000);
        QTRY_VERIFY_WITH_TIMEOUT(savedPosition(book) >= run.player.trackStart(1), 2000);
        QCOMPARE(run.player.state(), State::Playing);
        const QString track = query(QStringLiteral("SELECT t.display_path FROM progress p JOIN tracks t ON t.id = p.track_id WHERE p.book_id = ?"), book).toString();
        QVERIFY2(track.endsWith(QStringLiteral("Track 2.mp3")), qPrintable(track));

        // Quitting saves the latest position.
        QTest::qWait(600);
        const qint64 before = run.player.position();
        run.session.shutdown();
        QVERIFY(savedPosition(book) >= before - ToleranceMs);
    }

    void resumesAtNextTrackWhenSavedAtATrackEnd()
    {
        const qint64 book = m_books.value(QStringLiteral("The Folder Book"));
        QSqlQuery q(QSqlDatabase::database(QStringLiteral("check")));
        // 1.9 s into a ~2 s first track: within the snap distance of its end.
        q.prepare(QStringLiteral("INSERT INTO progress (book_id, track_id, offset_ms, book_position_ms, started_at, updated_at) "
                                 "SELECT ?, id, duration_ms - 100, duration_ms - 100, 1, 1 FROM tracks WHERE book_id = ? ORDER BY position LIMIT 1"));
        q.addBindValue(book);
        q.addBindValue(book);
        QVERIFY(q.exec());
        Run run(m_dbPath);
        QVERIFY(run.session.playBook(book));
        QCOMPARE(run.session.pendingStartTrack(), 1);
        QCOMPARE(run.session.pendingStartOffset(), 0);
        QTRY_COMPARE_WITH_TIMEOUT(run.player.state(), State::Playing, 5000);
        QCOMPARE(run.player.trackIndex(), 1);
        QVERIFY(run.player.trackPosition() < 500);
    }

    void remembersSpeedPerBook()
    {
        {
            Run run(m_dbPath);
            play(run, QStringLiteral("Fixture Book With Chapters"));
            run.player.setRate(1.75);
            play(run, QStringLiteral("The Folder Book"));
            QCOMPARE(run.player.rate(), 1.0);
            run.player.setRate(0.8);
        }
        Run run(m_dbPath);
        play(run, QStringLiteral("Fixture Book With Chapters"));
        QCOMPARE(run.player.rate(), 1.75);
        play(run, QStringLiteral("The Folder Book"));
        QCOMPARE(run.player.rate(), 0.8);
    }

    void endOfBookMarksFinishedAndReplayStartsOver()
    {
        const qint64 book = m_books.value(QStringLiteral("Fixture Book With Chapters"));
        Run run(m_dbPath);
        QSignalSpy revision(&run.session, &PlaybackSession::revisionChanged);
        play(run, QStringLiteral("Fixture Book With Chapters"));
        run.player.seekBook(5300);
        QTRY_COMPARE_WITH_TIMEOUT(run.player.state(), State::Paused, 3000);
        QTRY_VERIFY(finished(book));
        QVERIFY(revision.count() > 0);
        // Nothing saved afterwards may undo "finished".
        QTest::qWait(300);
        QVERIFY(finished(book));

        play(run, QStringLiteral("Fixture Book With Chapters"));
        QVERIFY2(run.player.position() < 1000, qPrintable(QString::number(run.player.position())));
        run.player.pause();
        QTRY_COMPARE(run.player.state(), State::Paused);
        QTRY_VERIFY(!finished(book));
    }

    void recordsListeningSessions()
    {
        const qint64 book = m_books.value(QStringLiteral("Disc Book"));
        Run run(m_dbPath);
        play(run, QStringLiteral("Disc Book"));
        QTest::qWait(5500);
        run.player.pause();
        QTRY_COMPARE(run.player.state(), State::Paused);
        run.session.shutdown();
        const QString sql = QStringLiteral("SELECT listened_ms FROM sessions WHERE book_id = ?");
        const qint64 listened = query(sql, book).toLongLong();
        QVERIFY2(listened >= 5000 && listened < 8000, qPrintable(QString::number(listened)));
        QVERIFY(query(QStringLiteral("SELECT end_position_ms > start_position_ms FROM sessions WHERE book_id = ?"), book).toBool());
    }

    void sleepTimerFadesAndPauses()
    {
        Run run(m_dbPath);
        run.session.setFadeMs(600);
        play(run, QStringLiteral("Disc Book"));
        run.session.startSleepTimer(1500);
        QVERIFY(run.session.getSleepActive());
        QTRY_VERIFY_WITH_TIMEOUT(run.session.getSleepFading(), 2000);
        QTRY_COMPARE_WITH_TIMEOUT(run.player.state(), State::Paused, 3000);
        QVERIFY(!run.session.getSleepActive());
        // The user's volume setting is untouched by the fade.
        QCOMPARE(run.player.volume(), 100);
    }

    void activityDuringTheFadeExtends()
    {
        Run run(m_dbPath);
        run.session.setFadeMs(1000);
        play(run, QStringLiteral("Disc Book"));
        run.session.startSleepTimer(1300);
        QTRY_VERIFY_WITH_TIMEOUT(run.session.getSleepFading(), 2000);
        run.session.userActivity();
        QVERIFY(!run.session.getSleepFading());
        QVERIFY(run.session.getSleepRemainingMs() >= 1000);
        QCOMPARE(run.player.state(), State::Playing);
        // A key press over the window (seen by the wiring) works as well.
        QTRY_VERIFY_WITH_TIMEOUT(run.session.getSleepFading(), 2000);
        QKeyEvent key(QEvent::KeyPress, Qt::Key_Shift, Qt::NoModifier);
        QCoreApplication::sendEvent(&run.player, &key);
        QVERIFY(!run.session.getSleepFading());
        run.session.cancelSleepTimer();
        QVERIFY(!run.session.getSleepActive());
    }

    void sleepAtEndOfChapter()
    {
        Run run(m_dbPath);
        run.session.setFadeMs(500);
        play(run, QStringLiteral("Fixture Book With Chapters"));
        run.player.seekBook(1000);
        QTRY_VERIFY(near(run.player.position(), 1000));
        run.session.sleepAtEndOfChapter();
        QVERIFY(run.session.getSleepEndOfChapter());
        QTRY_COMPARE_WITH_TIMEOUT(run.player.state(), State::Paused, 3000);
        // Chapter 1 ends at 2 s.
        QVERIFY2(run.player.position() >= 1700 && run.player.position() <= 2400, qPrintable(QString::number(run.player.position())));
    }

    void otherFilesDetachTheBook()
    {
        const qint64 book = m_books.value(QStringLiteral("Fixture Book With Chapters"));
        Run run(m_dbPath);
        play(run, QStringLiteral("Fixture Book With Chapters"));
        run.player.seekBook(2500);
        QTRY_VERIFY(near(run.player.position(), 2500));
        run.player.openFiles({QUrl::fromLocalFile(QStringLiteral(AUDIUM_FIXTURES_DIR "/single-mp3-chapters/book.mp3"))});
        QCOMPARE(run.session.getBookId(), 0);
        QTRY_VERIFY(near(savedPosition(book), 2500));
        // Playing the other file does not touch the book's progress.
        run.player.play();
        QTRY_COMPARE(run.player.state(), State::Playing);
        QTest::qWait(500);
        run.player.seekBook(4000);
        run.player.pause();
        QTRY_COMPARE(run.player.state(), State::Paused);
        run.session.shutdown();
        QVERIFY(near(savedPosition(book), 2500));
    }

    void bookmarksAtTheCurrentPosition()
    {
        const qint64 book = m_books.value(QStringLiteral("The Folder Book"));
        Run run(m_dbPath);
        QVERIFY(!run.session.addBookmark()); // nothing playing yet
        play(run, QStringLiteral("The Folder Book"));
        run.player.seekBook(3000);
        QTRY_VERIFY(near(run.player.position(), 3000));
        run.player.pause();
        QTRY_COMPARE(run.player.state(), State::Paused);
        QSignalSpy revision(&run.session, &PlaybackSession::revisionChanged);
        QVERIFY(run.session.addBookmark());
        QTRY_VERIFY(revision.count() > 0);
        const qint64 at = query(QStringLiteral("SELECT book_position_ms FROM bookmarks WHERE book_id = ?"), book).toLongLong();
        QVERIFY2(near(at, 3000), qPrintable(QString::number(at)));
        const QString track = query(QStringLiteral("SELECT t.display_path FROM bookmarks b JOIN tracks t ON t.id = b.track_id WHERE b.book_id = ?"), book).toString();
        QVERIFY2(track.endsWith(QStringLiteral("Track 2.mp3")), qPrintable(track));

        const qint64 id = query(QStringLiteral("SELECT id FROM bookmarks WHERE book_id = ?"), book).toLongLong();
        run.session.updateBookmark(id, QStringLiteral("Big reveal"), QStringLiteral("remember this"));
        QTRY_COMPARE(query(QStringLiteral("SELECT title FROM bookmarks WHERE book_id = ?"), book).toString(), QStringLiteral("Big reveal"));
        run.session.deleteBookmark(id);
        QTRY_VERIFY(!query(QStringLiteral("SELECT id FROM bookmarks WHERE book_id = ?"), book).isValid());
    }

    void playBookAtStartsOrSeeks()
    {
        Run run(m_dbPath);
        const qint64 book = m_books.value(QStringLiteral("The Folder Book"));
        // Not playing yet: loads at that position (a chapter or a bookmark).
        QVERIFY(run.session.playBookAt(book, 4500));
        QTRY_COMPARE_WITH_TIMEOUT(run.player.state(), State::Playing, 5000);
        QVERIFY2(run.player.trackIndex() == 2 && run.player.trackPosition() < 1200, qPrintable(QString::number(run.player.position())));
        // Already loaded: seeks without reloading, and plays if paused.
        QSignalSpy loads(&run.session, &PlaybackSession::loadRequested);
        run.player.pause();
        QTRY_COMPARE(run.player.state(), State::Paused);
        QVERIFY(run.session.playBookAt(book, 1000));
        QTRY_VERIFY(run.player.position() >= 1000 && run.player.position() < 1800);
        QTRY_COMPARE(run.player.state(), State::Playing);
        QCOMPARE(loads.count(), 0);
    }

    void playingAtAPositionAfterTheEndWorksNormally()
    {
        // Finish the book, then click a chapter of it: it must play and save
        // progress again (it used to only seek, with the session still
        // treating the book as finished).
        const qint64 book = m_books.value(QStringLiteral("Fixture Book With Chapters"));
        Run run(m_dbPath);
        QVERIFY(run.session.playBookAt(book, 5400));
        QTRY_COMPARE_WITH_TIMEOUT(run.player.state(), State::Playing, 5000);
        QTRY_VERIFY_WITH_TIMEOUT(finished(book), 3000);
        QVERIFY(run.session.playBookAt(book, 2000));
        QTRY_COMPARE_WITH_TIMEOUT(run.player.state(), State::Playing, 5000);
        QTRY_VERIFY(run.player.position() >= 1900 && run.player.position() < 3000);
        run.player.pause();
        QTRY_COMPARE(run.player.state(), State::Paused);
        QTRY_VERIFY(!finished(book));
        QTRY_VERIFY2(near(savedPosition(book), run.player.position()), qPrintable(QString::number(savedPosition(book))));

        // Starting a finished book counts as restarting it at once, even when
        // it is paused before the position has moved.
        QVERIFY(run.session.playBookAt(book, 5400));
        QTRY_VERIFY_WITH_TIMEOUT(finished(book), 4000);
        QVERIFY(run.session.playBookAt(book, 2000));
        QTRY_VERIFY_WITH_TIMEOUT(!finished(book), 3000);
        QTRY_VERIFY(near(savedPosition(book), 2000));
    }

    void skipIntervalsArePersisted()
    {
        {
            Run run(m_dbPath);
            QCOMPARE(run.session.getSkipBackMs(), 15000);
            run.session.setSkipBackMs(10000);
            run.session.setSkipForwardMs(45000);
        }
        Run run(m_dbPath);
        QCOMPARE(run.session.getSkipBackMs(), 10000);
        QCOMPARE(run.session.getSkipForwardMs(), 45000);
    }
};

QTEST_GUILESS_MAIN(TestSession)
#include "test_session.moc"
