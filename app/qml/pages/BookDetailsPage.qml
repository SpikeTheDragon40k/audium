import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.audium.app
import org.audium.bridge
import "../components/TimeFormat.js" as TimeFormat
import "../components/ReviewReasons.js" as ReviewReasons

Kirigami.ScrollablePage {
    id: page

    required property var bookId
    required property LibraryController library
    required property PlaybackSession session
    required property int revision

    signal nowPlayingRequested()
    /// kind: 1 author, 2 narrator, 3 series, 4 collection
    signal groupRequested(int kind, var groupId, string name)
    signal editRequested(var bookId)
    signal splitRequested(var bookId)

    function escapeHtml(s) {
        return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;")
    }
    function links(kind, items) {
        return items.map(i => `<a href="${kind}:${i.id}">${escapeHtml(i.name)}</a>`).join(", ")
    }
    function openLink(link) {
        const [kind, id] = link.split(":")
        const all = kind === "1" ? details.authorLinks : kind === "2" ? details.narratorLinks : []
        const item = all.find(i => String(i.id) === id)
        if (kind === "3") {
            page.groupRequested(3, details.seriesId, details.series)
        } else if (item) {
            page.groupRequested(Number(kind), item.id, item.name)
        }
    }
    function play(positionMs) {
        if (page.session.playBookAt(details.bookId, positionMs)) {
            page.nowPlayingRequested()
        }
    }

    objectName: "bookDetailsPage"
    title: details.valid ? details.title : i18nc("@title", "Book")

    actions: [
        Kirigami.Action {
            icon.name: "window-close"
            text: i18nc("@action:button", "Close")
            displayHint: Kirigami.DisplayHint.IconOnly
            onTriggered: applicationWindow().closePage(page)
        },
        Kirigami.Action {
            icon.name: "document-edit"
            text: i18nc("@action:button", "Edit Metadata…")
            enabled: details.valid
            onTriggered: page.editRequested(details.bookId)
        },
        Kirigami.Action {
            icon.name: "bookmarks"
            text: i18nc("@action:button", "Add to Collection")
            enabled: details.valid
            onTriggered: collectionMenu.popup()
        },
        Kirigami.Action {
            icon.name: "split"
            text: i18nc("@action:button", "Split Book…")
            enabled: details.valid && details.tracks.length > 1
            onTriggered: page.splitRequested(details.bookId)
        },
        Kirigami.Action {
            icon.name: "checkmark"
            text: i18nc("@action:button", "Mark as Reviewed")
            enabled: details.valid && details.reviewReasons.length > 0
            onTriggered: {
                const error = page.library.dismissReview(details.bookId)
                if (error.length > 0) {
                    applicationWindow().showPassiveNotification(error)
                }
            }
        }
    ]

    GroupListModel {
        id: allCollections
        databasePath: page.library.databasePath
        revision: page.revision
        kind: 3
    }

    QQC2.Menu {
        id: collectionMenu
        Instantiator {
            model: allCollections
            delegate: QQC2.MenuItem {
                required property var groupId
                required property string name
                text: name
                checkable: true
                checked: details.collections.some(c => c.id === groupId)
                onTriggered: {
                    const error = checked ? page.library.addToCollection(groupId, details.bookId)
                                          : page.library.removeFromCollection(groupId, details.bookId)
                    if (error.length > 0) {
                        applicationWindow().showPassiveNotification(error)
                    }
                }
            }
            onObjectAdded: (index, object) => collectionMenu.insertItem(index, object)
            onObjectRemoved: (index, object) => collectionMenu.removeItem(object)
        }
        QQC2.MenuSeparator {}
        QQC2.MenuItem {
            icon.name: "list-add"
            text: i18nc("@action:inmenu", "New Collection…")
            onTriggered: newCollectionDialog.open()
        }
    }

    Kirigami.PromptDialog {
        id: newCollectionDialog
        title: i18nc("@title:dialog", "New Collection")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onOpened: newCollectionName.forceActiveFocus()
        QQC2.TextField {
            id: newCollectionName
            placeholderText: i18nc("@info:placeholder", "Name")
            onAccepted: newCollectionDialog.accept()
        }
        onAccepted: {
            const id = page.library.createCollection(newCollectionName.text)
            if (id > 0) {
                page.library.addToCollection(id, details.bookId)
            }
            newCollectionName.text = ""
        }
    }

    BookmarkListModel {
        id: bookmarks
        databasePath: page.library.databasePath
        revision: page.revision
        bookId: page.bookId
    }

    Kirigami.Dialog {
        id: bookmarkDialog
        property var bookmarkId: 0
        function edit(id, title, note) {
            bookmarkId = id
            bookmarkTitle.text = title
            bookmarkNote.text = note
            open()
        }
        title: i18nc("@title:dialog", "Edit Bookmark")
        standardButtons: Kirigami.Dialog.Save | Kirigami.Dialog.Cancel
        padding: Kirigami.Units.largeSpacing
        ColumnLayout {
            QQC2.TextField {
                id: bookmarkTitle
                Layout.fillWidth: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 20
                placeholderText: i18nc("@info:placeholder", "Title")
            }
            QQC2.TextArea {
                id: bookmarkNote
                Layout.fillWidth: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 6
                placeholderText: i18nc("@info:placeholder", "Note")
                wrapMode: TextEdit.Wrap
            }
        }
        onAccepted: page.session.updateBookmark(bookmarkId, bookmarkTitle.text, bookmarkNote.text)
    }

    BookDetails {
        id: details
        databasePath: page.library.databasePath
        revision: page.revision
        bookId: page.bookId
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            type: Kirigami.MessageType.Warning
            visible: details.valid && !details.available
            text: i18n("This book's files are not available. Is the drive with your library connected?")
        }
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            type: Kirigami.MessageType.Information
            visible: details.valid && details.reviewReasons.length > 0
            text: i18n("This book may need a review:\n%1",
                       details.reviewReasons.map(c => "• " + ReviewReasons.describe(c, i18n)).join("\n"))
        }

        GridLayout {
            Layout.fillWidth: true
            columns: page.width > Kirigami.Units.gridUnit * 30 ? 2 : 1
            columnSpacing: Kirigami.Units.largeSpacing * 2
            rowSpacing: Kirigami.Units.largeSpacing

            BookCover {
                Layout.alignment: Qt.AlignTop | Qt.AlignHCenter
                Layout.preferredWidth: Kirigami.Units.gridUnit * 12
                Layout.preferredHeight: Layout.preferredWidth
                source: details.coverUrl
                available: details.available
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignTop
                spacing: Kirigami.Units.smallSpacing

                Kirigami.Heading {
                    Layout.fillWidth: true
                    text: details.title
                    wrapMode: Text.Wrap
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    visible: details.subtitle.length > 0
                    text: details.subtitle
                    wrapMode: Text.Wrap
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    visible: details.authorLinks.length > 0
                    text: i18nc("@info %1 is a list of links", "by %1", page.links(1, details.authorLinks))
                    textFormat: Text.StyledText
                    wrapMode: Text.Wrap
                    onLinkActivated: link => page.openLink(link)
                    HoverHandler { cursorShape: parent.hoveredLink ? Qt.PointingHandCursor : Qt.ArrowCursor }
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    visible: details.narratorLinks.length > 0
                    text: i18nc("@info %1 is a list of links", "Read by %1", page.links(2, details.narratorLinks))
                    textFormat: Text.StyledText
                    wrapMode: Text.Wrap
                    color: Kirigami.Theme.disabledTextColor
                    onLinkActivated: link => page.openLink(link)
                    HoverHandler { cursorShape: parent.hoveredLink ? Qt.PointingHandCursor : Qt.ArrowCursor }
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    visible: details.series.length > 0
                    text: i18nc("@info %1 is a link", "Series: %1", `<a href="3:${details.seriesId}">${page.escapeHtml(details.series)}</a>`)
                    textFormat: Text.StyledText
                    color: Kirigami.Theme.disabledTextColor
                    onLinkActivated: link => page.openLink(link)
                    HoverHandler { cursorShape: parent.hoveredLink ? Qt.PointingHandCursor : Qt.ArrowCursor }
                }
                Flow {
                    Layout.fillWidth: true
                    visible: details.collections.length > 0
                    spacing: Kirigami.Units.smallSpacing
                    Repeater {
                        model: details.collections
                        delegate: Kirigami.Chip {
                            required property var modelData
                            text: modelData.name
                            icon.name: "bookmarks"
                            checkable: false
                            closable: true
                            onClicked: page.groupRequested(4, modelData.id, modelData.name)
                            onRemoved: page.library.removeFromCollection(modelData.id, details.bookId)
                        }
                    }
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    text: [
                        TimeFormat.format(details.durationMs),
                        details.year > 0 ? String(details.year) : "",
                        details.genre
                    ].filter(s => s.length > 0).join(" · ")
                    color: Kirigami.Theme.disabledTextColor
                }

                QQC2.ProgressBar {
                    Layout.fillWidth: true
                    Layout.topMargin: Kirigami.Units.smallSpacing
                    visible: details.positionMs > 0 && !details.finished
                    from: 0
                    to: Math.max(1, details.durationMs)
                    value: details.positionMs
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    visible: details.finished || details.positionMs > 0
                    text: details.finished
                        ? i18nc("@info", "Finished")
                        : i18nc("@info", "%1 left", TimeFormat.format(Math.max(0, details.durationMs - details.positionMs)))
                    color: Kirigami.Theme.disabledTextColor
                }

                QQC2.Button {
                    Layout.topMargin: Kirigami.Units.largeSpacing
                    icon.name: details.finished ? "media-playlist-repeat" : "media-playback-start"
                    text: details.finished ? i18nc("@action:button", "Play Again")
                        : details.positionMs > 0 ? i18nc("@action:button", "Resume")
                        : i18nc("@action:button", "Play")
                    enabled: details.valid && details.available
                    onClicked: {
                        if (page.session.playBook(details.bookId)) {
                            page.nowPlayingRequested()
                        } else {
                            applicationWindow().showPassiveNotification(i18n("None of this book's files can be played."))
                        }
                    }
                }
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: details.description.length > 0
            text: details.description
            wrapMode: Text.Wrap
            textFormat: Text.PlainText
        }

        Kirigami.ListSectionHeader {
            Layout.fillWidth: true
            visible: details.chapters.length > 1
            text: i18ncp("@title:group", "%1 Chapter", "%1 Chapters", details.chapters.length)
        }
        // Chapter browser: click to play from a chapter.
        Repeater {
            model: details.chapters.length > 1 ? details.chapters : []
            delegate: QQC2.ItemDelegate {
                id: chapterRow
                required property var modelData
                Layout.fillWidth: true
                objectName: "chapterRow"
                enabled: details.available
                onClicked: page.play(modelData.startMs)
                QQC2.ToolTip.text: i18nc("@info:tooltip", "Play from here")
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                contentItem: RowLayout {
                    QQC2.Label {
                        Layout.fillWidth: true
                        text: chapterRow.modelData.title
                        elide: Text.ElideRight
                    }
                    QQC2.Label {
                        text: TimeFormat.format(chapterRow.modelData.startMs)
                        color: Kirigami.Theme.disabledTextColor
                    }
                }
            }
        }

        Kirigami.ListSectionHeader {
            Layout.fillWidth: true
            visible: bookmarks.count > 0
            text: i18ncp("@title:group", "%1 Bookmark", "%1 Bookmarks", bookmarks.count)
        }
        Repeater {
            model: bookmarks
            delegate: QQC2.ItemDelegate {
                id: bookmarkRow
                required property var bookmarkId
                required property string title
                required property string note
                required property real positionMs
                required property string chapterTitle
                Layout.fillWidth: true
                objectName: "bookmarkRow"
                enabled: details.available
                onClicked: page.play(positionMs)
                contentItem: RowLayout {
                    Kirigami.Icon {
                        source: "bookmark-new"
                        Layout.preferredWidth: Kirigami.Units.iconSizes.small
                        Layout.preferredHeight: Layout.preferredWidth
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0
                        QQC2.Label {
                            Layout.fillWidth: true
                            text: bookmarkRow.title.length > 0 ? bookmarkRow.title
                                : bookmarkRow.chapterTitle.length > 0 ? bookmarkRow.chapterTitle
                                : i18nc("@info untitled bookmark", "Bookmark")
                            elide: Text.ElideRight
                        }
                        QQC2.Label {
                            Layout.fillWidth: true
                            text: [TimeFormat.format(bookmarkRow.positionMs),
                                   bookmarkRow.title.length > 0 ? bookmarkRow.chapterTitle : "",
                                   bookmarkRow.note].filter(s => s.length > 0).join(" · ")
                            elide: Text.ElideRight
                            color: Kirigami.Theme.disabledTextColor
                            font: Kirigami.Theme.smallFont
                        }
                    }
                    QQC2.ToolButton {
                        icon.name: "document-edit"
                        text: i18nc("@action:button", "Edit bookmark")
                        display: QQC2.AbstractButton.IconOnly
                        onClicked: bookmarkDialog.edit(bookmarkRow.bookmarkId, bookmarkRow.title, bookmarkRow.note)
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                    }
                    QQC2.ToolButton {
                        icon.name: "edit-delete"
                        text: i18nc("@action:button", "Delete bookmark")
                        display: QQC2.AbstractButton.IconOnly
                        onClicked: page.session.deleteBookmark(bookmarkRow.bookmarkId)
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                    }
                }
            }
        }

        Kirigami.ListSectionHeader {
            Layout.fillWidth: true
            text: i18ncp("@title:group", "%1 File", "%1 Files", details.tracks.length)
        }
        Repeater {
            model: details.tracks
            delegate: RowLayout {
                required property var modelData
                Layout.fillWidth: true
                Kirigami.Icon {
                    Layout.preferredWidth: Kirigami.Units.iconSizes.small
                    Layout.preferredHeight: Layout.preferredWidth
                    source: modelData.missing ? "dialog-warning" : "audio-x-generic"
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    text: modelData.missing ? i18n("%1 (missing)", modelData.fileName) : modelData.fileName
                    elide: Text.ElideMiddle
                    color: modelData.missing ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                    QQC2.ToolTip.text: modelData.path
                    QQC2.ToolTip.visible: hoverHandler.hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    HoverHandler { id: hoverHandler }
                }
                QQC2.Label {
                    text: TimeFormat.format(modelData.durationMs)
                    color: Kirigami.Theme.disabledTextColor
                }
            }
        }
    }
}
