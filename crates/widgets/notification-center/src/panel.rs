//! Notification center panel ("Liquid Glass" Figma design).
//!
//!   NOTIFICATION CENTER / greeting                      [close]
//!   date card: 18 | Tuesday, June 2024         weather 22° Clear
//!   TODAY ............................................ Clear all
//!   cards (icon tile, app + age, summary, body, actions, x)
//!   EARLIER ...
//!   Do not disturb                                      [switch]

use crate::config::Config;
use crate::notification::{Notification, NotificationStore, Urgency};
use gtk4::prelude::*;
use gtk4::{
    gio, glib, Align, Box, Button, Label, Orientation, Overlay, PolicyType, ProgressBar,
    ScrolledWindow, Switch,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use widget_core::icons::Icon;

/// The list scrolls past this height (px)
const MAX_LIST_HEIGHT: i32 = 540;

/// Actions from the panel
#[derive(Debug, Clone)]
pub enum PanelAction {
    Close,
    ClearAll,
    DismissOne(u32),
    ActionInvoked(u32, String),
    SetDnd(bool),
}

#[derive(Default)]
struct Weather {
    fetched: Option<Instant>,
    temp: String,
    cond: String,
}

pub struct NotificationPanel {
    window: gtk4::Window,
    list: Box,
    greeting: Label,
    day_number: Label,
    weekday: Label,
    month: Label,
    weather_box: Box,
    weather_temp: Label,
    weather_cond: Label,
    dnd: Switch,
    dnd_syncing: Rc<Cell<bool>>,
    weather: Rc<RefCell<Weather>>,
    action_tx: mpsc::UnboundedSender<PanelAction>,
    config: Arc<Config>,
}

/// Work on a thread, finish on the UI thread
fn bg<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(T) + 'static) {
    glib::spawn_future_local(async move {
        if let Ok(v) = gio::spawn_blocking(work).await {
            done(v);
        }
    });
}

fn label(text: &str, class: &str) -> Label {
    let l = Label::new(Some(text));
    l.add_css_class(class);
    l.set_halign(Align::Start);
    l
}

fn round_button(icon: &'static str, size: i32) -> Button {
    let b = Button::new();
    b.add_css_class("nc-round-btn");
    b.set_valign(Align::Start);
    b.set_child(Some(&Icon::new(icon, size).image));
    b
}

