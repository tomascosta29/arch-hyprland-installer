import QtQuick

Item {
    id: root

    property string icon: ""
    property color iconColor: Colors.foreground
    property bool active: false
    property bool danger: false
    property int iconSize: Colors.iconSize
    property real iconVerticalOffset: 0

    signal triggered
    signal middleTriggered
    signal contextTriggered
    signal wheeled(var event)

    implicitWidth: 26
    implicitHeight: Colors.innerPillHeight

    Rectangle {
        anchors.fill: parent
        radius: 8
        color: {
            if (mouse.containsMouse)
                return root.danger
                    ? Qt.rgba(Colors.red.r, Colors.red.g, Colors.red.b, 0.14)
                    : Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.09);
            if (root.active)
                return Qt.rgba(Colors.accent.r, Colors.accent.g, Colors.accent.b, 0.10);
            return "transparent";
        }

        Behavior on color {
            ColorAnimation {
                duration: 100
            }
        }
    }

    Text {
        anchors.centerIn: parent
        anchors.verticalCenterOffset: root.iconVerticalOffset
        text: root.icon
        color: root.danger && mouse.containsMouse ? Colors.red : root.iconColor
        font.family: "JetBrainsMono Nerd Font"
        font.pixelSize: root.iconSize

        Behavior on color {
            ColorAnimation {
                duration: 100
            }
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
        cursorShape: Qt.PointingHandCursor
        hoverEnabled: true
        onClicked: event => {
            if (event.button === Qt.LeftButton)
                root.triggered();
            else if (event.button === Qt.MiddleButton)
                root.middleTriggered();
            else if (event.button === Qt.RightButton)
                root.contextTriggered();
        }
        onWheel: event => root.wheeled(event)
    }
}
