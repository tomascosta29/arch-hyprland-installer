import Quickshell
import QtQuick

Item {
    id: root

    property bool showingEvent: false
    property string eventIcon: ""
    property string eventTitle: ""
    property string eventDetail: ""
    property real eventProgress: -1
    property color eventTint: Colors.accent
    property int eventPriority: 0
    property string eventAction: ""
    property bool clockHovered: clockMouse.containsMouse
    readonly property string clockTimeFormat: {
        if (Profile.clockFormat === "12h")
            return root.clockHovered ? "h:mm:ss AP" : "h:mm AP";
        return root.clockHovered ? "HH:mm:ss" : "HH:mm";
    }

    implicitWidth: showingEvent ? Math.min(430, eventRow.implicitWidth + 24) : clockRow.implicitWidth
    implicitHeight: Colors.innerPillHeight

    Behavior on implicitWidth {
        NumberAnimation {
            duration: 190
            easing.type: Easing.OutCubic
        }
    }

    function reveal(icon, title, detail, progress, tint, duration, priority, action) {
        if (root.showingEvent && priority < root.eventPriority)
            return;

        root.eventIcon = icon;
        root.eventTitle = title;
        root.eventDetail = detail;
        root.eventProgress = progress;
        root.eventTint = tint;
        root.eventPriority = priority;
        root.eventAction = action;
        root.showingEvent = true;
        dismissTimer.interval = duration;
        dismissTimer.restart();
    }

    Rectangle {
        anchors.fill: parent
        radius: 8
        opacity: root.showingEvent ? 1 : 0
        color: Qt.rgba(root.eventTint.r, root.eventTint.g, root.eventTint.b, 0.075)

        Behavior on opacity {
            NumberAnimation {
                duration: 140
            }
        }
        Behavior on color {
            ColorAnimation {
                duration: 140
            }
        }
    }

    Row {
        id: clockRow

        anchors.centerIn: parent
        spacing: 9
        opacity: root.showingEvent ? 0 : 1
        scale: root.showingEvent ? 0.96 : 1

        Behavior on opacity {
            NumberAnimation {
                duration: 110
            }
        }
        Behavior on scale {
            NumberAnimation {
                duration: 160
                easing.type: Easing.OutCubic
            }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: Qt.formatDateTime(clock.date, "ddd, dd MMM")
            color: root.clockHovered ? Colors.foreground : Colors.foregroundDim
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 15
            font.weight: Font.Medium

            Behavior on color {
                ColorAnimation {
                    duration: 100
                }
            }
        }

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 1
            height: 15
            radius: 0.5
            color: Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.18)
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: Qt.formatDateTime(clock.date, root.clockTimeFormat)
            color: Colors.foreground
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 16
            font.weight: Font.DemiBold
        }
    }

    Row {
        id: eventRow

        anchors.centerIn: parent
        spacing: 8
        opacity: root.showingEvent ? 1 : 0
        scale: root.showingEvent ? 1 : 0.96

        Behavior on opacity {
            NumberAnimation {
                duration: 140
            }
        }
        Behavior on scale {
            NumberAnimation {
                duration: 180
                easing.type: Easing.OutCubic
            }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.eventIcon
            color: root.eventTint
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 19
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(235, implicitWidth)
            text: root.eventTitle
            elide: Text.ElideRight
            color: Colors.foreground
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 15
            font.weight: Font.DemiBold
        }

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.eventDetail !== ""
            width: visible ? 1 : 0
            height: 14
            color: Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.16)
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.eventDetail !== ""
            width: visible ? Math.min(130, implicitWidth) : 0
            text: root.eventDetail
            elide: Text.ElideRight
            color: Colors.foregroundDim
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 14
            font.weight: Font.Medium
        }
    }

    Rectangle {
        anchors {
            left: parent.left
            right: parent.right
            bottom: parent.bottom
            leftMargin: 7
            rightMargin: 7
            bottomMargin: 2
        }
        visible: root.showingEvent && root.eventProgress >= 0
        height: 2
        radius: 1
        color: Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.10)

        Rectangle {
            width: parent.width * Math.max(0, Math.min(1, root.eventProgress))
            height: parent.height
            radius: parent.radius
            color: root.eventTint

            Behavior on width {
                NumberAnimation {
                    duration: 120
                    easing.type: Easing.OutCubic
                }
            }
        }
    }

    MouseArea {
        id: clockMouse

        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        cursorShape: root.showingEvent && root.eventAction !== "" ? Qt.PointingHandCursor : Qt.ArrowCursor
        hoverEnabled: true
        onClicked: {
            if (root.showingEvent && root.eventAction !== "")
                StageBus.activate(root.eventAction);
        }
        onWheel: event => {
            if (root.showingEvent && root.eventAction !== "")
                StageBus.wheel(root.eventAction, event.angleDelta.y);
        }
    }

    Connections {
        target: StageBus

        function onEventRequested(icon, title, detail, progress, tint, duration, priority, action) {
            root.reveal(icon, title, detail, progress, tint, duration, priority, action);
        }
    }

    Timer {
        id: dismissTimer

        interval: 2200
        onTriggered: {
            root.showingEvent = false;
            root.eventPriority = 0;
            root.eventAction = "";
        }
    }

    SystemClock {
        id: clock
        precision: root.clockHovered ? SystemClock.Seconds : SystemClock.Minutes
    }
}
