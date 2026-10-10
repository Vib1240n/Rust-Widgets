//! Now Playing card: art, NOW PLAYING / title / artist, prev · play · next.
//! One `playerctl` call per tick; album art loaded off the UI thread
//! (file:// directly, http(s):// through GIO) and only when it changes.

use crate::icons::Icon;
use crate::sys;
use gtk4::prelude::*;
use gtk4::{gdk, glib, Align, Box, Button, Label, Orientation, Overflow, Picture};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
pub struct Media {
    card: Box,
    art: Picture,
    title: Label,
    artist: Label,
    play_icon: Icon,
    art_url: Rc<RefCell<String>>,
}

struct Info {
    playing: bool,
    title: String,
    artist: String,
    art: String,
}

fn ctl_button(icon: &'static str, class: &str, cmd: &'static str) -> (Button, Icon) {
    let b = Button::new();
    b.add_css_class("cc-media-btn");
    if !class.is_empty() {
        b.add_css_class(class);
    }
    let i = Icon::new(icon, 17);
    b.set_child(Some(&i.image));
    b.connect_clicked(move |_| sys::shell(&format!("playerctl {cmd}")));
    (b, i)
}

pub fn build() -> (Box, Media) {
    let card = Box::new(Orientation::Horizontal, 11);
    card.add_css_class("cc-card");
    card.add_css_class("cc-media");

    // art: rounded box that clips the picture
    let frame = Box::new(Orientation::Vertical, 0);
    frame.add_css_class("cc-album-art");
    frame.set_overflow(Overflow::Hidden);
    frame.set_valign(Align::Center);
    let art = Picture::new();
    art.set_content_fit(gtk4::ContentFit::Cover);
    art.set_size_request(49, 49);
    art.set_can_shrink(true);
    frame.append(&art);
    card.append(&frame);

    let track = Box::new(Orientation::Vertical, 0);
    track.set_hexpand(true);
    track.set_valign(Align::Center);
    let kicker = Label::new(Some("NOW PLAYING"));
    kicker.add_css_class("cc-track-kicker");
    kicker.set_halign(Align::Start);
    let title = Label::new(Some("Nothing playing"));
    title.add_css_class("cc-track-title");
    title.set_halign(Align::Start);
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    title.set_max_width_chars(22);
    let artist = Label::new(Some(""));
    artist.add_css_class("cc-track-artist");
    artist.set_halign(Align::Start);
    artist.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    artist.set_max_width_chars(24);
    track.append(&kicker);
    track.append(&title);
    track.append(&artist);
    card.append(&track);

    let actions = Box::new(Orientation::Horizontal, 2);
    actions.add_css_class("cc-media-actions");
    actions.set_valign(Align::Center);
    let (prev, _) = ctl_button("previous", "", "previous");
    let (play, play_icon) = ctl_button("play", "play", "play-pause");
    let (next, _) = ctl_button("next", "", "next");
    actions.append(&prev);
    actions.append(&play);
    actions.append(&next);
    card.append(&actions);

    let m = Media { card: card.clone(), art, title, artist, play_icon, art_url: Rc::new(RefCell::new(String::new())) };
    m.poll();
    (card, m)
}

fn read() -> Option<Info> {
    let s = sys::output(
        "playerctl",
        &["metadata", "--format", "{{status}}\t{{title}}\t{{artist}}\t{{mpris:artUrl}}"],
    )?;
    let mut f = s.trim_end_matches('\n').splitn(4, '\t');
    let status = f.next()?.to_string();
    let title = f.next().unwrap_or("").to_string();
    if title.is_empty() {
        return None;
    }
    Some(Info {
        playing: status == "Playing",
        title,
        artist: f.next().unwrap_or("").to_string(),
        art: f.next().unwrap_or("").to_string(),
    })
}

/// Art bytes from file:// or http(s):// (GIO handles remote URIs)
fn fetch_art(url: &str) -> Option<Vec<u8>> {
    if let Some(path) = url.strip_prefix("file://") {
        let decoded = glib::Uri::unescape_string(path, None::<&str>).map(|s| s.to_string()).unwrap_or(path.to_string());
        return std::fs::read(decoded).ok();
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    // curl, not GIO: GIO has no https backend on this system
    let out = std::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "6", url])
        .output()
        .ok()
        .filter(|o| o.status.success() && !o.stdout.is_empty())?;
    Some(out.stdout)
}

impl Media {
    pub fn poll(&self) {
        let m = self.clone();
        sys::bg(read, move |info| m.show(info));
    }

    fn show(&self, info: Option<Info>) {
        let Some(info) = info else {
            self.card.add_css_class("inactive");
            self.title.set_text("Nothing playing");
            self.artist.set_text("");
            self.play_icon.set_name("play");
            self.set_art(String::new());
            return;
        };
        self.card.remove_css_class("inactive");
        if self.title.text() != info.title {
            self.title.set_text(&info.title);
        }
        if self.artist.text() != info.artist {
            self.artist.set_text(&info.artist);
        }
        self.play_icon.set_name(if info.playing { "pause" } else { "play" });
        self.set_art(info.art);
    }

    fn set_art(&self, url: String) {
        if *self.art_url.borrow() == url {
            return;
        }
        *self.art_url.borrow_mut() = url.clone();
        if url.is_empty() {
            self.art.set_paintable(None::<&gdk::Paintable>); // CSS gradient shows through
            return;
        }
        let m = self.clone();
        let want = url.clone();
        sys::bg(
            move || fetch_art(&url),
            move |bytes| {
                // the track may have changed while loading
                if *m.art_url.borrow() != want {
                    return;
                }
                let tex = bytes.and_then(|b| gdk::Texture::from_bytes(&glib::Bytes::from_owned(b)).ok());
                m.art.set_paintable(tex.as_ref());
            },
        );
    }
}
