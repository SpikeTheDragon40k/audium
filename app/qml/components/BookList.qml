import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "TimeFormat.js" as TimeFormat

// Books as rows: small cover, title, authors and series, length or state.
ListView {
    id: list

    signal bookRequested(var bookId)

    objectName: "listView"
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    QQC2.ScrollBar.vertical: QQC2.ScrollBar {
        id: scrollBar
    }
    reuseItems: true
    delegate: QQC2.ItemDelegate {
        id: row
        required property var bookId
        required property string title
        required property string authors
        required property string series
        required property url coverUrl
        required property bool available
        required property real durationMs
        required property real progress
        required property bool finished
        objectName: "bookTile"
        // Room for the scroll bar, which is drawn over the rows.
        width: ListView.view.width - scrollBar.width
        onClicked: list.bookRequested(bookId)
        contentItem: RowLayout {
            spacing: Kirigami.Units.largeSpacing
            BookCover {
                Layout.preferredWidth: Kirigami.Units.gridUnit * 2.5
                Layout.preferredHeight: Layout.preferredWidth
                source: row.coverUrl
                available: row.available
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0
                QQC2.Label {
                    Layout.fillWidth: true
                    text: row.title
                    elide: Text.ElideRight
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    text: row.series.length > 0 ? i18nc("@info authors · series", "%1 · %2", row.authors, row.series) : row.authors
                    elide: Text.ElideRight
                    color: Kirigami.Theme.disabledTextColor
                    font: Kirigami.Theme.smallFont
                }
            }
            QQC2.Label {
                text: !row.available ? i18nc("@info", "Unavailable")
                    : row.finished ? i18nc("@info", "Finished")
                    : row.progress > 0 ? i18nc("@info percentage listened", "%1%", Math.round(row.progress * 100))
                    : TimeFormat.format(row.durationMs)
                color: Kirigami.Theme.disabledTextColor
            }
        }
    }
}
