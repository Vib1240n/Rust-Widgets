use gtk4::prelude::*;
use gtk4::{Box, Button, Image, Label, Orientation};
use std::cell::RefCell;
use std::process::Command;

thread_local! {
    static MEDIA_CONTAINER: RefCell<Option<Box>> = RefCell::new(None);
    static TITLE_LABEL: RefCell<Option<Label>> = RefCell::new(None);
    static ARTIST_LABEL: RefCell<Option<Label>> = RefCell::new(None);
    static PLAY_BTN: RefCell<Option<Button>> = RefCell::new(None);
}

pub fn build() -> Box {
    let container = Box::new(Orientation::Vertical, 8);
    container.add_css_class("media-section");

    let info = get_media_info();
    if info.is_none() {
        container.add_css_class("inactive");
    }

    // Top row: art + info
    let top_row = Box::new(Orientation::Horizontal, 0);

    let art = Image::from_icon_name("audio-x-generic-symbolic");
    art.add_css_class("media-art");
    art.set_pixel_size(48);
    top_row.append(&art);

    let info_box = Box::new(Orientation::Vertical, 2);
    info_box.add_css_class("media-info");
    info_box.set_valign(gtk4::Align::Center);
    info_box.set_hexpand(true);

    let title = Label::new(Some("No media playing"));
    title.add_css_class("media-title");
    title.set_halign(gtk4::Align::Start);
    title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    title.set_max_width_chars(25);
    info_box.append(&title);

    let artist = Label::new(Some(""));
    artist.add_css_class("media-artist");
    artist.set_halign(gtk4::Align::Start);
    artist.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    artist.set_max_width_chars(30);
    info_box.append(&artist);

    top_row.append(&info_box);
    container.append(&top_row);

    // Controls row
    let controls = Box::new(Orientation::Horizontal, 4);
    controls.add_css_class("media-controls");
    controls.set_halign(gtk4::Align::Center);

    let prev_btn = Button::new();
    prev_btn.add_css_class("media-btn");
    prev_btn.set_child(Some(&Image::from_icon_name("media-skip-backward-symbolic")));
    prev_btn.connect_clicked(|_| media_prev());
    controls.append(&prev_btn);

    let play_btn = Button::new();
    play_btn.add_css_class("media-btn");
    play_btn.add_css_class("play");
    play_btn.set_child(Some(&Image::from_icon_name(
        "media-playback-start-symbolic",
    )));
    play_btn.connect_clicked(|_| media_play_pause());
    controls.append(&play_btn);

    let next_btn = Button::new();
    next_btn.add_css_class("media-btn");
    next_btn.set_child(Some(&Image::from_icon_name("media-skip-forward-symbolic")));
    next_btn.connect_clicked(|_| media_next());
    controls.append(&next_btn);

    container.append(&controls);

    // Store refs for updates
    MEDIA_CONTAINER.with(|c| *c.borrow_mut() = Some(container.clone()));
    TITLE_LABEL.with(|t| *t.borrow_mut() = Some(title));
    ARTIST_LABEL.with(|a| *a.borrow_mut() = Some(artist));
    PLAY_BTN.with(|p| *p.borrow_mut() = Some(play_btn));

    // Initial update
    update();

    container
}

pub fn update() {
    if let Some(info) = get_media_info() {
        MEDIA_CONTAINER.with(|c| {
            if let Some(container) = c.borrow().as_ref() {
                container.remove_css_class("inactive");
            }
        });

        TITLE_LABEL.with(|t| {
            if let Some(label) = t.borrow().as_ref() {
                label.set_text(&info.title);
            }
        });

        ARTIST_LABEL.with(|a| {
            if let Some(label) = a.borrow().as_ref() {
                label.set_text(&info.artist);
            }
        });

        PLAY_BTN.with(|p| {
            if let Some(btn) = p.borrow().as_ref() {
                let icon_name = if info.playing {
                    "media-playback-pause-symbolic"
                } else {
                    "media-playback-start-symbolic"
                };
                btn.set_child(Some(&Image::from_icon_name(icon_name)));
            }
        });
    } else {
        MEDIA_CONTAINER.with(|c| {
            if let Some(container) = c.borrow().as_ref() {
                container.add_css_class("inactive");
            }
        });

        TITLE_LABEL.with(|t| {
            if let Some(label) = t.borrow().as_ref() {
                label.set_text("No media playing");
            }
        });

        ARTIST_LABEL.with(|a| {
            if let Some(label) = a.borrow().as_ref() {
                label.set_text("");
            }
        });
    }
}

struct MediaInfo {
    title: String,
    artist: String,
    playing: bool,
}

fn get_media_info() -> Option<MediaInfo> {
    let status = Command::new("playerctl").args(["status"]).output().ok()?;

    if !status.status.success() {
        return None;
    }

    let playing = String::from_utf8_lossy(&status.stdout).trim() == "Playing";

    let title = Command::new("playerctl")
        .args(["metadata", "title"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    let artist = Command::new("playerctl")
        .args(["metadata", "artist"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    if title.is_empty() {
        return None;
    }

    Some(MediaInfo {
        title,
        artist,
        playing,
    })
}

fn media_play_pause() {
    let _ = Command::new("playerctl").args(["play-pause"]).spawn();
}

fn media_next() {
    let _ = Command::new("playerctl").args(["next"]).spawn();
}

fn media_prev() {
    let _ = Command::new("playerctl").args(["previous"]).spawn();
}
