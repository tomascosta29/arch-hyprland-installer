import QtQuick

Rectangle {
    id: root

    readonly property real cpuTempValue: Number.parseFloat(Telemetry.cpuTemp || "") || 0
    readonly property real gpuTempValue: Number.parseFloat(Telemetry.gpuTemp || "") || 0
    readonly property real gpuUsageValue: Number.parseFloat(Telemetry.gpu || "") || 0
    readonly property bool showGpuDetail: Profile.telemetryGpu && root.gpuUsageValue >= 20 && (Telemetry.gpuPower !== "" || Telemetry.gpuFan !== "")
    readonly property bool showZram: Telemetry.zramUsedBytes >= 256 * 1024 * 1024
    readonly property bool online: Telemetry.connectivity === "online"

    implicitWidth: telemetryRow.implicitWidth + 18
    implicitHeight: 36
    radius: height / 2
    color: Qt.rgba(Colors.softBlue.r, Colors.softBlue.g, Colors.softBlue.b, 0.15)

    function rateText(bytesPerSecond) {
        if (bytesPerSecond < 1024)
            return Math.round(bytesPerSecond) + "B";
        if (bytesPerSecond < 1024 * 1024)
            return (bytesPerSecond / 1024).toFixed(bytesPerSecond < 10240 ? 1 : 0) + "K";
        return (bytesPerSecond / 1024 / 1024).toFixed(1) + "M";
    }

    function weatherText(value) {
        return (value || "—").replace("°C", "°");
    }

    function temperatureColor(value, warm, hot) {
        if (value >= hot)
            return Colors.red;
        if (value >= warm)
            return Colors.yellow;
        return Colors.foreground;
    }

    function connectivityText() {
        switch (Telemetry.connectivity) {
        case "portal":
            return "Sign in";
        case "limited":
            return "No internet";
        case "offline":
            return "Offline";
        default:
            return "Checking";
        }
    }

    function connectivityColor() {
        switch (Telemetry.connectivity) {
        case "online":
            return Colors.cyan;
        case "portal":
            return Colors.yellow;
        case "limited":
            return Colors.red;
        default:
            return Colors.foregroundDim;
        }
    }

    function byteText(bytes) {
        if (bytes < 1024 * 1024 * 1024)
            return (bytes / 1024 / 1024).toFixed(0) + "M";
        return (bytes / 1024 / 1024 / 1024).toFixed(1) + "G";
    }

    component Divider: Rectangle {
        anchors.verticalCenter: parent.verticalCenter
        width: 1
        height: 16
        color: Qt.rgba(Colors.foreground.r, Colors.foreground.g, Colors.foreground.b, 0.14)
    }

    component MetricIcon: Text {
        anchors.verticalCenter: parent.verticalCenter
        color: Colors.foregroundDim
        font.family: "JetBrainsMono Nerd Font"
        font.pixelSize: 16
    }

    component MetricLabel: Text {
        anchors.verticalCenter: parent.verticalCenter
        color: Colors.foregroundDim
        font.family: "JetBrainsMono Nerd Font"
        font.pixelSize: 11
        font.weight: Font.DemiBold
    }

    component MetricValue: Text {
        anchors.verticalCenter: parent.verticalCenter
        color: Colors.foreground
        font.family: "JetBrainsMono Nerd Font"
        font.pixelSize: 13
        font.weight: Font.DemiBold
    }

    Row {
        id: telemetryRow

        anchors.centerIn: parent
        spacing: 9

        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 5

            MetricIcon {
                text: "󰖐"
                color: Colors.cyan
            }
            MetricValue {
                text: root.weatherText(Telemetry.weather)
                color: Colors.cyan
            }
        }

        Divider {}

        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 5

            MetricIcon { text: "󰻠" }
            MetricLabel { text: "CPU" }
            MetricValue { text: Telemetry.cpu }
            MetricValue {
                text: "· " + Telemetry.cpuTemp
                color: root.temperatureColor(root.cpuTempValue, 75, 85)
            }
        }

        Divider {}

        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 5

            MetricIcon { text: "󰍛" }
            MetricLabel { text: "RAM" }
            MetricValue { text: Telemetry.memUsed + "/" + Telemetry.memTotal }
            MetricValue {
                visible: root.showZram
                text: "· Z " + root.byteText(Telemetry.zramUsedBytes)
                color: Colors.yellow
            }
        }

        Divider {
            visible: Profile.telemetryGpu
            width: visible ? 1 : 0
        }

        Row {
            anchors.verticalCenter: parent.verticalCenter
            visible: Profile.telemetryGpu
            spacing: 5

            MetricIcon { text: "󰢮" }
            MetricLabel { text: "GPU" }
            MetricValue { text: Telemetry.gpu }
            MetricValue {
                text: "· " + Telemetry.gpuTemp
                color: root.temperatureColor(root.gpuTempValue, 80, 90)
            }
            MetricValue {
                text: "· " + Telemetry.vramUsed + "/" + Telemetry.vramTotal
            }
            MetricValue {
                visible: root.showGpuDetail && Telemetry.gpuPower !== ""
                text: "· " + Telemetry.gpuPower
                color: Colors.lavender
            }
            MetricValue {
                visible: root.showGpuDetail && Telemetry.gpuFan !== ""
                text: "· " + Telemetry.gpuFan
                color: Colors.lavender
            }
        }

        Divider {}

        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 5

            MetricIcon { text: "󰋊" }
            MetricValue {
                text: Telemetry.diskFreeGib > 0 ? Telemetry.diskFreeGib + "G free" : "—"
                color: Telemetry.diskFreeGib > 0 && Telemetry.diskFreeGib <= 20 ? Colors.red : Telemetry.diskFreeGib > 0 && Telemetry.diskFreeGib <= 50 ? Colors.yellow : Colors.foreground
            }
        }

        Divider {}

        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: 7
                height: 7
                radius: width / 2
                color: root.connectivityColor()
            }
            MetricValue {
                visible: root.online
                text: "↓" + root.rateText(Observatory.healthy ? Observatory.networkRx : 0)
                color: Colors.cyan
            }
            MetricValue {
                visible: root.online
                text: "↑" + root.rateText(Observatory.healthy ? Observatory.networkTx : 0)
                color: Colors.lavender
            }
            MetricValue {
                visible: !root.online
                text: root.connectivityText()
                color: root.connectivityColor()
            }
            MetricValue {
                visible: root.online && Telemetry.latencyMs >= 200
                text: "· " + Telemetry.latencyMs + "ms"
                color: Telemetry.latencyMs >= 500 ? Colors.red : Colors.yellow
            }
        }

        Divider {}

        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 5

            MetricIcon {
                text: "󰏔"
                color: Telemetry.updateState === "error" ? Colors.red : Colors.foregroundDim
            }
            MetricValue {
                text: Telemetry.updateState === "error" ? "!" : Telemetry.updates
                color: Telemetry.updates >= 50 ? Colors.yellow : Colors.foreground
            }
        }
    }
}
