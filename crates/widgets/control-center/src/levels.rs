//! Display + Sound card, and the audio output picker view.
//!
//! Brightness: external monitors connected -> ddcutil (every external
//! display, VCP 0x10), otherwise the laptop backlight (sysfs read,
//! brightnessctl write). DDC is slow (~100-300 ms per call), so writes go
//! through one worker that always applies only the latest value.

use crate::icons::Icon;
use crate::sys;
use gtk4::prelude::*;
use gtk4::{glib, Align, Box, Button, Label, Orientation, Scale};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Brightness backends
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Backend {
    Backlight { dir: PathBuf, max: u32 },
    Ddc { displays: Vec<String> },
    None,
}

/// Any connected non-laptop output (DP/HDMI, incl. dock MST)?
fn externals_connected() -> bool {
    let Ok(rd) = std::fs::read_dir("/sys/class/drm") else { return false };
    rd.flatten().any(|e| {
        let n = e.file_name().to_string_lossy().into_owned();
        n.contains('-')
            && !n.contains("eDP")
            && !n.contains("Writeback")
            && std::fs::read_to_string(e.path().join("status")).is_ok_and(|s| s.trim() == "connected")
    })
}

fn backlight() -> Backend {
    let Ok(rd) = std::fs::read_dir("/sys/class/backlight") else { return Backend::None };
    for e in rd.flatten() {
        let dir = e.path();
        if let Some(max) = std::fs::read_to_string(dir.join("max_brightness"))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .filter(|m| *m > 0)
        {
            return Backend::Backlight { dir, max };
        }
    }
    Backend::None
}

/// Worker thread: detect the backend and read the current level.
fn detect() -> (Backend, Option<u32>) {
    if externals_connected() {
        let displays: Vec<String> = sys::output("ddcutil", &["detect", "--terse"])
            .map(|s| {
                s.lines()
                    .filter_map(|l| l.trim().strip_prefix("Display "))
                    .map(|n| n.trim().to_string())
                    .collect()
            })
            .unwrap_or_default();
        if !displays.is_empty() {
            let level = displays.iter().find_map(|d| ddc_get(d));
            return (Backend::Ddc { displays }, level);
        }
        tracing::warn!("external monitor connected but ddcutil found no displays (i2c access?)");
    }
    let b = backlight();
    let level = backlight_get(&b);
    (b, level)
}

fn ddc_get(display: &str) -> Option<u32> {
    // "VCP 10 C 50 100" -> 50 of 100
    let s = sys::output("ddcutil", &["getvcp", "10", "--display", display, "--terse"])?;
    let f: Vec<&str> = s.split_whitespace().collect();
    let cur: f64 = f.get(3)?.parse().ok()?;
    let max: f64 = f.get(4)?.parse().ok()?;
    (max > 0.0).then(|| (cur * 100.0 / max).round() as u32)
}

fn backlight_get(b: &Backend) -> Option<u32> {
    let Backend::Backlight { dir, max } = b else { return None };
    let cur: u32 = std::fs::read_to_string(dir.join("brightness")).ok()?.trim().parse().ok()?;
    Some(cur * 100 / max)
}

fn apply_level(b: &Backend, pct: u32) {
    match b {
        Backend::Ddc { displays } => {
            for d in displays {
                sys::run("ddcutil", &["setvcp", "10", &pct.to_string(), "--display", d, "--noverify"]);
            }
        }
        Backend::Backlight { .. } => {
            sys::run("brightnessctl", &["-q", "set", &format!("{pct}%")]);
        }
        Backend::None => {}
    }
}

/// Latest-value-wins writer: slider drags never queue up slow DDC writes.
#[derive(Clone)]
struct Writer {
    backend: Arc<Mutex<Backend>>,
    pending: Arc<Mutex<Option<u32>>>,
    busy: Arc<AtomicBool>,
}

