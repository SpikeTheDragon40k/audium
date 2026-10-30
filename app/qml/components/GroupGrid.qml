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
    readonly property int columns: Math.max(1, Math.floor(usableWidth / (Kirigami.Units.gridUnit * 10)))

    objectName: "groupView"
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    QQC2.ScrollBar.vertical: QQC2.ScrollBar {}
    cellWidth: Math.floor(usableWidth / columns)
    cellHeight: cellWidth + Kirigami.Units.gridUnit * 3
    leftMargin: Kirigami.Units.smallSpacing
    rightMargin: Kirigami.Units.smallSpacing
    topMargin: Kirigami.Units.smallSpacing
    reuseItems: true
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
        }
    }
}
