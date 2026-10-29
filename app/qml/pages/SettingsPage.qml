import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.audium.bridge

Kirigami.ScrollablePage {
    id: page

    required property LibraryController library

    objectName: "settingsPage"
    title: i18nc("@title", "Settings")

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Heading {
            level: 2
            text: i18nc("@title:group", "Library Folders")
        }
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: i18n("Audium only reads these folders. It never changes, moves, renames or deletes your audio files.")
            color: Kirigami.Theme.disabledTextColor
        }
        Repeater {
            model: page.library.folders
            delegate: RowLayout {
                required property int index
                required property string modelData
                Layout.fillWidth: true
                Kirigami.Icon {
                    source: "folder-sound"
                    Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                    Layout.preferredHeight: Layout.preferredWidth
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    text: modelData
                    elide: Text.ElideMiddle
                }
                QQC2.ToolButton {
                    icon.name: "list-remove"
                    text: i18nc("@action:button", "Remove")
                    display: QQC2.AbstractButton.IconOnly
                    QQC2.ToolTip.text: i18nc("@info:tooltip", "Remove from the library (files are not touched)")
                    QQC2.ToolTip.visible: hovered
                    onClicked: page.library.removeFolder(index)
                }
            }
        }
        RowLayout {
            QQC2.Button {
                icon.name: "folder-new"
                text: i18nc("@action:button", "Add Folder…")
                enabled: page.library.ready
                onClicked: folderDialog.open()
            }
            QQC2.Button {
                icon.name: "view-refresh"
                text: i18nc("@action:button", "Rescan")
                enabled: page.library.ready && !page.library.scanning && page.library.folders.length > 0
                onClicked: page.library.rescan()
            }
            QQC2.Button {
                icon.name: "view-refresh"
                text: i18nc("@action:button", "Re-read Everything")
                enabled: page.library.ready && !page.library.scanning && page.library.folders.length > 0
                QQC2.ToolTip.text: i18nc("@info:tooltip", "Also re-read folders that did not change, e.g. after editing desc.txt or metadata.json")
                QQC2.ToolTip.visible: hovered
                onClicked: page.library.fullRescan()
            }
        }
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: page.library.scanStatus
            visible: text.length > 0
        }

        Kirigami.Heading {
            level: 2
            text: i18nc("@title:group", "About")
        }
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: i18n("Audium %1 — offline audiobook player.\nLibrary database: %2", AppInfo.version, page.library.databasePath)
            textFormat: Text.PlainText
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
}
