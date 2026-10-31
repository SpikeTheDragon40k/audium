// Every page at a narrow, a normal and a wide window size: what a person
// sees must fit the window. With AUDIUM_SCREENSHOT_DIR set, each page is
// also saved as <size>-<page>.png for looking at.

#include <QApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickItem>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QtQml/QQmlExtensionPlugin>
#include <QTemporaryDir>
#include <QtTest>

#include <KLocalizedQmlContext>
#include <KLocalizedString>

#include "audium_bridge/src/book_list_model.cxxqt.h"
#include "audium_bridge/src/library_controller.cxxqt.h"
#include "audium_bridge/src/playback_session.cxxqt.h"
#include "playback/mpvplaybackbackend.h"
#include "sessionwiring.h"
#include "theme.h"
#include "uitest.h"

#include <memory>

Q_IMPORT_QML_PLUGIN(org_audium_appPlugin)

using namespace uitest;

namespace
{
// By class name: the Qt Quick classes are private, so no qobject_cast.
bool isA(const QObject *o, const char *className)
{
    for (const QMetaObject *mo = o->metaObject(); mo; mo = mo->superClass()) {
        if (qstrcmp(mo->className(), className) == 0) {
            return true;
        }
    }
    return false;
}

// A horizontally scrolling view (a row of covers): what it cuts off at its
// sides can be scrolled to. Not a tab bar: its tabs must all show.
bool scrollsSideways(const QQuickItem *item)
{
    if (!isA(item, "QQuickFlickable") || item->property("contentWidth").toReal() <= item->width() + 1) {
        return false;
    }
    for (const QQuickItem *a = item; a; a = a->parentItem()) {
        if (isA(a, "QQuickTabBar")) {
            return false;
        }
    }
    return true;
}

// A view that scrolls up and down: what it cuts off at its top and bottom
// can be scrolled to.
bool scrollsVertically(const QQuickItem *item)
{
    return isA(item, "QQuickFlickable") && item->property("contentHeight").toReal() > item->height() + 1;
}

// Scroll bars that are shown (there is something to scroll), anywhere in
// `item`. They are drawn over their view's content.
QList<QRectF> visibleScrollBars(QQuickItem *item)
{
    QList<QQuickItem *> items;
    collectItems(item, items);
    QList<QRectF> bars;
    for (QQuickItem *bar : std::as_const(items)) {
        if (isA(bar, "QQuickScrollBar") && bar->isVisible() && bar->opacity() > 0.01 && bar->property("size").toReal() < 1
            && bar->width() >= 1 && bar->height() >= 1) {
            bars << bar->mapRectToScene(bar->boundingRect());
        }
    }
    return bars;
}

QString describe(QQuickItem *item)
{
    return QStringLiteral("%1 \"%2\" at %3,%4 %5x%6")
        .arg(QString::fromLatin1(item->metaObject()->className()), item->property("text").toString())
        .arg(item->mapToScene(QPointF()).x()).arg(item->mapToScene(QPointF()).y())
        .arg(item->width()).arg(item->height());
}

// Text a person can't fully read, within `page` (which must be on screen):
// - text, a button or a tab cut off at the side of the window or of a
//   clipping view (unless that view scrolls sideways), or a tab not shown;
// - text cut off at the top or bottom of a clipping view that doesn't
//   scroll up and down (a row of covers), or under a scroll bar;
// - text sticking out of its button, tile or row.
// Buttons and tabs count as text: the desktop style draws their labels
// itself, with no text item.
QStringList unreadableText(QQuickWindow *window, QQuickItem *page)
{
    QStringList problems;
    QList<QQuickItem *> items;
    collectItems(page, items);
    const QList<QRectF> scrollBars = visibleScrollBars(page);
    const QRectF windowRect(QPointF(0, 0), window->size());
    for (QQuickItem *item : std::as_const(items)) {
        const bool isText = isA(item, "QQuickText");
        if (!(isText || isA(item, "QQuickAbstractButton")) || item->property("text").toString().trimmed().isEmpty()
            || !item->isVisible() || item->width() < 1 || item->height() < 1) {
            continue;
        }
        const QRectF rect = item->mapRectToScene(item->boundingRect());
        QRectF visible = rect & windowRect;
        bool sideways = false;
        bool vertical = false;
        bool transparent = false;
        bool inTabBar = false;
        QQuickItem *control = nullptr;
        for (QQuickItem *a = item->parentItem(); a && a != page->parentItem(); a = a->parentItem()) {
            transparent = transparent || a->opacity() < 0.01;
            sideways = sideways || scrollsSideways(a);
            vertical = vertical || scrollsVertically(a);
            inTabBar = inTabBar || isA(a, "QQuickTabBar");
            if (a->clip()) {
                visible &= a->mapRectToScene(a->boundingRect());
            }
            if (!control && isText && isA(a, "QQuickControl") && !isA(a, "QQuickPage") && !isA(a, "QQuickScrollView")) {
                control = a;
            }
        }
        if (transparent || item->opacity() < 0.01) {
            continue;
        }
        if (inTabBar && visible.width() < rect.width() - 1) {
            problems << QStringLiteral("tab not fully shown: ") + describe(item);
            continue;
        }
        if (visible.height() < 1) {
            continue; // scrolled out of view
        }
        if (!sideways && visible.width() >= 1 && visible.width() < rect.width() - 1) {
            problems << QStringLiteral("cut off: ") + describe(item);
        }
        if (!vertical && visible.width() >= 1 && visible.height() < rect.height() - 1) {
            problems << QStringLiteral("cut off at the top or bottom: ") + describe(item);
        }
        for (const QRectF &bar : scrollBars) {
            if (bar.intersects(visible)) {
                problems << QStringLiteral("under a scroll bar: ") + describe(item);
                break;
            }
        }
        if (control) {
            const QRectF box = control->mapRectToScene(control->boundingRect()).adjusted(-1, -1, 1, 1);
            if (!box.contains(rect)) {
                problems << QStringLiteral("sticks out of its %1: ").arg(QString::fromLatin1(control->metaObject()->className())) + describe(item);
            }
        }
    }
    return problems;
}
} // namespace

