use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
pub type Result<T> = std::result::Result<T, String>;
pub const ACTIONS: &[(&str, &str, Option<&str>)] = &[
    ("workspace_previous", "Previous workspace", Some("hyprctl")),
    ("workspace_next", "Next workspace", Some("hyprctl")),
    ("dictation", "Toggle dictation", Some("voxtype")),
    ("media_previous", "Previous track", Some("busctl")),
    ("media_toggle", "Play or pause", Some("busctl")),
    ("media_next", "Next track", Some("busctl")),
    ("mic_toggle", "Toggle microphone mute", Some("pactl")),
    ("mic_hold", "Hold to unmute", Some("pactl")),
    ("shortcut", "Keyboard shortcut", Some("wtype")),
    ("command", "Run command", None),
    ("none", "No action", None),
];
pub fn defaults() -> Value {
    json!({"version":1,"enabled":true,"active":"workspaces","presets":[]})
}
pub fn builtins() -> Vec<Value> {
    [("workspaces","Workspaces + dictation","Your original setup",["workspace_previous","dictation","workspace_next"]),
    ("media","Media controls","Keep listening, hands free",["media_previous","media_toggle","media_next"]),
    ("push-to-talk","Push-to-talk","Choose a microphone before using",["none","mic_hold","none"])]
    .iter().map(|(id,name,description,a)|json!({"id":id,"name":name,"description":description,"builtin":true,"actions":a.map(|t|json!({"type":t}))})).collect()
}
pub fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()))
}
pub fn expand(s: &str) -> String {
    if s == "~" {
        home().to_string_lossy().into()
    } else if let Some(tail) = s.strip_prefix("~/") {
        home().join(tail).to_string_lossy().into()
    } else {
        s.into()
    }
}
pub fn executable(s: &str) -> bool {
    #[cfg(test)]
    if crate::process::FAKE.with_borrow(|f| f.is_some()) {
        return true;
    }
    let check = |p: PathBuf| {
        p.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    if s.contains('/') {
        check(s.into())
    } else {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .any(|p| check(p.join(s)))
    }
}
pub fn source_valid(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && s.len() <= 250
        && !s.ends_with(".monitor")
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.:-".contains(&c))
}
pub fn action(a: &Value, available: bool) -> Result<Value> {
    let kind = a["type"].as_str().ok_or("Choose a valid action")?;
    let (_, label, dep) = ACTIONS
        .iter()
        .find(|x| x.0 == kind)
        .ok_or("Choose a valid action")?;
    if available && dep.is_some_and(|d| !executable(d)) {
        return Err(format!("{label} needs {}", dep.unwrap()));
    }
    let mut out = json!({"type":kind});
    if kind.starts_with("mic_") {
        let source = a["source"].as_str().unwrap_or("");
        if !source_valid(source) {
            return Err("Choose a microphone for this action".into());
        }
        out["source"] = json!(source);
    }
    if kind == "shortcut" {
        let key = a["key"].as_str().unwrap_or("");
        let special = [
            "Return",
            "Tab",
            "Escape",
            "space",
            "BackSpace",
            "Delete",
            "Insert",
            "Home",
            "End",
            "Page_Up",
            "Page_Down",
            "Left",
            "Right",
            "Up",
            "Down",
            "minus",
            "equal",
            "comma",
            "period",
            "slash",
            "semicolon",
            "apostrophe",
            "bracketleft",
            "bracketright",
            "backslash",
            "grave",
        ];
        let valid = (key.len() == 1
            && key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
            || special.contains(&key)
            || key
                .strip_prefix('F')
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| (1..=35).contains(&n) && key == format!("F{n}"));
        if !valid {
            return Err("Record a supported keyboard shortcut".into());
        }
        let mods = a.get("modifiers").cloned().unwrap_or(json!([]));
        let mods = mods.as_array().ok_or("Invalid shortcut modifiers")?;
        if mods.len() > 4
            || mods
                .iter()
                .any(|m| !matches!(m.as_str(), Some("ctrl" | "shift" | "alt" | "logo")))
        {
            return Err("Invalid shortcut modifiers".into());
        }
        let mut unique = Vec::new();
        for m in mods {
            if !unique.contains(m) {
                unique.push(m.clone());
            }
        }
        out["key"] = json!(key);
        out["modifiers"] = json!(unique);
    }
    if kind == "command" {
        let exe = a["executable"]
            .as_str()
            .ok_or("Enter an executable")?
            .trim();
        let args = a
            .get("arguments")
            .map(|v| v.as_str().ok_or("Arguments must be text"))
            .unwrap_or(Ok(""))?;
        let cwd = a
            .get("cwd")
            .map(|v| v.as_str().ok_or("Working directory must be text"))
            .unwrap_or(Ok(""))?
            .trim();
        if exe.is_empty() || exe.len() > 4096 || exe.contains('\0') {
            return Err("Enter an executable".into());
        }
        if args.len() > 8192 || args.contains('\0') {
            return Err("Arguments are too long or invalid".into());
        }
        crate::arguments::split(args).map_err(|e| e.to_string())?;
        if cwd.len() > 4096 || cwd.contains('\0') {
            return Err("Invalid working directory".into());
        }
        let exe = expand(exe);
        let cwd = expand(cwd);
        if available && !executable(&exe) {
            return Err("Executable not found or not executable".into());
        }
        if available && !cwd.is_empty() && !Path::new(&cwd).is_dir() {
            return Err("Working directory does not exist".into());
        }
        out["executable"] = json!(exe);
        out["arguments"] = json!(args);
        out["cwd"] = json!(cwd);
    }
    Ok(out)
}
pub fn preset(p: &Value, available: bool) -> Result<Value> {
    let name = p["name"].as_str().unwrap_or("").trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err("Give the preset a name (up to 60 characters)".into());
    }
    let acts = p["actions"]
        .as_array()
        .filter(|a| a.len() == 3)
        .ok_or("A preset must have three pedals")?;
    let id = p.get("id").and_then(Value::as_str).unwrap_or("");
    Ok(
        json!({"id":id,"name":name,"builtin":false,"actions":acts.iter().map(|a|action(a,available)).collect::<Result<Vec<_>>>()?}),
    )
}
pub fn validate(cfg: Value) -> Result<Value> {
    if cfg["version"] != 1 || !cfg["enabled"].is_boolean() {
        return Err("Unsupported configuration".into());
    }
    let presets = cfg["presets"]
        .as_array()
        .ok_or("Unsupported configuration")?
        .iter()
        .map(|p| preset(p, false))
        .collect::<Result<Vec<_>>>()?;
    let mut ids = std::collections::HashSet::new();
    for p in &presets {
        let id = p["id"].as_str().unwrap();
        if !id.starts_with("custom-") || !ids.insert(id) {
            return Err("Invalid preset IDs".into());
        }
    }
    if !["workspaces", "media"].contains(&cfg["active"].as_str().unwrap_or(""))
        && !ids.contains(cfg["active"].as_str().unwrap_or(""))
    {
        return Err("Invalid active preset".into());
    }
    Ok(json!({"version":1,"enabled":cfg["enabled"],"active":cfg["active"],"presets":presets}))
}
pub fn atomic_write(path: &Path, value: &Value) -> Result<()> {
    let parent = path.parent().ok_or("Invalid file path")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = parent.join(format!(
        ".pedal-{}-{}.tmp",
        std::process::id(),
        crate::unique_id()
    ));
    let result = (|| -> io::Result<()> {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
        f.write_all(b"\n")?;
        f.sync_all()?;
        fs::rename(&tmp, path)?;
        fs::File::open(parent)?.sync_all()
    })();
    let _ = fs::remove_file(tmp);
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcut_and_command_validation() {
        assert!(action(&json!({"type":"shortcut","key":"$(touch /tmp/no)"}), false).is_err());
        assert!(
            action(
                &json!({"type":"shortcut","key":"m","modifiers":["super"]}),
                false
            )
            .is_err()
        );
        let a = action(
            &json!({"type":"shortcut","key":"F35","modifiers":["ctrl","ctrl"]}),
            false,
        )
        .unwrap();
        assert_eq!(a["modifiers"], json!(["ctrl"]));
        assert!(action(&json!({"type":"shortcut","key":"F36"}), false).is_err());
        assert!(
            action(
                &json!({"type":"command","executable":"echo","arguments":"'unclosed"}),
                false
            )
            .is_err()
        );
        assert!(action(&json!({"type":"mic_hold","source":"source.monitor"}), false).is_err());
        assert!(!source_valid("--help"));
        let a=action(&json!({"type":"command","executable":"echo","arguments":"\"two words\" \"$(touch /tmp/no)\""}),false).unwrap();
        assert_eq!(
            crate::arguments::split(a["arguments"].as_str().unwrap()).unwrap(),
            ["two words", "$(touch /tmp/no)"]
        );
    }
    #[test]
    fn rejects_duplicate_custom_ids_and_unknown_active() {
        let p = json!({"id":"custom-one","name":"A","actions":[{"type":"none"},{"type":"none"},{"type":"none"}]});
        let mut c = defaults();
        c["presets"] = json!([p.clone(), p]);
        assert!(validate(c).is_err());
        let mut c = defaults();
        c["active"] = json!("missing");
        assert!(validate(c).is_err());
    }
}
