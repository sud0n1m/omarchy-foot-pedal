use crate::config::Result;
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};
struct Group(u32);
impl Drop for Group {
    fn drop(&mut self) {
        unsafe {
            libc::kill(-(self.0 as i32), libc::SIGKILL);
        }
    }
}
async fn collect(mut stream: impl AsyncRead + Unpin) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let n = stream.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        let keep = n.min(65536usize.saturating_sub(out.len()));
        out.extend_from_slice(&buffer[..keep]);
    }
    Ok(out)
}
pub async fn run(args: &[String], cwd: Option<&Path>, timeout: Duration) -> Result<String> {
    #[cfg(test)]
    if let Some(result) = fake(&args.iter().map(String::as_str).collect::<Vec<_>>()) {
        return result;
    }
    let mut command = Command::new(args.first().ok_or("Missing executable")?);
    command
        .args(&args[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .process_group(0);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let group = Group(child.id().ok_or("Process did not start")?);
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let result = tokio::time::timeout(timeout, async {
        tokio::try_join!(collect(stdout), collect(stderr), child.wait())
    })
    .await;
    let result = match result {
        Ok(r) => r.map_err(|e| e.to_string()),
        Err(_) => Err(format!(
            "Action timed out after {} seconds",
            timeout.as_secs_f32()
        )),
    };
    drop(group); // Kill descendants too, including on cancellation/error.
    if result.is_err() {
        let _ = child.wait().await;
    }
    let (stdout, stderr, status) = result?;
    if !status.success() {
        let error = String::from_utf8_lossy(&stderr)
            .trim()
            .chars()
            .take(250)
            .collect::<String>();
        return Err(if error.is_empty() {
            format!("{} exited {status}", args[0])
        } else {
            error
        });
    }
    Ok(String::from_utf8_lossy(&stdout).trim().into())
}
pub async fn cmd(args: &[&str]) -> Result<String> {
    run(
        &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        None,
        Duration::from_secs(5),
    )
    .await
}

#[cfg(test)]
#[derive(Default)]
pub struct Fake {
    pub calls: Vec<Vec<String>>,
    pub muted: bool,
    pub fail_restore: bool,
    pub fail_action: bool,
}
#[cfg(test)]
thread_local! {pub static FAKE:std::cell::RefCell<Option<Fake>> = const {std::cell::RefCell::new(None)};}
#[cfg(test)]
fn fake(args: &[&str]) -> Option<Result<String>> {
    FAKE.with_borrow_mut(|f| {
        f.as_mut().map(|f| {
            f.calls.push(args.iter().map(|s| s.to_string()).collect());
            if f.fail_action {
                return Err("Action failed".into());
            }
            if args.starts_with(&["pactl", "get-source-mute"]) {
                return Ok(if f.muted { "Mute: yes" } else { "Mute: no" }.into());
            }
            if args.starts_with(&["pactl", "set-source-mute"]) {
                if f.fail_restore && args[3] == "1" {
                    return Err("Disconnected".into());
                }
                f.muted = args[3] == "1";
            }
            if args.starts_with(&["pactl", "--format=json", "list"]) {
                return Ok(r#"[{"name":"mic","description":"Test microphone"}]"#.into());
            }
            if args.first() == Some(&"busctl") {
                if args.last() == Some(&"list") {
                    return Ok("org.mpris.MediaPlayer2.test 123 player".into());
                }
                if args.contains(&"get-property") {
                    return Ok("s \"Playing\"".into());
                }
            }
            Ok(String::new())
        })
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn timeout_and_bounded_output_and_literal_arguments() {
        let error = run(
            &["/usr/bin/sleep".into(), "10".into()],
            None,
            Duration::from_millis(20),
        )
        .await
        .unwrap_err();
        assert!(error.contains("timed out"));
        let literal = cmd(&["/usr/bin/printf", "%s", "$(touch should-not-exist)"])
            .await
            .unwrap();
        assert_eq!(literal, "$(touch should-not-exist)");
        let output = cmd(&["/bin/sh", "-c", "head -c 1000000 /dev/zero | tr '\\0' x"])
            .await
            .unwrap();
        assert_eq!(output.len(), 65536);
    }
}
