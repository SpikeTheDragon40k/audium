import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.audium.bridge

Kirigami.ScrollablePage {
    id: page

    required property LibraryController library
    required property PlaybackSession session

    objectName: "settingsPage"
    // In a wide window the content keeps a readable width, centred.
    readonly property real contentMaxWidth: Kirigami.Units.gridUnit * 45
    leftPadding: Math.max(Kirigami.Units.gridUnit, (width - contentMaxWidth) / 2)
    rightPadding: leftPadding
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
        // Wraps onto more lines in a narrow window.
        Flow {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing
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
            text: i18nc("@title:group", "Playback")
        }
        Kirigami.FormLayout {
            Layout.fillWidth: true
            QQC2.SpinBox {
                Kirigami.FormData.label: i18nc("@label:spinbox", "Skip back:")
                from: 1
                to: 600
                value: Math.round(page.session.skipBackMs / 1000)
                textFromValue: (v, locale) => i18ncp("@item:spinbox", "%1 second", "%1 seconds", v)
                valueFromText: (text, locale) => parseInt(text, 10)
                onValueModified: page.session.skipBackMs = value * 1000
            }
            QQC2.SpinBox {
                Kirigami.FormData.label: i18nc("@label:spinbox", "Skip forward:")
                from: 1
                to: 600
                value: Math.round(page.session.skipForwardMs / 1000)
                textFromValue: (v, locale) => i18ncp("@item:spinbox", "%1 second", "%1 seconds", v)
                valueFromText: (text, locale) => parseInt(text, 10)
                onValueModified: page.session.skipForwardMs = value * 1000
            }
            QQC2.Label {
                Kirigami.FormData.label: i18nc("@label", "Speed:")
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: i18n("Each book remembers its own speed.")
                color: Kirigami.Theme.disabledTextColor
            }
            QQC2.CheckBox {
                Kirigami.FormData.label: i18nc("@label", "Closing the window:")
                text: i18nc("@option:check", "Keep playing in the system tray")
                checked: page.session.closeToTray
                onToggled: page.session.closeToTray = checked
            }
        }

        Kirigami.Heading {
            level: 2
            text: i18nc("@title:group", "Appearance")
        }
        Kirigami.FormLayout {
            Layout.fillWidth: true
            QQC2.ComboBox {
                objectName: "themeBox"
                Kirigami.FormData.label: i18nc("@label:listbox", "Colours:")
                // Same order as PlaybackSession.theme.
                model: [
                    i18nc("@item:inlistbox colour theme", "System (Breeze)"),
                    i18nc("@item:inlistbox colour theme", "Dracula"),
                    i18nc("@item:inlistbox colour theme", "Nord")
                ]
                currentIndex: page.session.theme
                onActivated: index => page.session.theme = index
            }
        }

        Kirigami.Heading {
            level: 2
            text: i18nc("@title:group", "Your Data")
        }
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: i18n("Export saves your progress, bookmarks, listening history, edits, collections and settings to a file you can import here or on another computer. Books are recognised by their content, not by where the files are.")
            color: Kirigami.Theme.disabledTextColor
        }
        Flow {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing
            QQC2.Button {
                icon.name: "document-export"
                text: i18nc("@action:button", "Export…")
                enabled: page.library.ready
                onClicked: exportDialog.open()
            }
            QQC2.Button {
                icon.name: "document-import"
                text: i18nc("@action:button", "Import…")
                enabled: page.library.ready
                onClicked: importDialog.open()
            }
            QQC2.Button {
                icon.name: "document-save"
                text: i18nc("@action:button", "Back Up Database…")
                enabled: page.library.ready
                QQC2.ToolTip.text: i18nc("@info:tooltip", "Save a complete copy of Audium's library database")
                QQC2.ToolTip.visible: hovered
                onClicked: backupDialog.open()
            }
            QQC2.Button {
                objectName: "resetStatisticsButton"
                icon.name: "edit-clear-history"
                text: i18nc("@action:button", "Reset Statistics…")
                enabled: page.library.ready
                onClicked: resetStatisticsDialog.open()
            }
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

    function report(error, success) {
        applicationWindow().showPassiveNotification(error.length > 0 ? error : success)
    }

    FileDialog {
        id: exportDialog
        title: i18nc("@title:window", "Export Audium Data")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "json"
        nameFilters: [i18n("Audium export (%1)", "*.json")]
        currentFile: "audium-export-" + new Date().toISOString().slice(0, 10) + ".json"
        onAccepted: page.report(page.library.exportTo(selectedFile), i18n("Exported."))
    }
    FileDialog {
        id: backupDialog
        title: i18nc("@title:window", "Back Up Library Database")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "db"
        nameFilters: [i18n("SQLite database (%1)", "*.db")]
        currentFile: "audium-backup-" + new Date().toISOString().slice(0, 10) + ".db"
        onAccepted: page.report(page.library.backupDatabase(selectedFile), i18n("Database backed up."))
    }
    FileDialog {
        id: importDialog
        title: i18nc("@title:window", "Import Audium Data")
        fileMode: FileDialog.OpenFile
        nameFilters: [i18n("Audium export (%1)", "*.json"), i18n("All files (%1)", "*")]
        onAccepted: {
            const plan = page.library.previewImport(selectedFile)
            if (plan.error) {
                page.report(plan.error, "")
            } else {
                importPreview.plan = plan
                importPreview.open()
            }
        }
    }

    // Asks before deleting the listening history.
    Kirigami.PromptDialog {
        id: resetStatisticsDialog
        objectName: "resetStatisticsDialog"
        title: i18nc("@title:dialog", "Reset Statistics?")
        subtitle: i18n("All listening history will be deleted: time listened, streaks, the daily chart and the most listened books. Your progress, bookmarks and finished books are kept. This cannot be undone, unless you have an export or a database backup.")
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: Kirigami.Action {
            icon.name: "edit-clear-history"
            text: i18nc("@action:button", "Reset Statistics")
            onTriggered: {
                page.report(page.library.resetStatistics(), i18n("Statistics reset."))
                resetStatisticsDialog.close()
            }
        }
    }

    // Shows what an import will do before anything changes.
    Kirigami.Dialog {
        id: importPreview
        objectName: "importPreview"
        property var plan: ({})
        title: i18nc("@title:dialog", "Import")
        padding: Kirigami.Units.largeSpacing
        preferredWidth: Kirigami.Units.gridUnit * 26
        standardButtons: (plan.changes > 0 ? Kirigami.Dialog.Apply : 0) | Kirigami.Dialog.Cancel
        onApplied: {
            page.report(page.library.applyImport(), i18n("Imported."))
            close()
        }
        ColumnLayout {
            QQC2.Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: i18np("1 book in the file is in your library.", "%1 of %2 books in the file are in your library.",
                            importPreview.plan.matched || 0, importPreview.plan.booksInFile || 0)
            }
            Repeater {
                model: [
                    [importPreview.plan.progressUpdated, i18n("Listening positions to update")],
                    [importPreview.plan.progressKept, i18n("Positions kept because yours are newer")],
                    [importPreview.plan.bookmarksAdded, i18n("Bookmarks to add")],
                    [importPreview.plan.sessionsAdded, i18n("Listening sessions to add")],
                    [importPreview.plan.overridesUpdated, i18n("Metadata edits to apply")],
                    [importPreview.plan.overridesKept, i18n("Metadata edits kept because yours are newer")],
                    [importPreview.plan.speedsSet, i18n("Book speeds to set")],
                    [importPreview.plan.collectionsCreated, i18n("Collections to create")],
                    [importPreview.plan.collectionEntriesAdded, i18n("Books to add to collections")],
                    [importPreview.plan.settingsChanged, i18n("Settings to change")]
                ].filter(r => r[0] > 0)
                delegate: RowLayout {
                    required property var modelData
                    Layout.fillWidth: true
                    QQC2.Label {
                        Layout.fillWidth: true
                        text: modelData[1]
                    }
                    QQC2.Label {
                        text: modelData[0]
                        font.bold: true
                    }
                }
            }
            QQC2.Label {
                Layout.fillWidth: true
                visible: importPreview.plan.changes === 0
                wrapMode: Text.Wrap
                text: i18n("Nothing to change: your library already has everything in this file.")
            }
            QQC2.Label {
                Layout.fillWidth: true
                visible: (importPreview.plan.unmatched || []).length > 0
                wrapMode: Text.Wrap
                color: Kirigami.Theme.disabledTextColor
                text: i18n("Not in your library, skipped: %1", (importPreview.plan.unmatched || []).join(", "))
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
}
