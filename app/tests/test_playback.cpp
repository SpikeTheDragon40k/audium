// Integration tests for the playback state machine with the real libmpv
// backend. Audio goes to mpv's null output (real-time, silent).

#include <QSignalSpy>
#include <QtTest>

#include "playback/mpvplaybackbackend.h"

using State = PlaybackController::State;

namespace
{
constexpr int LoadTimeoutMs = 5000;
// Position comparisons allow for seek granularity and encoder padding.
constexpr qint64 ToleranceMs = 150;

QByteArray fixture(const char *relative)
{
    return QFile::encodeName(QStringLiteral(AUDIUM_FIXTURES_DIR "/") + QString::fromUtf8(relative));
}

TrackSource track(const char *relative, qint64 durationMs = 0)
{
    return TrackSource{fixture(relative), QString::fromUtf8(relative).section(QLatin1Char('/'), -1), durationMs};
}

const char *const M4b = "single-m4b/Ada Example - Fixture Book With Chapters.m4b";
const char *const Folder[] = {
    "folder-book/Bea Writer/The Folder Book/Track 1.mp3",
    "folder-book/Bea Writer/The Folder Book/Track 2.mp3",
    "folder-book/Bea Writer/The Folder Book/Track 10.mp3",
};

QList<TrackSource> folderBook(qint64 durationEach = 0)
{
    return {track(Folder[0], durationEach), track(Folder[1], durationEach), track(Folder[2], durationEach)};
}

bool near(qint64 actual, qint64 expected)
{
    return std::abs(actual - expected) <= ToleranceMs;
}
} // namespace

class TestPlayback : public QObject
{
    Q_OBJECT

    std::unique_ptr<MpvPlaybackBackend> player;

    void loadAndWait(const QList<TrackSource> &tracks, int startTrack = 0, qint64 offsetMs = 0)
    {
        player->load(tracks, {}, startTrack, offsetMs);
        QTRY_COMPARE_WITH_TIMEOUT(player->state(), State::Paused, LoadTimeoutMs);
    }

private Q_SLOTS:
    void initTestCase()
    {
        QVERIFY2(QFile::exists(QFile::decodeName(fixture(M4b))), "fixtures missing: run scripts/make-fixtures.sh or ctest");
    }

    void init()
    {
        player = std::make_unique<MpvPlaybackBackend>(QMap<QByteArray, QByteArray>{{"ao", "null"}});
        QVERIFY(player->isAvailable());
        QCOMPARE(player->state(), State::Stopped);
    }

    void cleanup()
    {
        player.reset();
    }

    void loadsSingleFileWithEmbeddedChapters()
    {
        loadAndWait({track(M4b)});
        QVERIFY2(near(player->duration(), 6000), qPrintable(QString::number(player->duration())));
        QTRY_COMPARE(player->chapters().size(), 3);
        const auto chapters = player->chapters();
        QCOMPARE(chapters[0].title, QStringLiteral("Opening"));
        QCOMPARE(chapters[1].title, QStringLiteral("Middle"));
        QCOMPARE(chapters[2].title, QStringLiteral("Ending"));
        QVERIFY(near(chapters[1].startMs, 2000));
        QVERIFY(near(chapters[2].startMs, 4000));
        QCOMPARE(player->chapterIndex(), 0);
        QCOMPARE(player->position(), 0);
    }

    void playAdvancesAndPauseHolds()
    {
        loadAndWait({track(M4b)});
        player->play();
        QTRY_COMPARE(player->state(), State::Playing);
        QTRY_VERIFY_WITH_TIMEOUT(player->position() > 300, 3000);
        player->pause();
        QTRY_COMPARE(player->state(), State::Paused);
        const qint64 held = player->position();
        QTest::qWait(400);
        QVERIFY(near(player->position(), held));
    }

    void seeksWithinTrack()
    {
        loadAndWait({track(M4b)});
        player->seekBook(4500);
        QTRY_VERIFY(near(player->position(), 4500));
        QCOMPARE(player->chapterIndex(), 2);
        QCOMPARE(player->chapterTitle(), QStringLiteral("Ending"));
    }

    void navigatesChapters()
    {
        loadAndWait({track(M4b)});
        QTRY_COMPARE(player->chapters().size(), 3);

        player->nextChapter();
        QTRY_VERIFY(near(player->position(), 2000));
        QCOMPARE(player->chapterIndex(), 1);

        // Within the grace period, "previous" goes to the previous chapter...
        player->seekBook(2500);
        QTRY_VERIFY(near(player->position(), 2500));
        player->previousChapter();
        QTRY_VERIFY(near(player->position(), 0));
        QCOMPARE(player->chapterIndex(), 0);

        // ...and past it, to the start of the current one.
        player->seekBook(5500 - 1);
        QTRY_VERIFY(near(player->position(), 5499));
        player->nextChapter(); // already in the last chapter: no-op
        QTRY_VERIFY(near(player->position(), 5499));
        player->seekBook(4000 + PlaybackController::PreviousChapterGraceMs - 2000); // 1 s into "Ending"
        QTRY_VERIFY(near(player->position(), 5000));
        player->previousChapter();
        QTRY_VERIFY(near(player->position(), 2000));
    }

    void multiFileBookTransitionsWithoutErrors()
    {
        QSignalSpy failed(player.get(), &PlaybackController::trackFailed);
        loadAndWait(folderBook());
        QCOMPARE(player->trackCount(), 3);
        // One chapter per track when there are no embedded chapters.
        QCOMPARE(player->chapters().size(), 3);

        player->seekTrack(0, 1600);
        QTRY_VERIFY(near(player->trackPosition(), 1600));
        player->play();
        QTRY_COMPARE_WITH_TIMEOUT(player->trackIndex(), 1, 3000);
        QCOMPARE(player->state(), State::Playing);
        // Track 1's duration is now known, so book positions continue from it.
        QVERIFY(player->trackStart(1) > 1900);
        QVERIFY(player->position() >= player->trackStart(1));
        QCOMPARE(failed.count(), 0);
    }

