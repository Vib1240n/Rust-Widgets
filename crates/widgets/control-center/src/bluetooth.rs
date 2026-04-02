use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Box, Button, Image, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, Spinner, Switch,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::process::Command;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct BluetoothDevice {
    pub address: String,
    pub name: String,
    pub paired: bool,
    pub trusted: bool,
    pub connected: bool,
    pub icon: String,
}

/// Container for the bluetooth panel that can be shown/hidden
pub struct BluetoothPanel {
    pub container: Box,
    device_list: ListBox,
    scanning_spinner: Spinner,
    scan_button: Button,
    is_scanning: Rc<RefCell<bool>>,
    devices: Rc<RefCell<HashMap<String, BluetoothDevice>>>,
}

impl BluetoothPanel {
    pub fn new(on_back: impl Fn() + 'static) -> Self {
        let container = Box::new(Orientation::Vertical, 0);
        container.add_css_class("bluetooth-panel");

        // Header with back button
        let header = Box::new(Orientation::Horizontal, 8);
        header.add_css_class("bluetooth-header");

        let back_btn = Button::new();
        back_btn.set_icon_name("go-previous-symbolic");
        back_btn.add_css_class("bluetooth-back-btn");
        back_btn.connect_clicked(move |_| on_back());
        header.append(&back_btn);

        let title = Label::new(Some("Bluetooth"));
        title.add_css_class("bluetooth-title");
        title.set_hexpand(true);
        title.set_halign(gtk4::Align::Start);
        header.append(&title);

        // Power toggle
        let power_switch = Switch::new();
        power_switch.set_active(is_bluetooth_enabled());
        power_switch.add_css_class("bluetooth-power");
        power_switch.connect_state_set(|_switch, state| {
            toggle_bluetooth_power(state);
            glib::Propagation::Proceed
        });
        header.append(&power_switch);

        container.append(&header);

        // Scanning controls
        let scan_row = Box::new(Orientation::Horizontal, 8);
        scan_row.add_css_class("bluetooth-scan-row");

        let scan_label = Label::new(Some("Devices"));
        scan_label.add_css_class("bluetooth-section-label");
        scan_label.set_hexpand(true);
        scan_label.set_halign(gtk4::Align::Start);
        scan_row.append(&scan_label);

        let scanning_spinner = Spinner::new();
        scanning_spinner.add_css_class("bluetooth-spinner");
        scanning_spinner.set_visible(false);
        scan_row.append(&scanning_spinner);

        let scan_button = Button::new();
        scan_button.set_icon_name("view-refresh-symbolic");
        scan_button.add_css_class("bluetooth-scan-btn");
        scan_button.set_tooltip_text(Some("Scan for devices"));
        scan_row.append(&scan_button);

        container.append(&scan_row);

        // Scrollable device list
        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_min_content_height(200);
        scroll.set_max_content_height(400);
        scroll.add_css_class("bluetooth-scroll");

        let device_list = ListBox::new();
        device_list.add_css_class("bluetooth-device-list");
        device_list.set_selection_mode(gtk4::SelectionMode::None);
        scroll.set_child(Some(&device_list));

        container.append(&scroll);

        // Scan hint
        let hint = Label::new(Some("Right-click or long-press to pair/trust"));
        hint.add_css_class("bluetooth-hint");
        container.append(&hint);

        let is_scanning = Rc::new(RefCell::new(false));
        let devices = Rc::new(RefCell::new(HashMap::new()));

        let panel = Self {
            container,
            device_list,
            scanning_spinner,
            scan_button: scan_button.clone(),
            is_scanning: is_scanning.clone(),
            devices: devices.clone(),
        };

        // Setup scan button
        let device_list_clone = panel.device_list.clone();
        let spinner_clone = panel.scanning_spinner.clone();
        let is_scanning_clone = is_scanning.clone();
        let devices_clone = devices.clone();

        scan_button.connect_clicked(move |btn| {
            if *is_scanning_clone.borrow() {
                return;
            }

            *is_scanning_clone.borrow_mut() = true;
            btn.set_sensitive(false);
            spinner_clone.set_visible(true);
            spinner_clone.start();

            // Start scanning in background
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                // Enable scanning for 10 seconds
                let _ = Command::new("bluetoothctl")
                    .args(["--timeout", "10", "scan", "on"])
                    .output();
                let _ = tx.send(());
            });

            // Poll for scan completion
            let spinner = spinner_clone.clone();
            let btn = btn.clone();
            let is_scanning = is_scanning_clone.clone();
            let device_list = device_list_clone.clone();
            let devices = devices_clone.clone();

            glib::timeout_add_local(Duration::from_millis(500), move || {
                if rx.try_recv().is_ok() {
                    // Scan finished
                    spinner.stop();
                    spinner.set_visible(false);
                    btn.set_sensitive(true);
                    *is_scanning.borrow_mut() = false;

                    // Refresh device list
                    refresh_device_list(&device_list, &devices);
                    return glib::ControlFlow::Break;
                }

                // Still scanning, refresh list periodically
                refresh_device_list(&device_list, &devices);
                glib::ControlFlow::Continue
            });
        });

        // Initial device load
        refresh_device_list(&panel.device_list, &panel.devices);

        panel
    }

    pub fn refresh(&self) {
        refresh_device_list(&self.device_list, &self.devices);
    }
}

