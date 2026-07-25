pragma Singleton

import Quickshell
import QtQuick

Singleton {
    id: root

    readonly property int externalLeaseMs: 12000
    property var activities: []
    property int revision: 0

    function externalKey(id) {
        return "external:" + id;
    }

    function activityIndex(id) {
        for (let index = 0; index < root.activities.length; index++) {
            if (root.activities[index].id === id)
                return index;
        }
        return -1;
    }

    function makeItem(id, title, detail, progress, monitor, priority, icon, tint, source, updated, expiresAt) {
        return {
            "id": id,
            "title": title || id,
            "detail": detail || "",
            "progress": progress === undefined ? -1 : progress,
            "monitor": monitor || "*",
            "priority": priority || 0,
            "icon": icon || "󰑮",
            "tint": tint || Colors.accent,
            "source": source || "external",
            "updated": updated || Date.now(),
            "expiresAt": expiresAt || 0
        };
    }

    function putInternal(item) {
        const next = root.activities.slice();
        const index = root.activityIndex(item.id);
        if (index >= 0)
            next[index] = item;
        else
            next.push(item);
        root.activities = next;
        root.revision++;
    }

    function upsertExternal(id, title, detail, progress, monitor, priority) {
        if (!id)
            return;

        const now = Date.now();
        root.putInternal(root.makeItem(root.externalKey(id), title, detail, progress, monitor, priority, "󰑮", Colors.accent, "external", now, now + root.externalLeaseMs));
    }

    function completeExternal(id, detail) {
        root.finishExternal(id, detail || "Complete", "󰄬", Colors.cyan, 5000);
    }

    function failExternal(id, detail) {
        root.finishExternal(id, detail || "Failed", "󰅖", Colors.red, 7000);
    }

    function finishExternal(id, detail, icon, tint, lifetime) {
        const index = root.activityIndex(root.externalKey(id));
        if (index < 0)
            return;

        const item = root.activities[index];
        root.putInternal(root.makeItem(item.id, item.title, detail, 1, item.monitor, item.priority + 1, icon, tint, "external", Date.now(), Date.now() + lifetime));
    }

    function removeInternal(id) {
        const next = root.activities.filter(item => item.id !== id);
        if (next.length === root.activities.length)
            return;
        root.activities = next;
        root.revision++;
    }

    function removeExternal(id) {
        root.removeInternal(root.externalKey(id));
    }

    function clearExternal() {
        const next = root.activities.filter(item => item.source !== "external");
        if (next.length === root.activities.length)
            return;
        root.activities = next;
        root.revision++;
    }

    function syncObservedTasks() {
        const tasks = Observatory.tasks;
        const previous = {};
        const seen = {};
        const next = root.activities.filter(item => item.source !== "task");

        for (let index = 0; index < root.activities.length; index++) {
            const item = root.activities[index];
            if (item.source === "task")
                previous[item.id] = item;
        }

        for (let index = 0; index < tasks.length; index++) {
            const task = tasks[index];
            const id = "task:" + task.pid;
            if (!task.pid || seen[id])
                continue;
            seen[id] = true;

            const old = previous[id];
            const discoveredAt = Date.now() - Math.max(0, Number(task.elapsed) || 0) * 1000;
            next.push(root.makeItem(
                id,
                task.title,
                "Workspace " + task.workspace + " · " + root.formatDuration(task.elapsed),
                -1,
                task.monitor || "*",
                Math.min(35, Number(task.priority) || 0),
                root.taskIcon(task.kind),
                root.taskTint(task.kind),
                "task",
                old ? old.updated : discoveredAt,
                0
            ));
        }

        root.activities = next;
        root.revision++;
    }

    function taskIcon(kind) {
        if (kind === "package")
            return "󰏖";
        if (kind === "transfer")
            return "󰇚";
        return "󰣪";
    }

    function taskTint(kind) {
        if (kind === "package")
            return Colors.yellow;
        if (kind === "transfer")
            return Colors.cyan;
        return Colors.accent;
    }

    function formatDuration(seconds) {
        const value = Math.max(0, Math.floor(Number(seconds) || 0));
        if (value < 60)
            return value + "s";
        const minutes = Math.floor(value / 60);
        const remainder = value % 60;
        if (minutes < 60)
            return minutes + "m " + remainder + "s";
        return Math.floor(minutes / 60) + "h " + (minutes % 60) + "m";
    }

    function matchesMonitor(item, monitor) {
        return item.monitor === "*" || item.monitor === "" || item.monitor === monitor;
    }

    function sourceRank(item) {
        return item.source === "external" ? 2 : 1;
    }

    function topForMonitor(monitor) {
        root.revision;
        if (!monitor)
            return null;
        const matches = root.activities.filter(item => root.matchesMonitor(item, monitor));
        matches.sort((left, right) => {
            const sourceDifference = root.sourceRank(right) - root.sourceRank(left);
            if (sourceDifference !== 0)
                return sourceDifference;
            if (left.priority !== right.priority)
                return right.priority - left.priority;
            return right.updated - left.updated;
        });
        return matches.length > 0 ? matches[0] : null;
    }

    function countForMonitor(monitor) {
        root.revision;
        if (!monitor)
            return 0;
        return root.activities.filter(item => root.matchesMonitor(item, monitor)).length;
    }

    Connections {
        target: Observatory

        function onRevisionChanged() {
            root.syncObservedTasks();
        }
    }

    Timer {
        interval: 1000
        running: true
        repeat: true
        onTriggered: {
            const now = Date.now();
            const next = root.activities.filter(item => item.expiresAt === 0 || item.expiresAt > now);
            if (next.length !== root.activities.length) {
                root.activities = next;
                root.revision++;
            }
        }
    }
}
