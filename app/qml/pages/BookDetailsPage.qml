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

    objectName: "bookDetailsPage"
    title: details.valid ? details.title : i18nc("@title", "Book")

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
                    visible: details.authors.length > 0
                    text: i18nc("@info", "by %1", details.authors)
                    wrapMode: Text.Wrap
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    visible: details.narrators.length > 0
                    text: i18nc("@info", "Read by %1", details.narrators)
                    wrapMode: Text.Wrap
                    color: Kirigami.Theme.disabledTextColor
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    visible: details.series.length > 0
                    text: i18nc("@info", "Series: %1", details.series)
                    color: Kirigami.Theme.disabledTextColor
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
        Repeater {
            model: details.chapters.length > 1 ? details.chapters : []
            delegate: RowLayout {
                required property var modelData
                Layout.fillWidth: true
                QQC2.Label {
                    Layout.fillWidth: true
                    text: modelData.title
                    elide: Text.ElideRight
                }
                QQC2.Label {
                    text: TimeFormat.format(modelData.startMs)
                    color: Kirigami.Theme.disabledTextColor
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
