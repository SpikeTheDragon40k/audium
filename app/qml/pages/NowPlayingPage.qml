import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.audium.app
import org.audium.bridge
import "../components/TimeFormat.js" as TimeFormat

Kirigami.ScrollablePage {
    id: page

    required property PlaybackController player
    required property PlaybackSession session
    required property int skipBackMs
    required property int skipForwardMs

    objectName: "nowPlayingPage"
    title: i18nc("@title", "Now Playing")

    header: Kirigami.InlineMessage {
        position: Kirigami.InlineMessage.Header
        type: page.player.state === PlaybackController.Error ? Kirigami.MessageType.Error : Kirigami.MessageType.Warning
        text: page.player.errorString
        visible: text.length > 0
    }

    ListView {
        id: chapterList

        model: page.player.chapters
        currentIndex: page.player.chapterIndex

        header: ColumnLayout {
            width: chapterList.width
            spacing: Kirigami.Units.largeSpacing

            BookCover {
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: Kirigami.Units.largeSpacing
                Layout.preferredWidth: Kirigami.Units.gridUnit * 10
                Layout.preferredHeight: Layout.preferredWidth
                visible: page.session.bookId > 0
                source: page.session.coverUrl
            }
            QQC2.Label {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                visible: page.session.bookTitle.length > 0
                text: page.session.bookTitle
                wrapMode: Text.Wrap
                color: Kirigami.Theme.disabledTextColor
            }
            Kirigami.Heading {
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.largeSpacing
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: page.player.chapterTitle.length > 0 ? page.player.chapterTitle : page.player.trackTitle
            }
            QQC2.Label {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                visible: page.player.trackCount > 1
                text: i18nc("@info", "Track %1 of %2", page.player.trackIndex + 1, page.player.trackCount)
                color: Kirigami.Theme.disabledTextColor
            }

            QQC2.Slider {
                id: seekSlider
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                from: 0
                to: Math.max(1, page.player.duration)
                focusPolicy: Qt.NoFocus
                enabled: page.player.hasMedia && page.player.duration > 0
                onPressedChanged: if (!pressed) page.player.seekBook(value)

                Binding {
                    seekSlider.value: page.player.position
                    when: !seekSlider.pressed
                    restoreMode: Binding.RestoreNone
                }
            }
            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                QQC2.Label {
                    text: TimeFormat.format(seekSlider.pressed ? seekSlider.value : page.player.position)
                }
                Item { Layout.fillWidth: true }
                QQC2.Label {
                    text: i18nc("@info remaining time", "−%1",
                                TimeFormat.format(page.player.duration - page.player.position))
                }
            }

            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: Kirigami.Units.largeSpacing

                QQC2.ToolButton {
                    icon.name: "media-skip-backward"
                    text: i18nc("@action:button", "Previous chapter")
                    display: QQC2.AbstractButton.IconOnly
                    focusPolicy: Qt.NoFocus
                    onClicked: page.player.previousChapter()
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                }
                QQC2.ToolButton {
                    icon.name: "media-seek-backward"
                    text: i18nc("@action:button", "Back %1 s", page.skipBackMs / 1000)
                    display: QQC2.AbstractButton.IconOnly
                    focusPolicy: Qt.NoFocus
                    onClicked: page.player.skip(-page.skipBackMs)
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                }
                QQC2.RoundButton {
                    icon.name: page.player.state === PlaybackController.Playing ? "media-playback-pause" : "media-playback-start"
                    icon.width: Kirigami.Units.iconSizes.medium
                    icon.height: Kirigami.Units.iconSizes.medium
                    text: page.player.state === PlaybackController.Playing ? i18nc("@action:button", "Pause") : i18nc("@action:button", "Play")
                    display: QQC2.AbstractButton.IconOnly
                    focusPolicy: Qt.NoFocus
                    enabled: page.player.hasMedia && page.player.state !== PlaybackController.Error
                    onClicked: page.player.togglePlayPause()
                }
                QQC2.ToolButton {
                    icon.name: "media-seek-forward"
                    text: i18nc("@action:button", "Forward %1 s", page.skipForwardMs / 1000)
                    display: QQC2.AbstractButton.IconOnly
                    focusPolicy: Qt.NoFocus
                    onClicked: page.player.skip(page.skipForwardMs)
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                }
                QQC2.ToolButton {
                    icon.name: "media-skip-forward"
                    text: i18nc("@action:button", "Next chapter")
                    display: QQC2.AbstractButton.IconOnly
                    focusPolicy: Qt.NoFocus
                    onClicked: page.player.nextChapter()
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                }
                SleepTimerButton {
                    session: page.session
                    enabledForBook: page.session.bookId > 0
                }
            }

            Kirigami.FormLayout {
                Layout.fillWidth: true

                QQC2.SpinBox {
                    id: speedBox
                    Kirigami.FormData.label: i18nc("@label:spinbox", "Speed:")
                    from: Math.round(page.player.minRate * 100)
                    to: Math.round(page.player.maxRate * 100)
                    stepSize: 5
                    value: Math.round(page.player.rate * 100)
                    editable: true
                    textFromValue: (v, locale) => i18nc("@item:spinbox playback speed", "%1×", (v / 100).toLocaleString(locale, "f", 2))
                    valueFromText: (text, locale) => Math.round(Number.fromLocaleString(locale, text.replace("×", "").trim()) * 100)
                    onValueModified: page.player.rate = value / 100
                }
                RowLayout {
                    Kirigami.FormData.label: i18nc("@label:slider", "Volume:")
                    QQC2.Slider {
                        id: volumeSlider
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 10
                        from: 0
                        to: page.player.maxVolume
                        stepSize: 1
                        focusPolicy: Qt.NoFocus
                        value: page.player.volume
                        onMoved: page.player.volume = value
                    }
                    QQC2.Label {
                        text: i18nc("@info volume percentage", "%1%", page.player.volume)
                    }
                }
            }

            Kirigami.ListSectionHeader {
                Layout.fillWidth: true
                visible: chapterList.count > 0
                text: i18nc("@title:group", "Chapters")
            }
        }

        delegate: QQC2.ItemDelegate {
            id: chapterDelegate
            required property int index
            required property var modelData

            width: ListView.view.width
            highlighted: index === page.player.chapterIndex
            onClicked: page.player.seekChapter(index)
            contentItem: RowLayout {
                QQC2.Label {
                    Layout.fillWidth: true
                    text: chapterDelegate.modelData.title.length > 0
                        ? chapterDelegate.modelData.title
                        : i18nc("@item untitled chapter", "Chapter %1", chapterDelegate.index + 1)
                    elide: Text.ElideRight
                    font.bold: chapterDelegate.highlighted
                }
                QQC2.Label {
                    text: TimeFormat.format(chapterDelegate.modelData.startMs)
                    color: Kirigami.Theme.disabledTextColor
                }
            }
        }
    }
}
