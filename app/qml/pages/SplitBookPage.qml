import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.audium.bridge

// Splits a wrongly grouped book: the checked files become a new book.
Kirigami.ScrollablePage {
    id: page

    required property var bookId
    required property LibraryController library
    required property int revision

    signal done()

    property var selected: []

    objectName: "splitBookPage"
    title: i18nc("@title", "Split Book")

    BookDetails {
        id: details
        databasePath: page.library.databasePath
        revision: page.revision
        bookId: page.bookId
    }

    actions: [
        Kirigami.Action {
            icon.name: "split"
            text: i18nc("@action:button", "Move to New Book")
            enabled: page.selected.length > 0 && page.selected.length < details.tracks.length
            onTriggered: {
                const error = page.library.splitBook(page.bookId, page.selected)
                applicationWindow().showPassiveNotification(error.length > 0 ? error : i18n("The book was split."))
                if (error.length === 0) {
                    page.done()
                }
            }
        }
    ]

    ListView {
        model: details.tracks
        header: QQC2.Label {
            width: ListView.view.width
            padding: Kirigami.Units.largeSpacing
            wrapMode: Text.Wrap
            text: i18n("Check the files that belong to a different book. They are moved to a new book; the files themselves are not touched.")
        }
        delegate: QQC2.CheckDelegate {
            required property var modelData
            width: ListView.view.width
            text: modelData.fileName
            checked: page.selected.includes(modelData.trackId)
            onToggled: {
                const s = page.selected.filter(id => id !== modelData.trackId)
                if (checked) {
                    s.push(modelData.trackId)
                }
                page.selected = s
            }
        }
    }
}
