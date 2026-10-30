import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Authors, narrators, series or collections as cards with a cover and the
// number of books.
GridView {
    id: grid

    signal groupRequested(var groupId, string name)
    // Right click or press-and-hold (used for renaming collections).
    signal groupMenuRequested(var groupId, string name, Item item)

    readonly property real usableWidth: width - leftMargin - rightMargin
    // As many columns as fit tiles of at least 9 grid units; the tiles
    // then share the width.
    readonly property int columns: Math.max(1, Math.floor(usableWidth / (Kirigami.Units.gridUnit * 9)))

    objectName: "groupView"
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    QQC2.ScrollBar.vertical: QQC2.ScrollBar {
        id: scrollBar
    }
    cellWidth: Math.floor(usableWidth / columns)
    cellHeight: cellWidth + metrics.groupText
    leftMargin: Kirigami.Units.smallSpacing
    // Room for the scroll bar, which is drawn over the view.
    rightMargin: Kirigami.Units.smallSpacing + scrollBar.width
    topMargin: Kirigami.Units.smallSpacing
    reuseItems: true

    TileMetrics {
        id: metrics
    }
    delegate: QQC2.ItemDelegate {
        id: tile
        required property var groupId
        required property string name
        required property int bookCount
        required property url coverUrl
        objectName: "groupTile"
        width: grid.cellWidth
        height: grid.cellHeight
        onClicked: grid.groupRequested(groupId, name)
        onPressAndHold: grid.groupMenuRequested(groupId, name, tile)
        TapHandler {
            acceptedButtons: Qt.RightButton
            onTapped: grid.groupMenuRequested(tile.groupId, tile.name, tile)
        }
        contentItem: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing
            BookCover {
                Layout.fillWidth: true
                Layout.preferredHeight: width
                source: tile.coverUrl
            }
            QQC2.Label {
                Layout.fillWidth: true
                text: tile.name
                elide: Text.ElideRight
                font.bold: true
            }
            QQC2.Label {
                Layout.fillWidth: true
                text: i18ncp("@info", "%1 book", "%1 books", tile.bookCount)
                color: Kirigami.Theme.disabledTextColor
                font: Kirigami.Theme.smallFont
            }
            Item {
                Layout.fillHeight: true
            }
        }
    }
}
