import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.audium.bridge
import "../components/TimeFormat.js" as TimeFormat

// Listening statistics, from the recorded listening sessions.
Kirigami.ScrollablePage {
    id: page

    required property LibraryController library
    required property int revision

    signal bookRequested(var bookId)

    objectName: "statsPage"
    // In a wide window the content keeps a readable width, centred.
    readonly property real contentMaxWidth: Kirigami.Units.gridUnit * 60
    leftPadding: Math.max(Kirigami.Units.gridUnit, (width - contentMaxWidth) / 2)
    rightPadding: leftPadding
    title: i18nc("@title", "Listening Statistics")

    StatsModel {
        id: stats
        databasePath: page.library.databasePath
        revision: page.revision
        days: 30
    }

    component Tile: Kirigami.AbstractCard {
        id: tile
        property string label
        property string value
        Layout.fillWidth: true
        Layout.minimumWidth: Kirigami.Units.gridUnit * 8
        contentItem: ColumnLayout {
            spacing: 0
            QQC2.Label {
                Layout.fillWidth: true
                text: tile.label
                color: Kirigami.Theme.disabledTextColor
                elide: Text.ElideRight
            }
            Kirigami.Heading {
                Layout.fillWidth: true
                level: 2
                text: tile.value
                elide: Text.ElideRight
            }
        }
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Kirigami.PlaceholderMessage {
            Layout.fillWidth: true
            visible: stats.totalMs === 0
            icon.name: "office-chart-bar"
            text: i18n("No listening recorded yet")
            explanation: i18n("Statistics appear here once you have listened to books from your library.")
        }

        GridLayout {
            Layout.fillWidth: true
            visible: stats.totalMs > 0
            // 6, 3, 2 or 1 per row, so the six tiles fill whole rows.
            readonly property int fit: Math.floor((page.width - page.leftPadding - page.rightPadding) / (Kirigami.Units.gridUnit * 10))
            columns: fit >= 6 ? 6 : fit >= 3 ? 3 : Math.max(1, fit)
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing
            Tile { label: i18nc("@label stat", "Today"); value: TimeFormat.duration(stats.todayMs, i18nc) }
            Tile { label: i18nc("@label stat", "This week"); value: TimeFormat.duration(stats.weekMs, i18nc) }
            Tile { label: i18nc("@label stat", "This month"); value: TimeFormat.duration(stats.monthMs, i18nc) }
            Tile { label: i18nc("@label stat", "All time"); value: TimeFormat.duration(stats.totalMs, i18nc) }
            Tile {
                label: i18nc("@label stat", "Current streak")
                value: i18ncp("@info days in a row", "%1 day", "%1 days", stats.streakDays)
                QQC2.ToolTip.text: i18nc("@info:tooltip", "Longest streak: %1", i18ncp("@info", "%1 day", "%1 days", stats.bestStreakDays))
                QQC2.ToolTip.visible: hovered
            }
            Tile {
                label: i18nc("@label stat", "Books finished")
                value: i18nc("@info finished books: total (this month)", "%1 (%2 this month)", stats.finishedTotal, stats.finishedMonth)
            }
        }

        Kirigami.Heading {
            Layout.fillWidth: true
            visible: stats.totalMs > 0
            level: 3
            text: i18nc("@title:group", "Listening per day, last 30 days")
        }
        DailyChart {
            Layout.fillWidth: true
            visible: stats.totalMs > 0
            days: stats.daily
            maxMs: stats.maxDailyMs
        }

        Kirigami.ListSectionHeader {
            Layout.fillWidth: true
            visible: stats.books.length > 0
            text: i18nc("@title:group", "Most Listened")
        }
        Repeater {
            model: stats.books
            delegate: QQC2.ItemDelegate {
                id: row
                required property var modelData
                Layout.fillWidth: true
                objectName: "statsBookRow"
                onClicked: page.bookRequested(modelData.bookId)
                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing
                    BookCover {
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 2
                        Layout.preferredHeight: Layout.preferredWidth
                        source: row.modelData.coverUrl
                    }
                    QQC2.Label {
                        Layout.fillWidth: true
                        text: row.modelData.title
                        elide: Text.ElideRight
                    }
                    QQC2.Label {
                        text: TimeFormat.duration(row.modelData.ms, i18nc)
                        color: Kirigami.Theme.disabledTextColor
                    }
                }
            }
        }
    }
}
