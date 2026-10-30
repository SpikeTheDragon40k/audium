import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Covers in a grid that fills its parent and scrolls itself.
GridView {
    id: grid

    signal bookRequested(var bookId)

    readonly property real usableWidth: width - leftMargin - rightMargin
    // As many columns as fit tiles of at least 9 grid units; the tiles
    // then share the width.
    readonly property int columns: Math.max(1, Math.floor(usableWidth / (Kirigami.Units.gridUnit * 9)))

    objectName: "gridView"
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    QQC2.ScrollBar.vertical: QQC2.ScrollBar {
        id: scrollBar
    }
    cellWidth: Math.floor(usableWidth / columns)
    cellHeight: cellWidth + metrics.bookText
    leftMargin: Kirigami.Units.smallSpacing
    // Room for the scroll bar, which is drawn over the view.
    rightMargin: Kirigami.Units.smallSpacing + scrollBar.width
    topMargin: Kirigami.Units.smallSpacing
    reuseItems: true

    TileMetrics {
        id: metrics
    }
    delegate: BookTile {
        width: grid.cellWidth
        height: grid.cellHeight
        onBookRequested: id => grid.bookRequested(id)
    }
}
