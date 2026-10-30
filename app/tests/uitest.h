// Helpers for UI tests that act like a person: real mouse clicks on items
// that are on screen, after they stopped moving.
#pragma once

#include <QDateTime>
#include <QProcess>
#include <QQuickItem>
#include <QQuickWindow>
#include <QtTest>

#include <algorithm>

namespace uitest
{
// All visual descendants (view delegates are visual, not QObject, children).
inline void collectItems(QQuickItem *item, QList<QQuickItem *> &out)
{
    for (QQuickItem *child : item->childItems()) {
        out << child;
        collectItems(child, out);
    }
}

// A book tile of `view` that is on screen and has a size.
inline QQuickItem *visibleTile(QQuickItem *view)
{
    const QRectF viewRect = view->mapRectToScene(view->boundingRect());
    QList<QQuickItem *> items;
    collectItems(view, items);
    for (QQuickItem *tile : std::as_const(items)) {
        if (tile->objectName() != QLatin1String("bookTile")) {
            continue;
        }
        const QRectF r = tile->mapRectToScene(tile->boundingRect());
        if (tile->isVisible() && r.width() > 10 && r.height() > 10 && viewRect.contains(r.center())) {
            return tile;
        }
    }
    return nullptr;
}

// With AUDIUM_SCREENSHOT_DIR set, saves what the window shows as <name>.png
// (for looking at layouts; not an assertion).
inline void screenshot(QQuickWindow *window, const QString &name)
{
    const QString dir = qEnvironmentVariable("AUDIUM_SCREENSHOT_DIR");
    if (!dir.isEmpty()) {
        QTest::qWait(300);
        window->grabWindow().save(dir + QLatin1Char('/') + name + QStringLiteral(".png"));
    }
}

// A tile named `objectName` of `view` that is on screen.
inline QQuickItem *visibleItem(QQuickItem *view, const QString &objectName)
{
    const QRectF viewRect = view->mapRectToScene(view->boundingRect());
    QList<QQuickItem *> items;
    collectItems(view, items);
    for (QQuickItem *item : std::as_const(items)) {
        const QRectF r = item->mapRectToScene(item->boundingRect());
        if (item->objectName() == objectName && item->isVisible() && r.width() > 10 && viewRect.contains(r.center())) {
            return item;
        }
    }
    return nullptr;
}

// Waits until `item` stops moving (page and column transitions), as a
// person would before clicking it.
inline bool waitUntilStill(QQuickItem *item)
{
    QRectF last = item->mapRectToScene(item->boundingRect());
    for (int i = 0; i < 40; ++i) {
        QTest::qWait(100);
        const QRectF now = item->mapRectToScene(item->boundingRect());
        if (now == last) {
            return true;
        }
        last = now;
    }
    return false;
}

// Like a user: move the pointer there (ending hovers and their tooltips
// elsewhere, which would otherwise swallow the press), then click.
inline void click(QQuickWindow *window, QQuickItem *item)
{
    waitUntilStill(item);
    const QPoint p = item->mapToScene(QPointF(item->width() / 2, item->height() / 2)).toPoint();
    QTest::mouseMove(window, p);
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, p);
}

// The visible view called `objectName` (views of hidden tabs stay alive).
inline QQuickItem *visibleView(QQuickWindow *window, const QString &objectName)
{
    for (QQuickItem *v : window->findChildren<QQuickItem *>(objectName)) {
        if (v->isVisible() && v->width() > 100 && v->height() > 100) {
            return v;
        }
    }
    return nullptr;
}

// A visible, enabled button showing the icon `iconName` (e.g. Kirigami's
// back button), anywhere in the window.
inline QQuickItem *visibleButton(QQuickWindow *window, const QString &iconName)
{
    QList<QQuickItem *> items;
    collectItems(window->contentItem(), items);
    for (QQuickItem *item : std::as_const(items)) {
        // `icon` is a QQuickIcon value type: read its name through the gadget.
        const QVariant icon = item->property("icon");
        const QMetaObject *mo = icon.metaType().metaObject();
        if (!(icon.metaType().flags() & QMetaType::IsGadget) || !mo || mo->indexOfProperty("name") < 0) {
            continue;
        }
        const QString name = mo->property(mo->indexOfProperty("name")).readOnGadget(icon.constData()).toString();
        const QRectF r = item->mapRectToScene(item->boundingRect());
        if (name == iconName && item->isVisible() && item->isEnabled() && r.width() > 4
            && QRectF(QPointF(0, 0), window->size()).contains(r.center())) {
            return item;
        }
    }
    return nullptr;
}

// Clicks `tile` like a user and checks that its book's details page opens.
inline bool clickOpensDetails(QQuickWindow *window, QQuickItem *tile)
{
    const qint64 id = tile->property("bookId").toLongLong();
    click(window, tile);
    return QTest::qWaitFor([&] {
        const auto pages = window->findChildren<QObject *>(QStringLiteral("bookDetailsPage"));
        return std::any_of(pages.begin(), pages.end(), [&](QObject *p) {
            return p->property("bookId").toLongLong() == id;
        });
    }, 3000);
}

// Records `ms` of listening to `bookId`, ending now, as if a session had been
// played. Done by the sqlite3 tool in a separate process: a second SQLite
// library in this process (e.g. Qt Sql) would break the app's file locks and
// make its queries fail with "disk I/O error".
inline bool addListening(const QString &databasePath, qint64 bookId, qint64 ms)
{
    const qint64 end = QDateTime::currentMSecsSinceEpoch();
    const QString sql = QStringLiteral("INSERT INTO sessions (book_id, started_at, ended_at, start_position_ms, end_position_ms, listened_ms) "
                                       "VALUES (%1, %2, %3, 0, %4, %4);")
                            .arg(bookId).arg(end - ms).arg(end).arg(ms);
    QProcess sqlite;
    sqlite.start(QStringLiteral("sqlite3"), {QStringLiteral("-cmd"), QStringLiteral(".timeout 5000"), databasePath, sql});
    return sqlite.waitForFinished(10000) && sqlite.exitStatus() == QProcess::NormalExit && sqlite.exitCode() == 0;
}

// Runs one SQL query with the sqlite3 tool (a separate process, see
// addListening) and returns its output, trimmed.
inline QString sqliteQuery(const QString &databasePath, const QString &sql)
{
    QProcess sqlite;
    sqlite.start(QStringLiteral("sqlite3"), {QStringLiteral("-cmd"), QStringLiteral(".timeout 5000"), databasePath, sql});
    sqlite.waitForFinished(10000);
    return QString::fromUtf8(sqlite.readAllStandardOutput()).trimmed();
}

// A visible, enabled button labelled `text`, anywhere in the window
// including popups and dialogs.
inline QQuickItem *visibleButtonWithText(QQuickWindow *window, const QString &text)
{
    QList<QQuickItem *> items;
    collectItems(window->contentItem()->parentItem() ? window->contentItem()->parentItem() : window->contentItem(), items);
    for (QQuickItem *item : std::as_const(items)) {
        const QRectF r = item->mapRectToScene(item->boundingRect());
        if (item->property("text").toString() == text && item->inherits("QQuickAbstractButton") && item->isVisible()
            && item->isEnabled() && r.width() > 4 && QRectF(QPointF(0, 0), window->size()).contains(r.center())) {
            return item;
        }
    }
    return nullptr;
}
} // namespace uitest