fn refresh_device_list(list: &ListBox, devices: &Rc<RefCell<HashMap<String, BluetoothDevice>>>) {
    // Clear existing
    while let Some(row) = list.first_child() {
        list.remove(&row);
    }

    // Get devices
    let new_devices = get_bluetooth_devices();
    *devices.borrow_mut() = new_devices
        .iter()
        .map(|d| (d.address.clone(), d.clone()))
        .collect();

    if new_devices.is_empty() {
        let empty_label = Label::new(Some("No devices found"));
        empty_label.add_css_class("bluetooth-empty");
        let row = ListBoxRow::new();
        row.set_selectable(false);
        row.set_child(Some(&empty_label));
        list.append(&row);
        return;
    }

    // Group devices: connected first, then paired, then available
    let mut connected: Vec<_> = new_devices.iter().filter(|d| d.connected).collect();
    let mut paired: Vec<_> = new_devices
        .iter()
        .filter(|d| d.paired && !d.connected)
        .collect();
    let mut available: Vec<_> = new_devices
        .iter()
        .filter(|d| !d.paired && !d.connected)
        .collect();

    connected.sort_by(|a, b| a.name.cmp(&b.name));
    paired.sort_by(|a, b| a.name.cmp(&b.name));
    available.sort_by(|a, b| a.name.cmp(&b.name));

    // Add section headers and devices
    if !connected.is_empty() {
        add_section_header(list, "Connected");
        for device in connected {
            add_device_row(list, device);
        }
    }

    if !paired.is_empty() {
        add_section_header(list, "Paired");
        for device in paired {
            add_device_row(list, device);
        }
    }

    if !available.is_empty() {
        add_section_header(list, "Available");
        for device in available {
            add_device_row(list, device);
        }
    }
}

fn add_section_header(list: &ListBox, title: &str) {
    let label = Label::new(Some(title));
    label.add_css_class("bluetooth-section-header");
    label.set_halign(gtk4::Align::Start);

    let row = ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.add_css_class("bluetooth-header-row");
    row.set_child(Some(&label));
    list.append(&row);
}

fn add_device_row(list: &ListBox, device: &BluetoothDevice) {
    let row = ListBoxRow::new();
    row.add_css_class("bluetooth-device-row");

    let hbox = Box::new(Orientation::Horizontal, 12);
    hbox.add_css_class("bluetooth-device");

    // Device icon
    let icon = Image::from_icon_name(&device.icon);
    icon.add_css_class("bluetooth-device-icon");
    if device.connected {
        icon.add_css_class("connected");
    }
    hbox.append(&icon);

    // Device info
    let info_box = Box::new(Orientation::Vertical, 2);
    info_box.set_hexpand(true);

    let name_label = Label::new(Some(&device.name));
    name_label.add_css_class("bluetooth-device-name");
    name_label.set_halign(gtk4::Align::Start);
    name_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    info_box.append(&name_label);

    let status = if device.connected {
        "Connected"
    } else if device.paired {
        "Paired"
    } else {
        "Available"
    };
    let status_label = Label::new(Some(status));
    status_label.add_css_class("bluetooth-device-status");
    if device.connected {
        status_label.add_css_class("connected");
    }
    status_label.set_halign(gtk4::Align::Start);
    info_box.append(&status_label);

    hbox.append(&info_box);

    // Connect/Disconnect button
    let action_btn = Button::new();
    action_btn.add_css_class("bluetooth-action-btn");

    if device.connected {
        action_btn.set_icon_name("network-offline-symbolic");
        action_btn.set_tooltip_text(Some("Disconnect"));
    } else {
        action_btn.set_icon_name("network-transmit-receive-symbolic");
        action_btn.set_tooltip_text(Some("Connect"));
    }

    let addr = device.address.clone();
    let is_connected = device.connected;
    let is_paired = device.paired;

    action_btn.connect_clicked(move |btn| {
        btn.set_sensitive(false);

        let addr = addr.clone();
        let was_connected = is_connected;
        let was_paired = is_paired;

        // Run in background thread
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let result = if was_connected {
                disconnect_device(&addr)
            } else {
                // If not paired, pair first
                if !was_paired {
                    if !pair_device(&addr) {
                        return;
                    }
                    // Trust the device
                    trust_device(&addr);
                }
                connect_device(&addr)
            };
            let _ = tx.send(result);
        });

        // Re-enable button when done
        let btn_clone = btn.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            if rx.try_recv().is_ok() {
                btn_clone.set_sensitive(true);
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    });

    hbox.append(&action_btn);

    row.set_child(Some(&hbox));

    // Right-click menu for pair/trust/remove
    let gesture = gtk4::GestureClick::new();
    gesture.set_button(3); // Right click
    let addr = device.address.clone();
    let paired = device.paired;
    let trusted = device.trusted;

    gesture.connect_released(move |_, _, x, y| {
        show_device_context_menu(&addr, paired, trusted, x, y);
    });
    row.add_controller(gesture);

    // Long press for touch devices
    let long_press = gtk4::GestureLongPress::new();
    let addr = device.address.clone();
    let paired = device.paired;
    let trusted = device.trusted;

    long_press.connect_pressed(move |_, x, y| {
        show_device_context_menu(&addr, paired, trusted, x, y);
    });
    row.add_controller(long_press);

    list.append(&row);
}

