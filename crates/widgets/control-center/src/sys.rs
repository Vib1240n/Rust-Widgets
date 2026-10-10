//! Running system commands without blocking the UI.

use gtk4::{gio, glib};
use std::process::{Command, Stdio};

/// stdout of a successful command
pub fn output(cmd: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(cmd).args(args).stderr(Stdio::null()).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Run and wait (use from worker threads only)
pub fn run(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Fire-and-forget through `sh -c` (~ expanded), fully detached via setsid
/// so it outlives the control center and never becomes a zombie.
pub fn shell(cmdline: &str) {
    let home = std::env::var("HOME").unwrap_or_default();
    let line = match cmdline.strip_prefix("~/") {
        Some(rest) => format!("{home}/{rest}"),
        None => cmdline.to_string(),
    };
    let _ = Command::new("setsid")
        .args(["-f", "sh", "-c", &line])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Do `work` on a worker thread, then `done` with its result on the UI thread.
pub fn bg<T, W, D>(work: W, done: D)
where
    T: Send + 'static,
    W: FnOnce() -> T + Send + 'static,
    D: FnOnce(T) + 'static,
{
    glib::spawn_future_local(async move {
        if let Ok(v) = gio::spawn_blocking(work).await {
            done(v);
        }
    });
}
