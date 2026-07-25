import Quickshell
import Quickshell.Hyprland
import QtQuick

Item {
    id: root

    property var ids: [1, 2, 3, 4]
    property int rev: 0

    implicitWidth: pill.implicitWidth
    implicitHeight: pill.implicitHeight

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            root.rev++;
        }
    }

    Rectangle {
        id: pill
        anchors.verticalCenter: parent.verticalCenter
        implicitWidth: workspaceRow.implicitWidth + 16
        implicitHeight: 36
        radius: height / 2
        color: Qt.rgba(Colors.backgroundAlt.r, Colors.backgroundAlt.g, Colors.backgroundAlt.b, 0.64)

        Row {
            id: workspaceRow
            anchors.centerIn: parent
            spacing: 4

            Repeater {
                model: root.ids

                Item {
                    id: workspace

                    required property int modelData
                    readonly property bool occupied: {
                        root.rev;
                        return Workspaces.isOccupied(modelData);
                    }
                    readonly property bool active: {
                        root.rev;
                        return Workspaces.isActive(modelData);
                    }
                    readonly property bool urgent: {
                        root.rev;
                        return Workspaces.isUrgent(modelData);
                    }

                    width: 48
                    height: 36

                    Rectangle {
                        anchors.centerIn: parent
                        width: 34
                        height: 34
                        radius: height / 2
                        color: {
                            if (workspace.urgent)
                                return Colors.red;
                            if (workspace.active)
                                return Colors.accent;
                            if (workspaceMouse.containsMouse)
                                return Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.08);
                            return "transparent";
                        }
                        border.width: workspace.active || workspace.urgent ? 1 : 0
                        border.color: Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.18)

                        Behavior on color {
                            ColorAnimation {
                                duration: 120
                            }
                        }
                    }

                    Text {
                        anchors.centerIn: parent
                        text: workspace.modelData
                        color: {
                            if (workspace.active || workspace.urgent)
                                return Colors.foreground;
                            if (workspace.occupied)
                                return Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.90);
                            return Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.68);
                        }
                        font.family: "JetBrainsMono Nerd Font"
                        font.pixelSize: 14
                        font.weight: workspace.active ? Font.DemiBold : Font.Medium

                        Behavior on color {
                            ColorAnimation {
                                duration: 120
                            }
                        }
                    }

                    Rectangle {
                        anchors {
                            right: parent.right
                            top: parent.top
                            rightMargin: 3
                            topMargin: 2
                        }
                        visible: Observatory.audio.active && Observatory.audio.workspace === workspace.modelData
                        width: 4
                        height: 4
                        radius: 2
                        color: Colors.lavender
                    }

                    MouseArea {
                        id: workspaceMouse

                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        hoverEnabled: true
                        onClicked: Workspaces.activate(workspace.modelData)
                        onWheel: event => {
                            Hyprland.dispatch(event.angleDelta.y > 0 ? "workspace e-1" : "workspace e+1");
                        }
                    }
                }
            }
        }
    }
}
