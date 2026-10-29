import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.audium.bridge

Kirigami.ApplicationWindow {
    id: root

    title: i18nc("@title:window", "Audium")
    minimumWidth: Kirigami.Units.gridUnit * 20
    minimumHeight: Kirigami.Units.gridUnit * 20
    width: Kirigami.Units.gridUnit * 50
    height: Kirigami.Units.gridUnit * 35

    pageStack.initialPage: Kirigami.Page {
        objectName: "foundationPage"
        title: i18nc("@title", "Library")

        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            icon.name: "audiobook"
            text: i18n("Audium %1", AppInfo.version)
            explanation: AppInfo.echo(i18n("The Rust bridge is working."))
        }
    }
}
