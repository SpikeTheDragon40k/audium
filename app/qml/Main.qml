import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.audium.app

Kirigami.ApplicationWindow {
    id: root

    required property PlaybackController player

    // Skip intervals; configurable in Milestone 3.
    readonly property int skipBackMs: 15000
    readonly property int skipForwardMs: 30000
    readonly property real rateStep: 0.1

    function showNowPlaying() {
        if (pageStack.depth < 2) {
            pageStack.push(nowPlayingPage)
        }
    }

    title: player.hasMedia && player.trackTitle.length > 0
        ? i18nc("@title:window %1 is the current track", "%1 — Audium", player.trackTitle)
        : i18nc("@title:window", "Audium")
    minimumWidth: Kirigami.Units.gridUnit * 20
    minimumHeight: Kirigami.Units.gridUnit * 20
    width: Kirigami.Units.gridUnit * 50
    height: Kirigami.Units.gridUnit * 35

    pageStack.initialPage: LibraryPage {
        player: root.player
        onNowPlayingRequested: root.showNowPlaying()
    }

    footer: MiniPlayer {
        player: root.player
        skipBackMs: root.skipBackMs
        skipForwardMs: root.skipForwardMs
        onNowPlayingRequested: root.showNowPlaying()
    }

    // One persistent instance: it keeps its state when closed and reopened.
    // (Pushing a Component makes Kirigami parent the page to a non-Item first,
    // which Qt warns about.)
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
