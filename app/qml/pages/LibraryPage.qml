import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.audium.app
import org.audium.bridge
import "../components/TimeFormat.js" as TimeFormat

Kirigami.Page {
    id: page

    required property PlaybackController player
    required property LibraryController library
    required property PlaybackSession session
    required property int revision

    signal bookRequested(var bookId)
    signal reviewRequested()
    signal settingsRequested()
    signal statsRequested()
    signal nowPlayingRequested()
    /// kind: 1 author, 3 series, 4 collection (BookListModel scope kinds)
    signal groupRequested(int kind, var groupId, string name)

    enum Tab { Home, Books, Authors, Series, Collections }
    property int tab: LibraryPage.Tab.Home
    /// Books tab: 0 grid, 1 list, 2 bookshelf
    property int booksView: 0

    /// Ctrl+F: show the Books tab and focus its search field.
    function focusSearch() {
        tab = LibraryPage.Tab.Books
        Qt.callLater(() => {
            if (view.item && view.item.searchField) {
                view.item.searchField.forceActiveFocus()
            }
        })
    }

    objectName: "libraryPage"
    title: i18nc("@title", "Library")
    padding: 0

    // With Kirigami 6.31, actions that are hidden when the page is created
    // (DisplayHint.AlwaysHide, or `visible: false`) make the action toolbar
    // destroy delegates during incubation (Qt warnings). So: no hidden
    // actions; disable instead, and let Kirigami overflow by itself.
    actions: [
        Kirigami.Action {
            icon.name: "view-refresh"
            text: i18nc("@action:button", "Rescan")
            enabled: page.library.ready && !page.library.scanning && page.library.folders.length > 0
            onTriggered: page.library.rescan()
        },
        Kirigami.Action {
            icon.name: "folder-new"
            text: i18nc("@action:button", "Add Library Folder…")
            enabled: page.library.ready
            onTriggered: folderDialog.open()
        },
        Kirigami.Action {
            icon.name: "document-open"
            text: i18nc("@action:button", "Open Files…")
            tooltip: i18nc("@info:tooltip", "Play files without adding them to the library")
            onTriggered: fileDialog.open()
        },
        Kirigami.Action {
            icon.name: "office-chart-bar"
            text: i18nc("@action:button", "Statistics")
            onTriggered: page.statsRequested()
        },
        Kirigami.Action {
            icon.name: "configure"
            text: i18nc("@action:button", "Settings")
            onTriggered: page.settingsRequested()
        }
    ]

    header: ColumnLayout {
        spacing: 0
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            position: Kirigami.InlineMessage.Header
            type: Kirigami.MessageType.Error
            text: page.library.ready
                ? page.library.errorString
                : i18n("The library database could not be opened: %1", page.library.errorString)
            visible: page.library.errorString.length > 0
        }
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            position: Kirigami.InlineMessage.Header
            type: Kirigami.MessageType.Warning
            visible: page.library.reviewCount > 0 && !page.library.scanning
            text: i18np("1 book may need a review.", "%1 books may need a review.", page.library.reviewCount)
            actions: Kirigami.Action {
                text: i18nc("@action:button", "Review")
                icon.name: "document-edit"
                onTriggered: page.reviewRequested()
            }
        }
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            position: Kirigami.InlineMessage.Header
            type: Kirigami.MessageType.Information
            text: page.library.scanStatus
            visible: page.library.scanning
            actions: Kirigami.Action {
                text: i18nc("@action:button", "Cancel")
                icon.name: "dialog-cancel"
                onTriggered: page.library.cancelScan()
            }
        }
        QQC2.ProgressBar {
            Layout.fillWidth: true
            visible: page.library.scanning
            from: 0
            to: Math.max(1, page.library.scanTotal)
            value: page.library.scanDone
            indeterminate: page.library.scanTotal === 0
        }
        QQC2.TabBar {
            Layout.fillWidth: true
            visible: books.count > 0
            currentIndex: page.tab
            QQC2.TabButton {
                text: i18nc("@title:tab", "Home")
                onClicked: page.tab = LibraryPage.Tab.Home
            }
            QQC2.TabButton {
                text: i18nc("@title:tab %1 is a number", "Books (%1)", books.count)
                onClicked: page.tab = LibraryPage.Tab.Books
            }
            QQC2.TabButton {
                text: i18nc("@title:tab", "Authors")
                onClicked: page.tab = LibraryPage.Tab.Authors
            }
            QQC2.TabButton {
                text: i18nc("@title:tab", "Series")
                onClicked: page.tab = LibraryPage.Tab.Series
            }
            QQC2.TabButton {
                text: i18nc("@title:tab", "Collections")
                onClicked: page.tab = LibraryPage.Tab.Collections
            }
        }
    }

    BookListModel {
        id: books
        databasePath: page.library.databasePath
        revision: page.revision
        filter: 0
    }
    BookListModel {
        id: continueListening
        databasePath: page.library.databasePath
        revision: page.revision
        filter: 2
    }
    BookListModel {
        id: recentlyAdded
        databasePath: page.library.databasePath
        revision: page.revision
        filter: 3
    }
    BookListModel {
        id: finishedBooks
        databasePath: page.library.databasePath
        revision: page.revision
        filter: 4
    }

    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        width: parent.width - Kirigami.Units.gridUnit * 4
        visible: books.count === 0 && !page.library.scanning
        icon.name: "audiobook"
        text: page.library.folders.length === 0 ? i18n("Your library is empty") : i18n("No audiobooks found")
        explanation: page.library.folders.length === 0
            ? i18n("Add the folder where your audiobooks are. Audium only reads it and never changes your files.")
            : i18n("No audio files were found in your library folders.")
        helpfulAction: Kirigami.Action {
            icon.name: "folder-new"
            text: i18nc("@action:button", "Add Library Folder…")
            enabled: page.library.ready
            onTriggered: folderDialog.open()
        }
    }

    // Each view fills the page and scrolls itself. (Inside a ScrollView a
    // Loader has no size, so the views were zero-sized and not clickable.)
    Loader {
        id: view
        anchors.fill: parent
        visible: books.count > 0
        sourceComponent: [homeComponent, booksComponent, authorsComponent, seriesComponent, collectionsComponent][page.tab]
    }

    Component {
        id: homeComponent
        Flickable {
            objectName: "homeView"
            clip: true
            contentWidth: width
            contentHeight: rows.implicitHeight + Kirigami.Units.largeSpacing
            boundsBehavior: Flickable.StopAtBounds
            QQC2.ScrollBar.vertical: QQC2.ScrollBar {}
            ColumnLayout {
                id: rows
                width: parent.width
                spacing: Kirigami.Units.largeSpacing
                BookRow {
                    Layout.fillWidth: true
                    title: i18nc("@title:group", "Continue Listening")
                    model: continueListening
                    onBookRequested: id => page.bookRequested(id)
                }
                BookRow {
                    Layout.fillWidth: true
                    title: i18nc("@title:group", "Recently Added")
                    model: recentlyAdded
                    onBookRequested: id => page.bookRequested(id)
                }
                BookRow {
                    Layout.fillWidth: true
                    title: i18nc("@title:group", "Finished")
                    model: finishedBooks
                    onBookRequested: id => page.bookRequested(id)
                }
            }
        }
    }

    Component {
        id: booksComponent
        ColumnLayout {
            property alias searchField: search
            spacing: 0
            RowLayout {
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.smallSpacing
                Kirigami.SearchField {
                    id: search
                    objectName: "librarySearch"
                    Layout.fillWidth: true
                    placeholderText: i18nc("@info:placeholder", "Search titles, authors, narrators, series…")
                    delaySearch: true
                    onAccepted: books.search = text
                }
                QQC2.ComboBox {
                    id: sortBox
                    enabled: page.booksView !== 2
                    model: [
                        i18nc("@item:inlistbox sort by", "Title"),
                        i18nc("@item:inlistbox sort by", "Author"),
                        i18nc("@item:inlistbox sort by", "Recently Played"),
                        i18nc("@item:inlistbox sort by", "Recently Added")
                    ]
                    currentIndex: books.sort
                    onActivated: index => books.sort = index
                    QQC2.ToolTip.text: i18nc("@info:tooltip", "Sort by")
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                }
                QQC2.ComboBox {
                    visible: page.booksView === 2
                    model: [
                        i18nc("@item:inlistbox group shelves by", "By Series"),
                        i18nc("@item:inlistbox group shelves by", "By Author"),
                        i18nc("@item:inlistbox group shelves by", "By Collection")
                    ]
                    currentIndex: shelfGrouping.value
                    onActivated: index => shelfGrouping.value = index
                }
                Repeater {
                    model: [
                        {icon: "view-list-icons", text: i18nc("@action:button", "Grid")},
                        {icon: "view-list-details", text: i18nc("@action:button", "List")},
                        {icon: "view-media-artist", text: i18nc("@action:button", "Bookshelf")}
                    ]
                    delegate: QQC2.ToolButton {
                        required property int index
                        required property var modelData
                        icon.name: modelData.icon
                        text: modelData.text
                        display: QQC2.AbstractButton.IconOnly
                        checkable: true
                        checked: page.booksView === index
                        onClicked: page.booksView = index
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    }
                }
            }
            Kirigami.PlaceholderMessage {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: books.count === 0 && books.search.length > 0
                icon.name: "edit-find"
                text: i18n("No books match “%1”", books.search)
            }
            BookGrid {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: page.booksView === 0 && books.count > 0
                model: books
                onBookRequested: id => page.bookRequested(id)
            }
            BookList {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: page.booksView === 1 && books.count > 0
                model: page.booksView === 1 ? books : null
                onBookRequested: id => page.bookRequested(id)
            }
            Loader {
                Layout.fillWidth: true
                Layout.fillHeight: true
                active: page.booksView === 2
                visible: active
                sourceComponent: Bookshelf {
                    databasePath: page.library.databasePath
                    revision: page.revision
                    grouping: shelfGrouping.value
                    onBookRequested: id => page.bookRequested(id)
                }
            }
        }
    }

    // The bookshelf grouping survives switching tabs.
    QtObject {
        id: shelfGrouping
        property int value: 0
    }

    Component {
        id: authorsComponent
        GroupGrid {
            model: GroupListModel {
                databasePath: page.library.databasePath
                revision: page.revision
                kind: 0
            }
            onGroupRequested: (id, name) => page.groupRequested(1, id, name)
        }
    }

    Component {
        id: seriesComponent
        GroupGrid {
            model: GroupListModel {
                databasePath: page.library.databasePath
                revision: page.revision
                kind: 2
            }
            onGroupRequested: (id, name) => page.groupRequested(3, id, name)
        }
    }

    Component {
        id: collectionsComponent
        ColumnLayout {
            spacing: 0
            RowLayout {
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.smallSpacing
                QQC2.Label {
                    Layout.fillWidth: true
                    text: i18n("Collections are your own lists of books. Add books from their details page.")
                    wrapMode: Text.Wrap
                    color: Kirigami.Theme.disabledTextColor
                }
                QQC2.Button {
                    icon.name: "list-add"
                    text: i18nc("@action:button", "New Collection…")
                    onClicked: nameDialog.ask(0, "")
                }
            }
            Kirigami.PlaceholderMessage {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: collections.count === 0
                icon.name: "bookmarks"
                text: i18n("No collections yet")
            }
            GroupGrid {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: collections.count > 0
                model: GroupListModel {
                    id: collections
                    databasePath: page.library.databasePath
                    revision: page.revision
                    kind: 3
                }
                onGroupRequested: (id, name) => page.groupRequested(4, id, name)
                onGroupMenuRequested: (id, name, item) => {
                    collectionMenu.collectionId = id
                    collectionMenu.collectionName = name
                    collectionMenu.popup(item)
                }
            }
        }
    }

    QQC2.Menu {
        id: collectionMenu
        property var collectionId: 0
        property string collectionName
        QQC2.MenuItem {
            icon.name: "edit-rename"
            text: i18nc("@action:inmenu", "Rename…")
            onTriggered: nameDialog.ask(collectionMenu.collectionId, collectionMenu.collectionName)
        }
        QQC2.MenuItem {
            icon.name: "edit-delete"
            text: i18nc("@action:inmenu", "Delete Collection")
            onTriggered: {
                const error = page.library.deleteCollection(collectionMenu.collectionId)
                applicationWindow().showPassiveNotification(error.length > 0 ? error
                    : i18n("Collection “%1” deleted. Its books are still in your library.", collectionMenu.collectionName))
            }
        }
    }

    // New collection (id 0) or rename.
    Kirigami.PromptDialog {
        id: nameDialog
        property var collectionId: 0
        function ask(id, name) {
            collectionId = id
            nameField.text = name
            open()
            nameField.forceActiveFocus()
        }
        title: collectionId === 0 ? i18nc("@title:dialog", "New Collection") : i18nc("@title:dialog", "Rename Collection")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        QQC2.TextField {
            id: nameField
            placeholderText: i18nc("@info:placeholder", "Name")
            onAccepted: nameDialog.accept()
        }
        onAccepted: {
            const name = nameField.text.trim()
            if (name.length === 0) {
                return
            }
            if (collectionId === 0) {
                if (page.library.createCollection(name) === 0) {
                    applicationWindow().showPassiveNotification(i18n("The collection could not be created."))
                }
            } else {
                const error = page.library.renameCollection(collectionId, name)
                if (error.length > 0) {
                    applicationWindow().showPassiveNotification(error)
                }
            }
        }
    }

    FolderDialog {
        id: folderDialog
        title: i18nc("@title:window", "Add Library Folder")
        onAccepted: {
            const error = page.library.addFolder(selectedFolder)
            if (error.length > 0) {
                applicationWindow().showPassiveNotification(error)
            }
        }
    }

    FileDialog {
        id: fileDialog
        title: i18nc("@title:window", "Open Audio Files")
        fileMode: FileDialog.OpenFiles
        nameFilters: [
            i18n("Audio files (%1)", "*.m4b *.m4a *.mp3 *.ogg *.opus *.flac *.aac *.wav *.wma"),
            i18n("All files (%1)", "*")
        ]
        onAccepted: {
            page.player.openFiles(selectedFiles)
            page.nowPlayingRequested()
        }
    }
}
