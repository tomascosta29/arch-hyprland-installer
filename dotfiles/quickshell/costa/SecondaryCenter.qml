import Quickshell
import Quickshell.Hyprland
import QtQuick

Item {
    id: root

    required property string screenName
    property var context: {
        Observatory.revision;
        return Observatory.contextFor(root.screenName);
    }
    property var activity: {
        ActivityBus.revision;
        return ActivityBus.topForMonitor(root.screenName);
    }
    readonly property int activityCount: {
        ActivityBus.revision;
        return ActivityBus.countForMonitor(root.screenName);
    }
    readonly property bool contextIsActive: context.valid && context.focused && Hyprland.focusedMonitor && Hyprland.focusedMonitor.name === root.screenName
    readonly property string mode: activity ? "activity" : contextIsActive ? "context" : "idle"

    implicitWidth: {
        if (mode === "activity")
            return Math.min(700, activityRow.implicitWidth + 24);
        if (mode === "context")
            return Math.min(720, contextRow.implicitWidth + 20);
        return idleRow.implicitWidth;
    }
    implicitHeight: Colors.innerPillHeight

    Behavior on implicitWidth {
        NumberAnimation {
            duration: 190
            easing.type: Easing.OutCubic
        }
    }

    function prettyApp(appClass) {
        const value = (appClass || "").toLowerCase();
        if (value.includes("firefox"))
            return "Firefox";
        if (value.includes("brave"))
            return "Brave";
        if (value.includes("cursor"))
            return "Cursor";
        if (value.includes("code"))
            return "Code";
        if (value.includes("alacritty"))
            return "Alacritty";
        if (value.includes("kitty"))
            return "Kitty";
        if (value.includes("steam"))
            return "Steam";
        if (!appClass)
            return "Desktop";
        return appClass.charAt(0).toUpperCase() + appClass.slice(1);
    }

    function appIcon(appClass) {
        const value = (appClass || "").toLowerCase();
        if (value.includes("firefox"))
            return "󰈹";
        if (value.includes("brave"))
            return "󰖟";
        if (value.includes("cursor") || value.includes("code"))
            return "󰨞";
        if (value.includes("alacritty") || value.includes("kitty"))
            return "󰆍";
        if (value.includes("steam"))
            return "󰓓";
        return "󰣆";
    }

    Rectangle {
        anchors.fill: parent
        radius: 8
        opacity: root.mode === "activity" ? 1 : 0
        color: root.activity ? Qt.rgba(root.activity.tint.r, root.activity.tint.g, root.activity.tint.b, 0.075) : "transparent"

        Behavior on opacity {
            NumberAnimation {
                duration: 140
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: root.mode === "activity" && root.activity
        acceptedButtons: Qt.RightButton
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: {
            if (root.activity)
                ActivityBus.removeInternal(root.activity.id);
        }
    }

    Row {
        id: contextRow

        anchors.centerIn: parent
        spacing: 8
        opacity: root.mode === "context" ? 1 : 0

        Behavior on opacity {
            NumberAnimation {
                duration: 130
            }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.appIcon(root.context.class)
            color: Colors.accent
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 17
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.prettyApp(root.context.class)
            color: Colors.foreground
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 13
            font.weight: Font.DemiBold
        }

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 1
            height: 12
            color: Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.16)
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(330, implicitWidth)
            text: root.context.title
            elide: Text.ElideRight
            color: Colors.foregroundDim
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 12
            font.weight: Font.Medium
        }

        Row {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.context.repo !== ""
            spacing: 5

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: 1
                height: 12
                color: Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.16)
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "󰘬"
                color: root.context.dirty > 0 ? Colors.yellow : Colors.foregroundDim
                font.family: "JetBrainsMono Nerd Font"
                font.pixelSize: 15
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(145, implicitWidth)
                text: root.context.repo + " · " + root.context.branch + (root.context.dirty > 0 ? " *" : "")
                elide: Text.ElideRight
                color: root.context.dirty > 0 ? Colors.yellow : Colors.foregroundDim
                font.family: "JetBrainsMono Nerd Font"
                font.pixelSize: 12
                font.weight: Font.Medium
            }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: Observatory.audio.active && Observatory.audio.workspace === root.context.workspace
            text: "󰎆"
            color: Colors.lavender
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 15
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: root.mode === "context" && root.context.address !== ""
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: Hyprland.dispatch("hl.dispatch(\"focuswindow\", \"address:" + root.context.address + "\")")
    }

    Row {
        id: activityRow

        anchors.centerIn: parent
        spacing: 8
        opacity: root.mode === "activity" ? 1 : 0

        Behavior on opacity {
            NumberAnimation {
                duration: 130
            }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.activity ? root.activity.icon : ""
            color: root.activity ? root.activity.tint : Colors.accent
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 17
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(300, implicitWidth)
            text: root.activity ? root.activity.title : ""
            elide: Text.ElideRight
            color: Colors.foreground
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 13
            font.weight: Font.DemiBold
        }

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.activity && root.activity.detail !== ""
            width: visible ? 1 : 0
            height: 12
            color: Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.16)
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.activity && root.activity.detail !== ""
            width: visible ? Math.min(180, implicitWidth) : 0
            text: root.activity ? root.activity.detail : ""
            elide: Text.ElideRight
            color: Colors.foregroundDim
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 12
            font.weight: Font.Medium
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.activityCount > 1
            text: "+" + (root.activityCount - 1)
            color: Colors.foregroundDim
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 11
            font.weight: Font.DemiBold
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
        visible: root.mode === "activity" && root.activity && root.activity.progress >= 0
        height: 2
        radius: 1
        color: Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.10)

        Rectangle {
            width: parent.width * Math.max(0, Math.min(1, root.activity ? root.activity.progress : 0))
            height: parent.height
            radius: parent.radius
            color: root.activity ? root.activity.tint : Colors.accent

            Behavior on width {
                NumberAnimation {
                    duration: 160
                    easing.type: Easing.OutCubic
                }
            }
        }
    }

    Row {
        id: idleRow

        anchors.centerIn: parent
        spacing: 8
        opacity: root.mode === "idle" ? 1 : 0

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: "󰥔"
            color: Colors.foregroundDim
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 16
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: Qt.formatDateTime(
                clock.date,
                Profile.clockFormat === "12h" ? "h:mm AP" : "HH:mm"
            )
            color: Colors.foregroundDim
            font.family: "JetBrainsMono Nerd Font"
            font.pixelSize: 13
            font.weight: Font.Medium
        }
    }

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
    }
}
