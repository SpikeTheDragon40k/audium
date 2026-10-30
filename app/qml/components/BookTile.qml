import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// A book cover with title, author and listening progress.
QQC2.ItemDelegate {
    id: tile

    required property var bookId
    required property string title
    required property string authors
    required property url coverUrl
    required property bool available
    required property real progress
    required property bool finished

    signal bookRequested(var bookId)

    objectName: "bookTile"
    onClicked: bookRequested(bookId)
    QQC2.ToolTip.text: available ? title : i18n("%1 (unavailable)", title)
    QQC2.ToolTip.visible: hovered
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay

    contentItem: ColumnLayout {
        spacing: Kirigami.Units.smallSpacing
        BookCover {
            Layout.fillWidth: true
            Layout.preferredHeight: width
            source: tile.coverUrl
            available: tile.available
            Kirigami.Icon {
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: Kirigami.Units.smallSpacing
                width: Kirigami.Units.iconSizes.smallMedium
                height: width
                visible: tile.finished
                source: "checkmark"
                QQC2.ToolTip.text: i18nc("@info:tooltip", "Finished")
            }
        }
        QQC2.ProgressBar {
            Layout.fillWidth: true
            from: 0
            to: 1
            value: tile.progress
            // Keeps the layout stable for books not started yet.
            opacity: tile.progress > 0 && !tile.finished ? 1 : 0
        }
        QQC2.Label {
            Layout.fillWidth: true
            text: tile.title
            elide: Text.ElideRight
            maximumLineCount: 2
            wrapMode: Text.Wrap
            font.bold: true
        }
        QQC2.Label {
            Layout.fillWidth: true
            text: tile.authors
            elide: Text.ElideRight
            color: Kirigami.Theme.disabledTextColor
            font: Kirigami.Theme.smallFont
        }
        // Covers line up at the top whatever the title's length.
        Item {
            Layout.fillHeight: true
        }
    }
}
