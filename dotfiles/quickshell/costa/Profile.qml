pragma Singleton

import Quickshell
import Quickshell.Io
import QtQuick

Singleton {
    id: root

    property string role: "single"
    property string primaryMonitor: ""
    property bool telemetryGpu: false
    property string clockFormat: "24h"

    readonly property string configRoot: Quickshell.env("HOME") + "/.config/quickshell/costa"

    function isPrimary(screenName) {
        if (!screenName)
            return false;
        if (root.role === "single" || root.primaryMonitor === "") {
            const screens = Quickshell.screens;
            return screens.length > 0 && screens[0].name === screenName;
        }
        return screenName === root.primaryMonitor;
    }

    function applyProfile(payload) {
        try {
            const data = JSON.parse(payload.trim() || "{}");
            if (typeof data.role === "string" && data.role !== "")
                root.role = data.role;
            if (typeof data.primary_monitor === "string")
                root.primaryMonitor = data.primary_monitor;
            if (typeof data.telemetry_gpu === "boolean")
                root.telemetryGpu = data.telemetry_gpu;
        } catch (error) {
            console.warn("Quickshell profile parse failed:", error);
        }
    }

    function applyUser(payload) {
        try {
            const data = JSON.parse(payload.trim() || "{}");
            if (data.clock_format === "12h" || data.clock_format === "24h")
                root.clockFormat = data.clock_format;
        } catch (error) {
            console.warn("Quickshell user settings parse failed:", error);
        }
    }

    FileView {
        path: root.configRoot + "/profile.json"
        watchChanges: true
        onFileChanged: reload()
        onLoaded: root.applyProfile(text())
    }

    FileView {
        path: root.configRoot + "/user.json"
        watchChanges: true
        onFileChanged: reload()
        onLoaded: root.applyUser(text())
    }
}
