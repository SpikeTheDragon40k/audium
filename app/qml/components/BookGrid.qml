import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Covers in a grid that fills its parent and scrolls itself.
GridView {
    id: grid

    signal bookRequested(var bookId)

    readonly property real usableWidth: width - leftMargin - rightMargin
    readonly property int columns: Math.max(1, Math.floor(usableWidth / (Kirigami.Units.gridUnit * 10)))

    objectName: "gridView"
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    QQC2.ScrollBar.vertical: QQC2.ScrollBar {}
    cellWidth: Math.floor(usableWidth / columns)
    cellHeight: cellWidth + Kirigami.Units.gridUnit * 4
    leftMargin: Kirigami.Units.smallSpacing
    rightMargin: Kirigami.Units.smallSpacing
    topMargin: Kirigami.Units.smallSpacing
    reuseItems: true
    delegate: BookTile {
        width: grid.cellWidth
        height: grid.cellHeight
        onBookRequested: id => grid.bookRequested(id)
    }
}
