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
    signal nowPlayingRequested()

    property bool gridView: true
    // "Home" (continue listening, recent, finished) or "All Books".
    property bool homeTab: true

    objectName: "libraryPage"
    title: i18nc("@title", "Library")
    padding: 0

    // With Kirigami 6.31, actions that are hidden when the page is created
    // (DisplayHint.AlwaysHide, or `visible: false`) make the action toolbar
    // destroy delegates during incubation (Qt warnings). So: no hidden
    // actions; disable instead, and let Kirigami overflow by itself.
    actions: [
        Kirigami.Action {
            icon.name: "view-list-icons"
            text: i18nc("@action:button", "Grid")
            checkable: true
            checked: page.gridView
            enabled: !page.homeTab
            displayHint: Kirigami.DisplayHint.IconOnly
            onTriggered: page.gridView = true
        },
        Kirigami.Action {
            icon.name: "view-list-details"
            text: i18nc("@action:button", "List")
            checkable: true
            checked: !page.gridView
            enabled: !page.homeTab
            displayHint: Kirigami.DisplayHint.IconOnly
            onTriggered: page.gridView = false
        },
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
            currentIndex: page.homeTab ? 0 : 1
            QQC2.TabButton {
                text: i18nc("@title:tab", "Home")
                onClicked: page.homeTab = true
            }
            QQC2.TabButton {
                text: i18nc("@title:tab %1 is a number", "All Books (%1)", books.count)
                onClicked: page.homeTab = false
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
        sourceComponent: page.homeTab ? homeComponent : page.gridView ? gridComponent : listComponent
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
        id: gridComponent
        GridView {
            id: grid
            objectName: "gridView"
            readonly property real usableWidth: width - leftMargin - rightMargin
            readonly property int columns: Math.max(1, Math.floor(usableWidth / (Kirigami.Units.gridUnit * 10)))
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            QQC2.ScrollBar.vertical: QQC2.ScrollBar {}
            model: books
            cellWidth: Math.floor(usableWidth / columns)
            cellHeight: cellWidth + Kirigami.Units.gridUnit * 4
            leftMargin: Kirigami.Units.smallSpacing
            rightMargin: Kirigami.Units.smallSpacing
            topMargin: Kirigami.Units.smallSpacing
            reuseItems: true
            delegate: BookTile {
                width: grid.cellWidth
                height: grid.cellHeight
                onBookRequested: id => page.bookRequested(id)
            }
        }
    }

    Component {
        id: listComponent
        ListView {
            id: list
            objectName: "listView"
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            QQC2.ScrollBar.vertical: QQC2.ScrollBar {}
            model: books
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
                width: list.width
                onClicked: page.bookRequested(bookId)
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
                        text: row.available ? TimeFormat.format(row.durationMs) : i18nc("@info", "Unavailable")
                        color: Kirigami.Theme.disabledTextColor
                    }
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
