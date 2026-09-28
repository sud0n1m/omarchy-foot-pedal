mod arguments;
mod config;
mod daemon;
mod device;
mod process;
mod usb_rule;
use serde_json::{Value, json};
use std::{
    fs,
    os::{
        fd::AsRawFd,
        unix::fs::{OpenOptionsExt, PermissionsExt},
    },
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};
fn unique_id() -> String {
    static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "{:x}{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}
fn paths() -> (PathBuf, PathBuf) {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| config::home().join(".config"))
        .join("foot-pedal/config.json");
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() })))
        .join("foot-pedal");
    (config, runtime)
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|s| s == "--version") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let result = tokio::task::LocalSet::new().run_until(start(args)).await;
    if let Err(error) = result {
        eprintln!("{}", json!({"type":"offline","error":error}));
        std::process::exit(1);
    }
}
async fn start(args: Vec<String>) -> config::Result<()> {
    if args.first().is_some_and(|arg| arg == "--install-usb-rule") {
        if args.len() != 1 {
            return Err("USB rule setup does not accept other arguments".into());
        }
        return usb_rule::install();
    }
    let (config, runtime) = paths();
    if let Some(arg) = args.first() {
        if arg == "--status" || arg == "--request" {
            let mut request = if arg == "--status" {
                json!({"op":"status"})
            } else {
                serde_json::from_str::<Value>(args.get(1).ok_or("Missing JSON request")?)
                    .map_err(|e| e.to_string())?
            };
            if !request.is_object() {
                return Err("Expected a JSON request object".into());
            }
            request["id"] = json!(1);
            let socket = UnixStream::connect(runtime.join("control.sock"))
                .await
                .map_err(|e| e.to_string())?;
            let (read, mut write) = socket.into_split();
            write
                .write_all(format!("{request}\n").as_bytes())
                .await
                .map_err(|e| e.to_string())?;
            let mut lines = BufReader::new(read).lines();
            while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
                let v: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
                if v["type"] == "result" {
                    println!("{v}");
                    return if v["ok"] == true {
                        Ok(())
                    } else {
                        Err(v["error"].to_string())
                    };
                }
            }
            return Err("Controls disconnected".into());
        }
        if arg != "--plugin-session" || args.len() != 1 {
            return Err(format!("Unknown argument: {arg}"));
        }
    }
    fs::create_dir_all(&runtime).map_err(|e| e.to_string())?;
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(runtime.join("daemon.lock"))
        .map_err(|e| e.to_string())?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("Pedal controls are already running. Remove the legacy user service before enabling the Rust plugin.".into());
    }
    daemon::Daemon::new(config, runtime)
        .serve(args.first().is_some_and(|s| s == "--plugin-session"))
        .await
}
