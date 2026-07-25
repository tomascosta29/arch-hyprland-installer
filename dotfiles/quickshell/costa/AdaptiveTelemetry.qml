import QtQuick

Rectangle {
    id: root

    readonly property real tempValue: Number.parseFloat(Telemetry.temp || "") || 0
    readonly property bool thermalWarning: tempValue >= 78
    readonly property var allEntries: [
        {
            "icon": "󰖐",
            "value": Telemetry.weather || "—",
            "tint": Colors.cyan,
            "id": "weather"
        },
        {
            "icon": "󰻠",
            "value": Telemetry.cpu,
            "tint": Colors.foreground,
            "id": "cpu"
        },
        {
            "icon": "󰍛",
            "value": Telemetry.mem,
            "tint": Colors.foreground,
            "id": "mem"
        },
        {
            "icon": "󰢮",
            "value": Telemetry.gpu,
            "tint": Colors.lavender,
            "id": "gpu"
        },
        {
            "icon": "󰔏",
            "value": Telemetry.temp,
            "tint": root.thermalWarning ? Colors.red : root.tempValue >= 70 ? Colors.yellow : Colors.cyan,
            "id": "temp"
        },
        {
            "icon": "󰇚",
            "value": Observatory.healthy ? root.rateText(Observatory.networkRx) : "—",
            "tint": Observatory.healthy ? Colors.cyan : Colors.foregroundDim,
            "id": "rx"
        },
        {
            "icon": "󰕒",
            "value": Observatory.healthy ? root.rateText(Observatory.networkTx) : "—",
            "tint": Observatory.healthy ? Colors.lavender : Colors.foregroundDim,
            "id": "tx"
        }
    ]
    readonly property var entries: root.allEntries.filter(entry => {
        if (!Profile.telemetryGpu && (entry.id === "gpu" || entry.id === "temp"))
            return false;
        return true;
    })

    implicitWidth: telemetryRow.implicitWidth + 20
    implicitHeight: 36
    radius: height / 2
    color: Qt.rgba(Colors.softBlue.r, Colors.softBlue.g, Colors.softBlue.b, 0.15)

    function rateText(bytesPerSecond) {
        if (bytesPerSecond < 1024)
            return Math.round(bytesPerSecond) + "B/s";
        if (bytesPerSecond < 1024 * 1024)
            return (bytesPerSecond / 1024).toFixed(bytesPerSecond < 10240 ? 1 : 0) + "K/s";
        return (bytesPerSecond / 1024 / 1024).toFixed(1) + "M/s";
    }

    Row {
        id: telemetryRow

        anchors.centerIn: parent
        spacing: 11

        Repeater {
            model: root.entries

            Row {
                required property var modelData

                spacing: 5

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: parent.modelData.icon
                    color: parent.modelData.tint
                    font.family: "JetBrainsMono Nerd Font"
                    font.pixelSize: 14
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: parent.modelData.value
                    color: parent.modelData.tint
                    font.family: "JetBrainsMono Nerd Font"
                    font.pixelSize: 12
                    font.weight: Font.DemiBold
                }
            }
        }
    }
}
