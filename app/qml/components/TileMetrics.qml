import QtQuick
import org.kde.kirigami as Kirigami

// Heights of the text under a cover (BookTile, group cards), so views can
// size their cells before any tile exists: a tile is its width (the square
// cover) plus `bookText` or `groupText`.
QtObject {
    readonly property FontMetrics body: FontMetrics {
        font.bold: true
    }
    readonly property FontMetrics small: FontMetrics {
        font: Kirigami.Theme.smallFont
    }
    /// Progress bar, two lines of title and one of authors, with spacing.
    readonly property real bookText: Kirigami.Units.smallSpacing * 5 + Math.ceil(body.height) * 2 + Math.ceil(small.height)
    /// One line of name and one of book count.
    readonly property real groupText: Kirigami.Units.smallSpacing * 3 + Math.ceil(body.height) + Math.ceil(small.height)
}
