import QtQuick
import Quickshell
import Quickshell.Io

// One Rust worker leased to this service instance. Closing its socket permits
// graceful microphone restoration even when Quickshell destroys/reloads QML.
Item {
    id: root
    property var shell: null
    property bool stopping: false
    property string error: ""
    property int failures: 0
    property int workerPid: 0
    readonly property bool running: owner.connected && workerPid > 0
    readonly property string executable: decodeURIComponent(Qt.resolvedUrl("../bin/foot-pedal").toString().replace(/^file:\/\//, ""))
    function launch() {
        if (stopping || running) return
        failures++
        worker.startDetached()
        connectSoon.restart()
    }
    function start() {error = ""; failures = 0; retry.stop(); launch()}
    Component.onCompleted: launch()
    Component.onDestruction: {stopping = true; retry.stop(); connectSoon.stop(); healthy.stop(); handshake.stop(); owner.connected = false}
    Process {id: worker; command: [root.executable, "--plugin-session"]}
    Socket {
        id: owner
        path: Quickshell.env("XDG_RUNTIME_DIR") + "/foot-pedal/owner.sock"
        parser: SplitParser {onRead: function(data) {
            try {
                root.workerPid = JSON.parse(data).pid || 0
                if (root.workerPid > 0) {retry.stop(); handshake.stop(); healthy.restart(); root.error = ""}
            } catch(e) {root.error = "Could not read controls startup response"}
        }}
        onConnectionStateChanged: {
            if (connected) handshake.restart()
            else {
                root.workerPid = 0
                healthy.stop(); handshake.stop()
                if (!root.stopping) retry.restart()
            }
        }
        onError: if (!root.stopping) retry.restart()
    }
    Timer {id: connectSoon; interval: 150; onTriggered: {owner.connected = true; retry.restart()}}
    Timer {
        id: retry; interval: 2000
        onTriggered: {
            if (root.running || root.stopping) return
            if (owner.connected) {restart(); return}
            if (root.failures < 5) root.launch()
            else root.error = "Controls could not start. If upgrading from 1.x, run foot-pedal-uninstall first. See the README for supported systems."
        }
    }
    Timer {id: handshake; interval: 15000; onTriggered: {owner.connected = false; retry.restart()}}
    Timer {id: healthy; interval: 10000; onTriggered: root.failures = 0}
    IpcHandler {
        target: "sudonim.foot-pedal-worker"
        function status(): string {return JSON.stringify({running:root.running,error:root.error,failures:root.failures,pid:root.workerPid})}
    }
}