class TestLayout : public QObject
{
    Q_OBJECT

    QTemporaryDir m_tmp;
    std::unique_ptr<MpvPlaybackBackend> m_player;
    std::unique_ptr<LibraryController> m_library;
    std::unique_ptr<PlaybackSession> m_session;
    std::unique_ptr<SessionWiring> m_wiring;
    std::unique_ptr<Theme> m_theme;
    std::unique_ptr<QQmlApplicationEngine> m_engine;
    QQuickWindow *m_window = nullptr;
    QQuickItem *m_libraryPage = nullptr;
    QObject *m_pageStack = nullptr;
    QObject *m_layers = nullptr;
    qint64 m_bookId = 0;
    QColor m_systemWindowColor;

    // Back to the library alone, with no layer open.
    void closeEverything()
    {
        while (m_layers->property("depth").toInt() > 1) {
            QTest::keyClick(m_window, Qt::Key_Escape);
            QTRY_VERIFY(!m_layers->property("busy").toBool());
        }
        while (m_pageStack->property("depth").toInt() > 1) {
            const int depth = m_pageStack->property("depth").toInt();
            QTest::keyClick(m_window, Qt::Key_Escape);
            QTRY_COMPARE(m_pageStack->property("depth").toInt(), depth - 1);
        }
    }

    // Lets transitions, incubation and image loading finish.
    void settle()
    {
        QTest::qWait(400);
        QTRY_VERIFY(!m_layers->property("busy").toBool());
    }