    void seeksAcrossTracks()
    {
        loadAndWait(folderBook(2000));
        player->seekBook(4500);
        QTRY_COMPARE(player->trackIndex(), 2);
        QTRY_VERIFY(near(player->trackPosition(), 500));
        QTRY_VERIFY(near(player->position(), 4500));
        QCOMPARE(player->state(), State::Paused);

        player->seekBook(500);
        QTRY_COMPARE(player->trackIndex(), 0);
        QTRY_VERIFY(near(player->position(), 500));
    }

    void skipWithUnknownDurationsGoesToNextTrack()
    {
        // As with "Open Files": only the loaded track's duration is known.
        loadAndWait(folderBook());
        QCOMPARE(player->trackStart(2), player->trackStart(1)); // tracks 1 and 2 unknown
        player->skip(30000);
        // Lands near the end of the first track of unknown length, not past it.
        QTRY_COMPARE(player->trackIndex(), 1);
        QTRY_VERIFY(player->trackDuration(1) > 0);
        QTRY_VERIFY(player->trackPosition() > player->trackDuration(1) - 1000);
        QTest::qWait(300);
        QCOMPARE(player->trackIndex(), 1);
        QCOMPARE(player->state(), State::Paused);
        player->skip(-3000);
        QTRY_COMPARE(player->trackIndex(), 0);
    }

    void resumesAtStartPosition()
    {
        loadAndWait(folderBook(2000), 1, 1200);
        QCOMPARE(player->trackIndex(), 1);
        QTRY_VERIFY(near(player->trackPosition(), 1200));
        QVERIFY(near(player->position(), 3200));
    }

    void rateIsAppliedAndClamped()
    {
        loadAndWait({track(M4b)});
        player->setRate(2.0);
        QTRY_COMPARE(player->rate(), 2.0);
        player->setRate(5.0);
        QTRY_COMPARE(player->rate(), PlaybackController::MaxRate);
        player->setRate(0.1);
        QTRY_COMPARE(player->rate(), PlaybackController::MinRate);

        // Playback still advances at the new speed (scaletempo2 is active).
        player->setRate(3.0);
        player->play();
        QTRY_VERIFY_WITH_TIMEOUT(player->position() > 1500, 2000);
    }

    void volumeIsAppliedAndClamped()
    {
        loadAndWait({track(M4b)});
        player->setVolume(40);
        QTRY_COMPARE(player->volume(), 40);
        player->setVolume(150);
        QTRY_COMPARE(player->volume(), 100);
        player->setVolume(-5);
        QTRY_COMPARE(player->volume(), 0);
    }

    void detectsEndOfBook()
    {
        QSignalSpy ended(player.get(), &PlaybackController::endOfBook);
        loadAndWait({track(M4b)});
        player->seekBook(5400);
        QTRY_VERIFY(near(player->position(), 5400));
        player->play();
        QTRY_COMPARE_WITH_TIMEOUT(ended.count(), 1, 3000);
        QTRY_COMPARE(player->state(), State::Paused);
        QVERIFY(near(player->position(), player->duration()));

        // Playing again restarts the book.
        player->play();
        QTRY_COMPARE(player->state(), State::Playing);
        QTRY_VERIFY(player->position() < 1500);
    }

    void corruptFileIsAnError()
    {
        QSignalSpy failed(player.get(), &PlaybackController::trackFailed);
        player->load({track("corrupt/garbage.m4b")});
        QTRY_COMPARE_WITH_TIMEOUT(player->state(), State::Error, LoadTimeoutMs);
        QVERIFY(!player->errorString().isEmpty());
        QCOMPARE(failed.count(), 1);
        QCOMPARE(failed.first().at(0).toInt(), 0);
    }

    void missingFileIsAnError()
    {
        player->load({TrackSource{fixture("does/not/exist.mp3"), QString(), 0}});
        QTRY_COMPARE_WITH_TIMEOUT(player->state(), State::Error, LoadTimeoutMs);
        QVERIFY(!player->errorString().isEmpty());
    }

    void relativePathIsRejected()
    {
        player->load({TrackSource{QByteArrayLiteral("relative.mp3"), QString(), 0}});
        QCOMPARE(player->state(), State::Error);
        QVERIFY(!player->errorString().isEmpty());
    }

    void failedTrackIsSkipped()
    {
        QSignalSpy failed(player.get(), &PlaybackController::trackFailed);
        player->load({track("corrupt/garbage.m4b"), track(Folder[0])});
        QTRY_COMPARE_WITH_TIMEOUT(player->state(), State::Paused, LoadTimeoutMs);
        QCOMPARE(player->trackIndex(), 1);
        QCOMPARE(failed.count(), 1);
        QCOMPARE(failed.first().at(0).toInt(), 0);
    }

    void playsUnicodePath()
    {
        loadAndWait({track("unicode/Ünïcødé Àuthor/Livre d’été 本/Chapitre 01 – début.mp3")});
        QVERIFY(player->duration() > 1500);
        player->play();
        QTRY_COMPARE(player->state(), State::Playing);
    }

    void stopClearsTheBook()
    {
        loadAndWait({track(M4b)});
        player->stop();
        QCOMPARE(player->state(), State::Stopped);
        QVERIFY(!player->hasMedia());
        QCOMPARE(player->position(), 0);
        QCOMPARE(player->trackIndex(), -1);
        // A new load works after stop.
        loadAndWait({track(M4b)});
    }
};

QTEST_GUILESS_MAIN(TestPlayback)
#include "test_playback.moc"
