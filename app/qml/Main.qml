import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.audium.bridge

Kirigami.ApplicationWindow {
    id: root

    required property PlaybackController player
    required property LibraryController library
    required property PlaybackSession session
    /// The tray icon (C++ TrayIcon), when there is one; not set in tests.
    property var tray: null
    /// True after the window was hidden to the tray instead of closing.
    property bool hiddenToTray: false

    readonly property int skipBackMs: session.skipBackMs
    readonly property int skipForwardMs: session.skipForwardMs
    readonly property real rateStep: 0.1
    // Library views reload when a scan or saved progress changed.
    readonly property int dataRevision: library.revision + session.revision

    // Removes the pages after `fromPage` and pushes a new page made from
    // `component`. Pages are created with the page stack as parent (pushing a
    // Component makes Kirigami parent it to a non-Item first, which Qt warns
    // about) and destroyed when they are removed again.
    function pushAfter(fromPage, component, properties) {
        while (pageStack.depth > 1 && pageStack.lastItem !== fromPage) {
            pageStack.pop()
        }
        const page = component.createObject(pageStack, properties)
        pageStack.push(page)
        return page
    }

    /// Closes a column page (book details, a group's books, review, Now
    /// Playing) and the pages to its right. In a wide window Kirigami gives
    /// such columns no working back button, so they carry a Close action.
    function closePage(page) {
        for (let i = 1; i < pageStack.depth; ++i) {
            if (pageStack.get(i) === page) {
                pageStack.pop(pageStack.get(i - 1))
                return
            }
        }
    }

    function showBook(bookId, fromPage) {
        const page = pushAfter(fromPage || libraryPage, bookDetailsComponent, {
            bookId: bookId,
            library: root.library,
            session: root.session,
            revision: Qt.binding(() => root.dataRevision)
        })
        page.nowPlayingRequested.connect(() => root.showNowPlaying())
        page.groupRequested.connect((kind, id, name) => root.showGroup(kind, id, name, page))
        page.editRequested.connect(id => root.showEditor(editMetadataComponent, id))
        page.splitRequested.connect(id => root.showEditor(splitBookComponent, id))
    }

    /// The books of an author (1), narrator (2), series (3) or collection (4).
    function showGroup(kind, groupId, name, fromPage) {
        const page = pushAfter(fromPage || libraryPage, booksComponent, {
            library: root.library,
            revision: Qt.binding(() => root.dataRevision),
            scopeKind: kind,
            scopeId: groupId,
            name: name
        })
        page.bookRequested.connect(id => root.showBook(id, page))
    }

    /// Metadata editor or split page for a book; closes itself when done.
    /// Metadata editor or split page for a book, as a layer over the window
    /// (with a back button); closes itself when done.
    function showEditor(component, bookId) {
        const page = pageStack.layers.push(component, {
            bookId: bookId,
            library: root.library,
            revision: Qt.binding(() => root.dataRevision)
        })
        page.done.connect(() => pageStack.layers.pop())
    }

    function showStats() {
        if (pageStack.layers.depth > 1) {
            return
        }
        const page = pageStack.layers.push(statsComponent, {
            library: root.library,
            revision: Qt.binding(() => root.dataRevision)
        })
        page.bookRequested.connect(id => {
            pageStack.layers.pop()
            root.showBook(id, libraryPage)
        })
    }

    function addBookmark() {
        if (root.session.addBookmark()) {
            root.showPassiveNotification(i18n("Bookmark added"), "short")
        } else {
            root.showPassiveNotification(i18n("Play a book from your library to add bookmarks."))
        }
    }

    function showReview() {
        const page = pushAfter(libraryPage, needsReviewComponent, {
            library: root.library,
            revision: Qt.binding(() => root.dataRevision)
        })
        page.bookRequested.connect(id => root.showBook(id, page))
    }

    // Settings is a layer over the window, with a back button. (As a column
    // beside the library, a wide window gave it no way to be closed.)
    function showSettings() {
        if (pageStack.layers.depth <= 1) {
            pageStack.layers.push(settingsComponent, {library: root.library, session: root.session})
        }
    }

    function showNowPlaying() {
        for (let i = 0; i < pageStack.depth; ++i) {
            if (pageStack.get(i) === nowPlayingPage) {
                pageStack.currentIndex = i
                return
            }
        }
        pageStack.push(nowPlayingPage)
    }

    title: player.hasMedia && player.trackTitle.length > 0
        ? i18nc("@title:window %1 is the current track", "%1 — Audium", player.trackTitle)
        : i18nc("@title:window", "Audium")
    // Closing the window while a book plays keeps it playing in the tray
    // (Settings → Playback); otherwise closing quits.
    onClosing: close => {
        if (root.session.closeToTray && root.tray && root.tray.available
                && root.player.state === PlaybackController.Playing) {
            close.accepted = false
            root.hiddenToTray = true
            root.hide()
        }
    }
    onVisibleChanged: if (visible) hiddenToTray = false

    minimumWidth: Kirigami.Units.gridUnit * 20
    // Below this the header, tabs and player leave no room for the books.
    minimumHeight: Kirigami.Units.gridUnit * 25
    /// Width of the column a page opens in, next to the library (book
    /// details, a group's books, Now Playing): about 40% of a wide window.
    readonly property int sideColumnWidth: Math.round(Math.max(Kirigami.Units.gridUnit * 20,
        Math.min(width * 0.4, Kirigami.Units.gridUnit * 34)))
    readonly property int defaultWidth: Kirigami.Units.gridUnit * 50
    readonly property int defaultHeight: Kirigami.Units.gridUnit * 35
    width: defaultWidth
    height: defaultHeight

    pageStack.defaultColumnWidth: sideColumnWidth
    pageStack.initialPage: LibraryPage {
        id: libraryPage
        // The library takes what the side columns leave (at least one
        // column's width; Kirigami scrolls when they don't all fit).
        Kirigami.ColumnView.fillWidth: true
        Kirigami.ColumnView.reservedSpace: Math.max(1, root.pageStack.depth - 1) * root.sideColumnWidth
        player: root.player
        library: root.library
        session: root.session
        revision: root.dataRevision
        onBookRequested: bookId => root.showBook(bookId, libraryPage)
        onGroupRequested: (kind, id, name) => root.showGroup(kind, id, name, libraryPage)
        onStatsRequested: root.showStats()
        onReviewRequested: root.showReview()
        onSettingsRequested: root.showSettings()
        onNowPlayingRequested: root.showNowPlaying()
    }

    Connections {
        target: root.pageStack
        function onPageRemoved(page) {
            // Delayed: Kirigami may still be creating the page's toolbar
            // asynchronously, and destroying it mid-way makes Qt warn.
            if (page !== libraryPage && page !== nowPlayingPage) {
                page.destroy(Kirigami.Units.humanMoment)
            }
        }
    }

    footer: MiniPlayer {
        player: root.player
        session: root.session
        skipBackMs: root.skipBackMs
        skipForwardMs: root.skipForwardMs
        onNowPlayingRequested: root.showNowPlaying()
    }

    Component {
        id: bookDetailsComponent
        BookDetailsPage {}
    }
    Component {
        id: needsReviewComponent
        NeedsReviewPage {}
    }
    Component {
        id: settingsComponent
        SettingsPage {}
    }
    Component {
        id: booksComponent
        BooksPage {}
    }
    Component {
        id: editMetadataComponent
        EditMetadataPage {}
    }
    Component {
        id: splitBookComponent
        SplitBookPage {}
    }
    Component {
        id: statsComponent
        StatsPage {}
    }

    // One persistent instance: it keeps its state when closed and reopened.
    // Created with the page stack as parent, like the other pages: declared
    // as a child of the window, it made Qt report binding loops when shown
    // in a narrow window.
    readonly property NowPlayingPage nowPlayingPage: nowPlayingComponent.createObject(pageStack, {
        visible: false,
        player: root.player,
        session: root.session,
        skipBackMs: Qt.binding(() => root.skipBackMs),
        skipForwardMs: Qt.binding(() => root.skipForwardMs)
    })
    Component {
        id: nowPlayingComponent
        NowPlayingPage {}
    }

    Shortcut {
        sequences: ["Space", "Media Play"]
        onActivated: root.player.togglePlayPause()
    }
    Shortcut {
        sequence: "Left"
        onActivated: root.player.skip(-root.skipBackMs)
    }
    Shortcut {
        sequence: "Right"
        onActivated: root.player.skip(root.skipForwardMs)
    }
    Shortcut {
        sequence: "["
        onActivated: root.player.rate = root.player.rate - root.rateStep
    }
    Shortcut {
        sequence: "]"
        onActivated: root.player.rate = root.player.rate + root.rateStep
    }
    // Placeholder until search arrives (Milestone 4).
    Shortcut {
        sequences: [StandardKey.Find]
        onActivated: {
            root.pageStack.currentIndex = 0
            libraryPage.focusSearch()
        }
    }
    Shortcut {
        sequence: "B"
        onActivated: root.addBookmark()
    }
    // Escape closes a layer (settings, editors, statistics), otherwise the
    // rightmost column page.
    Shortcut {
        sequence: "Escape"
        enabled: root.pageStack.layers.depth > 1 || root.pageStack.depth > 1
        onActivated: {
            if (root.pageStack.layers.depth > 1) {
                root.pageStack.layers.pop()
            } else {
                root.closePage(root.pageStack.lastItem)
            }
        }
    }
}