    // Saves a screenshot and checks the text of every page fully on screen.
    void look(const QString &name)
    {
        settle();
        screenshot(m_window, QString::fromLatin1(QTest::currentDataTag()) + QLatin1Char('-') + name);
        QList<QQuickItem *> pages;
        if (m_layers->property("depth").toInt() > 1) {
            pages << m_layers->property("currentItem").value<QQuickItem *>();
        } else {
            // The column pages (all Audium's pages are named "...Page").
            for (QQuickItem *page : m_window->findChildren<QQuickItem *>()) {
                const QRectF r = page->mapRectToScene(page->boundingRect());
                if (isA(page, "QQuickPage") && page->objectName().endsWith(QLatin1String("Page")) && page->isVisible()
                    && r.width() > 0 && r.left() >= -1 && r.right() <= m_window->width() + 1) {
                    pages << page;
                }
            }
        }
        QVERIFY(!pages.isEmpty());
        QStringList problems;
        for (QQuickItem *page : std::as_const(pages)) {
            problems << unreadableText(m_window, page);
        }
        // The player bar at the bottom of every page.
        problems << unreadableText(m_window, m_window->property("footer").value<QQuickItem *>());
        QVERIFY2(problems.isEmpty(), qPrintable(name + QStringLiteral(":\n  ") + problems.join(QStringLiteral("\n  "))));
    }

private Q_SLOTS:
    void initTestCase()
    {
        KLocalizedString::setApplicationDomain("audium");
        // Like main.cpp: Breeze controls. The offscreen platform has no icon
        // theme of its own.
        QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));
        QIcon::setThemeName(QStringLiteral("breeze"));
        // The desktop style draws controls with the widget style, which is
        // Breeze on Plasma; offscreen it would be Fusion, sized differently.
        QVERIFY2(QApplication::setStyle(QStringLiteral("Breeze")), "Breeze widget style not installed");
        QVERIFY(m_tmp.isValid());
        m_player = std::make_unique<MpvPlaybackBackend>(QMap<QByteArray, QByteArray>{{"ao", "null"}});
        m_library = std::make_unique<LibraryController>();
        m_session = std::make_unique<PlaybackSession>();
        m_wiring = std::make_unique<SessionWiring>(m_player.get(), m_session.get());
        m_systemWindowColor = QGuiApplication::palette().color(QPalette::Window);
        m_theme = std::make_unique<Theme>(m_session.get());
        QVERIFY(m_library->open(m_tmp.filePath(QStringLiteral("data")), m_tmp.filePath(QStringLiteral("cache"))));
        m_session->setDatabasePath(m_library->getDatabasePath());
        QCOMPARE(m_library->addFolder(QUrl::fromLocalFile(QStringLiteral(AUDIUM_FIXTURES_DIR))), QString());
        QTRY_VERIFY_WITH_TIMEOUT(!m_library->getScanning(), 30000);

        // The book with chapters and a cover, playing (so the mini-player shows).
        BookListModel books;
        books.setDatabasePath(m_library->getDatabasePath());
        const int idRole = books.roleNames().key("bookId");
        const int coverRole = books.roleNames().key("coverUrl");
        for (int row = 0; row < books.rowCount(QModelIndex()) && m_bookId == 0; ++row) {
            if (!books.data(books.index(row, 0), coverRole).toUrl().isEmpty()) {
                m_bookId = books.data(books.index(row, 0), idRole).toLongLong();
            }
        }
        QVERIFY(m_bookId > 0);

        // Twenty minutes of listening today, for the statistics page.
        QVERIFY(addListening(m_library->getDatabasePath(), m_bookId, 1200000));

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
        QVERIFY(m_window);
        QVERIFY(QTest::qWaitForWindowExposed(m_window));
        m_libraryPage = m_window->findChild<QQuickItem *>(QStringLiteral("libraryPage"));
        m_pageStack = m_window->property("pageStack").value<QObject *>();
        m_layers = m_pageStack->property("layers").value<QObject *>();
        QVERIFY(m_libraryPage && m_pageStack && m_layers);

        QVERIFY(m_session->playBook(m_bookId));
        QTRY_COMPARE_WITH_TIMEOUT(m_player->state(), PlaybackController::State::Playing, 5000);
        m_player->pause();
        QTRY_COMPARE(m_player->state(), PlaybackController::State::Paused);
    }

    void init()
    {
        // As in test_bridge: any warning fails, except Kirigami's harmless
        // "conflicting anchors" on every layer push.
        QTest::failOnWarning(QRegularExpression(QStringLiteral("^(?!.*StackView has detected conflicting anchors).*")));
    }

    void cleanupTestCase()
    {
        m_player->stop();
        m_session->shutdown();
        m_engine.reset();
    }

    void everyPageFits_data()
    {
        QTest::addColumn<QSize>("size");
        QTest::addColumn<int>("theme");
        QTest::addColumn<QColor>("background");
        // The window's minimum size, the default size, and a big screen;
        // then the colour themes, whose window background is checked.
        QTest::newRow("narrow") << QSize(0, 0) << int(Theme::System) << QColor();
        QTest::newRow("normal") << QSize(-1, -1) << int(Theme::System) << QColor();
        QTest::newRow("wide") << QSize(1920, 1080) << int(Theme::System) << QColor();
        QTest::newRow("dracula") << QSize(-1, -1) << int(Theme::Dracula) << QColor(0x28, 0x2a, 0x36);
        QTest::newRow("nord") << QSize(-1, -1) << int(Theme::Nord) << QColor(0x2e, 0x34, 0x40);
    }

    void everyPageFits()
    {
        QFETCH(QSize, size);
        QFETCH(int, theme);
        QFETCH(QColor, background);
        m_session->setTheme(theme);
        // A theme sets its colours; System gives the desktop's back.
        QCOMPARE(QGuiApplication::palette().color(QPalette::Window), background.isValid() ? background : m_systemWindowColor);
        if (size == QSize(0, 0)) {
            size = m_window->minimumSize();
        } else if (size == QSize(-1, -1)) {
            size = QSize(m_window->property("defaultWidth").toInt(), m_window->property("defaultHeight").toInt());
        }
        closeEverything();
        m_window->resize(size);
        QTRY_COMPARE(m_window->size(), size);

        const QStringList tabs = {QStringLiteral("home"), QStringLiteral("books"), QStringLiteral("authors"),
                                  QStringLiteral("series"), QStringLiteral("collections")};
        for (int tab = 0; tab < tabs.size(); ++tab) {
            QVERIFY(m_libraryPage->setProperty("tab", tab));
            look(tabs[tab]);
        }
        // Home again, scrolled to its last row.
        QVERIFY(m_libraryPage->setProperty("tab", 0));
        auto *home = m_window->findChild<QQuickItem *>(QStringLiteral("homeView"));
        QVERIFY(home);
        QVERIFY(home->setProperty("contentY", std::max<qreal>(0, home->property("contentHeight").toReal() - home->height())));
        look(QStringLiteral("home-end"));
        QVERIFY(home->setProperty("contentY", 0));
        QVERIFY(m_libraryPage->setProperty("tab", 1));
        QVERIFY(m_libraryPage->setProperty("booksView", 1));
        look(QStringLiteral("books-list"));
        QVERIFY(m_libraryPage->setProperty("booksView", 2));
        look(QStringLiteral("books-shelf"));
        QVERIFY(m_libraryPage->setProperty("booksView", 0));

        QVERIFY(QMetaObject::invokeMethod(m_window, "showBook", Q_ARG(QVariant, m_bookId), Q_ARG(QVariant, QVariant())));
        look(QStringLiteral("details"));
        QVERIFY(QMetaObject::invokeMethod(m_window, "showNowPlaying"));
        look(QStringLiteral("nowplaying"));
        closeEverything();
        QVERIFY(QMetaObject::invokeMethod(m_window, "showReview"));
        look(QStringLiteral("review"));
        closeEverything();
        QVERIFY(QMetaObject::invokeMethod(m_window, "showSettings"));
        look(QStringLiteral("settings"));
        closeEverything();
        QVERIFY(QMetaObject::invokeMethod(m_window, "showStats"));
        look(QStringLiteral("stats"));
        closeEverything();
        QVERIFY(m_libraryPage->setProperty("tab", 0));

        if (background.isValid()) {
            // What the window shows: the library page's background, in the
            // empty part of the Collections tab (there are none).
            QVERIFY(m_libraryPage->setProperty("tab", 4));
            settle();
            const QImage shot = m_window->grabWindow();
            const QPoint empty(m_window->width() / 4, m_window->height() * 3 / 5);
            const QColor seen = shot.pixelColor(empty * shot.devicePixelRatio());
            QVERIFY2(qAbs(seen.red() - background.red()) <= 12 && qAbs(seen.green() - background.green()) <= 12
                         && qAbs(seen.blue() - background.blue()) <= 12,
                     qPrintable(QStringLiteral("window shows %1, expected about %2").arg(seen.name(), background.name())));
        }
        QVERIFY(m_libraryPage->setProperty("tab", 0));
        m_session->setTheme(Theme::System);
    }
};

QTEST_MAIN(TestLayout)
#include "test_layout.moc"
