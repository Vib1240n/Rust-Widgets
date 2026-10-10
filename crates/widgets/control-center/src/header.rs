//! Date + clock heading.

use gtk4::prelude::*;
use gtk4::{glib, Align, Box, Label, Orientation};
use std::time::Duration;

pub fn build() -> Box {
    let heading = Box::new(Orientation::Vertical, 0);
    heading.add_css_class("cc-heading");

    let date = Label::new(None);
    date.add_css_class("cc-date");
    date.set_halign(Align::Start);
    heading.append(&date);

    let clock = Label::new(None);
    clock.add_css_class("cc-clock");
    clock.set_halign(Align::Start);
    heading.append(&clock);

    update(&date, &clock);
    let (d, c) = (date.downgrade(), clock.downgrade());
    glib::timeout_add_local(Duration::from_secs(1), move || match (d.upgrade(), c.upgrade()) {
        (Some(d), Some(c)) => {
            update(&d, &c);
            glib::ControlFlow::Continue
        }
        _ => glib::ControlFlow::Break,
    });
    heading
}

fn update(date: &Label, clock: &Label) {
    let Ok(now) = glib::DateTime::now_local() else { return };
    if let Ok(s) = now.format("%A, %e %B") {
        // "Tuesday,  9 June" -> "TUESDAY, 9 JUNE"
        let text = s.split_whitespace().collect::<Vec<_>>().join(" ").to_uppercase();
        if date.text() != text {
            date.set_text(&text);
        }
    }
    if let Ok(s) = now.format("%H:%M") {
        if clock.text() != s {
            clock.set_text(&s);
        }
    }
}
