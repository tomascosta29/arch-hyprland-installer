//@ pragma UseQApplication
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import QtQuick

Scope {
    PersistentProperties {
        id: shellState

        reloadableId: "costaShellState"
        property bool caffeine: false
    }

    IpcHandler {
        target: "stage"

        function show(title: string, detail: string): void {
            StageBus.push("󰋼", title, detail, -1, Colors.cyan, 2600, 20, "");
        }

        function volume(): void {
            StageBus.revealVolume();
        }

        function media(): void {
            StageBus.revealMedia(true);
        }
    }

    IpcHandler {
        target: "activity"

        function upsert(id: string, title: string, detail: string, progress: real, monitor: string, priority: int): void {
            ActivityBus.upsertExternal(id, title, detail, progress, monitor, priority);
        }

        function complete(id: string, detail: string): void {
            ActivityBus.completeExternal(id, detail);
        }

        function fail(id: string, detail: string): void {
            ActivityBus.failExternal(id, detail);
        }

        function remove(id: string): void {
            ActivityBus.removeExternal(id);
        }

        function clear(): void {
            ActivityBus.clearExternal();
        }

        function count(monitor: string): int {
            return ActivityBus.countForMonitor(monitor);
        }
    }

    Variants {
        model: Quickshell.screens

        PanelWindow {
            id: panel
            required property var modelData
            screen: modelData

            anchors {
                top: true
                left: true
                right: true
            }

            margins {
                top: Colors.barMargin
                left: Colors.barSideMargin
                right: Colors.barSideMargin
            }

            implicitHeight: Colors.barHeight
            exclusiveZone: Colors.barHeight
            color: "transparent"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None

            Bar {
                anchors.centerIn: parent
                width: parent.width
                height: parent.height
                modelData: panel.modelData
                persistentState: shellState
            }
        }
    }
}
