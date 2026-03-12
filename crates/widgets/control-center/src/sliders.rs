use crate::config::Config;
use gtk4::prelude::*;
use gtk4::{Box, ComboBoxText, Label, Orientation, Scale};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

thread_local! {
    static VOLUME_SCALE: RefCell<Option<Scale>> = RefCell::new(None);
    static VOLUME_LABEL: RefCell<Option<Label>> = RefCell::new(None);
    static BRIGHTNESS_SCALE: RefCell<Option<Scale>> = RefCell::new(None);
    static BRIGHTNESS_LABEL: RefCell<Option<Label>> = RefCell::new(None);
}

pub fn build(config: &Config) -> Box {
    let container = Box::new(Orientation::Vertical, 8);
    container.add_css_class("sliders-section");

    if config.sliders.volume {
        let (volume_row, scale, label) = create_volume_slider();
        container.append(&volume_row);

        VOLUME_SCALE.with(|s| *s.borrow_mut() = Some(scale));
        VOLUME_LABEL.with(|l| *l.borrow_mut() = Some(label));

        if config.sliders.volume_output_selector {
            let selector = create_output_selector();
            container.append(&selector);
        }
    }

    if config.sliders.brightness {
        let (brightness_row, scale, label) = create_brightness_slider();
        container.append(&brightness_row);

        BRIGHTNESS_SCALE.with(|s| *s.borrow_mut() = Some(scale));
        BRIGHTNESS_LABEL.with(|l| *l.borrow_mut() = Some(label));
    }

    container
}

fn create_volume_slider() -> (Box, Scale, Label) {
    let row = Box::new(Orientation::Horizontal, 8);
    row.add_css_class("slider-row");

    let icon = gtk4::Image::from_icon_name("audio-volume-high-symbolic");
    icon.add_css_class("slider-icon");
    row.append(&icon);

    let scale = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
    scale.set_hexpand(true);
    scale.set_value(get_volume() as f64);

    let scale_clone = scale.clone();
    scale.connect_value_changed(move |s| {
        set_volume(s.value() as u32);
    });
    row.append(&scale);

    let label = Label::new(Some(&format!("{}%", get_volume())));
    label.add_css_class("slider-value");
    row.append(&label);

    (row, scale_clone, label)
}

fn create_brightness_slider() -> (Box, Scale, Label) {
    let row = Box::new(Orientation::Horizontal, 8);
    row.add_css_class("slider-row");

    let icon = gtk4::Image::from_icon_name("display-brightness-symbolic");
    icon.add_css_class("slider-icon");
    row.append(&icon);

    let scale = Scale::with_range(Orientation::Horizontal, 5.0, 100.0, 1.0);
    scale.set_hexpand(true);
    scale.set_value(get_brightness() as f64);

    let scale_clone = scale.clone();
    scale.connect_value_changed(move |s| {
        set_brightness(s.value() as u32);
    });
    row.append(&scale);

    let label = Label::new(Some(&format!("{}%", get_brightness())));
    label.add_css_class("slider-value");
    row.append(&label);

    (row, scale_clone, label)
}

fn create_output_selector() -> ComboBoxText {
    let combo = ComboBoxText::new();
    combo.add_css_class("output-selector");

    let outputs = get_audio_outputs();
    let current = get_current_output();

    for (i, (id, name)) in outputs.iter().enumerate() {
        combo.append(Some(id), name);
        if id == &current {
            combo.set_active(Some(i as u32));
        }
    }

    combo.connect_changed(|c| {
        if let Some(id) = c.active_id() {
            set_audio_output(&id);
        }
    });

    combo
}

pub fn update(_config: &Config) {
    VOLUME_SCALE.with(|s| {
        if let Some(scale) = s.borrow().as_ref() {
            let vol = get_volume() as f64;
            if (scale.value() - vol).abs() > 1.0 {
                scale.set_value(vol);
            }
        }
    });

    VOLUME_LABEL.with(|l| {
        if let Some(label) = l.borrow().as_ref() {
            label.set_text(&format!("{}%", get_volume()));
        }
    });

    BRIGHTNESS_SCALE.with(|s| {
        if let Some(scale) = s.borrow().as_ref() {
            let br = get_brightness() as f64;
            if (scale.value() - br).abs() > 1.0 {
                scale.set_value(br);
            }
        }
    });

    BRIGHTNESS_LABEL.with(|l| {
        if let Some(label) = l.borrow().as_ref() {
            label.set_text(&format!("{}%", get_brightness()));
        }
    });
}

// Volume helpers (wpctl)
fn get_volume() -> u32 {
    Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<f32>().ok())
                .map(|v| (v * 100.0) as u32)
        })
        .unwrap_or(50)
}

fn set_volume(vol: u32) {
    let _ = Command::new("wpctl")
        .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{}%", vol)])
        .spawn();
}

// Brightness helpers (brightnessctl)
fn get_brightness() -> u32 {
    let current = Command::new("brightnessctl")
        .args(["get"])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<u32>()
                .ok()
        })
        .unwrap_or(0);

    let max = Command::new("brightnessctl")
        .args(["max"])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<u32>()
                .ok()
        })
        .unwrap_or(1);

    if max > 0 {
        (current * 100) / max
    } else {
        50
    }
}
fn set_brightness(br: u32) {
    let _ = Command::new("brightnessctl")
        .args(["set", &format!("{}%", br)])
        .spawn();
}

// Audio output helpers (wpctl)
fn get_audio_outputs() -> Vec<(String, String)> {
    Command::new("wpctl")
        .args(["status"])
        .output()
        .ok()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            let mut outputs = Vec::new();
            let mut in_sinks = false;

            for line in s.lines() {
                if line.contains("Sinks:") {
                    in_sinks = true;
                    continue;
                }
                if in_sinks {
                    if line.contains("Sources:") || line.trim().is_empty() {
                        break;
                    }
                    // Parse sink lines like " │  *  46. Device Name [vol: 1.00]"
                    if let Some(name_start) = line.find(". ") {
                        let rest = &line[name_start + 2..];
                        if let Some(name_end) = rest.find(" [") {
                            let name = rest[..name_end].trim().to_string();
                            // Extract ID (number before the .)
                            let id_part = line.split('.').next().unwrap_or("");
                            let id = id_part
                                .chars()
                                .filter(|c| c.is_ascii_digit())
                                .collect::<String>();
                            if !id.is_empty() {
                                outputs.push((id, name));
                            }
                        }
                    }
                }
            }
            outputs
        })
        .unwrap_or_default()
}

fn get_current_output() -> String {
    Command::new("wpctl")
        .args(["status"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            for line in s.lines() {
                if line.contains("*") && line.contains(".") {
                    let id_part = line.split('.').next().unwrap_or("");
                    return Some(id_part.chars().filter(|c| c.is_ascii_digit()).collect());
                }
            }
            None
        })
        .unwrap_or_default()
}

fn set_audio_output(id: &str) {
    let _ = Command::new("wpctl").args(["set-default", id]).spawn();
}
