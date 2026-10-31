import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// A titled, horizontally scrolling row of books (hidden when empty).
ColumnLayout {
    id: row

    required property string title
    required property var model

    signal bookRequested(var bookId)

    readonly property real tileWidth: Kirigami.Units.gridUnit * 9

    visible: list.count > 0
    spacing: 0

    TileMetrics {
        id: metrics
    }

    Kirigami.Heading {
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing
        Layout.topMargin: Kirigami.Units.largeSpacing
        level: 3
        text: row.title
    }
    ListView {
        id: list
        readonly property real tileHeight: row.tileWidth + metrics.bookText
        Layout.fillWidth: true
        // Room for the scroll bar, which is drawn over the view's bottom.
        Layout.preferredHeight: tileHeight + scrollBar.height
        orientation: ListView.Horizontal
        model: row.model
        clip: true
        spacing: Kirigami.Units.smallSpacing
        leftMargin: Kirigami.Units.smallSpacing
        rightMargin: Kirigami.Units.smallSpacing
        reuseItems: true
        QQC2.ScrollBar.horizontal: QQC2.ScrollBar {
            id: scrollBar
        }
        delegate: BookTile {
            width: row.tileWidth
            height: list.tileHeight
            onBookRequested: id => row.bookRequested(id)
        }
    }
}