fn show_device_context_menu(addr: &str, paired: bool, trusted: bool, _x: f64, _y: f64) {
    // For now, just do the most useful action inline
    // A proper popover menu would require more GTK setup
    let addr = addr.to_string();

    if !paired {
        // Pair and trust
        thread::spawn(move || {
            if pair_device(&addr) {
                trust_device(&addr);
            }
        });
    } else if !trusted {
        // Trust
        thread::spawn(move || {
            trust_device(&addr);
        });
    } else {
        // Remove device
        thread::spawn(move || {
            remove_device(&addr);
        });
    }
}

// ============================================================================
// Bluetooth Control Functions
// ============================================================================

pub fn is_bluetooth_enabled() -> bool {
    Command::new("bluetoothctl")
        .args(["show"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("Powered: yes"))
        .unwrap_or(false)
}

fn toggle_bluetooth_power(enable: bool) {
    let state = if enable { "on" } else { "off" };
    let _ = Command::new("bluetoothctl").args(["power", state]).spawn();
}

fn get_bluetooth_devices() -> Vec<BluetoothDevice> {
    let mut devices = Vec::new();

    // Get paired devices
    if let Ok(output) = Command::new("bluetoothctl")
        .args(["devices", "Paired"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(device) = parse_device_line(line) {
                devices.push(device);
            }
        }
    }

    // Get all known devices (includes discovered)
    if let Ok(output) = Command::new("bluetoothctl").args(["devices"]).output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(device) = parse_device_line(line) {
                // Only add if not already in list
                if !devices.iter().any(|d| d.address == device.address) {
                    devices.push(device);
                }
            }
        }
    }

    // Enrich with connection/paired/trusted status
    for device in &mut devices {
        if let Ok(output) = Command::new("bluetoothctl")
            .args(["info", &device.address])
            .output()
        {
            let info = String::from_utf8_lossy(&output.stdout);
            device.connected = info.contains("Connected: yes");
            device.paired = info.contains("Paired: yes");
            device.trusted = info.contains("Trusted: yes");

            // Get device icon/type
            if info.contains("Icon: audio-headset") || info.contains("Icon: audio-headphones") {
                device.icon = "audio-headphones-symbolic".to_string();
            } else if info.contains("Icon: audio-card") {
                device.icon = "audio-speakers-symbolic".to_string();
            } else if info.contains("Icon: input-keyboard") {
                device.icon = "input-keyboard-symbolic".to_string();
            } else if info.contains("Icon: input-mouse") {
                device.icon = "input-mouse-symbolic".to_string();
            } else if info.contains("Icon: input-gaming") {
                device.icon = "input-gaming-symbolic".to_string();
            } else if info.contains("Icon: phone") {
                device.icon = "phone-symbolic".to_string();
            } else if info.contains("Icon: computer") {
                device.icon = "computer-symbolic".to_string();
            }
        }
    }

    devices
}

fn parse_device_line(line: &str) -> Option<BluetoothDevice> {
    // Format: "Device XX:XX:XX:XX:XX:XX Device Name"
    let parts: Vec<&str> = line.splitn(3, ' ').collect();
    if parts.len() >= 3 && parts[0] == "Device" {
        let address = parts[1].to_string();
        let name = parts[2].to_string();

        Some(BluetoothDevice {
            address,
            name,
            paired: false,
            trusted: false,
            connected: false,
            icon: "bluetooth-symbolic".to_string(),
        })
    } else {
        None
    }
}

fn connect_device(address: &str) -> bool {
    Command::new("bluetoothctl")
        .args(["connect", address])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn disconnect_device(address: &str) -> bool {
    Command::new("bluetoothctl")
        .args(["disconnect", address])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn pair_device(address: &str) -> bool {
    Command::new("bluetoothctl")
        .args(["pair", address])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn trust_device(address: &str) -> bool {
    Command::new("bluetoothctl")
        .args(["trust", address])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn remove_device(address: &str) -> bool {
    Command::new("bluetoothctl")
        .args(["remove", address])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
