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

    visible: list.count > 0
    spacing: 0

    Kirigami.Heading {
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.largeSpacing
        Layout.topMargin: Kirigami.Units.largeSpacing
        level: 3
        text: row.title
    }
    ListView {
        id: list
        Layout.fillWidth: true
        Layout.preferredHeight: Kirigami.Units.gridUnit * 13
        orientation: ListView.Horizontal
        model: row.model
        clip: true
        spacing: Kirigami.Units.smallSpacing
        leftMargin: Kirigami.Units.smallSpacing
        rightMargin: Kirigami.Units.smallSpacing
        reuseItems: true
        QQC2.ScrollBar.horizontal: QQC2.ScrollBar {}
        delegate: BookTile {
            width: Kirigami.Units.gridUnit * 9
            height: ListView.view.height
            onBookRequested: id => row.bookRequested(id)
        }
    }
}
