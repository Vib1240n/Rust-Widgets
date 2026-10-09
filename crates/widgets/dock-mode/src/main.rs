//! rw-dock: lid + dock aware internal panel switching for Hyprland.
//!
//!   externals | lid    | internal panel
//!   none      | any    | on   (never disable the only screen)
//!   present   | open   | on   (3-monitor)
//!   present   | closed | off  (externals only)
//!
//! Also:
//! - forces displays on (DPMS) when the lid opens, a monitor is added or the
//!   machine resumes, so screens never stay dark if hypridle is gone
//! - kicks one `hyprctl reload` if the dock is attached but its displays never
//!   came back from s2idle
//!
//! Starts before Hyprland and waits for it, reconnects if Hyprland restarts.
//!
//! rw-dock                                  run daemon
//! rw-dock status|reconcile|pause|unpause   talk to the running daemon

use serde::Deserialize;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{channel, Sender};
use std::thread::{sleep, spawn};
use std::time::{Duration, Instant};
use tracing::{info, warn};

const SOCK: &str = "/tmp/rw-dock.sock";

// ---------- config: ~/.config/rw/dock-mode/config.toml (all optional) ----------

#[derive(Deserialize)]
#[serde(default)]
struct Config {
    internal: String,
    internal_on: String,
    internal_off: String,
    debounce_ms: u64,
    kick_after_resume: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            internal: "eDP-1".into(),
            internal_on: r#"hl.monitor({ output = "eDP-1", mode = "2880x1800@120", position = "760x2560", scale = 1.5 })"#.into(),
            internal_off: r#"hl.monitor({ output = "eDP-1", disabled = true })"#.into(),
            debounce_ms: 300,
            kick_after_resume: true,
        }
    }
}

impl Config {
    fn load() -> Self {
        let path = dirs::config_dir().unwrap_or_default().join("rw/dock-mode/config.toml");
        match std::fs::read_to_string(&path) {
            Err(_) => Self::default(),
            Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
                warn!("{}: {e}, using defaults", path.display());
                Self::default()
            }),
        }
    }
}

// ---------- system state ----------

fn lid_closed() -> bool {
    let Ok(dir) = std::fs::read_dir("/proc/acpi/button/lid") else { return false };
    dir.flatten()
        .any(|e| std::fs::read_to_string(e.path().join("state")).is_ok_and(|s| s.contains("closed")))
}

/// Any Thunderbolt device besides the host router: `0-2` yes, `0-0` / `0-0:2.1` / `domain0` no.
fn dock_present() -> bool {
    let Ok(dir) = std::fs::read_dir("/sys/bus/thunderbolt/devices") else { return false };
    dir.flatten().any(|e| {
        let n = e.file_name();
        let n = n.to_string_lossy();
        n.split_once('-').is_some_and(|(_, r)| r != "0" && !r.contains(':'))
    })
}

// ---------- hyprland ----------

/// Newest live Hyprland instance. Blocks until one exists (we start before Hyprland).
fn hypr_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/run/user/1000"))
        .join("hypr");
    loop {
        let live = std::fs::read_dir(&base).ok().and_then(|d| {
            d.flatten()
                .filter(|e| UnixStream::connect(e.path().join(".socket2.sock")).is_ok())
                .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
                .map(|e| e.path())
        });
        if let Some(p) = live {
            return p;
        }
        sleep(Duration::from_millis(500));
    }
}

fn hypr_request(cmd: &str) -> Option<String> {
    let mut s = UnixStream::connect(hypr_dir().join(".socket.sock")).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    s.write_all(cmd.as_bytes()).ok()?;
    let mut out = String::new();
    s.read_to_string(&mut out).ok()?;
    Some(out)
}

fn hyprctl(args: &[&str]) {
    let sig = hypr_dir().file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    match Command::new("hyprctl").args(args).env("HYPRLAND_INSTANCE_SIGNATURE", sig).output() {
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout);
            if !o.status.success() || !matches!(out.trim(), "ok" | "") {
                warn!("hyprctl {args:?}: {}", out.trim());
            }
        }
        Err(e) => warn!("hyprctl: {e}"),
    }
}

