import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Window 2.15

Rectangle {
    id: root
    width: Screen.width
    height: Screen.height
    color: config.backgroundFill || "#192330"

    property int sessionIndex: sessionModel.lastIndex
    property date now: new Date()

    Timer {
        id: clockTimer
        interval: 60000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: {
            root.now = new Date()
            // Keep ticks aligned to the wall-clock minute boundary.
            var msIntoMinute = root.now.getSeconds() * 1000 + root.now.getMilliseconds()
            interval = msIntoMinute === 0 ? 60000 : 60000 - msIntoMinute
        }
    }

    Image {
        id: wallpaper
        anchors.fill: parent
        source: config.background ? Qt.resolvedUrl(config.background) : ""
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: true
        opacity: status === Image.Ready ? 1.0 : 0.0
    }

    Rectangle {
        anchors.fill: parent
        color: "#000000"
        opacity: 0.35
    }

    Column {
        anchors.centerIn: parent
        spacing: 18
        width: 360

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatDateTime(root.now, "HH:mm")
            color: config.foreground || "#cdcecf"
            font.family: config.fontFamily || "sans-serif"
            font.pixelSize: 64
            font.bold: true
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatDateTime(root.now, "dddd, dd MMMM")
            color: config.foregroundDim || "#738091"
            font.family: config.fontFamily || "sans-serif"
            font.pixelSize: 18
        }

        TextField {
            id: userField
            width: parent.width
            height: 48
            text: userModel.lastUser
            color: config.foreground || "#cdcecf"
            font.pixelSize: 16
            leftPadding: 14
            rightPadding: 14
            verticalAlignment: Text.AlignVCenter
            selectByMouse: true
            KeyNavigation.tab: passwordField

            background: Rectangle {
                radius: 12
                color: "#cc192330"
                border.width: 2
                border.color: userField.activeFocus ? (config.accent || "#719cd6") : "#55738c"
            }
        }

        TextField {
            id: passwordField
            width: parent.width
            height: 48
            echoMode: TextInput.Password
            placeholderText: "Password"
            placeholderTextColor: config.foregroundDim || "#738091"
            color: config.foreground || "#cdcecf"
            font.pixelSize: 16
            leftPadding: 14
            rightPadding: 14
            verticalAlignment: Text.AlignVCenter
            focus: true
            KeyNavigation.backtab: userField
            Keys.onPressed: function (event) {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    loginButton.clicked()
                }
            }

            background: Rectangle {
                radius: 12
                color: "#cc192330"
                border.width: 2
                border.color: passwordField.activeFocus ? (config.accent || "#719cd6") : "#55738c"
            }
        }

        Text {
            id: errorMessage
            width: parent.width
            visible: text.length > 0
            wrapMode: Text.WordWrap
            color: "#c94f6d"
            font.pixelSize: 14
            horizontalAlignment: Text.AlignHCenter
        }

        Button {
            id: loginButton
            width: parent.width
            height: 48
            text: "Sign in"
            font.pixelSize: 16
            font.bold: true

            contentItem: Text {
                text: loginButton.text
                color: config.backgroundFill || "#192330"
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                font: loginButton.font
            }

            background: Rectangle {
                radius: 12
                color: config.accent || "#719cd6"
            }

            onClicked: {
                errorMessage.text = ""
                sddm.login(userField.text, passwordField.text, root.sessionIndex)
            }
        }
    }

    Connections {
        target: sddm
        function onLoginFailed() {
            errorMessage.text = "Authentication failed"
            passwordField.selectAll()
            passwordField.forceActiveFocus()
        }
    }

    Component.onCompleted: {
        if (userField.text.length > 0) {
            passwordField.forceActiveFocus()
        } else {
            userField.forceActiveFocus()
        }
    }
}
