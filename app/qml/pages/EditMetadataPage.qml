import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.audium.bridge

// Edits a book's metadata. Changes are stored as overrides in Audium's
// database; the audio files are never written.
Kirigami.ScrollablePage {
    id: page

    required property var bookId
    required property LibraryController library
    required property int revision

    signal done()

    objectName: "editMetadataPage"
    title: i18nc("@title", "Edit Metadata")

    BookDetails {
        id: details
        databasePath: page.library.databasePath
        revision: page.revision
        bookId: page.bookId
    }

    // field -> {label, multiline, current value as shown}
    readonly property var fields: [
        {key: "title", label: i18nc("@label:textbox", "Title:"), value: details.title},
        {key: "subtitle", label: i18nc("@label:textbox", "Subtitle:"), value: details.subtitle},
        {key: "authors", label: i18nc("@label:textbox", "Authors (one per line):"), value: details.authorLinks.map(a => a.name).join("\n"), lines: true},
        {key: "narrators", label: i18nc("@label:textbox", "Narrators (one per line):"), value: details.narratorLinks.map(a => a.name).join("\n"), lines: true},
        {key: "series", label: i18nc("@label:textbox", "Series:"), value: seriesName()},
        {key: "series_index", label: i18nc("@label:textbox", "Number in series:"), value: seriesIndex()},
        {key: "year", label: i18nc("@label:textbox", "Year:"), value: details.year > 0 ? String(details.year) : ""},
        {key: "genre", label: i18nc("@label:textbox", "Genre:"), value: details.genre},
        {key: "description", label: i18nc("@label:textbox", "Description:"), value: details.description, lines: true}
    ]

    function seriesName() {
        const s = details.series
        const i = s.lastIndexOf(" #")
        return i > 0 ? s.substring(0, i) : s
    }
    function seriesIndex() {
        const s = details.series
        const i = s.lastIndexOf(" #")
        return i > 0 ? s.substring(i + 2) : ""
    }

    actions: [
        Kirigami.Action {
            icon.name: "document-save"
            text: i18nc("@action:button", "Save")
            enabled: details.valid
            onTriggered: page.save()
        }
    ]

    function save() {
        const keys = []
        const values = []
        for (let i = 0; i < form.count; ++i) {
            const field = form.itemAt(i)
            if (field.edited) {
                keys.push(field.key)
                // An emptied field removes the override (back to the tags).
                values.push(field.text)
            }
        }
        if (keys.length > 0) {
            const error = page.library.editMetadata(page.bookId, keys, values)
            if (error.length > 0) {
                applicationWindow().showPassiveNotification(error)
                return
            }
            applicationWindow().showPassiveNotification(i18n("Saved. The book is being updated."))
        }
        page.done()
    }

    ColumnLayout {
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: i18n("Your changes are kept in Audium's library and never written to the audio files. Empty a field to use what the files say again.")
            color: Kirigami.Theme.disabledTextColor
        }
        Kirigami.FormLayout {
            Layout.fillWidth: true
            Repeater {
                id: form
                model: page.fields
                delegate: Loader {
                    id: field
                    required property var modelData
                    readonly property string key: modelData.key
                    readonly property string text: item ? item.text : ""
                    readonly property bool edited: item ? item.text !== modelData.value : false
                    Kirigami.FormData.label: modelData.label
                    Layout.fillWidth: true
                    sourceComponent: modelData.lines ? area : line
                    Component {
                        id: line
                        QQC2.TextField {
                            text: field.modelData.value
                            font.italic: Object.keys(details.overrides).includes(field.key)
                        }
                    }
                    Component {
                        id: area
                        QQC2.TextArea {
                            text: field.modelData.value
                            wrapMode: TextEdit.Wrap
                            Layout.minimumHeight: Kirigami.Units.gridUnit * 3
                            font.italic: Object.keys(details.overrides).includes(field.key)
                        }
                    }
                }
            }
        }
        QQC2.Label {
            Layout.fillWidth: true
            visible: Object.keys(details.overrides).length > 0
            wrapMode: Text.Wrap
            text: i18n("Fields in italics have been edited before.")
            color: Kirigami.Theme.disabledTextColor
            font: Kirigami.Theme.smallFont
        }
    }
}