#[derive(Deserialize)]
struct Monitor {
    name: String,
    disabled: bool,
}

// ---------- core ----------

struct State {
    mode: &'static str,
    paused: bool,
    resumed: Option<Instant>,
    kicked: bool,
    lid_prev: Option<bool>,
    /// set by lid-open / monitor-added / resume; consumed by reconcile
    wake: bool,
}

fn reconcile(cfg: &Config, st: &mut State) {
    let lid = lid_closed();
    if st.lid_prev == Some(true) && !lid {
        st.wake = true; // lid just opened
    }
    st.lid_prev = Some(lid);

    // Displays on first: DPMS state survives monitor enable/disable, and if
    // hypridle isn't around nothing else would turn them back on.
    if std::mem::take(&mut st.wake) {
        info!("waking displays");
        hyprctl(&["dispatch", r#"hl.dsp.dpms("on")"#]);
    }

    let Some(json) = hypr_request("j/monitors all") else { return warn!("hyprland request failed") };
    let mons: Vec<Monitor> = match serde_json::from_str(&json) {
        Ok(m) => m,
        Err(e) => return warn!("monitors json: {e}"),
    };
    let externals = mons.iter().filter(|m| m.name != cfg.internal).count();
    let internal_on = mons.iter().any(|m| m.name == cfg.internal && !m.disabled);

    st.mode = match (externals > 0, lid) {
        (true, true) => "docked-closed",
        (true, false) => "docked-open",
        (false, true) => "laptop-lid-closed",
        (false, false) => "laptop",
    };

    // Dock attached but its displays never came back after s2idle: one reload kick.
    let since = st.resumed.map(|t| t.elapsed());
    if externals == 0
        && cfg.kick_after_resume
        && !st.kicked
        && dock_present()
        && since.is_some_and(|d| d > Duration::from_secs(4) && d < Duration::from_secs(30))
    {
        st.kicked = true;
        info!("dock attached but no external outputs after resume, reloading hyprland");
        return hyprctl(&["reload"]);
    }

    let want_on = externals == 0 || !lid;
    if st.paused || want_on == internal_on {
        return;
    }
    info!(mode = st.mode, "{} {}", cfg.internal, if want_on { "on" } else { "off" });
    hyprctl(&["eval", if want_on { &cfg.internal_on } else { &cfg.internal_off }]);
}

// ---------- event sources ----------

enum Ev {
    Check,
    Wake,
    Resume,
    Cmd(String, UnixStream),
}

fn watch_hyprland(tx: Sender<Ev>) {
    loop {
        let Ok(sock) = UnixStream::connect(hypr_dir().join(".socket2.sock")) else {
            sleep(Duration::from_secs(1));
            continue;
        };
        info!("connected to hyprland events");
        let _ = tx.send(Ev::Check);
        for line in BufReader::new(sock).lines().map_while(Result::ok) {
            let ev = match line.split(">>").next() {
                Some("monitoradded") => Ev::Wake,
                Some("monitorremoved" | "configreloaded") => Ev::Check,
                _ => continue,
            };
            let _ = tx.send(ev);
        }
        warn!("hyprland event socket closed, reconnecting");
        sleep(Duration::from_secs(1));
    }
}

/// Subscribe to one system-bus signal and forward each hit as an event.
/// Each watcher owns its connection and reconnect loop, so a dropped bus can
/// never leave an orphaned thread/connection behind.
fn watch_system_signal(
    tx: Sender<Ev>,
    path: Option<&'static str>,
    interface: &'static str,
    member: &'static str,
    to_event: fn(&zbus::message::Message) -> Option<Ev>,
) {
    use zbus::{blocking::{Connection, MessageIterator}, message::Type, MatchRule};
    loop {
        let result = (|| -> zbus::Result<()> {
            let conn = Connection::system()?;
            let mut rule = MatchRule::builder().msg_type(Type::Signal);
            if let Some(p) = path {
                rule = rule.path(p)?;
            }
            let rule = rule.interface(interface)?.member(member)?.build();
            for msg in MessageIterator::for_match_rule(rule, &conn, None)? {
                if let Some(ev) = to_event(&msg?) {
                    let _ = tx.send(ev);
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            warn!("system bus ({member}): {e}");
        }
        sleep(Duration::from_secs(2));
    }
}

/// UPower PropertiesChanged (LidIsClosed and friends)
fn watch_lid(tx: Sender<Ev>) {
    watch_system_signal(
        tx,
        Some("/org/freedesktop/UPower"),
        "org.freedesktop.DBus.Properties",
        "PropertiesChanged",
        |_| Some(Ev::Check),
    );
}

/// logind PrepareForSleep(false) = resumed
fn watch_sleep(tx: Sender<Ev>) {
    watch_system_signal(tx, None, "org.freedesktop.login1.Manager", "PrepareForSleep", |m| {
        matches!(m.body().deserialize::<bool>(), Ok(false)).then_some(Ev::Resume)
    });
}

// ---------- control socket ----------

fn serve(tx: Sender<Ev>) {
    let _ = std::fs::remove_file(SOCK);
    let listener = match UnixListener::bind(SOCK) {
        Ok(l) => l,
        Err(e) => return warn!("bind {SOCK}: {e}"),
    };
    for stream in listener.incoming().flatten() {
        let mut line = String::new();
        let _ = BufReader::new(&stream).read_line(&mut line);
        let _ = tx.send(Ev::Cmd(line.trim().to_owned(), stream));
    }
}

/// Returns true if a reconcile is needed.
fn handle(ev: Ev, st: &mut State, tx: &Sender<Ev>) -> bool {
    match ev {
        Ev::Check => true,
        Ev::Wake => {
            st.wake = true;
            true
        }
        Ev::Resume => {
            info!("resumed");
            st.resumed = Some(Instant::now());
            st.kicked = false;
            st.wake = true;
            let tx = tx.clone();
            // dock DP links can take several seconds to come back after s2idle
            spawn(move || {
                for s in [3, 5, 10] {
                    sleep(Duration::from_secs(s));
                    let _ = tx.send(Ev::Check);
                }
            });
            true
        }
        Ev::Cmd(cmd, mut s) => {
            let (reply, check) = match cmd.as_str() {
                "status" => (
                    format!(
                        "mode: {}\nlid: {}\ndock: {}\npaused: {}\n",
                        st.mode,
                        if lid_closed() { "closed" } else { "open" },
                        if dock_present() { "attached" } else { "none" },
                        st.paused
                    ),
                    false,
                ),
                "reconcile" | "show" => ("ok\n".into(), true),
                "pause" => {
                    st.paused = true;
                    ("paused\n".into(), false)
                }
                "unpause" => {
                    st.paused = false;
                    ("unpaused\n".into(), true)
                }
                _ => ("commands: status reconcile pause unpause\n".into(), false),
            };
            let _ = s.write_all(reply.as_bytes());
            check
        }
    }
}

fn client(cmd: &str) {
    let mut s = match UnixStream::connect(SOCK) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rw-dock is not running ({e})");
            std::process::exit(1)
        }
    };
    let _ = writeln!(s, "{cmd}");
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    print!("{out}");
}

fn main() {
    if let Some(cmd) = std::env::args().nth(1) {
        return client(&cmd);
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .without_time() // journald timestamps
        .init();

    let cfg = Config::load();
    let (tx, rx) = channel();

    let workers: [fn(Sender<Ev>); 4] = [watch_hyprland, watch_lid, watch_sleep, serve];
    for f in workers {
        let t = tx.clone();
        spawn(move || f(t));
    }

    let mut st = State {
        mode: "unknown",
        paused: false,
        resumed: None,
        kicked: false,
        lid_prev: None,
        wake: false,
    };
    let debounce = Duration::from_millis(cfg.debounce_ms);

    while let Ok(first) = rx.recv() {
        let mut check = handle(first, &mut st, &tx);
        sleep(debounce); // coalesce the burst a dock plug-in produces
        for ev in rx.try_iter() {
            check |= handle(ev, &mut st, &tx);
        }
        if check {
            reconcile(&cfg, &mut st);
        }
    }
}