impl NotificationPanel {
    pub fn new(
        app: &gtk4::Application,
        config: Arc<Config>,
        action_tx: mpsc::UnboundedSender<PanelAction>,
    ) -> Self {
        let window = gtk4::Window::builder()
            .application(app)
            .title("Notifications")
            .decorated(false)
            .resizable(false)
            .build();
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_namespace("rust-widgets");
        window.set_keyboard_mode(KeyboardMode::OnDemand);
        apply_position(&window, &config);

        // margin keeps the blurred area off the rounded corners
        let outer = Box::new(Orientation::Vertical, 0);
        outer.set_margin_start(16);
        outer.set_margin_end(16);
        outer.set_margin_top(16);
        outer.set_margin_bottom(16);

        let panel = Box::new(Orientation::Vertical, 0);
        panel.add_css_class("widget-container");
        panel.add_css_class("cc-panel");
        panel.add_css_class("nc-panel");
        panel.set_width_request(config.appearance.panel_width);

        // ---- heading ----
        let heading = Box::new(Orientation::Horizontal, 0);
        heading.add_css_class("nc-heading");
        let titles = Box::new(Orientation::Vertical, 0);
        titles.set_hexpand(true);
        titles.append(&label("NOTIFICATION CENTER", "nc-eyebrow"));
        let greeting = label("Hello", "nc-greeting");
        titles.append(&greeting);
        heading.append(&titles);
        let close = round_button("close", 17);
        let tx = action_tx.clone();
        close.connect_clicked(move |_| {
            let _ = tx.send(PanelAction::Close);
        });
        heading.append(&close);
        panel.append(&heading);

        // ---- date card ----
        let date_card = Box::new(Orientation::Horizontal, 0);
        date_card.add_css_class("cc-card");
        date_card.add_css_class("nc-date-card");
        let day_number = label("", "nc-date-number");
        day_number.set_valign(Align::Center);
        date_card.append(&day_number);
        let copy = Box::new(Orientation::Vertical, 0);
        copy.add_css_class("nc-date-copy");
        copy.set_hexpand(true);
        copy.set_valign(Align::Center);
        let weekday = label("", "nc-weekday");
        let month = label("", "nc-month");
        copy.append(&weekday);
        copy.append(&month);
        date_card.append(&copy);
        let weather_box = Box::new(Orientation::Horizontal, 8);
        weather_box.add_css_class("nc-weather");
        weather_box.set_valign(Align::Center);
        weather_box.append(&Icon::new("sun", 19).image);
        let wt = Box::new(Orientation::Vertical, 0);
        let weather_temp = label("", "nc-weather-temp");
        let weather_cond = label("", "nc-weather-cond");
        wt.append(&weather_temp);
        wt.append(&weather_cond);
        weather_box.append(&wt);
        weather_box.set_visible(false);
        date_card.append(&weather_box);
        panel.append(&date_card);

        // ---- notifications ----
        let list = Box::new(Orientation::Vertical, 8);
        list.add_css_class("nc-list-box");
        let scroll = ScrolledWindow::new();
        scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
        scroll.set_propagate_natural_height(true);
        scroll.set_max_content_height(MAX_LIST_HEIGHT);
        scroll.add_css_class("nc-scroller");
        scroll.set_child(Some(&list));
        panel.append(&scroll);

        // ---- do not disturb ----
        let dnd_row = Box::new(Orientation::Horizontal, 9);
        dnd_row.add_css_class("nc-dnd-row");
        let dnd_icon = Box::new(Orientation::Vertical, 0);
        dnd_icon.add_css_class("nc-dnd-icon");
        dnd_icon.set_valign(Align::Center);
        dnd_icon.set_vexpand(false);
        let mi = Icon::new("moon", 17);
        mi.image.set_vexpand(true);
        dnd_icon.append(&mi.image);
        dnd_row.append(&dnd_icon);
        let dt = Box::new(Orientation::Vertical, 0);
        dt.set_hexpand(true);
        dt.set_valign(Align::Center);
        dt.append(&label("Do not disturb", "nc-dnd-title"));
        dt.append(&label("Only critical notifications pop up", "nc-dnd-sub"));
        dnd_row.append(&dt);
        let dnd = Switch::new();
        dnd.add_css_class("cc-switch");
        dnd.set_valign(Align::Center);
        dnd_row.append(&dnd);
        panel.append(&dnd_row);

        let dnd_syncing = Rc::new(Cell::new(false));
        let (tx, guard) = (action_tx.clone(), dnd_syncing.clone());
        dnd.connect_state_set(move |_, on| {
            if !guard.get() {
                let _ = tx.send(PanelAction::SetDnd(on));
            }
            glib::Propagation::Proceed
        });

        outer.append(&panel);
        window.set_child(Some(&outer));

        if config.behavior.close_on_escape {
            let keys = gtk4::EventControllerKey::new();
            let tx = action_tx.clone();
            keys.connect_key_pressed(move |_, key, _, _| {
                if key == gtk4::gdk::Key::Escape {
                    let _ = tx.send(PanelAction::Close);
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
            window.add_controller(keys);
        }
        if config.behavior.close_on_unfocus {
            let focus = gtk4::EventControllerFocus::new();
            let tx = action_tx.clone();
            focus.connect_leave(move |_| {
                let _ = tx.send(PanelAction::Close);
            });
            window.add_controller(focus);
        }

        Self {
            window,
            list,
            greeting,
            day_number,
            weekday,
            month,
            weather_box,
            weather_temp,
            weather_cond,
            dnd,
            dnd_syncing,
            weather: Rc::new(RefCell::new(Weather::default())),
            action_tx,
            config,
        }
    }

    pub fn show(&self) {
        self.refresh_header();
        self.refresh_weather();
        self.window.present();
        if self.config.animation.enabled {
            animate_panel_in(&self.window, &self.config);
        }
    }

    pub fn hide(&self) {
        if self.config.animation.enabled {
            let w = self.window.clone();
            animate_panel_out(&self.window, &self.config, move || w.set_visible(false));
        } else {
            self.window.set_visible(false);
        }
    }

    pub fn toggle(&self) {
        if self.window.is_visible() {
            self.hide();
        } else {
            self.show();
        }
    }

    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    /// Reflect the daemon's DND state in the switch (without re-sending it)
    pub fn set_dnd(&self, enabled: bool) {
        self.dnd_syncing.set(true);
        self.dnd.set_active(enabled);
        self.dnd_syncing.set(false);
    }

    fn refresh_header(&self) {
        let Ok(now) = glib::DateTime::now_local() else { return };
        let greeting = match now.hour() {
            5..=11 => "Good morning",
            12..=16 => "Good afternoon",
            17..=21 => "Good evening",
            _ => "Good night",
        };
        self.greeting.set_text(greeting);
        self.day_number.set_text(&now.day_of_month().to_string());
        if let Ok(s) = now.format("%A") {
            self.weekday.set_text(&s);
        }
        if let Ok(s) = now.format("%B %Y") {
            self.month.set_text(&s);
        }
    }

    /// wttr.in, only when shown and the cached value is older than refresh_minutes
    fn refresh_weather(&self) {
        let cfg = &self.config.weather;
        if !cfg.enabled {
            self.weather_box.set_visible(false);
            return;
        }
        let w = self.weather.borrow();
        let fresh = w.fetched.is_some_and(|t| t.elapsed() < Duration::from_secs(cfg.refresh_minutes.max(5) * 60));
        let have = !w.temp.is_empty();
        drop(w);
        if have {
            self.weather_temp.set_text(&self.weather.borrow().temp);
            self.weather_cond.set_text(&self.weather.borrow().cond);
            self.weather_box.set_visible(true);
        }
        if fresh {
            return;
        }
        let loc = cfg.location.trim().replace(' ', "+");
        let unit = match cfg.units.as_str() {
            "metric" => "&m",
            "imperial" => "&u",
            _ => "",
        };
        let url = format!("https://wttr.in/{loc}?format=%t|%C{unit}");
        let (cache, temp_l, cond_l, wbox) =
            (self.weather.clone(), self.weather_temp.clone(), self.weather_cond.clone(), self.weather_box.clone());
        bg(
            move || {
                // curl, not GIO: GIO has no https backend on this system
                let out = std::process::Command::new("curl")
                    .args(["-fsS", "--max-time", "6", &url])
                    .output()
                    .ok()
                    .filter(|o| o.status.success())?;
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let (t, c) = s.split_once('|')?;
                // "+22°C" -> "22°"
                let temp = t.trim().trim_start_matches('+').trim_end_matches(['C', 'F']).to_string();
                (!temp.is_empty() && !s.contains("Unknown")).then(|| (temp, c.trim().to_string()))
            },
            move |res| {
                let mut w = cache.borrow_mut();
                w.fetched = Some(Instant::now());
                if let Some((t, c)) = res {
                    temp_l.set_text(&t);
                    cond_l.set_text(&c);
                    wbox.set_visible(true);
                    w.temp = t;
                    w.cond = c;
                }
            },
        );
    }

    pub fn update(&self, store: &NotificationStore) {
        while let Some(c) = self.list.first_child() {
            self.list.remove(&c);
        }
        let all = store.all();
        if all.is_empty() {
            self.list.append(&self.section("TODAY", false));
            self.list.append(&caught_up());
            return;
        }
        let (today, earlier): (Vec<&Notification>, Vec<&Notification>) = all.into_iter().partition(|n| n.is_today());
        let mut first = true;
        for (title, items) in [("TODAY", today), ("EARLIER", earlier)] {
            if items.is_empty() {
                continue;
            }
            self.list.append(&self.section(title, first));
            first = false;
            for n in items {
                self.list.append(&self.notice(n));
            }
        }
    }

    fn section(&self, title: &str, with_clear: bool) -> Box {
        let row = Box::new(Orientation::Horizontal, 0);
        row.add_css_class("nc-section-label");
        let l = label(title, "nc-section-title");
        l.set_hexpand(true);
        row.append(&l);
        if with_clear {
            let clear = Button::with_label("Clear all");
            clear.add_css_class("nc-clear-all");
            let tx = self.action_tx.clone();
            clear.connect_clicked(move |_| {
                let _ = tx.send(PanelAction::ClearAll);
            });
            row.append(&clear);
        }
        row
    }

    fn notice(&self, n: &Notification) -> Overlay {
        let overlay = Overlay::new();
        let card = Box::new(Orientation::Horizontal, 10);
        card.add_css_class("cc-card");
        card.add_css_class("nc-notice");

        // icon tile: category glyph on a coloured tile, else the app's icon
        let tile = Box::new(Orientation::Vertical, 0);
        tile.add_css_class("nc-notice-icon");
        tile.set_valign(Align::Start);
        tile.set_vexpand(false);
        let (kind, glyph) = classify(n);
        tile.add_css_class(&format!("nc-kind-{kind}"));
        let img = match glyph {
            Some(g) => Icon::new(g, 19).image,
            None => app_image(n).unwrap_or_else(|| Icon::new("bell", 19).image),
        };
        img.set_vexpand(true);
        img.set_halign(Align::Center);
        tile.append(&img);
        card.append(&tile);

        // content
        let content = Box::new(Orientation::Vertical, 0);
        content.add_css_class("nc-notice-content");
        content.set_hexpand(true);
        let meta = Box::new(Orientation::Horizontal, 10);
        let app = label(if n.app_name.is_empty() { "Notification" } else { n.app_name.as_str() }, "nc-notice-app");
        app.set_hexpand(true);
        app.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        meta.append(&app);
        meta.append(&label(&n.short_age(), "nc-notice-time"));
        content.append(&meta);

        if !n.summary.is_empty() {
            let s = label(&strip_markup(&n.summary), "nc-notice-summary");
            s.set_wrap(true);
            s.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            s.set_lines(2);
            s.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            s.set_xalign(0.0);
            content.append(&s);
        }
        if !n.body.is_empty() {
            let b = label(&strip_markup(&n.body), "nc-notice-body");
            b.set_wrap(true);
            b.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            b.set_lines(2);
            b.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            b.set_xalign(0.0);
            content.append(&b);
        }
        if let Some(p) = n.progress {
            let bar = ProgressBar::new();
            bar.set_fraction(p.clamp(0, 100) as f64 / 100.0);
            bar.add_css_class("nc-notice-progress");
            content.append(&bar);
        }
        let actions: Vec<_> = n.actions.iter().filter(|(id, _)| id != "default").collect();
        if !actions.is_empty() {
            let row = Box::new(Orientation::Horizontal, 6);
            for (id, text) in actions {
                let b = Button::with_label(text);
                b.add_css_class("nc-notice-action");
                let (tx, nid, key) = (self.action_tx.clone(), n.id, id.clone());
                b.connect_clicked(move |_| {
                    let _ = tx.send(PanelAction::ActionInvoked(nid, key.clone()));
                });
                row.append(&b);
            }
            content.append(&row);
        }
        card.append(&content);
        overlay.set_child(Some(&card));

        // dismiss (top-right, like the design's absolute button)
        let dismiss = Button::new();
        dismiss.add_css_class("nc-notice-dismiss");
        dismiss.set_halign(Align::End);
        dismiss.set_valign(Align::Start);
        dismiss.set_child(Some(&Icon::new("close", 14).image));
        let (tx, nid) = (self.action_tx.clone(), n.id);
        dismiss.connect_clicked(move |_| {
            let _ = tx.send(PanelAction::DismissOne(nid));
        });
        overlay.add_overlay(&dismiss);

        // click the card: default action, else open the app
        let click = gtk4::GestureClick::new();
        let (tx, nid) = (self.action_tx.clone(), n.id);
        let has_default = n.actions.iter().any(|(a, _)| a == "default");
        let entry = n.desktop_entry.clone().unwrap_or_else(|| n.app_name.clone());
        click.connect_released(move |_, _, _, _| {
            if has_default {
                let _ = tx.send(PanelAction::ActionInvoked(nid, "default".into()));
            } else if let Some(info) = desktop_info(&entry) {
                let _ = info.launch(&[], gio::AppLaunchContext::NONE);
            }
        });
        content.add_controller(click);
        overlay
    }
}

fn caught_up() -> Box {
    let b = Box::new(Orientation::Vertical, 0);
    b.add_css_class("nc-caught-up");
    b.set_valign(Align::Center);
    let tile = Box::new(Orientation::Vertical, 0);
    tile.add_css_class("nc-caught-up-icon");
    tile.set_halign(Align::Center);
    tile.set_vexpand(false); // stop the icon's vexpand propagating: the tile stays 45x45
    let i = Icon::new("bell", 22);
    i.image.set_vexpand(true);
    tile.append(&i.image);
    b.append(&tile);
    let t = Label::new(Some("You're all caught up"));
    t.add_css_class("nc-caught-up-title");
    b.append(&t);
    let s = Label::new(Some("New notifications will appear here."));
    s.add_css_class("nc-caught-up-sub");
    b.append(&s);
    b
}

/// (css kind, design glyph). Unknown apps get their own icon (glyph None).
fn classify(n: &Notification) -> (&'static str, Option<&'static str>) {
    let app = n.app_name.to_lowercase();
    let entry = n.desktop_entry.as_deref().unwrap_or("").to_lowercase();
    let cat = n.category.as_deref().unwrap_or("");
    let summary = n.summary.to_lowercase();
    let any = |words: &[&str]| words.iter().any(|w| app.contains(w) || entry.contains(w));

    if n.urgency == Urgency::Critical {
        return ("critical", Some("alert"));
    }
    if cat.starts_with("im") || cat.starts_with("email")
        || any(&["discord", "vesktop", "telegram", "signal", "whatsapp", "slack", "element", "messages", "thunderbird", "mail"])
    {
        return ("message", Some("message"));
    }
    if summary.contains("screenshot") || any(&["grim", "hyprshot", "swappy", "flameshot", "satty", "screenshot"]) {
        return ("capture", Some("camera"));
    }
    if cat.starts_with("transfer") || summary.contains("update") || summary.contains("download")
        || any(&["pacman", "paru", "yay", "pamac", "update", "software", "octopi"])
    {
        return ("update", Some("download"));
    }
    ("app", None)
}

fn desktop_info(entry: &str) -> Option<gio::DesktopAppInfo> {
    let id = if entry.ends_with(".desktop") { entry.to_string() } else { format!("{}.desktop", entry.to_lowercase()) };
    gio::DesktopAppInfo::new(&id)
}

/// The sender's icon: app_icon (name or path), else its desktop entry's icon
fn app_image(n: &Notification) -> Option<gtk4::Image> {
    let img = if let Some(path) = n.app_icon.strip_prefix("file://").or(Some(n.app_icon.as_str())).filter(|p| p.starts_with('/')) {
        std::path::Path::new(path).exists().then(|| gtk4::Image::from_file(path))?
    } else if !n.app_icon.is_empty()
        && gtk4::gdk::Display::default()
            .map(|d| gtk4::IconTheme::for_display(&d).has_icon(&n.app_icon))
            .unwrap_or(false)
    {
        gtk4::Image::from_icon_name(&n.app_icon)
    } else {
        let entry = n.desktop_entry.clone().unwrap_or_else(|| n.app_name.clone());
        let icon = desktop_info(&entry)?.icon()?;
        gtk4::Image::from_gicon(&icon)
    };
    img.set_pixel_size(22);
    Some(img)
}

fn apply_position(window: &gtk4::Window, config: &Config) {
    for e in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        window.set_anchor(e, false);
    }
    let edges: &[Edge] = match config.position.anchor.as_str() {
        "top-left" => &[Edge::Top, Edge::Left],
        "top-center" => &[Edge::Top],
        "bottom-left" => &[Edge::Bottom, Edge::Left],
        "bottom-center" => &[Edge::Bottom],
        "bottom-right" => &[Edge::Bottom, Edge::Right],
        _ => &[Edge::Top, Edge::Right],
    };
    for e in edges {
        window.set_anchor(*e, true);
    }
    window.set_margin(Edge::Top, config.position.margin_top);
    window.set_margin(Edge::Right, config.position.margin_right);
    window.set_margin(Edge::Bottom, config.position.margin_bottom);
    window.set_margin(Edge::Left, config.position.margin_left);
}

fn animate_panel_in(window: &gtk4::Window, config: &Config) {
    let steps = 20u32;
    let step = Duration::from_millis(config.animation.duration / steps as u64);
    let count = Rc::new(Cell::new(0u32));
    let (start, end) = (-200, config.position.margin_top);
    window.set_margin(Edge::Top, start);
    window.set_opacity(0.0);
    let w = window.clone();
    glib::timeout_add_local(step, move || {
        let i = count.get() + 1;
        count.set(i);
        let t = 1.0 - (1.0 - i as f64 / steps as f64).powi(3);
        w.set_margin(Edge::Top, start + ((end - start) as f64 * t) as i32);
        w.set_opacity(t);
        if i >= steps {
            w.set_margin(Edge::Top, end);
            w.set_opacity(1.0);
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

fn animate_panel_out<F: Fn() + 'static>(window: &gtk4::Window, config: &Config, done: F) {
    let steps = 15u32;
    let step = Duration::from_millis(config.animation.duration / steps as u64);
    let count = Rc::new(Cell::new(0u32));
    let (start, end) = (window.margin(Edge::Top), -200);
    let w = window.clone();
    glib::timeout_add_local(step, move || {
        let i = count.get() + 1;
        count.set(i);
        let t = (i as f64 / steps as f64).powi(3);
        w.set_margin(Edge::Top, start + ((end - start) as f64 * t) as i32);
        w.set_opacity(1.0 - t);
        if i >= steps {
            done();
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

/// Strip basic HTML/pango markup
fn strip_markup(text: &str) -> String {
    let mut r = text.to_string();
    for tag in ["<b>", "</b>", "<i>", "</i>", "<u>", "</u>", "<br>", "<br/>"] {
        r = r.replace(tag, "");
    }
    r.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&").replace("&quot;", "\"")
}
