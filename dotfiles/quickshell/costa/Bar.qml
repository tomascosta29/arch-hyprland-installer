import Quickshell
import Quickshell.Bluetooth
import Quickshell.Networking
import Quickshell.Services.SystemTray
import Quickshell.Wayland
import QtQuick

Item {
    id: root

    required property var modelData
    required property var persistentState
    readonly property bool primary: Profile.isPrimary(modelData.name)
    readonly property string costa: Quickshell.env("HOME") + "/.local/bin/costa-utils"
    readonly property bool caffeine: persistentState.caffeine
    readonly property var bluetoothAdapter: Bluetooth.defaultAdapter
    readonly property bool bluetoothConnected: Bluetooth.devices.values.length > 0
    readonly property bool networkConnected: {
        const devices = Networking.devices.values;
        for (let index = 0; index < devices.length; index++) {
            if (devices[index].connected)
                return true;
        }
        return false;
    }
    readonly property var audio: StageBus.sink && StageBus.sink.audio ? StageBus.sink.audio : null

    width: modelData.width - Colors.barSideMargin * 2
    height: Colors.barHeight

    IdleInhibitor {
        enabled: root.primary && root.caffeine
        window: QsWindow.window
    }

    Rectangle {
        id: body

        anchors.fill: parent
        radius: Colors.pillRadius
        border.width: 1
        border.color: Qt.rgba(0.298, 0.337, 0.416, 0.72)
        color: Qt.rgba(0.18, 0.204, 0.251, 0.94)

        Rectangle {
            anchors {
                top: parent.top
                left: parent.left
                right: parent.right
                leftMargin: Colors.pillRadius
                rightMargin: Colors.pillRadius
            }
            height: 1
            color: Qt.rgba(0.847, 0.871, 0.914, 0.07)
        }

        Row {
            anchors {
                left: parent.left
                leftMargin: 7
                verticalCenter: parent.verticalCenter
            }
            spacing: 9

            BarAction {
                visible: root.primary
                implicitWidth: 34
                icon: "󰣇"
                iconSize: 23
                iconVerticalOffset: 3
                iconColor: Colors.accent
                onTriggered: Quickshell.execDetached([root.costa, "--app-menu"])
            }

            WorkspacePill {
                ids: root.primary ? [1, 2, 3, 4] : [5, 6, 7, 8]
            }
        }

        Loader {
            anchors.centerIn: parent
            sourceComponent: root.primary ? centerStageComponent : secondaryCenterComponent
        }

        Loader {
            anchors {
                right: parent.right
                rightMargin: 7
                verticalCenter: parent.verticalCenter
            }
            sourceComponent: root.primary ? sessionComponent : adaptiveTelemetryComponent
        }
    }

    Component {
        id: centerStageComponent

        CenterStage {}
    }

    Component {
        id: secondaryCenterComponent

        SecondaryCenter {
            screenName: root.modelData.name
        }
    }

    Component {
        id: sessionComponent

        Rectangle {
            implicitWidth: sessionRow.implicitWidth + 14
            implicitHeight: 36
            radius: height / 2
            color: Qt.rgba(Colors.backgroundAlt.r, Colors.backgroundAlt.g, Colors.backgroundAlt.b, 0.52)

            Row {
                id: sessionRow

                anchors.centerIn: parent
                spacing: 1

                Row {
                    id: trayGroup

                    visible: SystemTray.items.values.length > 0
                    spacing: 1

                    Repeater {
                        model: SystemTray.items

                        Item {
                            id: trayItem

                            required property var modelData
                            width: 26
                            height: 36

                            Rectangle {
                                anchors.fill: parent
                                radius: 8
                                color: trayMouse.containsMouse ? Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.09) : "transparent"

                                Behavior on color {
                                    ColorAnimation {
                                        duration: 100
                                    }
                                }
                            }

                            Image {
                                anchors.centerIn: parent
                                width: 18
                                height: 18
                                source: trayItem.modelData.icon
                                sourceSize.width: 18
                                sourceSize.height: 18
                                fillMode: Image.PreserveAspectFit
                                smooth: true
                                asynchronous: true
                            }

                            MouseArea {
                                id: trayMouse

                                anchors.fill: parent
                                acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                                cursorShape: Qt.PointingHandCursor
                                hoverEnabled: true

                                onClicked: event => {
                                    if (event.button === Qt.LeftButton) {
                                        trayItem.modelData.activate();
                                    } else if (event.button === Qt.MiddleButton) {
                                        trayItem.modelData.secondaryActivate();
                                    } else if (trayItem.modelData.hasMenu) {
                                        const point = QsWindow.mapFromItem(trayItem, trayItem.width / 2, trayItem.height);
                                        trayItem.modelData.display(QsWindow.window, point.x, point.y);
                                    }
                                }
                                onWheel: event => trayItem.modelData.scroll(event.angleDelta.y, false)
                            }
                        }
                    }
                }

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: trayGroup.visible
                    width: trayGroup.visible ? 1 : 0
                    height: 14
                    color: Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.14)
                }

                BarAction {
                    icon: root.caffeine ? "󰅶" : "󰾪"
                    iconSize: 18
                    iconColor: root.caffeine ? Colors.yellow : Colors.foregroundDim
                    active: root.caffeine
                    onTriggered: {
                        root.persistentState.caffeine = !root.caffeine;
                        StageBus.push(root.caffeine ? "󰅶" : "󰾪", "Keep awake", root.caffeine ? "Enabled" : "Disabled", -1, root.caffeine ? Colors.yellow : Colors.foregroundDim, 1800, 20, "");
                    }
                }

                BarAction {
                    icon: "󰅌"
                    iconSize: 18
                    iconColor: Colors.foregroundDim
                    onTriggered: Quickshell.execDetached([root.costa, "--clipper"])
                }

                BarAction {
                    icon: "󰄀"
                    iconSize: 18
                    iconColor: Colors.foregroundDim
                    onTriggered: Quickshell.execDetached([root.costa, "--blinker"])
                }

                BarAction {
                    icon: "󰒓"
                    iconSize: 18
                    iconColor: Colors.foregroundDim
                    onTriggered: Quickshell.execDetached([root.costa, "--control-center"])
                }

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: 1
                    height: 14
                    color: Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.14)
                }

                BarAction {
                    icon: !root.bluetoothAdapter || !root.bluetoothAdapter.enabled ? "󰂲" : root.bluetoothConnected ? "󰂱" : "󰂯"
                    iconSize: 18
                    iconColor: root.bluetoothConnected ? Colors.cyan : root.bluetoothAdapter && root.bluetoothAdapter.enabled ? Colors.foreground : Colors.foregroundDim
                    onTriggered: Quickshell.execDetached([root.costa, "--bluetooth-menu"])
                }

                BarAction {
                    icon: !root.audio || root.audio.muted ? "󰝟" : root.audio.volume >= 0.66 ? "󰕾" : root.audio.volume >= 0.33 ? "󰖀" : "󰕿"
                    iconSize: 18
                    iconColor: root.audio && root.audio.muted ? Colors.red : Colors.foreground
                    onTriggered: Quickshell.execDetached([root.costa, "--volume-menu"])
                    onMiddleTriggered: StageBus.toggleMute()
                    onContextTriggered: Quickshell.execDetached([root.costa, "--volume-menu"])
                    onWheeled: event => StageBus.adjustVolume(event.angleDelta.y > 0 ? 0.05 : -0.05)
                }

                BarAction {
                    icon: root.networkConnected ? "󰖩" : "󰖪"
                    iconSize: 18
                    iconColor: root.networkConnected ? Colors.foreground : Colors.foregroundDim
                    onTriggered: Quickshell.execDetached([root.costa, "--network-menu"])
                }

                BarAction {
                    icon: "󰐥"
                    iconSize: 18
                    danger: true
                    onTriggered: Quickshell.execDetached([root.costa, "--power-menu"])
                }
            }
        }
    }

    Component {
        id: adaptiveTelemetryComponent

        AdaptiveTelemetry {}
    }
}
