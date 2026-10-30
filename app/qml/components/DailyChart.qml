import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import "TimeFormat.js" as TimeFormat

// Listening time per day as thin bars on a baseline (one series, so no
// legend: the page heading names it). Each bar has a tooltip with the exact
// value; Mondays are labelled under the axis.
Item {
    id: chart

    /// [{date, year, month, day, weekday, ms}], oldest first
    required property var days
    required property real maxMs

    readonly property int gap: 2
    readonly property real plotHeight: height - axisLabels.height - Kirigami.Units.smallSpacing
    readonly property real barWidth: days.length > 0 ? Math.max(2, (width - gap * (days.length - 1)) / days.length) : 0
    // A round upper bound for the scale: the next whole number of 15 minutes.
    readonly property real scaleMs: Math.max(15 * 60000, Math.ceil(maxMs / (15 * 60000)) * 15 * 60000)

    objectName: "dailyChart"
    implicitHeight: Kirigami.Units.gridUnit * 10

    // Recessive guide at the top of the scale, with its value.
    Rectangle {
        x: 0
        y: 0
        width: chart.width
        height: 1
        color: Kirigami.Theme.disabledTextColor
        opacity: 0.25
    }
    QQC2.Label {
        anchors.right: parent.right
        y: 2
        text: TimeFormat.duration(chart.scaleMs, i18nc)
        font: Kirigami.Theme.smallFont
        color: Kirigami.Theme.disabledTextColor
    }
    // Baseline.
    Rectangle {
        x: 0
        y: chart.plotHeight
        width: chart.width
        height: 1
        color: Kirigami.Theme.disabledTextColor
        opacity: 0.6
    }

    Repeater {
        model: chart.days
        delegate: Item {
            id: bar
            required property var modelData
            required property int index
            x: index * (chart.barWidth + chart.gap)
            y: 0
            width: chart.barWidth
            height: chart.plotHeight

            // The bar, with rounded top corners only: a rounded rectangle
            // whose lower part is squared off by a plain one.
            readonly property real barHeight: modelData.ms > 0 ? Math.max(2, chart.plotHeight * modelData.ms / chart.scaleMs) : 0
            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: bar.barHeight
                radius: Math.min(4, width / 2)
                color: Kirigami.Theme.highlightColor
                visible: bar.barHeight > 0
                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: Math.min(parent.height, parent.radius)
                    color: parent.color
                }
            }
            HoverHandler {
                id: hover
            }
            QQC2.ToolTip.visible: hover.hovered
            QQC2.ToolTip.delay: 0
            QQC2.ToolTip.text: i18nc("@info:tooltip date: listening time", "%1: %2",
                                     new Date(modelData.year, modelData.month - 1, modelData.day).toLocaleDateString(Qt.locale(), Locale.ShortFormat),
                                     TimeFormat.duration(modelData.ms, i18nc))
        }
    }

    Item {
        id: axisLabels
        anchors.bottom: parent.bottom
        width: parent.width
        height: dayLabel.implicitHeight
        QQC2.Label {
            id: dayLabel
            visible: false
            text: "0"
            font: Kirigami.Theme.smallFont
        }
        Repeater {
            model: chart.days
            delegate: QQC2.Label {
                required property var modelData
                required property int index
                visible: modelData.weekday === 0
                x: Math.min(chart.width - width, index * (chart.barWidth + chart.gap))
                text: new Date(modelData.year, modelData.month - 1, modelData.day).toLocaleDateString(Qt.locale(), "d MMM")
                font: Kirigami.Theme.smallFont
                color: Kirigami.Theme.disabledTextColor
            }
        }
    }
}
