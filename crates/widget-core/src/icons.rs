//! Icons from the Figma design (24x24 viewBox, stroke 1.8, round caps),
//! rendered as SVG textures in the colour CSS gives the widget, so they
//! follow the theme and state classes (.on, .selected...) like text does.

use gtk4::prelude::*;
use gtk4::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

fn paths(name: &str) -> &'static str {
    match name {
        "wifi" => r#"<path d="M4.5 9.5a11.2 11.2 0 0 1 15 0"/><path d="M7.5 13a6.8 6.8 0 0 1 9 0"/><path d="M10.5 16.3a2.3 2.3 0 0 1 3 0"/><circle cx="12" cy="19" r=".7" fill="currentColor" stroke="none"/>"#,
        "bluetooth" => r#"<path d="m9 6 6 5-6 5V3l6 5-9 8"/>"#,
        "plane" => r#"<path d="m21 16-8.2-3.5v5.7l2.2 1.5V21l-3-1-3 1v-1.3l2.2-1.5v-5.7L3 16v-1.7l8.2-5V4.7a1 1 0 0 1 2 0v4.6l7.8 5Z"/>"#,
        "moon" => r#"<path d="M20 15.2A8.3 8.3 0 0 1 8.8 4 8.5 8.5 0 1 0 20 15.2Z"/>"#,
        "sun" => r#"<circle cx="12" cy="12" r="3.5"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>"#,
        "volume" => r#"<path d="M5 10v4h3l4 3V7l-4 3H5Z"/><path d="M15 9.2a4 4 0 0 1 0 5.6M17.7 6.7a7.5 7.5 0 0 1 0 10.6"/>"#,
        "chevron" => r#"<path d="m9 18 6-6-6-6"/>"#,
        "back" => r#"<path d="m15 18-6-6 6-6"/>"#,
        "check" => r#"<path d="M5 12.5l4.5 4.5L19 7"/>"#,
        "lock" => r#"<rect x="5" y="10" width="14" height="11" rx="3"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/>"#,
        "power" => r#"<path d="M12 2v10M6.4 5.4a8 8 0 1 0 11.2 0"/>"#,
        "speaker" => r#"<rect x="6" y="2" width="12" height="20" rx="3"/><circle cx="12" cy="15" r="3.5"/><circle cx="12" cy="7" r="1" fill="currentColor"/>"#,
        "headphones" => r#"<path d="M4 14v-2a8 8 0 0 1 16 0v2"/><path d="M4 14v4a2 2 0 0 0 2 2h2v-8H6a2 2 0 0 0-2 2ZM20 14v4a2 2 0 0 1-2 2h-2v-8h2a2 2 0 0 1 2 2Z"/>"#,
        "previous" => r#"<path d="M6 5v14M19 6l-9 6 9 6Z"/>"#,
        "play" => r#"<path d="m8 5 11 7-11 7Z" fill="currentColor"/>"#,
        "pause" => r#"<path d="M9 6v12M15 6v12" stroke-width="3"/>"#,
        "next" => r#"<path d="M18 5v14M5 6l9 6-9 6Z"/>"#,
        "bell" => r#"<path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9Z"/><path d="M10 21h4"/>"#,
        "close" => r#"<path d="m6 6 12 12M18 6 6 18"/>"#,
        "download" => r#"<path d="M12 3v12M7 10l5 5 5-5"/><path d="M5 21h14"/>"#,
        "message" => r#"<path d="M21 12a8 8 0 0 1-9 8 9 9 0 0 1-4-.9L3 21l1.8-4A8 8 0 1 1 21 12Z"/><path d="M8 12h.01M12 12h.01M16 12h.01" stroke-width="2.5"/>"#,
        "camera" => r#"<path d="M4 7h3l1.5-2h7L17 7h3a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2Z"/><circle cx="12" cy="13" r="3.5"/>"#,
        // not in the Figma file, drawn to match (stroke 1.8, round)
        "alert" => r#"<path d="M12 3 2.5 20h19L12 3Z"/><path d="M12 10v4M12 17h.01"/>"#,
        "cloud" => r#"<path d="M7 18a4.5 4.5 0 0 1-.6-9A6 6 0 0 1 18 9.5a4.2 4.2 0 0 1-.5 8.5H7Z"/>"#,
        "coffee" => r#"<path d="M5 9h11v5a5 5 0 0 1-5 5h-1a5 5 0 0 1-5-5V9Z"/><path d="M16 11h1.5a2.5 2.5 0 0 1 0 5H16"/><path d="M9 3v2.5M12.5 3v2.5"/>"#,
        "shield" => r#"<path d="M12 3 5 6v5c0 4.4 3 8.2 7 10 4-1.8 7-5.6 7-10V6l-7-3Z"/>"#,
        _ => r#"<circle cx="12" cy="12" r="8"/>"#,
    }
}

thread_local! {
    /// (name, colour, size) -> texture. Few combinations; bounded in practice.
    static CACHE: RefCell<HashMap<(String, String, i32), gdk::Texture>> = RefCell::new(HashMap::new());
}

/// Render `name` at `size` logical px (2x for HiDPI) in `color`.
pub fn texture(name: &str, color: &gdk::RGBA, size: i32) -> Option<gdk::Texture> {
    let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let hex = format!("#{:02x}{:02x}{:02x}", c(color.red()), c(color.green()), c(color.blue()));
    let alpha = color.alpha().clamp(0.0, 1.0);
    let key = (name.to_string(), format!("{hex}/{alpha:.3}"), size);
    if let Some(t) = CACHE.with(|m| m.borrow().get(&key).cloned()) {
        return Some(t);
    }
    let px = size * 2;
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{px}" height="{px}" viewBox="0 0 24 24" fill="none" stroke="currentColor" color="{hex}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><g opacity="{alpha:.3}">{}</g></svg>"#,
        paths(name)
    );
    let tex = gdk::Texture::from_bytes(&glib::Bytes::from_owned(svg.into_bytes())).ok()?;
    CACHE.with(|m| m.borrow_mut().insert(key, tex.clone()));
    Some(tex)
}

/// An icon whose colour comes from CSS (`color:` on .cc-icon or its parents).
#[derive(Clone)]
pub struct Icon {
    pub image: gtk4::Image,
    name: Rc<Cell<&'static str>>,
    size: i32,
}

impl Icon {
    pub fn new(name: &'static str, size: i32) -> Self {
        let image = gtk4::Image::new();
        image.set_pixel_size(size);
        image.add_css_class("cc-icon");
        let name = Rc::new(Cell::new(name));
        let n = name.clone();
        // the realize callback gets the widget itself: no Rc cycle
        image.connect_realize(move |img| paint(img, n.get(), size));
        Self { image, name, size }
    }

    pub fn set_name(&self, name: &'static str) {
        if self.name.get() != name {
            self.name.set(name);
            self.refresh_later();
        }
    }

    /// Re-read the CSS colour on the next idle (after a state class change
    /// has restyled the widget).
    pub fn refresh_later(&self) {
        let img = self.image.downgrade();
        let name = self.name.clone();
        let size = self.size;
        glib::idle_add_local_once(move || {
            if let Some(img) = img.upgrade() {
                paint(&img, name.get(), size);
            }
        });
    }
}

fn paint(img: &gtk4::Image, name: &str, size: i32) {
    if let Some(t) = texture(name, &img.color(), size) {
        img.set_paintable(Some(&t));
    }
}
