import QtQuick
import QtQuick.Controls as Controls
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.Commons
import qs.Ui

Panel {
    id: root
    moduleName: "sudonim.foot-pedal"
    ipcTarget: "sudonim.foot-pedal"
    implicitWidth: barButton.implicitWidth
    implicitHeight: barButton.implicitHeight
    readonly property color fg: Color.popups.text
    property var state: ({connected:false,input_ready:false,startup:false,presets:[], actions:[], sources:[], pressed:[false,false,false], errors:[], enabled:false})
    property string focusedControl: ""
    property bool online: false
    readonly property string releaseVersion: "1.2.2"
    readonly property string managerPath: decodeURIComponent(Qt.resolvedUrl("../scripts/manage.py").toString().replace(/^file:\/\//, ""))
    property var installation: ({installed:false,version:""})
    property bool installationChecked: false
    property string setupMessage: ""
    readonly property bool maintenance: manager.running
    readonly property bool needsUpdate: (online && state.version !== releaseVersion) || (installationChecked && installation.installed && installation.version !== releaseVersion)
    property string error: ""
    property string page: "main"
    property string selectedPreset: "workspaces"
    property var draft: ({id:"", name:"", actions:[{type:"none"},{type:"none"},{type:"none"}]})
    property int pedal: 1
    property bool dirty: false
    property bool discardPrompt: false
    property bool recording: false
    property bool pending: false
    property string pendingOp: ""
    property int serial: 0
    readonly property var currentPreset: findPreset(state.active)
    readonly property var currentAction: draft.actions[pedal] || ({type:"none"})
    readonly property bool testing: state.testing === true
    readonly property var actionOptions: (state.actions || []).map(function(a) { return {value:a.value,label:a.label + (a.available ? "" : " (needs " + a.dependency + ")")} })
    function copy(o) { return JSON.parse(JSON.stringify(o)) }
    function findPreset(id) { return (state.presets || []).find(function(p) { return p.id === id }) || {name:"Workspaces + dictation",actions:[{type:"workspace_previous"},{type:"dictation"},{type:"workspace_next"}]} }
    function actionName(a) { var row=(state.actions || []).find(function(x) {return x.value===a.type}); return row ? row.label : a.type }
    function chord(a) { return (a.modifiers || []).map(function(m){return {ctrl:"Ctrl",shift:"Shift",alt:"Alt",logo:"Super"}[m]}).concat(a.key || []).join(" + ") }
    function navigate(to) { page=to; error=""; discardPrompt=false; scroll.contentY=0; Qt.callLater(function(){keys.forceActiveFocus()}) }
    function request(op, extra) {
        if (!online) {if (op!=="refresh" && op!=="test_heartbeat") error="Controls aren't running. Install or start controls below.";return}
        var msg=extra || {}; msg.op=op;msg.id=++serial
        if (op!=="test_heartbeat" && op!=="refresh") {pending=true;pendingOp=op; requestTimeout.restart()}
        if (op==="refresh") connectionTimeout.restart()
        connection.write(JSON.stringify(msg)+"\n");connection.flush()
    }
    function receive(line) {
        try {
            var msg=JSON.parse(line)
            if (msg.type==="offline") {online=false;error=msg.error;return}
            if (!online) error=""
            online=true;connectionTimeout.stop()
            if (msg.type==="state") state=msg.state
            if (msg.type==="result") {
                if (msg.op!=="test_heartbeat" && msg.op!=="refresh") {pending=false;requestTimeout.stop()}
                if (!msg.ok) {error=msg.error;return}
                if (msg.state && msg.state.presets) state=msg.state
                if (msg.op==="save" || msg.op==="activate") {dirty=false;navigate("main")}
                if (msg.op==="delete") selectedPreset=state.active
                if (msg.op==="test_start") navigate("test")
                if (msg.op==="test_end") navigate("main")
            }
        } catch(e) {error="Could not read pedal service response"}
    }
    function edit(p, duplicate) {
        draft=copy(p);draft.id=(p.builtin || duplicate) ? "" : p.id
        draft.name=(p.builtin || duplicate) ? (p.id==="push-to-talk" ? "My push-to-talk" : "My daily controls") : p.name
        dirty=false;pedal=1;recording=false;navigate("edit");request("refresh")
    }
    function patch(fields) {
        var d=copy(draft),a=d.actions[pedal];for (var k in fields) a[k]=fields[k]
        d.actions[pedal]=a;draft=d;dirty=true
    }
    function chooseAction(value) {
        var d=copy(draft);d.actions[pedal]={type:value};draft=d;dirty=true
    }
    function setName(value) {var d=copy(draft);d.name=value;draft=d;dirty=true}
    function back() {
        if(recording){recording=false;return}
        if(page==="test"){request("test_end");return}
        if(page==="edit" && dirty){discardPrompt=true;return}
        if(page!=="main")navigate("main");else root.close()
    }
    function capture(e) {
        e.accepted=true
        if(e.isAutoRepeat)return
        if(e.key===Qt.Key_Escape){recording=false;return}
        if([Qt.Key_Control,Qt.Key_Shift,Qt.Key_Alt,Qt.Key_Meta,Qt.Key_Super_L,Qt.Key_Super_R].indexOf(e.key)>=0)return
        var key="", mods=[]
        if(e.key>=Qt.Key_A && e.key<=Qt.Key_Z)key=String.fromCharCode(e.key).toLowerCase()
        else if(e.key>=Qt.Key_0 && e.key<=Qt.Key_9)key=String.fromCharCode(e.key)
        else if(e.key>=Qt.Key_F1 && e.key<=Qt.Key_F35)key="F"+(e.key-Qt.Key_F1+1)
        else {var map={};map[Qt.Key_Return]="Return";map[Qt.Key_Enter]="Return";map[Qt.Key_Tab]="Tab";map[Qt.Key_Space]="space";map[Qt.Key_Backspace]="BackSpace";map[Qt.Key_Delete]="Delete";map[Qt.Key_Insert]="Insert";map[Qt.Key_Home]="Home";map[Qt.Key_End]="End";map[Qt.Key_PageUp]="Page_Up";map[Qt.Key_PageDown]="Page_Down";map[Qt.Key_Left]="Left";map[Qt.Key_Right]="Right";map[Qt.Key_Up]="Up";map[Qt.Key_Down]="Down";map[Qt.Key_Minus]="minus";map[Qt.Key_Equal]="equal";map[Qt.Key_Comma]="comma";map[Qt.Key_Period]="period";map[Qt.Key_Slash]="slash";key=map[e.key] || ""}
        if(!key){error="That key isn't supported. Try a letter, arrow or function key.";return}
        if(e.modifiers & Qt.ControlModifier)mods.push("ctrl")
        if(e.modifiers & Qt.ShiftModifier)mods.push("shift")
        if(e.modifiers & Qt.AltModifier)mods.push("alt")
        if(e.modifiers & Qt.MetaModifier)mods.push("logo")
        patch({key:key,modifiers:mods});recording=false;error=""
    }
    onOpenedChanged: {
        recording=false
        if(opened){probe.running=true;request("refresh");if(page==="main")selectedPreset=state.active || "workspaces"}
        else if(testing)request("test_end")
    }
    function disconnected() {
        online=false;pending=false;requestTimeout.stop();connectionTimeout.stop();reconnect.restart()
    }
    function timedOut() {
        connection.connected=false;disconnected();error="Request timed out. Check controls and try again."
    }
    Component.onCompleted: {connection.connected=true;probe.running=true}
    Socket {
        id: connection
        path: Quickshell.env("XDG_RUNTIME_DIR")+"/foot-pedal/control.sock"
        parser: SplitParser {onRead: function(data){root.receive(data)}}
        onConnectionStateChanged: {
            if(connected){reconnect.stop();connectionTimeout.restart()}
            else root.disconnected()
        }
        onError: root.disconnected()
    }
    Timer {id: reconnect;interval:2000;onTriggered:connection.connected=true}
    Timer {id: connectionTimeout;interval:12000;onTriggered:root.timedOut()}
    Timer {id: requestTimeout;interval:12000;onTriggered:root.timedOut()}
    Timer {
        interval:1000;running:root.opened && root.testing && root.online;repeat:true
        onTriggered:root.request("test_heartbeat")
    }
    Process {id: starter;command:["systemctl","--user","start","streamdeck-pedal-actions.service"];onExited:function(code){if(code!==0)root.error="Unable to start pedal controls. Check the user service journal.";else {root.error="";reconnect.restart()}}}
    Process {
        id: probe
        command:["python3",root.managerPath,"status"]
        stdout: SplitParser {onRead:function(data){try {root.installation=JSON.parse(data);root.installationChecked=true} catch(e){root.error="Could not check controls installation"}}}
        onExited:function(code){if(code!==0)root.error="Could not check controls. Verify Python 3 is installed."}
    }
    function installControls() {
        error="";setupMessage="";manager.result=({});manager.running=true
    }
    Process {
        id: manager
        property var result: ({})
        command:["python3",root.managerPath,"install"]
        stdout: SplitParser {onRead:function(data){try {manager.result=JSON.parse(data)} catch(e){}}}
        onExited:function(code){
            if(code!==0 || !result.ok)root.error=result.error || "Controls setup failed. Run install.sh in a terminal for details."
            else {root.setupMessage=result.message;root.navigate("main")}
            probe.running=true
            reconnect.restart()
        }
    }
    IpcHandler {
        target:"sudonim.foot-pedal-status"
        function status():string{return JSON.stringify({online:root.online,page:root.page,opened:root.opened,dirty:root.dirty,selectedPreset:root.selectedPreset,draft:root.draft,focus:root.focusedControl,recording:root.recording,pending:root.pending,error:root.error,state:root.state,installation:root.installation,maintenance:root.maintenance,setupMessage:root.setupMessage,theme:{background:String(Color.popups.background),text:String(root.fg),accent:String(Color.accent),font:Style.font.family,radius:Style.cornerRadius},geometry:{x:popup.cardOrigin.x,y:popup.cardOrigin.y,width:popup.contentWidth,height:popup.contentHeight}})}
    }
    component Label: Text {
        color:root.fg;font.family:Style.font.family;font.pixelSize:Style.font.body
        textFormat:Text.PlainText;wrapMode:Text.WordWrap
    }
    component Caption: Label {font.pixelSize:Style.font.bodySmall;opacity:0.75}
    component Action: Button {
        objectName:text
        foreground:root.fg;fontFamily:Style.font.family;focusable:true;enabled:!root.pending && !root.maintenance
        implicitHeight:Math.max(Style.space(32),implicitContentHeight)
        property real implicitContentHeight:Style.space(32)
        opacity:enabled ? 1 : 0.45
        onActiveFocusChanged:if(activeFocus)root.ensureVisible(this)
    }
    function ensureVisible(item) {
        focusedControl=item.objectName || ""
        var y=item.mapToItem(content,0,0).y
        if(y<scroll.contentY)scroll.contentY=Math.max(0,y-Style.space(4))
        else if(y+item.height>scroll.contentY+scroll.height)scroll.contentY=Math.min(Math.max(0,content.height-scroll.height),y+item.height-scroll.height+Style.space(4))
    }
    component Field: TextField {objectName:"field";width:parent.width;foreground:root.fg;onActiveFocusChanged:if(activeFocus)root.ensureVisible(this)}
    component Divider: Rectangle {width:parent.width;height:1;color:root.fg;opacity:0.16}
    component Pedals: Row {
        id: diagram
        property bool testMode:false
        width:parent.width;spacing:Style.space(8)
        Repeater {
            model:3
            delegate: Item {
                id:pedalItem
                required property int index
                width:(diagram.width-diagram.spacing*2)*(index===1 ? 0.4 : 0.3);height:Style.space(124)
                readonly property bool down:root.state.pressed && root.state.pressed[index]
                Action {
                    objectName:"pedal:"+pedalItem.index;anchors.fill:parent;text:"";bordered:true;selected:diagram.testMode && pedalItem.down
                    enabled:diagram.testMode ? false : root.online && !root.pending
                    opacity:1
                    Accessible.name:["Left","Middle","Right"][pedalItem.index]+" pedal: "+root.actionName(root.currentPreset.actions[pedalItem.index])
                    onClicked:{root.edit(root.currentPreset,false);root.pedal=pedalItem.index}
                }
                Column {
                    anchors.fill:parent;anchors.margins:Style.space(10);spacing:Style.space(12)
                    Caption {text:["LEFT","MIDDLE","RIGHT"][pedalItem.index]}
                    Label {font.pixelSize:Style.font.display;color:Color.accent;text:diagram.testMode ? (pedalItem.down ? "↓" : "—") : ["←","◎","→"][pedalItem.index]}
                    Label {width:parent.width;text:diagram.testMode ? (pedalItem.down ? "Pressed" : "Released") : root.actionName(root.currentPreset.actions[pedalItem.index]);font.bold:true}
                }
            }
        }
    }
    BarIconButton {
        id:barButton;anchors.fill:parent;bar:root.bar;text:""
        iconComponent: Component {
            Canvas {
                id:icon
                readonly property color ink:root.online && root.state.enabled ? barButton.foreground : Color.muted
                onInkChanged:requestPaint();onWidthChanged:requestPaint();onHeightChanged:requestPaint()
                onPaint:{var c=getContext("2d");c.reset();c.scale(width/24,height/24);c.strokeStyle=ink;c.lineWidth=1.5;c.strokeRect(1.5,6,5,13);c.strokeRect(8.5,4,7,15);c.strokeRect(17.5,6,5,13)}
            }
        }
        tooltipText:"Foot Pedal · "+(!root.online ? "Controls offline" : root.testing ? "Testing" : !root.state.connected ? "Disconnected" : !root.state.enabled ? "Paused" : root.currentPreset.name)
        onPressed:root.toggle()
    }
    KeyboardPanel {
        id:popup;anchorItem:barButton;owner:root;bar:root.bar;open:root.opened
        padding:Style.spacing.panelPadding;focusTarget:keys
        contentWidth:popup.fittedContentWidth(Style.space(root.page==="main" || root.page==="test" ? 580 : 520))
        contentHeight:popup.fittedContentHeight(content.implicitHeight,Style.space(780))
        WlrLayershell.keyboardFocus:root.recording && root.opened ? WlrKeyboardFocus.Exclusive : (open ? (focusPrimed ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.Exclusive) : WlrKeyboardFocus.None)
        FocusScope {
            id:keys;anchors.fill:parent
            Keys.onPressed:function(e){if(root.recording)root.capture(e);else if(e.key===Qt.Key_Escape){root.back();e.accepted=true}}
            Keys.onReleased:function(e){if(root.recording)e.accepted=true}
            Flickable {
                id:scroll;anchors.fill:parent;contentHeight:content.implicitHeight;clip:true;boundsBehavior:Flickable.StopAtBounds
                Controls.ScrollBar.vertical:Controls.ScrollBar {}
                Column {
                    id:content;width:parent.width;spacing:Style.spacing.panelGap;enabled:!root.maintenance
                    Row {
                        width:parent.width
                        Column {
                            width:parent.width-closeButton.width;spacing:Style.space(5)
                            Label {font.pixelSize:Style.font.heading;font.bold:true;text:root.page==="main" ? "Foot Pedal" : root.page==="presets" ? "Choose a preset" : root.page==="test" ? "Test your pedals" : ["Left","Middle","Right"][root.pedal]+" pedal"}
                            Caption {width:parent.width;text:!root.online ? "Controls aren't running" : !root.state.connected ? (root.state.device_error || "Pedal disconnected — reconnect USB") : "Elgato Stream Deck Pedal · Connected"}
                        }
                        Action {id:closeButton;text:root.page==="main" ? "×" : "←";onClicked:root.back();tooltipText:root.page==="main" ? "Close" : "Back"}
                    }
                    Label {width:parent.width;visible:root.error!=="" || !!root.state.config_error;text:root.error || root.state.config_error || "";color:Color.urgent}
                    Caption {width:parent.width;visible:root.maintenance || root.setupMessage!=="";text:root.maintenance ? "Installing controls…" : root.setupMessage}
                    Column {
                        width:parent.width;spacing:Style.space(8)
                        visible:root.installationChecked && (!root.installation.installed || root.needsUpdate)
                        Caption {width:parent.width;text:root.needsUpdate ? "A controls update is ready. Your presets and startup preference will be kept." : "Install the background controls to use the pedal. This adds a user service and starts it at sign-in. No administrator access is needed."}
                        Action {text:root.needsUpdate ? "Update controls" : "Install controls";bordered:true;onClicked:root.installControls()}
                    }
                    Action {visible:!root.online && root.installation.installed;text:"Start controls";bordered:true;onClicked:starter.running=true}
                    Caption {width:parent.width;visible:root.state.device_error==="Device access denied";text:"USB access needs setup. Follow the USB permissions instructions in the plugin README, then reconnect the pedal."}
                    Column {
                        visible:root.discardPrompt;width:parent.width;spacing:Style.space(8)
                        Label {text:"Discard unsaved changes?";font.bold:true}
                        Row {spacing:Style.space(8);Action {text:"Keep editing";bordered:true;onClicked:root.discardPrompt=false} Action {text:"Discard";bordered:true;onClicked:{root.dirty=false;root.navigate("main")}}}
                    }
                    Column {
                        visible:root.page==="main" && root.online;width:parent.width;spacing:Style.spacing.panelGap
                        Divider {}
                        Row {
                            width:parent.width
                            Column {width:parent.width-presetsButton.width-Style.space(12);spacing:Style.space(5)
                                Caption {text:"ACTIVE PRESET"}
                                Label {width:parent.width;text:root.currentPreset.name;font.pixelSize:Style.font.title;font.bold:true}
                            }
                            Action {id:presetsButton;text:"Change preset ↓";bordered:true;enabled:root.online && !root.pending;onClicked:{root.selectedPreset=root.state.active;root.navigate("presets")}}
                        }
                        Caption {width:parent.width;text:root.state.active==="workspaces" ? "Your original setup. Tap once to run an action." : "Select a pedal to change its action."}
                        Caption {width:parent.width;visible:root.state.connected && !root.state.input_ready;text:"Press and release any pedal once to initialize input."}
                        Pedals {}
                        Caption {width:parent.width;text:"Select a pedal to edit. Changes are saved as a preset."}
                        Repeater {model:root.state.errors || [];delegate:Label {required property string modelData;required property int index;width:content.width;visible:modelData!=="";text:["Left","Middle","Right"][index]+": "+modelData;color:Color.urgent}}
                        Divider {}
                        Toggle {width:parent.width;label:"Pedal actions · "+(root.state.enabled ? "On" : "Off");description:"Pause all three pedals without changing your preset.";checked:root.state.enabled===true;foreground:root.fg;enabled:root.online && !root.pending;onClicked:root.request("enabled",{value:!root.state.enabled})}
                        Toggle {width:parent.width;label:"Start at sign-in · "+(root.state.startup ? "On" : "Off");description:"Keep your controls ready in the background.";checked:root.state.startup===true;foreground:root.fg;enabled:root.online && !root.pending;onClicked:root.request("startup",{value:!root.state.startup})}
                        Divider {}
                        Row {width:parent.width;Caption {width:parent.width-testButton.width;anchors.verticalCenter:parent.verticalCenter;text:root.pending ? "Saving…" : "All changes saved"} Action {id:testButton;text:"Test pedals";bordered:true;enabled:root.online && root.state.connected && !root.pending;onClicked:root.request("test_start")}}
                    }
                    Column {
                        visible:root.page==="presets";width:parent.width;spacing:Style.spacing.panelGap
                        Caption {text:"Choose a starting point for all three pedals."}
                        Repeater {
                            model:root.state.presets || []
                            delegate:Item {
                                id:presetItem;required property var modelData
                                width:parent.width;height:presetInfo.implicitHeight+Style.space(24)
                                Action {objectName:"preset:"+presetItem.modelData.id;anchors.fill:parent;selected:root.selectedPreset===presetItem.modelData.id;bordered:true;onClicked:root.selectedPreset=presetItem.modelData.id}
                                Column {id:presetInfo;anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;anchors.margins:Style.space(12);spacing:Style.space(8)
                                    Label {width:parent.width;font.bold:true;text:presetItem.modelData.name+(root.state.active===presetItem.modelData.id ? " · Active" : "")}
                                    Caption {width:parent.width;text:presetItem.modelData.description || "Custom preset"}
                                    Caption {width:parent.width;text:presetItem.modelData.actions.map(root.actionName).join(" / ")}
                                }
                            }
                        }
                        Row {spacing:Style.space(8)
                            Action {text:"+ Custom";bordered:true;onClicked:root.edit(root.currentPreset,true)}
                            Action {text:root.findPreset(root.selectedPreset).builtin ? "Customize" : "Edit";bordered:true;onClicked:root.edit(root.findPreset(root.selectedPreset),false)}
                            Action {text:"Duplicate";bordered:true;onClicked:root.edit(root.findPreset(root.selectedPreset),true)}
                            Action {text:"Delete";visible:!root.findPreset(root.selectedPreset).builtin;enabled:root.selectedPreset!==root.state.active && !root.pending;onClicked:root.request("delete",{preset:root.selectedPreset})}
                        }
                        Divider {}
                        Row {width:parent.width;Caption {width:parent.width-usePreset.width;anchors.verticalCenter:parent.verticalCenter;text:root.selectedPreset==="push-to-talk" ? "Choose a microphone next." : "Select a preset, then apply it."}
                            Action {id:usePreset;text:root.selectedPreset==="push-to-talk" ? "Set up mic →" : "Use preset";bordered:true;onClicked:{if(root.selectedPreset==="push-to-talk")root.edit(root.findPreset(root.selectedPreset),true);else root.request("activate",{preset:root.selectedPreset})}}
                        }
                    }
                    Column {
                        visible:root.page==="edit";width:parent.width;spacing:Style.spacing.panelGap
                        Row {spacing:Style.space(8);Repeater {model:["Left","Middle","Right"];delegate:Action {required property string modelData;required property int index;text:modelData;selected:root.pedal===index;bordered:true;onClicked:{root.pedal=index;root.recording=false}}}}
                        Caption {text:"ACTION"}
                        Dropdown {id:actionPicker;width:parent.width;showLabel:false;options:root.actionOptions;value:root.currentAction.type;onChanged:function(value){root.chooseAction(value)}}
                        Caption {width:parent.width;visible:root.currentAction.type==="dictation";text:"Tap once to start Voxtype dictation; tap again to stop."}
                        Column {visible:root.currentAction.type==="shortcut";width:parent.width;spacing:Style.space(8)
                            Caption {text:"SHORTCUT"}
                            Label {width:parent.width;text:root.recording ? "Press a shortcut now · Esc cancels" : root.chord(root.currentAction) || "No shortcut recorded";font.pixelSize:Style.font.title;color:root.recording ? Color.accent : root.fg}
                            Action {text:root.recording ? "Cancel recording" : "Record shortcut";bordered:true;onClicked:{root.recording=!root.recording;keys.forceActiveFocus()}}
                            Caption {width:parent.width;text:"Sent to the focused app once per press."}
                        }
                        Column {visible:root.currentAction.type.indexOf("mic_")===0;width:parent.width;spacing:Style.space(8)
                            Caption {text:"MICROPHONE"}
                            Dropdown {width:parent.width;showLabel:false;options:[{value:"",label:"Choose a microphone…"}].concat(root.state.sources || []);value:root.currentAction.source || "";onChanged:function(value){root.patch({source:value})}}
                            Action {text:"Refresh microphones";onClicked:root.request("refresh")}
                            Caption {width:parent.width;text:root.currentAction.type==="mic_hold" ? "Unmutes this microphone while held. Release restores its previous mute state." : "Toggles mute on this microphone. This is separate from dictation."}
                        }
                        Column {visible:root.currentAction.type==="command";width:parent.width;spacing:Style.space(8)
                            Caption {text:"EXECUTABLE"}
                            Field {text:root.currentAction.executable || "";placeholderText:"/path/to/program";onTextEdited:root.patch({executable:text})}
                            Caption {text:"ARGUMENTS"}
                            Field {text:root.currentAction.arguments || "";placeholderText:'--option "two words"';onTextEdited:root.patch({arguments:text})}
                            Caption {text:"WORKING DIRECTORY (OPTIONAL)"}
                            Field {text:root.currentAction.cwd || "";placeholderText:"Home directory by default";onTextEdited:root.patch({cwd:text})}
                            Caption {width:parent.width;text:"Arguments support quotes. No shell expansion or pipes. Commands time out after 10 seconds."}
                        }
                        Divider {}
                        Label {text:"Trigger: "+(root.currentAction.type==="mic_hold" ? "While held" : "On press")}
                        Caption {text:root.draft.id ? "PRESET NAME" : "SAVE AS PRESET"}
                        Field {id:presetName;text:root.draft.name;placeholderText:"My daily controls";onTextEdited:root.setName(text)}
                        Caption {width:parent.width;text:root.draft.id ? "Save updates this custom preset." : "Your other pedal assignments are kept. Built-in presets stay available."}
                        Row {spacing:Style.space(8);Action {text:"Cancel";bordered:true;onClicked:root.back()} Action {text:"Save & use";bordered:true;enabled:root.online && !root.pending && !root.recording;onClicked:root.request("save",{preset:root.draft})}}
                    }
                    Column {
                        visible:root.page==="test";width:parent.width;spacing:Style.spacing.panelGap
                        Label {width:parent.width;font.bold:true;text:root.testing ? "Test mode is on" : "Test session ended"}
                        Caption {width:parent.width;text:root.testing ? "Actions are paused. Presses only light up the diagram." : "Normal controls have resumed. Start another test to check inputs."}
                        Pedals {testMode:true}
                        Caption {text:"LAST INPUT"}
                        Label {width:parent.width;font.bold:true;text:root.state.last_input ? ["Left","Middle","Right"][root.state.last_input.pedal]+" pedal "+(root.state.last_input.pressed ? "pressed" : "released") : "Waiting for a pedal press…"}
                        Caption {width:parent.width;text:root.state.last_input ? "Assigned action: "+root.actionName(root.currentPreset.actions[root.state.last_input.pedal]) : "Press and release each of the three pedals."}
                        Divider {}
                        Caption {width:parent.width;text:"Your previous action state returns when you leave."}
                        Row {spacing:Style.space(8);Action {text:"Done testing";bordered:true;onClicked:root.request("test_end")} Action {visible:!root.testing;text:"Test again";bordered:true;enabled:root.online && root.state.connected && !root.pending;onClicked:root.request("test_start")}}
                    }
                }
            }
        }
    }
}
