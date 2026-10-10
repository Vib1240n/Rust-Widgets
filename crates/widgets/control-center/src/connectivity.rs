//! Wi-Fi + Bluetooth card: icon tile, name + subtitle, switch, chevron.
//!   Wi-Fi:     nmcli radio wifi on|off
//!   Bluetooth: rfkill (radio) + bluetoothctl power

use crate::icons::Icon;
use crate::sys;
use gtk4::prelude::*;
use gtk4::{glib, Align, Box, Button, Label, Orientation, Separator, Switch};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Wifi,
    Bluetooth,
}

#[derive(Clone)]
struct Row {
    kind: Kind,
    tile: Box,
    icon: Icon,
    subtitle: Label,
    switch: Switch,
    /// set while the code (not the user) moves the switch
    syncing: Rc<Cell<bool>>,
}

#[derive(Clone)]
pub struct Connectivity {
    wifi: Row,
    bt: Row,
}

struct State {
    on: bool,
    detail: String,
}

fn row(kind: Kind, on_details: impl Fn() + 'static) -> (Box, Row) {
    let (icon_name, title) = match kind {
        Kind::Wifi => ("wifi", "Wi-Fi"),
        Kind::Bluetooth => ("bluetooth", "Bluetooth"),
    };
    let r = Box::new(Orientation::Horizontal, 11);
    r.add_css_class("cc-connection-row");

    let tile = Box::new(Orientation::Vertical, 0);
    tile.add_css_class("cc-feature-icon");
    tile.set_valign(Align::Center);
    tile.set_vexpand(false); // don't inherit the icon's vexpand
    let icon = Icon::new(icon_name, 20);
    icon.image.set_halign(Align::Center);
    icon.image.set_valign(Align::Center);
    icon.image.set_vexpand(true);
    tile.append(&icon.image);
    r.append(&tile);

    // name + subtitle (clicking it flips the switch, like the design)
    let copy = Button::new();
    copy.add_css_class("cc-feature-copy");
    copy.set_hexpand(true);
    let text = Box::new(Orientation::Vertical, 2);
    let t = Label::new(Some(title));
    t.add_css_class("cc-feature-title");
    t.set_halign(Align::Start);
    let subtitle = Label::new(Some("…"));
    subtitle.add_css_class("cc-feature-sub");
    subtitle.set_halign(Align::Start);
    subtitle.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    text.append(&t);
    text.append(&subtitle);
    copy.set_child(Some(&text));
    r.append(&copy);

    let switch = Switch::new();
    switch.add_css_class("cc-switch");
    switch.set_valign(Align::Center);
    r.append(&switch);

    let chevron = Button::new();
    chevron.add_css_class("cc-detail-button");
    chevron.set_valign(Align::Center);
    chevron.set_child(Some(&Icon::new("chevron", 17).image));
    chevron.connect_clicked(move |_| on_details());
    r.append(&chevron);

    let row = Row { kind, tile, icon, subtitle, switch: switch.clone(), syncing: Rc::new(Cell::new(false)) };

    let sw = switch.clone();
    copy.connect_clicked(move |_| sw.set_active(!sw.is_active()));

    let rr = row.clone();
    switch.connect_state_set(move |_, on| {
        if !rr.syncing.get() {
            set_power(&rr, on);
        }
        glib::Propagation::Proceed
    });
    (r, row)
}

pub fn build(on_wifi_details: impl Fn() + 'static, on_bt_details: impl Fn() + 'static) -> (Box, Connectivity) {
    let card = Box::new(Orientation::Vertical, 0);
    card.add_css_class("cc-card");
    card.add_css_class("cc-connectivity");

    let (wifi_box, wifi) = row(Kind::Wifi, on_wifi_details);
    card.append(&wifi_box);
    let hair = Separator::new(Orientation::Horizontal);
    hair.add_css_class("cc-hairline");
    card.append(&hair);
    let (bt_box, bt) = row(Kind::Bluetooth, on_bt_details);
    card.append(&bt_box);

    let c = Connectivity { wifi, bt };
    c.poll();
    (card, c)
}

impl Connectivity {
    /// Refresh both rows off the UI thread.
    pub fn poll(&self) {
        for row in [self.wifi.clone(), self.bt.clone()] {
            let kind = row.kind;
            sys::bg(move || read_state(kind), move |s| apply(&row, s));
        }
    }
}

fn apply(row: &Row, s: State) {
    row.syncing.set(true);
    row.switch.set_active(s.on);
    row.syncing.set(false);
    if s.on {
        row.tile.add_css_class("on");
    } else {
        row.tile.remove_css_class("on");
    }
    row.icon.refresh_later();
    row.subtitle.set_text(&s.detail);
}

fn set_power(row: &Row, on: bool) {
    // optimistic UI, then confirm from the system
    if on {
        row.tile.add_css_class("on");
    } else {
        row.tile.remove_css_class("on");
    }
    row.icon.refresh_later();
    row.subtitle.set_text(if on { "Turning on…" } else { "Off" });

    let kind = row.kind;
    let r = row.clone();
    sys::bg(
        move || {
            let st = if on { "on" } else { "off" };
            match kind {
                Kind::Wifi => {
                    sys::run("nmcli", &["radio", "wifi", st]);
                }
                Kind::Bluetooth => {
                    if on {
                        sys::run("rfkill", &["unblock", "bluetooth"]);
                        std::thread::sleep(Duration::from_millis(400)); // adapter re-appears
                        sys::run("bluetoothctl", &["power", "on"]);
                    } else {
                        sys::run("bluetoothctl", &["power", "off"]);
                        sys::run("rfkill", &["block", "bluetooth"]);
                    }
                }
            }
            // let NetworkManager / bluez settle before reading back
            std::thread::sleep(Duration::from_millis(1500));
            read_state(kind)
        },
        move |s| apply(&r, s),
    );
}

fn read_state(kind: Kind) -> State {
    match kind {
        Kind::Wifi => {
            let on = sys::output("nmcli", &["radio", "wifi"]).is_some_and(|s| s.trim() == "enabled");
            let ssid = sys::output("nmcli", &["-t", "-f", "TYPE,NAME", "connection", "show", "--active"])
                .and_then(|s| {
                    s.lines()
                        .find_map(|l| l.strip_prefix("802-11-wireless:").map(|n| n.replace("\\:", ":")))
                });
            let detail = match (on, ssid) {
                (false, _) => "Off".into(),
                (true, Some(n)) => n,
                (true, None) => "Not connected".into(),
            };
            State { on, detail }
        }
        Kind::Bluetooth => {
            let on = sys::output("bluetoothctl", &["show"]).is_some_and(|s| s.contains("Powered: yes"));
            let names: Vec<String> = if on {
                sys::output("bluetoothctl", &["devices", "Connected"])
                    .map(|s| {
                        s.lines()
                            .filter_map(|l| l.strip_prefix("Device "))
                            .filter_map(|l| l.split_once(' ').map(|(_, n)| n.to_string()))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let detail = match (on, names.len()) {
                (false, _) => "Off".into(),
                (true, 0) => "On".into(),
                (true, 1) => names[0].clone(),
                (true, n) => format!("{} +{}", names[0], n - 1),
            };
            State { on, detail }
        }
    }
}
