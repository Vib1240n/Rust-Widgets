use crate::config::Config;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Box, Button, Label, Orientation};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

// Callback type for showing bluetooth panel - use full path to avoid gtk4::Box conflict
pub type ShowBluetoothCallback = Rc<RefCell<Option<std::boxed::Box<dyn Fn()>>>>;

thread_local! {
    static SHOW_BLUETOOTH_CALLBACK: ShowBluetoothCallback = Rc::new(RefCell::new(None));
}

pub fn set_bluetooth_callback<F: Fn() + 'static>(callback: F) {
    SHOW_BLUETOOTH_CALLBACK.with(|cb: &ShowBluetoothCallback| {
        *cb.borrow_mut() = Some(std::boxed::Box::new(callback));
    });
}

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
        grid.append(&create_bluetooth_toggle());
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

/// Create the bluetooth toggle with special long-press handling
fn create_bluetooth_toggle() -> Button {
    let btn = Button::new();
    btn.add_css_class("toggle-btn");
    if is_bluetooth_enabled() {
        btn.add_css_class("active");
    }

    let content = Box::new(Orientation::Vertical, 4);
    content.set_halign(gtk4::Align::Center);
    content.set_valign(gtk4::Align::Center);

    let icon = gtk4::Image::from_icon_name("bluetooth-symbolic");
    icon.add_css_class("toggle-icon");
    content.append(&icon);

    let lbl = Label::new(Some("Bluetooth"));
    lbl.add_css_class("toggle-label");
    content.append(&lbl);

    btn.set_child(Some(&content));

    // Track press timing for long-press detection
    let press_start = Rc::new(RefCell::new(None::<std::time::Instant>));
    let long_press_triggered = Rc::new(RefCell::new(false));

    // Mouse press - start timing
    let gesture_press = gtk4::GestureClick::new();
    gesture_press.set_button(1); // Left click

    let press_start_clone = press_start.clone();
    let long_press_triggered_clone = long_press_triggered.clone();

    gesture_press.connect_pressed(move |_, _, _, _| {
        *press_start_clone.borrow_mut() = Some(std::time::Instant::now());
        *long_press_triggered_clone.borrow_mut() = false;
    });

    // Setup long-press timer
    let press_start_timer = press_start.clone();
    let long_press_triggered_timer = long_press_triggered.clone();
    let btn_timer = btn.clone();

    gesture_press.connect_pressed(move |_, _, _, _| {
        let press_start = press_start_timer.clone();
        let long_press_triggered = long_press_triggered_timer.clone();
        let btn = btn_timer.clone();

        // Check after 500ms if still pressed
        glib::timeout_add_local_once(Duration::from_millis(500), move || {
            if let Some(start) = *press_start.borrow() {
                if start.elapsed() >= Duration::from_millis(450) {
                    // Long press detected
                    *long_press_triggered.borrow_mut() = true;

                    // Visual feedback
                    btn.add_css_class("long-pressed");
                    let btn_clone = btn.clone();
                    glib::timeout_add_local_once(Duration::from_millis(150), move || {
                        btn_clone.remove_css_class("long-pressed");
                    });

                    // Trigger bluetooth panel
                    SHOW_BLUETOOTH_CALLBACK.with(|cb: &ShowBluetoothCallback| {
                        if let Some(ref callback) = *cb.borrow() {
                            callback();
                        }
                    });
                }
            }
        });
    });

    btn.add_controller(gesture_press);

    // Mouse release - toggle if it was a short click
    let btn_clone = btn.clone();
    let press_start_release = press_start.clone();
    let long_press_triggered_release = long_press_triggered.clone();

    btn.connect_clicked(move |_| {
        // Clear the press start time
        let was_long_press = *long_press_triggered_release.borrow();
        *press_start_release.borrow_mut() = None;

        // Only toggle if it wasn't a long press
        if !was_long_press {
            toggle_bluetooth();
            if btn_clone.has_css_class("active") {
                btn_clone.remove_css_class("active");
            } else {
                btn_clone.add_css_class("active");
            }
        }
    });

    // Right click - open bluetooth panel
    let gesture_right = gtk4::GestureClick::new();
    gesture_right.set_button(3);
    gesture_right.connect_released(move |_, _, _, _| {
        SHOW_BLUETOOTH_CALLBACK.with(|cb: &ShowBluetoothCallback| {
            if let Some(ref callback) = *cb.borrow() {
                callback();
            }
        });
    });
    btn.add_controller(gesture_right);

    // GTK long press gesture for touch
    let long_press_gesture = gtk4::GestureLongPress::new();
    long_press_gesture.set_delay_factor(1.0);

    let btn_long = btn.clone();
    long_press_gesture.connect_pressed(move |_, _, _| {
        // Visual feedback
        btn_long.add_css_class("long-pressed");
        let btn = btn_long.clone();
        glib::timeout_add_local_once(Duration::from_millis(150), move || {
            btn.remove_css_class("long-pressed");
        });

        SHOW_BLUETOOTH_CALLBACK.with(|cb: &ShowBluetoothCallback| {
            if let Some(ref callback) = *cb.borrow() {
                callback();
            }
        });
    });
    btn.add_controller(long_press_gesture);

    btn
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
    // Check for common caffeinate indicators
    std::path::Path::new("/tmp/caffeinate.pid").exists()
        || Command::new("pgrep")
            .args(["-x", "caffeine"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

fn toggle_caffeinate() {
    // Try user's custom script first, then fallback to systemd-inhibit
    let config_dir = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("~/.config"));
    let script_path = config_dir.join("rw/scripts/toggle-caffeinate.sh");

    if script_path.exists() {
        let _ = Command::new("sh")
            .args(["-c", &script_path.to_string_lossy().to_string()])
            .spawn();
    } else if is_caffeinate_enabled() {
        // Kill existing caffeinate
        let _ = std::fs::remove_file("/tmp/caffeinate.pid");
        let _ = Command::new("pkill")
            .args(["-f", "systemd-inhibit.*caffeinate"])
            .spawn();
    } else {
        // Start caffeinate using systemd-inhibit
        let _ = Command::new("sh")
            .args(["-c", "echo $$ > /tmp/caffeinate.pid && exec systemd-inhibit --what=idle --who=rust-widgets --why=Caffeinate --mode=block sleep infinity"])
            .spawn();
    }
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
