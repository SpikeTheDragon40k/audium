import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.audium.bridge
import "../components/ReviewReasons.js" as ReviewReasons

// Books whose grouping or metadata is uncertain. Merge, split and editing
// arrive in Milestone 4; for now the list explains what is uncertain.
Kirigami.ScrollablePage {
    id: page

    required property LibraryController library

    signal bookRequested(var bookId)

    objectName: "needsReviewPage"
    title: i18nc("@title", "Needs Review")

    ListView {
        id: list
        model: BookListModel {
            databasePath: page.library.databasePath
            revision: page.library.revision
            filter: 1
        }

        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: list.count === 0
            icon.name: "checkmark"
            text: i18n("Nothing to review")
        }

        delegate: QQC2.ItemDelegate {
            id: row
            required property var bookId
            required property string title
            required property string authors
            required property var reviewReasons
            width: ListView.view.width
            onClicked: page.bookRequested(bookId)
            contentItem: ColumnLayout {
                spacing: 0
                QQC2.Label {
                    Layout.fillWidth: true
                    text: row.authors.length > 0 ? i18nc("@info title by authors", "%1 — %2", row.title, row.authors) : row.title
                    elide: Text.ElideRight
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    text: row.reviewReasons.map(c => ReviewReasons.describe(c, i18n)).join(" · ")
                    wrapMode: Text.Wrap
                    color: Kirigami.Theme.disabledTextColor
                    font: Kirigami.Theme.smallFont
                }
            }
        }
    }
}
