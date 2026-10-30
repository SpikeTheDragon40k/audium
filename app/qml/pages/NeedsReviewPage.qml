import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.audium.bridge
import "../components/ReviewReasons.js" as ReviewReasons

// Books whose grouping or metadata is uncertain. Select books that are
// really one book and merge them, open one to edit or split it, or mark it
// as reviewed.
Kirigami.ScrollablePage {
    id: page

    required property LibraryController library
    required property int revision

    signal bookRequested(var bookId)

    // Selected book ids, in the order they were checked.
    property var selected: []

    objectName: "needsReviewPage"
    // A side column next to the library, as wide as pageStack's column
    // width (Main.qml). Kirigami would otherwise stretch the last column.
    Kirigami.ColumnView.fillWidth: false
    title: i18nc("@title", "Needs Review")

    actions: [
        Kirigami.Action {
            icon.name: "window-close"
            text: i18nc("@action:button", "Close")
            displayHint: Kirigami.DisplayHint.IconOnly
            onTriggered: applicationWindow().closePage(page)
        },
        Kirigami.Action {
            icon.name: "merge"
            text: i18nc("@action:button", "Merge Selected")
            tooltip: i18nc("@info:tooltip", "The selected books are one book: merge them into the first one selected")
            enabled: page.selected.length >= 2
            onTriggered: {
                const [target, ...others] = page.selected
                const error = page.library.mergeBooks(target, others)
                applicationWindow().showPassiveNotification(error.length > 0 ? error : i18n("Books merged."))
                if (error.length === 0) {
                    page.selected = []
                }
            }
        }
    ]

    ListView {
        id: list
        model: BookListModel {
            databasePath: page.library.databasePath
            revision: page.revision
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
            objectName: "reviewRow"
            width: ListView.view.width
            onClicked: page.bookRequested(bookId)
            contentItem: RowLayout {
                QQC2.CheckBox {
                    checked: page.selected.includes(row.bookId)
                    onToggled: {
                        const s = page.selected.filter(id => id !== row.bookId)
                        if (checked) {
                            s.push(row.bookId)
                        }
                        page.selected = s
                    }
                    QQC2.ToolTip.text: i18nc("@info:tooltip", "Select to merge")
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                }
                ColumnLayout {
                    Layout.fillWidth: true
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
                QQC2.ToolButton {
                    icon.name: "checkmark"
                    text: i18nc("@action:button", "Mark as reviewed")
                    display: QQC2.AbstractButton.IconOnly
                    onClicked: {
                        const error = page.library.dismissReview(row.bookId)
                        if (error.length > 0) {
                            applicationWindow().showPassiveNotification(error)
                        }
                    }
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                }
            }
        }
    }
}
