//! Quick tiles, two per row ([tiles] items = ["airplane", "caffeinate", "vpn"]).

use crate::config::Config;
use crate::icons::Icon;
use crate::sys;
use gtk4::prelude::*;
use gtk4::{Align, Box, Button, Grid, Label, Orientation};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Airplane,
    Caffeinate,
    Vpn,
}

#[derive(Clone)]
struct Tile {
    kind: Kind,
    button: Button,
    icon: Icon,
    state: Label,
}

#[derive(Clone)]
pub struct Tiles {
    tiles: Vec<Tile>,
    cfg: Config,
}

fn tile(kind: Kind, cfg: &Config) -> Tile {
    let (icon_name, title) = match kind {
        Kind::Airplane => ("plane", "Airplane"),
        Kind::Caffeinate => ("coffee", "Caffeine"),
        Kind::Vpn => ("shield", "VPN"),
    };
    let button = Button::new();
    button.add_css_class("cc-card");
    button.add_css_class("cc-tile");
    button.set_hexpand(true);

    let content = Box::new(Orientation::Horizontal, 10);
    let icon_box = Box::new(Orientation::Vertical, 0);
    icon_box.add_css_class("cc-tile-icon");
    icon_box.set_valign(Align::Center);
    icon_box.set_vexpand(false);
    let icon = Icon::new(icon_name, 20);
    icon.image.set_vexpand(true);
    icon_box.append(&icon.image);
    content.append(&icon_box);

    let text = Box::new(Orientation::Vertical, 2);
    text.set_valign(Align::Center);
    let t = Label::new(Some(title));
    t.add_css_class("cc-tile-title");
    t.set_halign(Align::Start);
    let state = Label::new(Some("Off"));
    state.add_css_class("cc-tile-state");
    state.set_halign(Align::Start);
    text.append(&t);
    text.append(&state);
    content.append(&text);
    button.set_child(Some(&content));

    let tl = Tile { kind, button: button.clone(), icon, state };
    let t2 = tl.clone();
    let cfg = cfg.clone();
    button.connect_clicked(move |_| toggle(&t2, &cfg));
    tl
}

pub fn build(cfg: &Config) -> Option<(Grid, Tiles)> {
    let kinds: Vec<Kind> = cfg
        .tiles
        .items
        .iter()
        .filter_map(|s| match s.as_str() {
            "airplane" => Some(Kind::Airplane),
            "caffeinate" | "caffeine" => Some(Kind::Caffeinate),
            "vpn" => Some(Kind::Vpn),
            other => {
                tracing::warn!("unknown tile '{other}' (airplane, caffeinate, vpn)");
                None
            }
        })
        .collect();
    if kinds.is_empty() {
        return None;
    }
    let grid = Grid::new();
    grid.add_css_class("cc-quick-grid");
    grid.set_column_spacing(10);
    grid.set_row_spacing(10);
    grid.set_column_homogeneous(true);

    let tiles: Vec<Tile> = kinds.into_iter().map(|k| tile(k, cfg)).collect();
    for (i, t) in tiles.iter().enumerate() {
        grid.attach(&t.button, (i % 2) as i32, (i / 2) as i32, 1, 1);
    }
    let t = Tiles { tiles, cfg: cfg.clone() };
    t.poll();
    Some((grid, t))
}

impl Tiles {
    pub fn poll(&self) {
        for t in self.tiles.clone() {
            let (kind, iface) = (t.kind, self.cfg.commands.vpn_interface.clone());
            sys::bg(move || is_on(kind, &iface), move |on| show(&t, on));
        }
    }
}

fn show(t: &Tile, on: bool) {
    if on {
        t.button.add_css_class("selected");
    } else {
        t.button.remove_css_class("selected");
    }
    t.state.set_text(if on { "On" } else { "Off" });
    t.icon.refresh_later();
}

fn toggle(t: &Tile, cfg: &Config) {
    let was_on = t.button.has_css_class("selected");
    show(t, !was_on); // optimistic
    let (kind, caf, iface) = (t.kind, cfg.commands.caffeinate.clone(), cfg.commands.vpn_interface.clone());
    let t = t.clone();
    sys::bg(
        move || {
            match kind {
                Kind::Airplane => {
                    sys::run("rfkill", &[if was_on { "unblock" } else { "block" }, "all"]);
                }
                Kind::Caffeinate => sys::shell(&format!("{caf} {}", if was_on { "off" } else { "on" })),
                Kind::Vpn => {
                    sys::run("sudo", &["-n", "wg-quick", if was_on { "down" } else { "up" }, &iface]);
                }
            }
            std::thread::sleep(Duration::from_millis(700));
            is_on(kind, &iface)
        },
        move |on| show(&t, on),
    );
}

fn is_on(kind: Kind, vpn_iface: &str) -> bool {
    match kind {
        // airplane = every radio soft-blocked
        Kind::Airplane => {
            let Ok(rd) = std::fs::read_dir("/sys/class/rfkill") else { return false };
            let soft: Vec<bool> = rd
                .flatten()
                .filter_map(|e| std::fs::read_to_string(e.path().join("soft")).ok())
                .map(|s| s.trim() == "1")
                .collect();
            !soft.is_empty() && soft.iter().all(|b| *b)
        }
        Kind::Caffeinate => sys::output("systemctl", &["--user", "is-active", "caffeinate.service"])
            .is_some_and(|s| s.trim() == "active"),
        Kind::Vpn => sys::output("ip", &["link", "show", vpn_iface]).is_some_and(|s| s.contains("UP")),
    }
}
