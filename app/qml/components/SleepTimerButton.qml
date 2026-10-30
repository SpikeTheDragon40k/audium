import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.audium.bridge
import "TimeFormat.js" as TimeFormat

// Sleep timer menu; shows the time left while the timer runs.
QQC2.ToolButton {
    id: button

    required property PlaybackSession session
    property bool enabledForBook: true

    readonly property string remainingText: session.sleepEndOfChapter
        ? i18nc("@info sleep timer", "End of chapter")
        : TimeFormat.format(session.sleepRemainingMs)

    icon.name: "clock"
    text: session.sleepActive ? remainingText : i18nc("@action:button", "Sleep Timer")
    display: session.sleepActive ? QQC2.AbstractButton.TextBesideIcon : QQC2.AbstractButton.IconOnly
    checkable: false
    focusPolicy: Qt.NoFocus
    enabled: enabledForBook
    onClicked: menu.popup()

    QQC2.ToolTip.text: session.sleepFading
        ? i18nc("@info:tooltip", "Fading out — move the mouse or press a key to keep listening")
        : i18nc("@info:tooltip", "Sleep timer")
    QQC2.ToolTip.visible: hovered || session.sleepFading
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay

    QQC2.Menu {
        id: menu
        Repeater {
            model: [5, 10, 15, 30, 45, 60, 90]
            delegate: QQC2.MenuItem {
                required property int modelData
                text: i18ncp("@action:inmenu", "%1 minute", "%1 minutes", modelData)
                onTriggered: button.session.startSleepTimer(modelData * 60000)
            }
        }
        QQC2.MenuItem {
            text: i18nc("@action:inmenu", "End of Chapter")
            onTriggered: button.session.sleepAtEndOfChapter()
        }
        QQC2.MenuSeparator {
            visible: button.session.sleepActive
        }
        QQC2.MenuItem {
            visible: button.session.sleepActive
            height: visible ? implicitHeight : 0
            icon.name: "dialog-cancel"
            text: i18nc("@action:inmenu", "Turn Off Sleep Timer")
            onTriggered: button.session.cancelSleepTimer()
        }
    }
}
