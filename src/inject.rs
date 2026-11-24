//! Delivering a finished transcript to the focused window, and desktop notifications.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

/// A daemon started outside the graphical session (a terminal over SSH, a service) has no
/// WAYLAND_DISPLAY; without it neither typing nor the clipboard can reach the desktop.
pub fn ensure_session_env() {
    let runtime = crate::config::runtime_dir();
    if std::env::var_os("XDG_RUNTIME_DIR").is_none() {
        std::env::set_var("XDG_RUNTIME_DIR", &runtime);
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        if let Ok(entries) = std::fs::read_dir(&runtime) {
            let mut sockets: Vec<String> = entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.starts_with("wayland-") && !name.ends_with(".lock"))
                .collect();
            sockets.sort();
            if let Some(socket) = sockets.first() {
                std::env::set_var("WAYLAND_DISPLAY", socket);
            }
        }
    }
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        let bus = runtime.join("bus");
        if bus.exists() {
            std::env::set_var("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}", bus.display()));
        }
    }
}

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let reason = String::from_utf8_lossy(&output.stderr);
        Err(format!("{program}: {}", reason.trim().lines().last().unwrap_or("failed")))
    }
}

pub fn copy(text: &str) -> Result<(), String> {
    let mut child = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("wl-copy: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("wl-copy: no stdin")?
        .write_all(text.as_bytes())
        .map_err(|e| format!("wl-copy: {e}"))?;
    match child.wait() {
        Ok(status) if status.success() => Ok(()),
        _ => Err("wl-copy failed".into()),
    }
}

/// Returns how the text was delivered. Typing falls back to the clipboard rather than losing
/// a transcript: the user can still paste it.
pub fn deliver(text: &str, mode: &str) -> Result<&'static str, String> {
    match mode {
        "print" => {
            println!("{text}");
            Ok("printed")
        }
        "copy" => copy(text).map(|_| "copied to the clipboard"),
        "paste" => {
            copy(text)?;
            std::thread::sleep(std::time::Duration::from_millis(80));
            run("wtype", &["-M", "ctrl", "v", "-m", "ctrl"]).map(|_| "pasted")
        }
        _ => match run("wtype", &["--", text]) {
            Ok(()) => Ok("typed"),
            Err(typing) => match copy(text) {
                Ok(()) => Err(format!("could not type ({typing}); the text is on the clipboard")),
                Err(clipboard) => Err(format!("could not type ({typing}) or copy ({clipboard})")),
            },
        },
    }
}

static NOTIFICATION_ID: AtomicU32 = AtomicU32::new(0);

/// One notification that is replaced in place, so a dictation never stacks popups.
pub fn notify(title: &str, body: &str, timeout_ms: u32, urgent: bool) {
    let previous = NOTIFICATION_ID.load(Ordering::Relaxed).to_string();
    let timeout = timeout_ms.to_string();
    let mut command = Command::new("notify-send");
    command.args(["-a", "OSTT", "-i", "audio-input-microphone", "-p", "-t", &timeout]);
    command.args(["-u", if urgent { "critical" } else { "low" }]);
    if previous != "0" {
        command.args(["-r", &previous]);
    }
    command.arg(title);
    if !body.is_empty() {
        command.arg(body);
    }
    if let Ok(output) = command.stdin(Stdio::null()).output() {
        if let Ok(id) = String::from_utf8_lossy(&output.stdout).trim().parse::<u32>() {
            NOTIFICATION_ID.store(id, Ordering::Relaxed);
        }
    }
}
