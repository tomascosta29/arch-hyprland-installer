pragma Singleton

import Quickshell
import Quickshell.Io
import QtQuick

Singleton {
    id: root

    property string cpu: "…"
    property string gpu: "…"
    property string memUsed: "…"
    property string memTotal: "…"
    property string cpuTemp: "…"
    property string gpuTemp: "…"
    property string vramUsed: "…"
    property string vramTotal: "…"
    property string gpuPower: ""
    property string gpuFan: ""
    property string weather: ""
    property int diskFreeGib: 0
    property real zramUsedBytes: 0
    property string connectivity: "checking"
    property string networkInterface: ""
    property int latencyMs: 0
    property int updates: 0
    property string updateState: "checking"
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
            root.memUsed = sample.mem_used || "—";
            root.memTotal = sample.mem_total || "—";
            root.gpu = sample.gpu || "—";
            root.cpuTemp = sample.cpu_temp || "—";
            root.gpuTemp = sample.gpu_temp || "—";
            root.vramUsed = sample.vram_used || "—";
            root.vramTotal = sample.vram_total || "—";
            root.gpuPower = sample.gpu_power || "";
            root.gpuFan = sample.gpu_fan || "";
        } catch (error) {
            console.warn("System telemetry parse failed:", error);
        }
    }

    function updateStatus(payload) {
        try {
            const sample = JSON.parse(payload.trim());
            root.diskFreeGib = Number(sample.disk_free_gib) || 0;
            root.zramUsedBytes = Number(sample.zram_used_bytes) || 0;
            root.connectivity = sample.connectivity || "checking";
            root.networkInterface = sample.interface || "";
            root.latencyMs = Number(sample.latency_ms) || 0;
        } catch (error) {
            console.warn("Status telemetry parse failed:", error);
        }
    }

    function updatePackages(payload) {
        try {
            const sample = JSON.parse(payload.trim());
            root.updateState = sample.class || "error";
            const count = Number.parseInt(sample.text || "0", 10);
            root.updates = Number.isFinite(count) ? count : 0;
        } catch (error) {
            root.updateState = "error";
            console.warn("Package telemetry parse failed:", error);
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

    Process {
        id: statusProc

        command: [Quickshell.env("HOME") + "/.config/quickshell/costa/scripts/status-telemetry"]
        running: true
        stdout: StdioCollector {
            onStreamFinished: root.updateStatus(text)
        }
    }

    Process {
        id: packageProc

        command: [Quickshell.env("HOME") + "/.config/scripts/check_updates"]
        running: true
        stdout: StdioCollector {
            onStreamFinished: root.updatePackages(text)
        }
    }

    Timer {
        interval: 5000
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

    Timer {
        interval: 60000
        running: true
        repeat: true
        onTriggered: root.restart(statusProc)
    }

    Timer {
        interval: 1800000
        running: true
        repeat: true
        onTriggered: root.restart(packageProc)
    }
}
