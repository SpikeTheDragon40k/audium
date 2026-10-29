import QtQuick
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami
import org.audium.app
import org.audium.bridge

// Placeholder until the library arrives in Milestone 2.
Kirigami.Page {
    id: page

    required property PlaybackController player

    signal nowPlayingRequested()

    objectName: "libraryPage"
    title: i18nc("@title", "Library")

    actions: [
        Kirigami.Action {
            icon.name: "document-open"
            text: i18nc("@action:button", "Open Files…")
            tooltip: i18nc("@info:tooltip", "Temporary: play files without a library")
            onTriggered: fileDialog.open()
        }
    ]

    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        width: parent.width - Kirigami.Units.gridUnit * 4
        icon.name: "audiobook"
        text: i18n("Audium %1", AppInfo.version)
        explanation: i18n("The library is not implemented yet. Use “Open Files…” to play audio files as one book.")
        helpfulAction: Kirigami.Action {
            icon.name: "document-open"
            text: i18nc("@action:button", "Open Files…")
            onTriggered: fileDialog.open()
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
