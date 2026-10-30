import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.audium.app
import org.audium.bridge
import "TimeFormat.js" as TimeFormat

// Persistent player bar shown at the bottom of every page.
QQC2.ToolBar {
    id: bar

    required property PlaybackController player
    required property PlaybackSession session
    required property int skipBackMs
    required property int skipForwardMs

    signal nowPlayingRequested()

    visible: player.hasMedia
    position: QQC2.ToolBar.Footer

    contentItem: RowLayout {
        spacing: Kirigami.Units.smallSpacing

        QQC2.ToolButton {
            icon.name: "media-seek-backward"
            text: i18nc("@action:button", "Back %1 s", bar.skipBackMs / 1000)
            display: QQC2.AbstractButton.IconOnly
            focusPolicy: Qt.NoFocus
            onClicked: bar.player.skip(-bar.skipBackMs)
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        }
        QQC2.ToolButton {
            icon.name: bar.player.state === PlaybackController.Playing ? "media-playback-pause" : "media-playback-start"
            text: bar.player.state === PlaybackController.Playing ? i18nc("@action:button", "Pause") : i18nc("@action:button", "Play")
            display: QQC2.AbstractButton.IconOnly
            focusPolicy: Qt.NoFocus
            enabled: bar.player.state !== PlaybackController.Error
            onClicked: bar.player.togglePlayPause()
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        }
        QQC2.ToolButton {
            icon.name: "media-seek-forward"
            text: i18nc("@action:button", "Forward %1 s", bar.skipForwardMs / 1000)
            display: QQC2.AbstractButton.IconOnly
            focusPolicy: Qt.NoFocus
            onClicked: bar.player.skip(bar.skipForwardMs)
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        }

        QQC2.ItemDelegate {
            Layout.fillWidth: true
            focusPolicy: Qt.NoFocus
            onClicked: bar.nowPlayingRequested()
            contentItem: ColumnLayout {
                spacing: 0
                QQC2.Label {
                    Layout.fillWidth: true
                    text: {
                        const part = bar.player.chapterTitle.length > 0 ? bar.player.chapterTitle : bar.player.trackTitle
                        return bar.session.bookTitle.length > 0 && bar.session.bookTitle !== part
                            ? i18nc("@info book title — chapter", "%1 — %2", bar.session.bookTitle, part)
                            : part
                    }
                    elide: Text.ElideRight
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    text: bar.player.state === PlaybackController.Error
                        ? bar.player.errorString
                        : i18nc("@info position / duration", "%1 / %2",
                                TimeFormat.format(bar.player.position), TimeFormat.format(bar.player.duration))
                    color: bar.player.state === PlaybackController.Error ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.disabledTextColor
                    font: Kirigami.Theme.smallFont
                    elide: Text.ElideRight
                }
            }
        }

        SleepTimerButton {
            session: bar.session
            // Only for library books: it needs the session's chapters.
            enabledForBook: bar.session.bookId > 0
        }

        QQC2.Label {
            visible: !Kirigami.Settings.isMobile && Math.abs(bar.player.rate - 1.0) > 0.001
            text: i18nc("@info playback speed", "%1×", bar.player.rate.toLocaleString(Qt.locale(), "f", 2))
        }
    }
}
