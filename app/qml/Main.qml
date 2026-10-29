import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.audium.app
import org.audium.bridge

Kirigami.ApplicationWindow {
    id: root

    required property PlaybackController player
    required property LibraryController library
    required property LibraryPlayback libraryPlayback

    // Skip intervals; configurable in Milestone 3.
    readonly property int skipBackMs: 15000
    readonly property int skipForwardMs: 30000
    readonly property real rateStep: 0.1

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

    function showBook(bookId, fromPage) {
        const page = pushAfter(fromPage || libraryPage, bookDetailsComponent, {
            bookId: bookId,
            library: root.library,
            libraryPlayback: root.libraryPlayback
        })
        page.nowPlayingRequested.connect(() => root.showNowPlaying())
    }

    function showReview() {
        const page = pushAfter(libraryPage, needsReviewComponent, {library: root.library})
        page.bookRequested.connect(id => root.showBook(id, page))
    }

    function showSettings() {
        pushAfter(libraryPage, settingsComponent, {library: root.library})
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
    minimumWidth: Kirigami.Units.gridUnit * 20
    minimumHeight: Kirigami.Units.gridUnit * 20
    width: Kirigami.Units.gridUnit * 50
    height: Kirigami.Units.gridUnit * 35

    pageStack.initialPage: LibraryPage {
        id: libraryPage
        player: root.player
        library: root.library
        onBookRequested: bookId => root.showBook(bookId, libraryPage)
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

    // One persistent instance: it keeps its state when closed and reopened.
    NowPlayingPage {
        id: nowPlayingPage
        visible: false
        player: root.player
        skipBackMs: root.skipBackMs
        skipForwardMs: root.skipForwardMs
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
}
