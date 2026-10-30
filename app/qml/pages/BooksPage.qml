import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.audium.bridge

// The books of one author, narrator, series or collection.
Kirigami.Page {
    id: page

    required property LibraryController library
    required property int revision
    /// BookListModel scope kind: 1 author, 2 narrator, 3 series, 4 collection
    required property int scopeKind
    required property var scopeId
    required property string name

    signal bookRequested(var bookId)

    property bool listView: false

    objectName: "booksPage"

    actions: [
        Kirigami.Action {
            icon.name: "window-close"
            text: i18nc("@action:button", "Close")
            displayHint: Kirigami.DisplayHint.IconOnly
            onTriggered: applicationWindow().closePage(page)
        }
    ]
    padding: 0
    title: name

    header: RowLayout {
        Kirigami.SearchField {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.smallSpacing
            visible: books.count > 6 || books.search.length > 0
            delaySearch: true
            onAccepted: books.search = text
        }
        Item {
            Layout.fillWidth: true
            visible: !(books.count > 6 || books.search.length > 0)
        }
        QQC2.ToolButton {
            icon.name: page.listView ? "view-list-icons" : "view-list-details"
            text: page.listView ? i18nc("@action:button", "Grid") : i18nc("@action:button", "List")
            display: QQC2.AbstractButton.IconOnly
            onClicked: page.listView = !page.listView
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
        }
    }

    BookListModel {
        id: books
        databasePath: page.library.databasePath
        revision: page.revision
        scopeKind: page.scopeKind
        scopeId: page.scopeId
        // Series keep their reading order; the rest sort by title.
        sort: 0
    }

    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        width: parent.width - Kirigami.Units.gridUnit * 4
        visible: books.count === 0
        icon.name: "bookmarks"
        text: page.scopeKind === 4 ? i18n("This collection is empty") : i18n("No books")
        explanation: page.scopeKind === 4 ? i18n("Add books to it from their details page.") : ""
    }
    BookGrid {
        anchors.fill: parent
        visible: !page.listView && books.count > 0
        model: books
        onBookRequested: id => page.bookRequested(id)
    }
    BookList {
        anchors.fill: parent
        visible: page.listView && books.count > 0
        model: page.listView ? books : null
        onBookRequested: id => page.bookRequested(id)
    }
}
