import QtQuick
import org.kde.kirigami as Kirigami

// A thin listening-progress bar under a cover. Drawn here rather than by the
// desktop style: the style's bar is taller than a tile allows under Breeze
// and has no height at all under other widget styles (Fusion).
Rectangle {
    id: line

    property real value: 0 // 0..1

    implicitHeight: Kirigami.Units.smallSpacing
    radius: height / 2
    color: Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.15)

    Accessible.role: Accessible.ProgressBar
    Accessible.name: i18nc("@info accessible name", "%1% listened", Math.round(line.value * 100))

    Rectangle {
        width: parent.width * Math.max(0, Math.min(1, line.value))
        height: parent.height
        radius: parent.radius
        color: Kirigami.Theme.highlightColor
    }
}
