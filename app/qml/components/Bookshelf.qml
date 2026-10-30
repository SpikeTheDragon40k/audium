import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.audium.bridge

// Covers standing on shelves, one shelf per series, author or collection,
// with listening progress under each cover. Grouping by series ends with a
// shelf of the books in no series.
ListView {
    id: shelves

    required property string databasePath
    required property int revision
    /// 0 series, 1 authors, 2 collections
    property int grouping: 0

    signal bookRequested(var bookId)

    objectName: "bookshelfView"
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    QQC2.ScrollBar.vertical: QQC2.ScrollBar {}
    // Shelves and covers are recycled while scrolling, as in the grid.
    reuseItems: true
    spacing: Kirigami.Units.largeSpacing

    model: GroupListModel {
        databasePath: shelves.databasePath
        revision: shelves.revision
        kind: shelves.grouping === 0 ? 2 : shelves.grouping === 1 ? 0 : 3
    }

    footer: shelves.grouping === 0 ? looseShelf : null

    Component {
        id: looseShelf
        Shelf {
            width: shelves.width
            shelfName: i18nc("@title:group", "Not in a Series")
            scopeKind: 5
            scopeId: 0
        }
    }

    delegate: Shelf {
        required property var groupId
        required property string name
        width: ListView.view.width
        scopeKind: shelves.grouping === 0 ? 3 : shelves.grouping === 1 ? 1 : 4
        scopeId: groupId
        shelfName: name
    }

    component Shelf: ColumnLayout {
        id: shelf
        property string shelfName
        property int scopeKind
        property var scopeId
        visible: books.count > 0
        spacing: 0

        // The database path is set last, once the scope is: every property
        // change reloads the model, and before the scope arrives that would
        // be the whole library (twice per shelf, with many shelves).
        BookListModel {
            id: books
            revision: shelves.revision
            scopeKind: shelf.scopeKind
            scopeId: shelf.scopeId
        }
        Component.onCompleted: books.databasePath = Qt.binding(() => shelves.databasePath)
        Kirigami.Heading {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            level: 3
            text: shelf.shelfName
            elide: Text.ElideRight
        }
        ListView {
            Layout.fillWidth: true
            // A cover (the tile's width) and its progress bar.
            Layout.preferredHeight: Kirigami.Units.gridUnit * 7 + Kirigami.Units.smallSpacing * 4
            orientation: ListView.Horizontal
            model: books
            clip: true
            reuseItems: true
            spacing: Kirigami.Units.largeSpacing
            leftMargin: Kirigami.Units.largeSpacing
            rightMargin: Kirigami.Units.largeSpacing
            QQC2.ScrollBar.horizontal: QQC2.ScrollBar {}
            delegate: QQC2.ItemDelegate {
                id: standing
                required property var bookId
                required property string title
                required property url coverUrl
                required property bool available
                required property real progress
                required property bool finished
                objectName: "bookTile"
                width: Kirigami.Units.gridUnit * 7
                height: ListView.view.height
                onClicked: shelves.bookRequested(bookId)
                QQC2.ToolTip.text: title
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                contentItem: ColumnLayout {
                    spacing: Kirigami.Units.smallSpacing
                    Item { Layout.fillHeight: true }
                    BookCover {
                        Layout.fillWidth: true
                        Layout.preferredHeight: width
                        source: standing.coverUrl
                        available: standing.available
                    }
                    QQC2.ProgressBar {
                        Layout.fillWidth: true
                        from: 0
                        to: 1
                        value: standing.finished ? 1 : standing.progress
                        opacity: standing.progress > 0 || standing.finished ? 1 : 0.25
                    }
                }
            }
        }
        // The shelf board the covers stand on.
        Rectangle {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            Layout.preferredHeight: Kirigami.Units.smallSpacing * 2
            radius: Kirigami.Units.cornerRadius
            color: Kirigami.Theme.alternateBackgroundColor
            border.color: Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.15)
        }
    }
}
