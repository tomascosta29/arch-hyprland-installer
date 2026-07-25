pragma Singleton

import Quickshell
import Quickshell.Io
import QtQuick

Singleton {
    id: root

    property string cpu: "…"
    property string gpu: "…"
    property string mem: "…"
    property string temp: "…"
    property string weather: ""
    property real previousCpuTotal: -1
    property real previousCpuIdle: -1

    function restart(proc) {
        if (proc.running)
            return;
        proc.running = true;
    }

    function updateSystem(payload) {
        try {
            const sample = JSON.parse(payload.trim());
            const total = Number(sample.total);
            const idle = Number(sample.idle);

            if (Number.isFinite(total) && Number.isFinite(idle) && root.previousCpuTotal >= 0) {
                const totalDelta = total - root.previousCpuTotal;
                const idleDelta = idle - root.previousCpuIdle;
                if (totalDelta > 0) {
                    const usage = 100 * (totalDelta - idleDelta) / totalDelta;
                    root.cpu = Math.round(Math.max(0, Math.min(100, usage))) + "%";
                }
            }

            if (Number.isFinite(total) && Number.isFinite(idle)) {
                root.previousCpuTotal = total;
                root.previousCpuIdle = idle;
            }
            root.mem = sample.mem || "—";
            root.gpu = sample.gpu || "—";
            root.temp = sample.temp || "—";
        } catch (error) {
            console.warn("System telemetry parse failed:", error);
        }
    }

    Process {
        id: systemProc

        command: ["bash", Quickshell.env("HOME") + "/.config/quickshell/costa/scripts/system-telemetry"]
        running: true
        stdout: StdioCollector {
            onStreamFinished: root.updateSystem(text)
        }
    }

    Process {
        id: weatherProc

        command: ["bash", Quickshell.env("HOME") + "/.config/quickshell/costa/scripts/weather"]
        running: true
        stdout: StdioCollector {
            onStreamFinished: root.weather = text.trim()
        }
    }

    Timer {
        interval: 2000
        running: true
        repeat: true
        onTriggered: root.restart(systemProc)
    }

    Timer {
        interval: 1800000
        running: true
        repeat: true
        onTriggered: root.restart(weatherProc)
    }
}
