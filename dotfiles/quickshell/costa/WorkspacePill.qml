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
        color: Qt.rgba(Colors.softBlue.r, Colors.softBlue.g, Colors.softBlue.b, 0.15)

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
                        height: 30
                        radius: 10
                        color: {
                            if (workspace.urgent)
                                return Qt.rgba(Colors.red.r, Colors.red.g, Colors.red.b, 0.10);
                            if (workspaceMouse.containsMouse)
                                return Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.08);
                            return "transparent";
                        }

                        Behavior on color {
                            ColorAnimation {
                                duration: 120
                            }
                        }
                    }

                    Text {
                        anchors {
                            centerIn: parent
                            verticalCenterOffset: -2
                        }
                        text: workspace.modelData
                        color: {
                            if (workspace.urgent)
                                return Colors.red;
                            if (workspace.active)
                                return Colors.accent;
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
                            horizontalCenter: parent.horizontalCenter
                            bottom: parent.bottom
                            bottomMargin: 3
                        }
                        width: workspace.active || workspace.urgent || workspace.occupied ? 20 : 0
                        height: 3
                        radius: height / 2
                        color: workspace.urgent ? Colors.red : workspace.active ? Colors.accent : Colors.foreground
                        opacity: 1

                        Behavior on width {
                            NumberAnimation {
                                duration: 150
                                easing.type: Easing.OutCubic
                            }
                        }

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
                            Hyprland.dispatch(event.angleDelta.y > 0 ? "hl.dsp.focus({ workspace = \"e-1\" })" : "hl.dsp.focus({ workspace = \"e+1\" })");
                        }
                    }
                }
            }
        }
    }
}
