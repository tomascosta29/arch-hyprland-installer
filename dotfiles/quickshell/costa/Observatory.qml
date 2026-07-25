pragma Singleton

import Quickshell
import Quickshell.Io
import QtQuick

Singleton {
    id: root

    readonly property int refreshInterval: 3000
    readonly property int refreshTimeout: 8000
    readonly property int staleAfter: 12000
    property var snapshot: root.emptySnapshot()
    property bool healthy: false
    property real refreshStartedAt: 0
    property real lastSuccessfulRefresh: 0
    property int revision: 0

    function emptySnapshot() {
        return {
            "contexts": {},
            "tasks": [],
            "audio": {
                "active": false,
                "workspace": 0
            },
            "network": {
                "interface": "",
                "rx": 0,
                "tx": 0
            }
        };
    }

    readonly property var audio: snapshot.audio || {
        "active": false,
        "workspace": 0
    }
    readonly property var tasks: snapshot.tasks || []
    readonly property real networkRx: snapshot.network ? snapshot.network.rx || 0 : 0
    readonly property real networkTx: snapshot.network ? snapshot.network.tx || 0 : 0

    function contextFor(monitorName) {
        const contexts = root.snapshot.contexts || {};
        return contexts[monitorName] || {
            "valid": false,
            "focused": false,
            "workspace": 0,
            "pid": 0,
            "address": "",
            "class": "",
            "title": "",
            "repo": "",
            "branch": "",
            "dirty": 0
        };
    }

    function refresh() {
        if (snapshotProcess.running)
            return;
        snapshotProcess.running = true;
    }

    function acceptSnapshot(value) {
        if (!value || typeof value !== "object" || Array.isArray(value))
            throw new Error("snapshot root is not an object");

        root.snapshot = {
            "contexts": value.contexts && typeof value.contexts === "object" ? value.contexts : {},
            "tasks": Array.isArray(value.tasks) ? value.tasks : [],
            "audio": value.audio && typeof value.audio === "object" ? value.audio : {
                "active": false,
                "workspace": 0
            },
            "network": value.network && typeof value.network === "object" ? value.network : {
                "interface": "",
                "rx": 0,
                "tx": 0
            }
        };
        root.lastSuccessfulRefresh = Date.now();
        root.healthy = true;
        root.revision++;
    }

    function invalidateIfStale(now) {
        if (!root.healthy || now - root.lastSuccessfulRefresh < root.staleAfter)
            return;

        console.warn("Observatory snapshot became stale; clearing dynamic state");
        root.snapshot = root.emptySnapshot();
        root.healthy = false;
        root.revision++;
    }

    Process {
        id: snapshotProcess

        command: [Quickshell.env("HOME") + "/.config/quickshell/costa/scripts/observatory-snapshot"]
        running: true
        onStarted: root.refreshStartedAt = Date.now()
        onExited: (exitCode, exitStatus) => {
            if (exitCode !== 0)
                console.warn("Observatory snapshot exited with code", exitCode);
        }
        stdout: StdioCollector {
            onStreamFinished: {
                const payload = text.trim();
                if (payload === "")
                    return;

                try {
                    root.acceptSnapshot(JSON.parse(payload));
                } catch (error) {
                    console.warn("Observatory snapshot parse failed:", error);
                }
            }
        }
    }

    Timer {
        interval: root.refreshInterval
        running: true
        repeat: true
        onTriggered: {
            const now = Date.now();
            if (snapshotProcess.running && now - root.refreshStartedAt >= root.refreshTimeout) {
                console.warn("Observatory snapshot timed out; restarting");
                snapshotProcess.running = false;
            }
            root.refresh();
            root.invalidateIfStale(now);
        }
    }
}
