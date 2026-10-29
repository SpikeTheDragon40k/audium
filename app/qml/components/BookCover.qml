import QtQuick
import org.kde.kirigami as Kirigami

// A book cover, or a placeholder icon when there is none.
Item {
    id: cover

    property url source
    property bool available: true

    implicitWidth: Kirigami.Units.gridUnit * 8
    implicitHeight: implicitWidth
    opacity: available ? 1 : 0.5

    Rectangle {
        anchors.fill: parent
        radius: Kirigami.Units.cornerRadius
        color: Kirigami.Theme.alternateBackgroundColor
        visible: image.status !== Image.Ready
    }
    Kirigami.Icon {
        anchors.centerIn: parent
        width: Math.min(parent.width, parent.height) / 2
        height: width
        source: "audiobook"
        fallback: "media-optical-audio"
        color: Kirigami.Theme.disabledTextColor
        visible: image.status !== Image.Ready
    }
    Image {
        id: image
        anchors.fill: parent
        source: cover.source
        sourceSize.width: width * Screen.devicePixelRatio
        sourceSize.height: height * Screen.devicePixelRatio
        fillMode: Image.PreserveAspectFit
        asynchronous: true
        cache: true
        visible: status === Image.Ready
    }
}
