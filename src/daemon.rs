use crate::{
    config::{self, Result},
    device,
    process::{cmd, run},
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    rc::Rc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, unix::AsyncFd},
    net::{UnixListener, UnixStream},
    sync::{Mutex, Notify, watch},
    task::JoinHandle,
    time::Instant,
};

struct State {
    config: Value,
    config_error: String,
    connected: bool,
    device_error: String,
    pressed: [bool; 3],
    armed: [bool; 3],
    baseline: bool,
    errors: [String; 3],
    busy: HashSet<usize>,
    epoch: u64,
    test: Option<u64>,
    test_until: Instant,
    last_input: Value,
    sources: Value,
    actions: Value,
    closing: bool,
    rescan: bool,
    monitor: bool,
}
struct Hold {
    muted: bool,
    owners: HashSet<usize>,
}
pub struct Daemon {
    config_path: PathBuf,
    runtime: PathBuf,
    state: RefCell<State>,
    holds: Mutex<HashMap<String, Hold>>,
    mutation: Mutex<()>,
    wake: Notify,
    updates: watch::Sender<String>,
    tasks: RefCell<Vec<JoinHandle<()>>>,
}
impl Daemon {
    pub fn new(config_path: PathBuf, runtime: PathBuf) -> Rc<Self> {
        let mut config = config::defaults();
        let mut error = String::new();
        if config_path.exists() {
            let loaded = fs::read(&config_path)
                .map_err(|e| e.to_string())
                .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
                .and_then(config::validate);
            match loaded {
                Ok(c) => config = c,
                Err(e) => {
                    error = format!(
                        "Configuration needs attention: {e}. Original setup loaded, actions paused."
                    );
                    config["enabled"] = json!(false);
                }
            }
        }
        let (updates, _) = watch::channel(String::new());
        let daemon = Rc::new(Self {
            config_path,
            runtime,
            state: RefCell::new(State {
                config,
                config_error: error,
                connected: false,
                device_error: String::new(),
                pressed: [false; 3],
                armed: [false; 3],
                baseline: false,
                errors: Default::default(),
                busy: HashSet::new(),
                epoch: 0,
                test: None,
                test_until: Instant::now(),
                last_input: Value::Null,
                sources: json!([]),
                actions: json!([]),
                closing: false,
                rescan: true,
                monitor: false,
            }),
            holds: Mutex::new(HashMap::new()),
            mutation: Mutex::new(()),
            wake: Notify::new(),
            updates,
            tasks: RefCell::new(Vec::new()),
        });
        daemon.refresh_actions();
        daemon
    }
    fn presets(&self) -> Vec<Value> {
        let mut presets = config::builtins();
        presets.extend(
            self.state.borrow().config["presets"]
                .as_array()
                .unwrap()
                .clone(),
        );
        presets
    }
    fn active(&self) -> Value {
        let id = self.state.borrow().config["active"].clone();
        self.presets().into_iter().find(|p| p["id"] == id).unwrap()
    }
    fn allowed(&self) -> bool {
        let s = self.state.borrow();
        s.config["enabled"] == true && s.test.is_none() && s.connected && !s.closing
    }
    fn valid_hold(&self, i: usize, epoch: u64) -> bool {
        let s = self.state.borrow();
        s.epoch == epoch && s.pressed[i] && self.allowed()
    }
    fn refresh_actions(&self) {
        self.state.borrow_mut().actions=json!(config::ACTIONS.iter().map(|(value,label,dep)|json!({"value":value,"label":label,"available":dep.is_none_or(config::executable),"dependency":dep})).collect::<Vec<_>>());
    }
    pub fn status(&self) -> Value {
        let s = self.state.borrow();
        let mut busy = s.busy.iter().copied().collect::<Vec<_>>();
        busy.sort();
        json!({"version":env!("CARGO_PKG_VERSION"),"runtime":"rust","lifecycle":"plugin","discovery":if s.monitor {"udev"}else{"poll"},"connected":s.connected,"device_error":s.device_error,"config_error":s.config_error,"input_ready":s.baseline,"enabled":s.config["enabled"],"startup":true,"active":s.config["active"],"presets":self.presets(),"pressed":s.pressed,"last_input":s.last_input,"errors":s.errors,"busy":busy,"testing":s.test.is_some(),"sources":s.sources,"actions":s.actions})
    }
    fn publish(&self) {
        let packet = format!("{}\n", json!({"type":"state","state":self.status()}));
        self.updates.send_if_modified(|old| {
            if *old == packet {
                false
            } else {
                *old = packet;
                true
            }
        });
        self.wake.notify_one();
    }
    fn spawn(self: &Rc<Self>, future: impl std::future::Future<Output = ()> + 'static) {
        let mut tasks = self.tasks.borrow_mut();
        tasks.retain(|t| !t.is_finished());
        tasks.push(tokio::task::spawn_local(future));
    }
    async fn sources(&self) {
        let rows = cmd(&["pactl", "--format=json", "list", "sources"])
            .await
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .unwrap_or(json!([]));
        self.state.borrow_mut().sources=json!(rows.as_array().unwrap_or(&vec![]).iter().filter(|r|r["name"].as_str().is_some_and(config::source_valid)).map(|r|json!({"value":r["name"],"label":r.get("description").unwrap_or(&r["name"])})).collect::<Vec<_>>());
    }
    fn input(self: &Rc<Self>, current: [bool; 3]) {
        if !current.iter().any(|v| *v) {
            self.state.borrow_mut().baseline = true;
        }
        for (i, down) in current.into_iter().enumerate() {
            let (changed, fire, epoch) = {
                let mut s = self.state.borrow_mut();
                let was = s.pressed[i];
                s.pressed[i] = down;
                if !down {
                    s.armed[i] = true;
                }
                let fire = down && !was && s.armed[i];
                if down != was {
                    s.last_input = json!({"pedal":i,"pressed":down,"time":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64()});
                    if down {
                        s.armed[i] = false;
                    }
                }
                (down != was, fire, s.epoch)
            };
            if changed {
                let d = self.clone();
                if !down {
                    self.spawn(async move {
                        d.release(Some(i)).await;
                    });
                } else if fire && self.allowed() {
                    let action = self.active()["actions"][i].clone();
                    self.spawn(async move {
                        d.execute(i, action, epoch).await;
                    });
                }
            }
        }
        self.publish();
    }
    fn disconnect(self: &Rc<Self>, reason: &str) {
        {
            let mut s = self.state.borrow_mut();
            s.connected = false;
            s.baseline = false;
            s.device_error = reason.into();
            s.pressed = [false; 3];
            s.armed = [false; 3];
            s.epoch += 1;
            s.rescan = true;
        }
        let d = self.clone();
        self.spawn(async move {
            d.release(None).await;
        });
        self.publish();
    }
    async fn execute(self: Rc<Self>, i: usize, action: Value, epoch: u64) {
        if self.state.borrow().epoch != epoch || !self.allowed() {
            return;
        }
        {
            let mut s = self.state.borrow_mut();
            if !s.busy.insert(i) {
                s.errors[i] = "Previous action is still running".into();
                drop(s);
                self.publish();
                return;
            }
            s.errors[i].clear();
        }
        self.publish();
        if let Err(e) = self.perform(i, &action, epoch).await {
            self.state.borrow_mut().errors[i] = e;
        }
        self.state.borrow_mut().busy.remove(&i);
        self.publish();
    }
    async fn perform(&self, i: usize, a: &Value, epoch: u64) -> Result<()> {
        let a = config::action(a, true)?;
        let kind = a["type"].as_str().unwrap();
        match kind {
            "none" => {}
            "workspace_previous" | "workspace_next" => {
                let step = if kind == "workspace_previous" {
                    "e-1"
                } else {
                    "e+1"
                };
                cmd(&[
                    "hyprctl",
                    "eval",
                    &format!("hl.dispatch(hl.dsp.focus({{ workspace = \"{step}\" }}))"),
                ])
                .await?;
            }
            "dictation" => {
                cmd(&["voxtype", "record", "toggle"]).await?;
            }
            "media_previous" | "media_toggle" | "media_next" => {
                let all = cmd(&["busctl", "--user", "--no-pager", "--no-legend", "list"]).await?;
                let mut players = all
                    .lines()
                    .filter_map(|l| l.split_whitespace().next())
                    .filter(|p| p.starts_with("org.mpris.MediaPlayer2."))
                    .collect::<Vec<_>>();
                players.sort();
                let mut target = *players.first().ok_or("No media player is available")?;
                for p in &players {
                    if cmd(&[
                        "busctl",
                        "--user",
                        "get-property",
                        p,
                        "/org/mpris/MediaPlayer2",
                        "org.mpris.MediaPlayer2.Player",
                        "PlaybackStatus",
                    ])
                    .await
                    .is_ok_and(|s| s.contains("\"Playing\""))
                    {
                        target = p;
                        break;
                    }
                }
                let method = match kind {
                    "media_previous" => "Previous",
                    "media_next" => "Next",
                    _ => "PlayPause",
                };
                cmd(&[
                    "busctl",
                    "--user",
                    "call",
                    target,
                    "/org/mpris/MediaPlayer2",
                    "org.mpris.MediaPlayer2.Player",
                    method,
                ])
                .await?;
            }
            "shortcut" => {
                let mut args = vec!["wtype".to_string()];
                let mods = a["modifiers"].as_array().unwrap();
                for m in mods {
                    args.extend(["-M".into(), m.as_str().unwrap().into()]);
                }
                args.extend(["-k".into(), a["key"].as_str().unwrap().into()]);
                for m in mods.iter().rev() {
                    args.extend(["-m".into(), m.as_str().unwrap().into()]);
                }
                run(&args, None, Duration::from_secs(5)).await?;
            }
            "command" => {
                let mut args = vec![a["executable"].as_str().unwrap().into()];
                args.extend(
                    crate::arguments::split(a["arguments"].as_str().unwrap())
                        .map_err(|e| e.to_string())?,
                );
                let cwd = a["cwd"].as_str().unwrap();
                let cwd = if cwd.is_empty() {
                    config::home()
                } else {
                    cwd.into()
                };
                run(&args, Some(&cwd), Duration::from_secs(10)).await?;
            }
            "mic_toggle" => {
                let holds = self.holds.lock().await;
                let source = a["source"].as_str().unwrap();
                if holds.contains_key(source) {
                    return Err("Microphone is held by push-to-talk".into());
                }
                cmd(&["pactl", "set-source-mute", source, "toggle"]).await?;
            }
            "mic_hold" => {
                let mut holds = self.holds.lock().await;
                if !self.valid_hold(i, epoch) {
                    return Ok(());
                }
                let source = a["source"].as_str().unwrap();
                if !holds.contains_key(source) {
                    let muted = cmd(&["pactl", "get-source-mute", source])
                        .await?
                        .to_lowercase()
                        .contains("yes");
                    if !self.valid_hold(i, epoch) {
                        return Ok(());
                    }
                    holds.insert(
                        source.into(),
                        Hold {
                            muted,
                            owners: HashSet::new(),
                        },
                    );
                    // Persist restoration before changing the microphone.
                    if let Err(e) = self.write_recovery(&holds) {
                        holds.remove(source);
                        return Err(e);
                    }
                    if let Err(e) = cmd(&["pactl", "set-source-mute", source, "0"]).await {
                        let _ = self.restore(&mut holds, source).await;
                        return Err(e);
                    }
                }
                holds.get_mut(source).unwrap().owners.insert(i);
                if !self.valid_hold(i, epoch) {
                    holds.get_mut(source).unwrap().owners.remove(&i);
                    if holds[source].owners.is_empty() {
                        self.restore(&mut holds, source).await?;
                    }
                }
            }
            _ => return Err("Unknown action".into()),
        }
        Ok(())
    }
    fn write_recovery(&self, holds: &HashMap<String, Hold>) -> Result<()> {
        config::atomic_write(
            &self.runtime.join("microphone-recovery.json"),
            &json!(
                holds
                    .iter()
                    .map(|(k, h)| (k, h.muted))
                    .collect::<HashMap<_, _>>()
            ),
        )
    }
    async fn restore(&self, holds: &mut HashMap<String, Hold>, source: &str) -> Result<()> {
        cmd(&[
            "pactl",
            "set-source-mute",
            source,
            if holds[source].muted { "1" } else { "0" },
        ])
        .await?;
        holds.remove(source);
        self.write_recovery(holds)?;
        for e in &mut self.state.borrow_mut().errors {
            if e.starts_with("Microphone restor") {
                e.clear();
            }
        }
        Ok(())
    }
    async fn release(&self, i: Option<usize>) {
        let mut holds = self.holds.lock().await;
        let sources = holds.keys().cloned().collect::<Vec<_>>();
        for source in sources {
            if let Some(i) = i {
                holds.get_mut(&source).unwrap().owners.remove(&i);
            } else {
                holds.get_mut(&source).unwrap().owners.clear();
            }
            if holds[&source].owners.is_empty()
                && let Err(e) = self.restore(&mut holds, &source).await
            {
                self.state.borrow_mut().errors[i.filter(|i| *i < 3).unwrap_or(1)] =
                    format!("Microphone restore failed: {e}");
            }
        }
        drop(holds);
        self.publish();
    }
    async fn recover(&self) {
        let pending = fs::read(self.runtime.join("microphone-recovery.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .unwrap_or(json!({}));
        if let Some(pending) = pending.as_object() {
            let mut holds = self.holds.lock().await;
            for (source, muted) in pending {
                if config::source_valid(source)
                    && let Some(muted) = muted.as_bool()
                {
                    holds.insert(
                        source.clone(),
                        Hold {
                            muted,
                            owners: HashSet::new(),
                        },
                    );
                }
            }
        }
        self.release(None).await;
    }
    async fn transition(&self) {
        {
            let mut s = self.state.borrow_mut();
            s.epoch += 1;
            s.armed = s.pressed.map(|p| !p);
        }
        self.release(None).await;
    }
    async fn save(&self, cfg: Value) -> Result<()> {
        if !self.state.borrow().config_error.is_empty() {
            return Err(format!(
                "{} Fix the file before saving.",
                self.state.borrow().config_error
            ));
        }
        config::atomic_write(&self.config_path, &cfg)?;
        {
            let mut s = self.state.borrow_mut();
            s.config = cfg;
            s.errors = Default::default();
        }
        self.transition().await;
        self.publish();
        Ok(())
    }
    async fn rpc(&self, msg: &Value, owner: u64) -> Result<Value> {
        let op = msg["op"].as_str().ok_or("Expected a request object")?;
        if op == "status" {
            return Ok(self.status());
        }
        if op == "refresh" {
            self.state.borrow_mut().rescan = true;
            self.refresh_actions();
            self.sources().await;
            self.publish();
            return Ok(self.status());
        }
        if op == "test_heartbeat" {
            if self.state.borrow().test == Some(owner) {
                self.state.borrow_mut().test_until = Instant::now() + Duration::from_secs(4);
                self.wake.notify_one();
            }
            return Ok(json!({}));
        }
        let _guard = self.mutation.lock().await;
        match op {
            "enabled" => {
                let value = msg["value"].as_bool().ok_or("Expected On or Off")?;
                let mut cfg = self.state.borrow().config.clone();
                cfg["enabled"] = json!(value);
                self.save(cfg).await?;
            }
            "activate" => {
                let p = self
                    .presets()
                    .into_iter()
                    .find(|p| p["id"] == msg["preset"])
                    .ok_or("Preset not found")?;
                config::preset(&p, true)?;
                let mut cfg = self.state.borrow().config.clone();
                cfg["active"] = p["id"].clone();
                self.save(cfg).await?;
            }
            "save" => {
                let mut p = config::preset(&msg["preset"], true)?;
                if p["actions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|a| a["type"].as_str().unwrap().starts_with("mic_"))
                {
                    self.sources().await;
                }
                for a in p["actions"].as_array().unwrap() {
                    if a["type"].as_str().unwrap().starts_with("mic_")
                        && !self
                            .state
                            .borrow()
                            .sources
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|s| s["value"] == a["source"])
                    {
                        return Err("Selected microphone is not available".into());
                    }
                }
                let mut cfg = self.state.borrow().config.clone();
                let presets = cfg["presets"].as_array_mut().unwrap();
                if p["id"] != "" && !presets.iter().any(|x| x["id"] == p["id"]) {
                    return Err("Cannot overwrite a built-in preset".into());
                }
                if p["id"] == "" {
                    if presets.len() >= 128 {
                        return Err("Maximum of 128 custom presets reached".into());
                    }
                    p["id"] = json!(format!("custom-{}", crate::unique_id()));
                }
                presets.retain(|x| x["id"] != p["id"]);
                presets.push(p.clone());
                cfg["active"] = p["id"].clone();
                self.save(cfg).await?;
            }
            "delete" => {
                let mut cfg = self.state.borrow().config.clone();
                if msg["preset"] == cfg["active"] {
                    return Err("Switch to another preset before deleting this one".into());
                }
                let ps = cfg["presets"].as_array_mut().unwrap();
                if !ps.iter().any(|p| p["id"] == msg["preset"]) {
                    return Err("Only custom presets can be deleted".into());
                }
                ps.retain(|p| p["id"] != msg["preset"]);
                self.save(cfg).await?;
            }
            "test_start" => {
                if !self.state.borrow().connected {
                    return Err("Connect the pedal before testing".into());
                }
                if self.state.borrow().test.is_some_and(|id| id != owner) {
                    return Err("Another window is testing the pedal".into());
                }
                {
                    let mut s = self.state.borrow_mut();
                    s.test = Some(owner);
                    s.test_until = Instant::now() + Duration::from_secs(4);
                }
                self.transition().await;
            }
            "test_end" => {
                if self.state.borrow().test == Some(owner) {
                    self.end_test().await;
                }
            }
            _ => return Err("Unknown request".into()),
        }
        self.publish();
        Ok(self.status())
    }
    async fn end_test(&self) {
        self.state.borrow_mut().test = None;
        self.transition().await;
        self.publish();
    }
    async fn client(self: Rc<Self>, socket: UnixStream, owner: u64) {
        let (read, mut write) = socket.into_split();
        let mut reader = BufReader::new(read);
        let mut updates = self.updates.subscribe();
        let mut request_buffer = Vec::new();
        let initial = updates.borrow_and_update().clone();
        if write_packet(&mut write, &initial).await.is_err() {
            return;
        }
        loop {
            tokio::select! {
                request=read_request(&mut reader,&mut request_buffer)=>{
                    let Ok(Some(line))=request else{break};
                    let msg=serde_json::from_str::<Value>(&line).unwrap_or(Value::Null);
                    let result=match self.rpc(&msg,owner).await {Ok(state)=>json!({"type":"result","id":msg["id"],"ok":true,"op":msg["op"],"state":state}),Err(error)=>json!({"type":"result","id":msg["id"],"ok":false,"op":msg["op"],"error":error})};
                    if write_packet(&mut write,&format!("{result}\n")).await.is_err(){break;}
                },
                changed=updates.changed()=>{if changed.is_err(){break;}let packet=updates.borrow_and_update().clone();if write_packet(&mut write,&packet).await.is_err(){break;}}
            }
        }
        if self.state.borrow().test == Some(owner) {
            self.end_test().await;
        }
    }
    async fn watch(self: Rc<Self>) {
        let mut monitor = device::Monitor::new()
            .ok()
            .and_then(|m| AsyncFd::new(m).ok());
        self.state.borrow_mut().monitor = monitor.is_some();
        let mut hid = None;
        let mut retry = false;
        let mut next_scan = Instant::now();
        let mut next_restore = Instant::now();
        loop {
            if self.state.borrow().closing {
                break;
            }
            if hid.is_none()
                && (self.state.borrow().rescan
                    || ((monitor.is_none() || retry) && Instant::now() >= next_scan))
            {
                self.state.borrow_mut().rescan = false;
                retry = false;
                next_scan = Instant::now() + Duration::from_secs(2);
                if let Some(path) = device::find() {
                    match device::open(&path).and_then(AsyncFd::new) {
                        Ok(fd) => {
                            hid = Some(fd);
                            let mut s = self.state.borrow_mut();
                            s.connected = true;
                            s.device_error.clear();
                            s.baseline = false;
                            s.armed = [false; 3];
                        }
                        Err(e) => {
                            self.state.borrow_mut().device_error =
                                if e.kind() == std::io::ErrorKind::PermissionDenied {
                                    "Device access denied".into()
                                } else {
                                    e.to_string()
                                };
                            retry = true;
                        }
                    }
                } else {
                    self.state.borrow_mut().device_error =
                        "Pedal disconnected — reconnect USB".into();
                }
                self.publish();
            }
            if self.state.borrow().test.is_some()
                && Instant::now() >= self.state.borrow().test_until
            {
                self.end_test().await;
            }
            let needs_restore = self
                .holds
                .try_lock()
                .is_ok_and(|h| h.values().any(|v| v.owners.is_empty()));
            if needs_restore && Instant::now() >= next_restore {
                self.release(Some(usize::MAX)).await;
                next_restore = Instant::now() + Duration::from_secs(2);
            }
            let mut deadlines = Vec::new();
            if hid.is_none() && (monitor.is_none() || retry) {
                deadlines.push(next_scan);
            }
            if self.state.borrow().test.is_some() {
                deadlines.push(self.state.borrow().test_until);
            }
            if needs_restore {
                deadlines.push(next_restore);
            }
            let deadline = deadlines.into_iter().min();
            tokio::select! {
                _=self.wake.notified()=>{},
                _=async {if let Some(d)=deadline {tokio::time::sleep_until(d).await;}else{std::future::pending::<()>().await;}}=>{},
                event=async {match &monitor {Some(m)=>m.readable().await,None=>std::future::pending().await}}=>{
                    let failed=match event {Ok(mut guard)=>{let result=guard.get_inner().drain();guard.clear_ready();result.is_err()},Err(_)=>true};
                    if failed {monitor=None;self.state.borrow_mut().monitor=false;}self.state.borrow_mut().rescan=true;
                },
                event=async {match &hid {Some(fd)=>fd.readable().await,None=>std::future::pending().await}}=>{
                    let result=match event {Ok(mut guard)=>{let result=device::read(guard.get_inner());guard.clear_ready();result},Err(e)=>Err(e)};
                    match result {Ok(input)=>self.input(input),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{},Err(e)=>{hid=None;self.disconnect(&e.to_string());}}
                }
            }
        }
    }
    pub async fn serve(self: Rc<Self>, plugin_session: bool) -> Result<()> {
        // A detached worker is leased by the service's socket, so QML destruction
        // cannot SIGKILL it in the middle of restoring a held microphone.
        let owner_path = self.runtime.join("owner.sock");
        let owner_listener = if plugin_session {
            let _ = fs::remove_file(&owner_path);
            let listener = UnixListener::bind(&owner_path).map_err(|e| e.to_string())?;
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
            Some(listener)
        } else {
            None
        };
        let owner_lifetime = async move {
            let Some(listener) = owner_listener else {
                std::future::pending::<()>().await;
                return;
            };
            let accepted = tokio::time::timeout(Duration::from_secs(15), listener.accept()).await;
            if let Ok(Ok((mut owner, _))) = accepted {
                drop(listener);
                let hello = format!("{}\n", json!({"pid":std::process::id()}));
                if owner.write_all(hello.as_bytes()).await.is_ok() {
                    let mut data = [0u8; 1];
                    while matches!(owner.read(&mut data).await,Ok(n) if n>0) {}
                }
            }
        };
        tokio::pin!(owner_lifetime);
        self.recover().await;
        self.sources().await;
        self.publish();
        let socket = self.runtime.join("control.sock");
        let _ = fs::remove_file(&socket);
        let listener = UnixListener::bind(&socket).map_err(|e| e.to_string())?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
        let watcher = tokio::task::spawn_local(self.clone().watch());
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .map_err(|e| e.to_string())?;
        let mut interrupt =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                .map_err(|e| e.to_string())?;
        let mut serial = 0;
        let mut clients = Vec::<JoinHandle<()>>::new();
        loop {
            tokio::select! {
                _=terminate.recv()=>break,_=interrupt.recv()=>break,
            _=&mut owner_lifetime=>break,
                connection=listener.accept()=>{if let Ok((stream,_))=connection{clients.retain(|c|!c.is_finished());if clients.len()<32{serial+=1;clients.push(tokio::task::spawn_local(self.clone().client(stream,serial)));}}}
            }
        }
        {
            let mut s = self.state.borrow_mut();
            s.closing = true;
            s.epoch += 1;
        }
        watcher.abort();
        let _ = watcher.await;
        for c in clients {
            c.abort();
            let _ = c.await;
        }
        let tasks = std::mem::take(&mut *self.tasks.borrow_mut());
        for t in tasks {
            t.abort();
            let _ = t.await;
        }
        self.release(None).await;
        let _ = fs::remove_file(socket);
        if plugin_session {
            let _ = fs::remove_file(owner_path);
        }
        Ok(())
    }
}
async fn read_request(
    reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
    bytes: &mut Vec<u8>,
) -> std::io::Result<Option<String>> {
    let remaining = 65537usize.saturating_sub(bytes.len());
    let n = reader
        .take(remaining as u64)
        .read_until(b'\n', bytes)
        .await?;
    if n == 0 {
        return Ok(None);
    }
    if bytes.len() > 65536 || bytes.last() != Some(&b'\n') {
        return Err(std::io::Error::other("Request too large"));
    }
    String::from_utf8(std::mem::take(bytes))
        .map(Some)
        .map_err(std::io::Error::other)
}

async fn write_packet(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    packet: &str,
) -> std::io::Result<()> {
    tokio::time::timeout(Duration::from_secs(2), writer.write_all(packet.as_bytes()))
        .await
        .map_err(std::io::Error::other)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{FAKE, Fake};
    fn setup() -> (tempfile::TempDir, Rc<Daemon>) {
        FAKE.with_borrow_mut(|f| {
            *f = Some(Fake {
                muted: true,
                ..Default::default()
            })
        });
        let temp = tempfile::tempdir().unwrap();
        let d = Daemon::new(temp.path().join("config.json"), temp.path().into());
        d.state.borrow_mut().connected = true;
        (temp, d)
    }
    async fn drain(d: &Daemon) {
        loop {
            let tasks = std::mem::take(&mut *d.tasks.borrow_mut());
            if tasks.is_empty() {
                break;
            }
            for task in tasks {
                task.await.unwrap();
            }
        }
    }
    async fn press(d: &Rc<Daemon>, i: usize) {
        d.input([false; 3]);
        let mut a = [false; 3];
        a[i] = true;
        d.input(a);
        drain(d).await;
    }
    async fn mic(d: &Daemon, two: bool) {
        let a = json!({"type":"mic_hold","source":"mic"});
        d.rpc(&json!({"op":"save","preset":{"id":"","name":"Hold","actions":[if two{a.clone()}else{json!({"type":"none"})},a,{"type":"none"}]}}),1).await.unwrap();
        FAKE.with_borrow_mut(|f| f.as_mut().unwrap().calls.clear());
    }
    fn muted() -> bool {
        FAKE.with_borrow(|f| f.as_ref().unwrap().muted)
    }
    fn calls() -> Vec<Vec<String>> {
        FAKE.with_borrow(|f| f.as_ref().unwrap().calls.clone())
    }
    macro_rules! local_test {
        ($name:ident,$body:expr) => {
            #[tokio::test(flavor = "current_thread")]
            async fn $name() {
                tokio::task::LocalSet::new().run_until($body).await;
            }
        };
    }
    local_test!(original_edges_and_first_held_report, async {
        let (_t, d) = setup();
        d.input([true; 3]);
        drain(&d).await;
        assert!(calls().is_empty());
        d.input([false; 3]);
        d.input([true; 3]);
        drain(&d).await;
        d.input([true; 3]);
        drain(&d).await;
        assert_eq!(calls().len(), 3);
        assert!(calls().contains(&vec!["voxtype".into(), "record".into(), "toggle".into()]));
    });
    local_test!(test_owner_and_rearming_and_pause, async {
        let (_t, d) = setup();
        d.rpc(&json!({"op":"test_start"}), 1).await.unwrap();
        press(&d, 1).await;
        assert!(calls().is_empty());
        assert!(d.rpc(&json!({"op":"test_start"}), 2).await.is_err());
        d.rpc(&json!({"op":"test_end"}), 2).await.unwrap();
        assert_eq!(d.state.borrow().test, Some(1));
        d.rpc(&json!({"op":"test_end"}), 1).await.unwrap();
        d.input([false, true, false]);
        drain(&d).await;
        assert!(calls().is_empty());
        press(&d, 1).await;
        assert_eq!(calls().len(), 1);
        d.rpc(&json!({"op":"enabled","value":false}), 1)
            .await
            .unwrap();
        d.rpc(&json!({"op":"test_start"}), 1).await.unwrap();
        d.end_test().await;
        press(&d, 1).await;
        assert_eq!(calls().len(), 1);
    });
    local_test!(preset_persistence_and_invalid_edits, async {
        let (t, d) = setup();
        let p = json!({"id":"","name":"My shortcut","actions":[{"type":"workspace_previous"},{"type":"shortcut","key":"m","modifiers":["ctrl","shift"]},{"type":"workspace_next"}]});
        d.rpc(&json!({"op":"save","preset":p}), 1).await.unwrap();
        let before = fs::read(&d.config_path).unwrap();
        let fresh = Daemon::new(d.config_path.clone(), t.path().into());
        assert_eq!(fresh.active()["name"], "My shortcut");
        assert_eq!(fresh.active()["actions"][0]["type"], "workspace_previous");
        assert!(
            d.rpc(&json!({"op":"save","preset":config::builtins()[0]}), 1)
                .await
                .is_err()
        );
        assert!(
            d.rpc(&json!({"op":"activate","preset":"push-to-talk"}), 1)
                .await
                .is_err()
        );
        assert!(
            d.rpc(&json!({"op":"enabled","value":"false"}), 1)
                .await
                .is_err()
        );
        assert_eq!(fs::read(&d.config_path).unwrap(), before);
        fs::write(&d.config_path, b"{broken").unwrap();
        let broken = Daemon::new(d.config_path.clone(), t.path().into());
        assert_eq!(broken.status()["enabled"], false);
        assert!(broken.save(config::defaults()).await.is_err());
        assert_eq!(fs::read(&d.config_path).unwrap(), b"{broken");
    });
    local_test!(
        microphone_release_disconnect_pause_switch_and_recovery,
        async {
            for mode in ["release", "disconnect", "pause", "switch", "recovery"] {
                let (t, d) = setup();
                mic(&d, false).await;
                press(&d, 1).await;
                assert!(!muted());
                match mode {
                    "release" => {
                        d.input([false; 3]);
                        drain(&d).await;
                    }
                    "disconnect" => {
                        d.disconnect("USB removed");
                        drain(&d).await;
                    }
                    "pause" => {
                        d.rpc(&json!({"op":"enabled","value":false}), 1)
                            .await
                            .unwrap();
                    }
                    "switch" => {
                        d.rpc(&json!({"op":"activate","preset":"workspaces"}), 1)
                            .await
                            .unwrap();
                    }
                    _ => {
                        let fresh = Daemon::new(d.config_path.clone(), t.path().into());
                        fresh.recover().await;
                    }
                }
                assert!(muted(), "{mode}");
            }
        }
    );
    local_test!(microphone_initial_unmuted_overlap_and_fast_release, async {
        let (_t, d) = setup();
        mic(&d, true).await;
        d.input([false; 3]);
        d.input([true, true, false]);
        drain(&d).await;
        assert!(!muted());
        d.input([false, true, false]);
        drain(&d).await;
        assert!(!muted());
        d.input([false; 3]);
        drain(&d).await;
        assert!(muted());
        FAKE.with_borrow_mut(|f| f.as_mut().unwrap().muted = false);
        press(&d, 1).await;
        d.input([false; 3]);
        drain(&d).await;
        assert!(!muted());
        FAKE.with_borrow_mut(|f| {
            let f = f.as_mut().unwrap();
            f.muted = true;
            f.calls.clear();
        });
        d.input([false, true, false]);
        d.input([false; 3]);
        drain(&d).await;
        assert!(muted());
        assert!(!calls().iter().any(|c| c.get(3).is_some_and(|v| v == "0")));
    });
    local_test!(failed_restore_retains_journal_and_retries, async {
        let (_t, d) = setup();
        mic(&d, false).await;
        press(&d, 1).await;
        FAKE.with_borrow_mut(|f| f.as_mut().unwrap().fail_restore = true);
        d.rpc(&json!({"op":"enabled","value":false}), 1)
            .await
            .unwrap();
        assert!(d.holds.lock().await["mic"].owners.is_empty());
        let journal: Value =
            serde_json::from_slice(&fs::read(d.runtime.join("microphone-recovery.json")).unwrap())
                .unwrap();
        assert_eq!(journal["mic"], true);
        FAKE.with_borrow_mut(|f| f.as_mut().unwrap().fail_restore = false);
        d.release(Some(usize::MAX)).await;
        assert!(muted());
        assert!(d.holds.lock().await.is_empty());
    });
    local_test!(media_routes_playing_player_and_errors, async {
        let (_t, d) = setup();
        d.rpc(&json!({"op":"activate","preset":"media"}), 1)
            .await
            .unwrap();
        press(&d, 1).await;
        assert!(
            calls()
                .iter()
                .any(|c| c.last().is_some_and(|v| v == "PlayPause"))
        );
        FAKE.with_borrow_mut(|f| f.as_mut().unwrap().fail_action = true);
        press(&d, 0).await;
        assert_eq!(d.state.borrow().errors[0], "Action failed");
        assert!(d.state.borrow().errors[1].is_empty());
    });
    local_test!(state_dedup_and_test_expiry_without_idle_polling, async {
        let (_t, d) = setup();
        d.publish();
        let mut updates = d.updates.subscribe();
        updates.borrow_and_update();
        d.publish();
        assert!(!updates.has_changed().unwrap());
        // Run the actual discovery loop with no physical device present.
        d.state.borrow_mut().connected = false;
        d.state.borrow_mut().test = Some(1);
        d.state.borrow_mut().test_until = Instant::now() + Duration::from_millis(40);
        let watcher = tokio::task::spawn_local(d.clone().watch());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(d.state.borrow().test, None);
        updates.borrow_and_update();
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(!updates.has_changed().unwrap());
        watcher.abort();
        let _ = watcher.await;
    });
    local_test!(
        socket_partial_request_survives_state_push_and_owner_disconnect,
        async {
            let (_t, d) = setup();
            d.publish();
            let (server, client) = UnixStream::pair().unwrap();
            let task = tokio::task::spawn_local(d.clone().client(server, 7));
            let (read, mut write) = client.into_split();
            let mut lines = BufReader::new(read).lines();
            lines.next_line().await.unwrap();
            write.write_all(b"{\"op\":\"test_").await.unwrap();
            tokio::task::yield_now().await;
            d.state.borrow_mut().errors[0] = "push".into();
            d.publish();
            lines.next_line().await.unwrap();
            write.write_all(b"start\",\"id\":4}\n").await.unwrap();
            loop {
                let line = lines.next_line().await.unwrap().unwrap();
                let v: Value = serde_json::from_str(&line).unwrap();
                if v["type"] == "result" {
                    assert_eq!(v["ok"], true);
                    break;
                }
            }
            assert_eq!(d.state.borrow().test, Some(7));
            drop(write);
            drop(lines);
            task.await.unwrap();
            assert_eq!(d.state.borrow().test, None);
        }
    );
    #[test]
    fn hid_report_fixture() {
        use std::os::fd::FromRawFd;
        let mut fds = [0; 2];
        assert_eq!(
            unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_NONBLOCK | libc::O_CLOEXEC) },
            0
        );
        let read = unsafe { fs::File::from_raw_fd(fds[0]) };
        let mut write = unsafe { fs::File::from_raw_fd(fds[1]) };
        use std::io::Write;
        write.write_all(&[0, 0, 0, 0, 1, 0, 1, 0]).unwrap();
        assert_eq!(device::read(&read).unwrap(), [true, false, true]);
        drop(write);
        assert!(device::read(&read).is_err());
    }
    local_test!(watch_loop_dispatches_hid_and_disconnects, async {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let (t, d) = setup();
        let path = t.path().join("hid-fixture");
        let c = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        let mut writer = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&path)
            .unwrap();
        device::TEST_DEVICE.with_borrow_mut(|p| *p = Some(path.clone()));
        d.state.borrow_mut().connected = false;
        let watcher = tokio::task::spawn_local(d.clone().watch());
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(d.state.borrow().connected);
        writer.write_all(&[0u8; 8]).unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        writer.write_all(&[0, 0, 0, 0, 0, 1, 0, 0]).unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(calls().contains(&vec!["voxtype".into(), "record".into(), "toggle".into()]));
        device::TEST_DEVICE.with_borrow_mut(|p| *p = None);
        drop(writer);
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!d.state.borrow().connected);
        watcher.abort();
        let _ = watcher.await;
    });
    local_test!(
        plugin_owner_disconnect_restores_microphone_before_exit,
        async {
            let (_t, d) = setup();
            let serving = tokio::task::spawn_local(d.clone().serve(true));
            for _ in 0..100 {
                if d.runtime.join("owner.sock").exists() {
                    break;
                }
                tokio::task::yield_now().await;
            }
            let owner = UnixStream::connect(d.runtime.join("owner.sock"))
                .await
                .unwrap();
            let mut owner = BufReader::new(owner);
            let mut hello = String::new();
            owner.read_line(&mut hello).await.unwrap();
            assert!(hello.contains("pid"));
            d.state.borrow_mut().connected = true;
            mic(&d, false).await;
            press(&d, 1).await;
            assert!(!muted());
            drop(owner);
            tokio::time::timeout(Duration::from_secs(1), serving)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert!(muted());
            assert!(!d.runtime.join("owner.sock").exists());
            assert!(!d.runtime.join("control.sock").exists());
        }
    );
}
