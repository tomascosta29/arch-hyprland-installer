pragma Singleton

import Quickshell
import Quickshell.Hyprland
import Quickshell.Services.Mpris
import Quickshell.Services.Pipewire
import QtQuick

Singleton {
    id: root

    signal eventRequested(string icon, string title, string detail, real progress, color tint, int duration, int priority, string action)

    readonly property var sink: Pipewire.defaultAudioSink
    property int mediaRevision: 0
    property var player: {
        root.mediaRevision;
        return root.selectPlayer(Mpris.players.values);
    }
    property string lastTrack: ""
    property bool lastPlaying: false
    property bool observersReady: false

    function push(icon, title, detail, progress, tint, duration, priority, action) {
        eventRequested(icon || "", title || "", detail || "", progress === undefined ? -1 : progress, tint || Colors.accent, duration || 2200, priority || 0, action || "");
    }

    function selectPlayer(players) {
        for (let index = 0; index < players.length; index++) {
            const candidate = players[index];
            const identity = (candidate.identity || "").toLowerCase();
            if (!identity.includes("playerctld") && candidate.canControl && candidate.isPlaying)
                return candidate;
        }
        for (let index = 0; index < players.length; index++) {
            const candidate = players[index];
            const identity = (candidate.identity || "").toLowerCase();
            if (!identity.includes("playerctld") && candidate.canControl && candidate.trackTitle)
                return candidate;
        }
        return null;
    }

    function revealVolume() {
        if (!root.sink || !root.sink.audio)
            return;

        const audio = root.sink.audio;
        const percent = Math.round(audio.volume * 100);
        root.push(audio.muted ? "󰝟" : percent >= 66 ? "󰕾" : percent >= 33 ? "󰖀" : "󰕿", audio.muted ? "Muted" : "Volume", audio.muted ? "Click to unmute" : percent + "%", audio.muted ? 0 : Math.min(1, audio.volume), audio.muted ? Colors.red : Colors.accent, 1800, 40, "volume-mute");
    }

    function adjustVolume(delta) {
        if (!root.sink || !root.sink.audio)
            return;

        const audio = root.sink.audio;
        if (audio.muted)
            audio.muted = false;
        audio.volume = Math.max(0, Math.min(1.5, audio.volume + delta));
        volumeRevealTimer.restart();
    }

    function toggleMute() {
        if (!root.sink || !root.sink.audio)
            return;
        root.sink.audio.muted = !root.sink.audio.muted;
        volumeRevealTimer.restart();
    }

    function revealMedia(force) {
        if (!root.player)
            return;

        const title = root.player.trackTitle || "Unknown track";
        const playing = root.player.isPlaying;
        if (!force && title === root.lastTrack && playing === root.lastPlaying)
            return;

        root.lastTrack = title;
        root.lastPlaying = playing;
        root.push(playing ? "󰐊" : "󰏤", title, root.player.trackArtist || root.player.identity || "", -1, Colors.lavender, 3600, 25, "media-toggle");
    }

    function revealWorkspace() {
        const workspace = Hyprland.focusedWorkspace;
        if (!workspace)
            return;

        const count = workspace.toplevels ? workspace.toplevels.values.length : 0;
        root.push("󰍹", "Workspace " + workspace.id, count === 0 ? "Empty" : count + (count === 1 ? " window" : " windows"), -1, Colors.accent, 1250, 10, "");
    }

    function activate(action) {
        if (action === "volume-mute") {
            root.toggleMute();
        } else if (action === "media-toggle" && root.player) {
            root.player.togglePlaying();
        }
    }

    function wheel(action, delta) {
        if (action === "volume-mute") {
            root.adjustVolume(delta > 0 ? 0.05 : -0.05);
        } else if (action === "media-toggle" && root.player) {
            if (delta > 0 && root.player.canGoPrevious)
                root.player.previous();
            else if (delta < 0 && root.player.canGoNext)
                root.player.next();
        }
    }

    onPlayerChanged: {
        if (root.observersReady && root.player)
            root.revealMedia(true);
        else {
            root.lastTrack = root.player ? root.player.trackTitle : "";
            root.lastPlaying = root.player ? root.player.isPlaying : false;
        }
    }

    PwObjectTracker {
        objects: root.sink ? [root.sink] : []
    }

    Connections {
        target: root.sink && root.sink.audio ? root.sink.audio : null

        function onVolumesChanged() {
            if (root.observersReady)
                volumeRevealTimer.restart();
        }

        function onMutedChanged() {
            if (root.observersReady)
                volumeRevealTimer.restart();
        }
    }

    Connections {
        target: root.player

        function onTrackTitleChanged() {
            if (root.observersReady)
                mediaRevealTimer.restart();
        }

        function onIsPlayingChanged() {
            if (root.observersReady)
                mediaRevealTimer.restart();
        }
    }

    Connections {
        target: Hyprland

        function onFocusedWorkspaceChanged() {
            if (root.observersReady)
                root.revealWorkspace();
        }
    }

    Timer {
        interval: 700
        running: true
        onTriggered: {
            root.lastTrack = root.player ? root.player.trackTitle : "";
            root.lastPlaying = root.player ? root.player.isPlaying : false;
            root.observersReady = true;
        }
    }

    Timer {
        interval: 2000
        running: true
        repeat: true
        onTriggered: root.mediaRevision++
    }

    Timer {
        id: volumeRevealTimer

        interval: 70
        onTriggered: root.revealVolume()
    }

    Timer {
        id: mediaRevealTimer

        interval: 80
        onTriggered: root.revealMedia(false)
    }
}
