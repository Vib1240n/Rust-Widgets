use crate::config::Config;
use gtk4::prelude::*;
use gtk4::{Box, Button, Label, Orientation};
use std::process::Command;

pub fn build(config: &Config) -> Box {
    let container = Box::new(Orientation::Vertical, 8);
    container.add_css_class("toggles-section");

    let grid = Box::new(Orientation::Horizontal, 8);
    grid.add_css_class("toggle-grid");
    grid.set_homogeneous(true);

    if config.toggles.wifi {
        grid.append(&create_toggle(
            "network-wireless-symbolic",
            "WiFi",
            is_wifi_enabled(),
            || toggle_wifi(),
            || open_network_settings(),
        ));
    }

    if config.toggles.bluetooth {
        grid.append(&create_toggle(
            "bluetooth-symbolic",
            "Bluetooth",
            is_bluetooth_enabled(),
            || toggle_bluetooth(),
            || open_bluetooth_settings(),
        ));
    }

    if config.toggles.dnd {
        grid.append(&create_toggle(
            "notifications-disabled-symbolic",
            "DND",
            is_dnd_enabled(),
            || toggle_dnd(),
            || {},
        ));
    }

    if config.toggles.caffeinate {
        grid.append(&create_toggle(
            "display-brightness-symbolic",
            "Caffeine",
            is_caffeinate_enabled(),
            || toggle_caffeinate(),
            || {},
        ));
    }

    if config.toggles.night_light {
        grid.append(&create_toggle(
            "night-light-symbolic",
            "Night",
            is_night_light_enabled(),
            || toggle_night_light(),
            || {},
        ));
    }

    if config.toggles.vpn {
        grid.append(&create_toggle(
            "network-vpn-symbolic",
            "VPN",
            is_vpn_enabled(),
            || toggle_vpn(),
            || {},
        ));
    }

    container.append(&grid);
    container
}

fn create_toggle<F, G>(
    icon_name: &str,
    label: &str,
    active: bool,
    on_click: F,
    on_right_click: G,
) -> Button
where
    F: Fn() + 'static,
    G: Fn() + 'static,
{
    let btn = Button::new();
    btn.add_css_class("toggle-btn");
    if active {
        btn.add_css_class("active");
    }

    let content = Box::new(Orientation::Vertical, 4);
    content.set_halign(gtk4::Align::Center);
    content.set_valign(gtk4::Align::Center);

    let icon = gtk4::Image::from_icon_name(icon_name);
    icon.add_css_class("toggle-icon");
    content.append(&icon);

    let lbl = Label::new(Some(label));
    lbl.add_css_class("toggle-label");
    content.append(&lbl);

    btn.set_child(Some(&content));

    // Left click - toggle
    let btn_clone = btn.clone();
    btn.connect_clicked(move |_| {
        on_click();
        if btn_clone.has_css_class("active") {
            btn_clone.remove_css_class("active");
        } else {
            btn_clone.add_css_class("active");
        }
    });

    // Right click - open settings
    let gesture = gtk4::GestureClick::new();
    gesture.set_button(3); // Right click
    gesture.connect_released(move |_, _, _, _| {
        on_right_click();
    });
    btn.add_controller(gesture);

    btn
}

// WiFi
fn is_wifi_enabled() -> bool {
    Command::new("nmcli")
        .args(["radio", "wifi"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "enabled")
        .unwrap_or(false)
}

fn toggle_wifi() {
    let enabled = is_wifi_enabled();
    let state = if enabled { "off" } else { "on" };
    let _ = Command::new("nmcli").args(["radio", "wifi", state]).spawn();
}

fn open_network_settings() {
    let _ = Command::new("nm-connection-editor").spawn();
}

// Bluetooth
fn is_bluetooth_enabled() -> bool {
    Command::new("bluetoothctl")
        .args(["show"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("Powered: yes"))
        .unwrap_or(false)
}

fn toggle_bluetooth() {
    let enabled = is_bluetooth_enabled();
    let state = if enabled { "off" } else { "on" };
    let _ = Command::new("bluetoothctl").args(["power", state]).spawn();
}

fn open_bluetooth_settings() {
    let _ = Command::new("blueman-manager").spawn();
}

// DND (swaync)
fn is_dnd_enabled() -> bool {
    Command::new("swaync-client")
        .args(["--get-dnd"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}

fn toggle_dnd() {
    let _ = Command::new("swaync-client").args(["--toggle-dnd"]).spawn();
}

// Caffeinate
fn is_caffeinate_enabled() -> bool {
    std::path::Path::new("/tmp/caffeinate.pid").exists()
}

fn toggle_caffeinate() {
    let _ = Command::new("sh")
        .args(["-c", "~/Development/bash_scripts/toggle-caffeinate.sh"])
        .spawn();
}

// Night Light (gammastep)
fn is_night_light_enabled() -> bool {
    Command::new("pgrep")
        .args(["-x", "gammastep"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn toggle_night_light() {
    if is_night_light_enabled() {
        let _ = Command::new("pkill").args(["-x", "gammastep"]).spawn();
    } else {
        let _ = Command::new("gammastep").spawn();
    }
}

// VPN (WireGuard)
fn is_vpn_enabled() -> bool {
    Command::new("ip")
        .args(["link", "show", "proton-us"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("state UP"))
        .unwrap_or(false)
}

fn toggle_vpn() {
    if is_vpn_enabled() {
        let _ = Command::new("sudo")
            .args(["wg-quick", "down", "proton-us"])
            .spawn();
    } else {
        let _ = Command::new("sudo")
            .args(["wg-quick", "up", "proton-us"])
            .spawn();
    }
}
