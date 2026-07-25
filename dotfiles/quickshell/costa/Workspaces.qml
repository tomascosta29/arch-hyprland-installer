pragma Singleton
import Quickshell
import Quickshell.Hyprland
import QtQuick

Singleton {
    id: root

    function workspaceById(id) {
        const list = Hyprland.workspaces.values;
        for (let i = 0; i < list.length; ++i) {
            if (list[i].id === id)
                return list[i];
        }
        return null;
    }

    function isOccupied(id) {
        const ws = workspaceById(id);
        return !!(ws && ws.toplevels && ws.toplevels.values && ws.toplevels.values.length > 0);
    }

    function isActive(id) {
        const ws = workspaceById(id);
        return !!(ws && ws.active);
    }

    function isUrgent(id) {
        const ws = workspaceById(id);
        return !!(ws && ws.urgent);
    }

    function activate(id) {
        Hyprland.dispatch("workspace " + id);
    }
}