impl Writer {
    fn set(&self, pct: u32) {
        *self.pending.lock().unwrap() = Some(pct);
        if self.busy.swap(true, Ordering::SeqCst) {
            return; // the running worker will pick it up
        }
        let w = self.clone();
        std::thread::spawn(move || {
            loop {
                let next = w.pending.lock().unwrap().take();
                let Some(pct) = next else { break };
                let b = w.backend.lock().unwrap().clone();
                apply_level(&b, pct);
            }
            w.busy.store(false, Ordering::SeqCst);
            // a value may have landed between take() and store(false)
            if w.pending.lock().unwrap().is_some() {
                let v = w.pending.lock().unwrap().take();
                if let Some(v) = v {
                    w.set(v);
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Volume / outputs (wpctl)
// ---------------------------------------------------------------------------

/// (percent, muted)
fn volume_get() -> Option<(u32, bool)> {
    let s = sys::output("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"])?;
    let v: f32 = s.split_whitespace().nth(1)?.parse().ok()?;
    Some(((v * 100.0).round() as u32, s.contains("MUTED")))
}

fn sink_description() -> Option<String> {
    let s = sys::output("wpctl", &["inspect", "@DEFAULT_AUDIO_SINK@"])?;
    s.lines().find_map(|l| {
        let l = l.trim().trim_start_matches('*').trim();
        l.strip_prefix("node.description = ").map(|v| v.trim_matches('"').to_string())
    })
}

/// (id, name, is_default) from `wpctl status` Sinks
fn sinks() -> Vec<(String, String, bool)> {
    let Some(s) = sys::output("wpctl", &["status"]) else { return Vec::new() };
    let mut out = Vec::new();
    let mut in_sinks = false;
    for line in s.lines() {
        if line.contains("Sinks:") {
            in_sinks = true;
            continue;
        }
        if !in_sinks {
            continue;
        }
        if line.contains("Sources:") || line.contains("Filters:") || line.trim_matches(['│', ' ']).is_empty() {
            break;
        }
        let is_default = line.contains('*');
        let Some(dot) = line.find(". ") else { continue };
        let id: String = line[..dot].chars().filter(|c| c.is_ascii_digit()).collect();
        let rest = &line[dot + 2..];
        let name = rest.split(" [").next().unwrap_or(rest).trim().to_string();
        if !id.is_empty() {
            out.push((id, name, is_default));
        }
    }
    out
}

/// "Anker USB Audio Analog Stereo" -> ("Anker USB Audio", "Analog Stereo")
fn split_name(desc: &str) -> (String, String) {
    for suffix in ["Analog Stereo", "Digital Stereo", "Stereo", "Mono", "Pro"] {
        if let Some(head) = desc.strip_suffix(suffix) {
            let head = head.trim();
            if !head.is_empty() {
                return (head.to_string(), suffix.to_string());
            }
        }
    }
    (desc.to_string(), "Output".to_string())
}

// ---------------------------------------------------------------------------
// UI
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Levels {
    bright_scale: Scale,
    bright_value: Label,
    bright_title: Label,
    vol_scale: Scale,
    vol_value: Label,
    out_title: Label,
    out_sub: Label,
    syncing: Rc<Cell<bool>>,
    /// skip polling the slider the user is dragging
    last_vol_input: Rc<Cell<Option<Instant>>>,
    writer: Writer,
    ticks: Rc<Cell<u32>>,
}

fn title_row(left: &str) -> (Box, Label, Label) {
    let r = Box::new(Orientation::Horizontal, 0);
    r.add_css_class("cc-level-title");
    let l = Label::new(Some(left));
    l.set_halign(Align::Start);
    l.set_hexpand(true);
    let v = Label::new(Some("…"));
    v.set_halign(Align::End);
    r.append(&l);
    r.append(&v);
    (r, l, v)
}

fn slider_row(icon: &'static str, min: f64) -> (Box, Scale) {
    let r = Box::new(Orientation::Horizontal, 9);
    r.add_css_class("cc-slider-wrap");
    let i = Icon::new(icon, 17);
    i.image.add_css_class("cc-slider-icon");
    r.append(&i.image);
    let s = Scale::with_range(Orientation::Horizontal, min, 100.0, 1.0);
    s.set_draw_value(false);
    s.set_hexpand(true);
    s.add_css_class("cc-slider");
    r.append(&s);
    (r, s)
}

pub fn build(on_outputs: impl Fn() + 'static) -> (Box, Levels) {
    let card = Box::new(Orientation::Vertical, 0);
    card.add_css_class("cc-card");
    card.add_css_class("cc-levels");

    let (bt_row, bright_title, bright_value) = title_row("DISPLAY");
    card.append(&bt_row);
    let (bs_row, bright_scale) = slider_row("sun", 1.0);
    bright_scale.set_sensitive(false); // until the backend answers
    card.append(&bs_row);

    let (vt_row, _vt, vol_value) = title_row("SOUND");
    vt_row.add_css_class("cc-level-title-second");
    card.append(&vt_row);
    let (vs_row, vol_scale) = slider_row("volume", 0.0);
    card.append(&vs_row);

    // output row
    let out = Button::new();
    out.add_css_class("cc-output-row");
    let oc = Box::new(Orientation::Horizontal, 9);
    let oi = Box::new(Orientation::Vertical, 0);
    oi.add_css_class("cc-output-icon");
    oi.set_valign(Align::Center);
    oi.set_vexpand(false);
    let ic = Icon::new("speaker", 16);
    ic.image.set_vexpand(true);
    oi.append(&ic.image);
    oc.append(&oi);
    let ot = Box::new(Orientation::Vertical, 1);
    ot.set_hexpand(true);
    ot.set_valign(Align::Center);
    let out_title = Label::new(Some("…"));
    out_title.add_css_class("cc-output-title");
    out_title.set_halign(Align::Start);
    out_title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    let out_sub = Label::new(Some(""));
    out_sub.add_css_class("cc-output-sub");
    out_sub.set_halign(Align::Start);
    ot.append(&out_title);
    ot.append(&out_sub);
    oc.append(&ot);
    let ch = Icon::new("chevron", 16);
    ch.image.add_css_class("cc-chevron");
    oc.append(&ch.image);
    out.set_child(Some(&oc));
    out.connect_clicked(move |_| on_outputs());
    card.append(&out);

    let writer = Writer {
        backend: Arc::new(Mutex::new(Backend::None)),
        pending: Arc::new(Mutex::new(None)),
        busy: Arc::new(AtomicBool::new(false)),
    };
    let lv = Levels {
        bright_scale: bright_scale.clone(),
        bright_value,
        bright_title,
        vol_scale: vol_scale.clone(),
        vol_value,
        out_title,
        out_sub,
        syncing: Rc::new(Cell::new(false)),
        last_vol_input: Rc::new(Cell::new(None)),
        writer,
        ticks: Rc::new(Cell::new(0)),
    };

    // user input
    let l = lv.clone();
    bright_scale.connect_value_changed(move |s| {
        if l.syncing.get() {
            return;
        }
        let pct = s.value().round() as u32;
        l.bright_value.set_text(&format!("{pct}%"));
        l.writer.set(pct);
    });
    let l = lv.clone();
    vol_scale.connect_value_changed(move |s| {
        if l.syncing.get() {
            return;
        }
        let pct = s.value().round() as u32;
        l.vol_value.set_text(&format!("{pct}%"));
        l.last_vol_input.set(Some(Instant::now()));
        let _ = std::process::Command::new("wpctl")
            .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{pct}%")])
            .spawn()
            .map(|mut c| std::thread::spawn(move || c.wait()));
    });

    // brightness backend (DDC detection takes a moment)
    let l = lv.clone();
    sys::bg(detect, move |(backend, level)| {
        let label = match &backend {
            Backend::Ddc { displays } if displays.len() > 1 => format!("DISPLAYS · {}", displays.len()),
            _ => "DISPLAY".to_string(),
        };
        l.bright_title.set_text(&label);
        let usable = !matches!(backend, Backend::None);
        *l.writer.backend.lock().unwrap() = backend;
        l.bright_scale.set_sensitive(usable && level.is_some());
        match level {
            Some(v) => {
                l.syncing.set(true);
                l.bright_scale.set_value(v as f64);
                l.syncing.set(false);
                l.bright_value.set_text(&format!("{v}%"));
            }
            None => l.bright_value.set_text("—"),
        }
    });

    lv.poll();
    (card, lv)
}

impl Levels {
    /// Volume every tick; output name every 3rd. Brightness isn't polled
    /// (DDC is slow and only changes through this slider or the OSD keys).
    pub fn poll(&self) {
        let n = self.ticks.get();
        self.ticks.set(n.wrapping_add(1));
        let with_sink = n % 3 == 0;
        let l = self.clone();
        sys::bg(
            move || (volume_get(), if with_sink { sink_description() } else { None }),
            move |(vol, desc)| {
                let dragging = l.last_vol_input.get().is_some_and(|t| t.elapsed() < Duration::from_millis(1500));
                if let Some((v, muted)) = vol {
                    if !dragging {
                        l.syncing.set(true);
                        l.vol_scale.set_value(v as f64);
                        l.syncing.set(false);
                        l.vol_value.set_text(&if muted { "Muted".into() } else { format!("{v}%") });
                    }
                    if muted {
                        l.vol_scale.add_css_class("muted");
                    } else {
                        l.vol_scale.remove_css_class("muted");
                    }
                }
                if let Some(d) = desc {
                    let (t, s) = split_name(&d);
                    l.out_title.set_text(&t);
                    l.out_sub.set_text(&s);
                }
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Output picker view (slides in like the Bluetooth panel)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct OutputsView {
    list: Box,
    on_done: Rc<RefCell<Option<std::boxed::Box<dyn Fn()>>>>,
}

pub fn outputs_view(on_back: impl Fn() + 'static) -> (Box, OutputsView) {
    let view = Box::new(Orientation::Vertical, 0);
    view.add_css_class("cc-subview");

    let header = Box::new(Orientation::Horizontal, 8);
    header.add_css_class("cc-subheader");
    let back = Button::new();
    back.add_css_class("cc-detail-button");
    back.set_child(Some(&Icon::new("back", 17).image));
    let title = Label::new(Some("Sound output"));
    title.add_css_class("cc-subtitle");
    header.append(&back);
    header.append(&title);
    view.append(&header);

    let card = Box::new(Orientation::Vertical, 0);
    card.add_css_class("cc-card");
    card.add_css_class("cc-list-card");
    view.append(&card);

    let on_back: Rc<dyn Fn()> = Rc::new(on_back);
    let ob = on_back.clone();
    back.connect_clicked(move |_| ob());

    let ov = OutputsView { list: card, on_done: Rc::new(RefCell::new(None)) };
    let ob = on_back.clone();
    *ov.on_done.borrow_mut() = Some(std::boxed::Box::new(move || ob()));
    (view, ov)
}

impl OutputsView {
    pub fn refresh(&self) {
        let ov = self.clone();
        sys::bg(sinks, move |list| {
            while let Some(c) = ov.list.first_child() {
                ov.list.remove(&c);
            }
            if list.is_empty() {
                let l = Label::new(Some("No outputs"));
                l.add_css_class("cc-feature-sub");
                ov.list.append(&l);
            }
            for (id, name, is_default) in list {
                let b = Button::new();
                b.add_css_class("cc-list-row");
                let r = Box::new(Orientation::Horizontal, 9);
                let (t, s) = split_name(&name);
                let text = Box::new(Orientation::Vertical, 1);
                text.set_hexpand(true);
                let tl = Label::new(Some(&t));
                tl.add_css_class("cc-output-title");
                tl.set_halign(Align::Start);
                let sl = Label::new(Some(&s));
                sl.add_css_class("cc-output-sub");
                sl.set_halign(Align::Start);
                text.append(&tl);
                text.append(&sl);
                r.append(&text);
                if is_default {
                    b.add_css_class("selected");
                    r.append(&Icon::new("check", 16).image);
                }
                b.set_child(Some(&r));
                let done = ov.on_done.clone();
                b.connect_clicked(move |_| {
                    sys::shell(&format!("wpctl set-default {id}"));
                    // give the main view's next poll a moment, then go back
                    let done = done.clone();
                    glib::timeout_add_local_once(Duration::from_millis(150), move || {
                        if let Some(f) = done.borrow().as_ref() {
                            f();
                        }
                    });
                });
                ov.list.append(&b);
            }
        });
    }
}
