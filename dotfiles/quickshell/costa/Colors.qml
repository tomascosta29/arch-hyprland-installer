pragma Singleton

import Quickshell
import Quickshell.Io
import QtQuick

Singleton {
    id: root

    // Canonical palette from theme pack colors.css (@define-color …).
    property color background: "#192330"
    property color backgroundAlt1: "#212E3F"
    property color backgroundAlt2: "#29394F"
    property color backgroundAlt3: "#39506D"
    property color backgroundAlt4: "#4F6B95"
    property color foreground: "#CDCECF"
    property color foregroundDim: "#738091"
    property color softBlue: "#719CD6"
    property color softCyan: "#63CDCF"
    property color softGreen: "#81B29A"
    property color softYellow: "#DBC074"
    property color softPeach: "#F4A261"
    property color softLavender: "#9D79D6"
    property color softRed: "#C94F6D"
    property color softGrey: "#61758A"

    // Compatibility aliases used by the bar modules.
    readonly property color backgroundAlt: backgroundAlt1
    readonly property color accent: softBlue
    readonly property color cyan: softCyan
    readonly property color lavender: softLavender
    readonly property color red: softRed
    readonly property color yellow: softYellow

    readonly property int barHeight: 44
    readonly property int barMargin: 8
    readonly property int barSideMargin: 10
    readonly property int pillRadius: 12
    readonly property int innerPillHeight: 30
    readonly property int iconSize: 17
    readonly property int dotSize: 8

    readonly property string palettePath: Quickshell.env("HOME") + "/.config/quickshell/costa/colors.css"

    function applyCss(payload) {
        const map = {};
        const pattern = /@define-color\s+([\w-]+)\s+(#[A-Fa-f0-9]{6})\s*;/gi;
        let match = pattern.exec(payload);
        while (match !== null) {
            map[match[1].toLowerCase()] = match[2];
            match = pattern.exec(payload);
        }

        function take(name, fallback) {
            return map[name] || fallback;
        }

        root.background = take("background", root.background);
        root.backgroundAlt1 = take("background-alt1", root.backgroundAlt1);
        root.backgroundAlt2 = take("background-alt2", root.backgroundAlt2);
        root.backgroundAlt3 = take("background-alt3", root.backgroundAlt3);
        root.backgroundAlt4 = take("background-alt4", root.backgroundAlt4);
        root.foreground = take("foreground", root.foreground);
        root.foregroundDim = take("foreground-dim", root.foregroundDim);
        root.softBlue = take("soft-blue", root.softBlue);
        root.softCyan = take("soft-cyan", root.softCyan);
        root.softGreen = take("soft-green", root.softGreen);
        root.softYellow = take("soft-yellow", root.softYellow);
        root.softPeach = take("soft-peach", root.softPeach);
        root.softLavender = take("soft-lavender", root.softLavender);
        root.softRed = take("soft-red", root.softRed);
        root.softGrey = take("soft-grey", root.softGrey);
    }

    FileView {
        path: root.palettePath
        watchChanges: true
        onFileChanged: reload()
        onLoaded: root.applyCss(text())
    }
}
