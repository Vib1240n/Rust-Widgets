//! Footer: Sleep · Lock · Power (right-aligned pills).

use crate::icons::Icon;
use crate::sys;
use gtk4::prelude::*;
use gtk4::{Align, Box, Button, Label, Orientation};

fn pill(icon: &'static str, label: &str) -> Button {
    let b = Button::new();
    b.add_css_class("cc-footer-btn");
    let c = Box::new(Orientation::Horizontal, 6);
    c.append(&Icon::new(icon, 17).image);
    c.append(&Label::new(Some(label)));
    b.set_child(Some(&c));
    b
}

/// `close`: hide the control center before acting
pub fn build(power_cmd: String, close: impl Fn() + Clone + 'static) -> Box {
    let row = Box::new(Orientation::Horizontal, 8);
    row.add_css_class("cc-footer");
    row.set_halign(Align::End);

    let sleep = pill("moon", "Sleep");
    let c = close.clone();
    sleep.connect_clicked(move |_| {
        c();
        sys::shell("sleep 0.4 && systemctl suspend");
    });

    let lock = pill("lock", "Lock");
    let c = close.clone();
    lock.connect_clicked(move |_| {
        c();
        sys::shell("loginctl lock-session");
    });

    let power = pill("power", "Power");
    power.connect_clicked(move |_| {
        close();
        sys::shell(&power_cmd);
    });

    row.append(&sleep);
    row.append(&lock);
    row.append(&power);
    row
}
