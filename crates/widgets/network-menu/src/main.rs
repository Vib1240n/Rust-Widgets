mod config;
mod css;
mod network;

use config::Config;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box, Button, Image, Label, Orientation, PasswordEntry,
    Popover, PositionType, ScrolledWindow, Separator, Switch,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const APP_ID: &str = "com.vib1240n.rust-widgets.network-menu";
const BINARY_NAME: &str = "rw-network";

/// Late-bound, self-referential "rebuild the list" callback.
type RebuildCell = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

fn main() {
    if is_already_running() {
        eprintln!("rw-network is already running");
        std::process::exit(0);
    }

    tracing_subscriber::fmt().with_env_filter("info").init();

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run();
}

fn is_already_running() -> bool {
    let my_pid = std::process::id();
    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if let Ok(pid) = name_str.parse::<u32>() {
                if pid == my_pid {
                    continue;
                }
                let comm_path = entry.path().join("comm");
                if let Ok(comm) = std::fs::read_to_string(&comm_path) {
                    if comm.trim() == BINARY_NAME {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn build_ui(app: &Application) {
    let config = Rc::new(Config::load());
    css::load();

    let window = Rc::new(
        ApplicationWindow::builder()
            .application(app)
            .title("Network")
            .build(),
    );

    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_namespace("rust-widgets");
    window.set_keyboard_mode(KeyboardMode::OnDemand);
    apply_position(&window, &config);

    let container = Box::new(Orientation::Vertical, 0);
    container.add_css_class("widget-container");
    container.set_width_request(config.appearance.width);

    // ===== Header: icon + status + wifi switch =====
    let header = Box::new(Orientation::Horizontal, 10);
    header.add_css_class("network-header");

    let header_icon = Image::from_icon_name("network-wireless-symbolic");
    header_icon.add_css_class("network-header-icon");
    header.append(&header_icon);

    let header_text = Box::new(Orientation::Vertical, 0);
    header_text.set_hexpand(true);
    let title = Label::new(Some("Network"));
    title.add_css_class("network-title");
    title.set_halign(Align::Start);
    let status_label = Label::new(Some("…"));
    status_label.add_css_class("network-status");
    status_label.set_halign(Align::Start);
    status_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    status_label.set_max_width_chars(30);
    header_text.append(&title);
    header_text.append(&status_label);
    header.append(&header_text);

    let wifi_switch = Switch::new();
    wifi_switch.add_css_class("network-switch");
    wifi_switch.set_valign(Align::Center);
    header.append(&wifi_switch);

    container.append(&header);
    container.append(&Separator::new(Orientation::Horizontal));

    // ===== Actions row =====
    let actions = Box::new(Orientation::Horizontal, 8);
    actions.add_css_class("network-actions");
    let avail = Label::new(Some("Available networks"));
    avail.add_css_class("section-title");
    avail.set_halign(Align::Start);
    avail.set_hexpand(true);
    actions.append(&avail);

    let rescan_btn = Button::new();
    rescan_btn.add_css_class("network-rescan-btn");
    rescan_btn.set_child(Some(&Image::from_icon_name("view-refresh-symbolic")));
    actions.append(&rescan_btn);
    container.append(&actions);

    // ===== Network list =====
    let scroll = ScrolledWindow::new();
    scroll.add_css_class("network-scroll");
    scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroll.set_propagate_natural_height(true);
    scroll.set_max_content_height(360);

    let list = Rc::new(Box::new(Orientation::Vertical, 4));
    list.add_css_class("network-list");
    scroll.set_child(Some(&*list));
    container.append(&scroll);

    window.set_child(Some(&container));

    // ===== Rebuild list (self-referential so row actions can re-trigger it) =====
    let rebuild: RebuildCell = Rc::new(RefCell::new(None));
    {
        let list = list.clone();
        let max = config.appearance.max_networks;
        let rebuild_inner = rebuild.clone();
        let f: Rc<dyn Fn()> = Rc::new(move || {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            let nets = network::list_networks(max);
            if nets.is_empty() {
                let empty = Label::new(Some("No networks found"));
                empty.add_css_class("network-empty");
                empty.set_halign(Align::Center);
                list.append(&empty);
                return;
            }
            for net in &nets {
                list.append(&build_network_row(net, &rebuild_inner));
            }
        });
        *rebuild.borrow_mut() = Some(f);
    }

    // ===== Status refresh =====
    let refresh_status: Rc<dyn Fn()> = {
        let status_label = status_label.clone();
        let header_icon = header_icon.clone();
        Rc::new(move || {
            let st = network::get_status();
            let (icon, text) = match st.conn_type {
                network::ConnType::Ethernet => {
                    ("network-wired-symbolic", format!("Ethernet · {}", st.name))
                }
                network::ConnType::Wifi => {
                    ("network-wireless-symbolic", format!("Wi-Fi · {}", st.name))
                }
                network::ConnType::None => {
                    if st.wifi_enabled {
                        ("network-wireless-offline-symbolic", "Disconnected".into())
                    } else {
                        ("network-wireless-disabled-symbolic", "Wi-Fi off".into())
                    }
                }
            };
            header_icon.set_icon_name(Some(icon));
            status_label.set_text(&text);
        })
    };

    // Wifi switch: set state before wiring handler to avoid feedback loop
    wifi_switch.set_active(network::wifi_enabled());
    {
        let rebuild = rebuild.clone();
        wifi_switch.connect_state_set(move |_, state| {
            network::set_wifi_enabled(state);
            schedule_rebuild(&rebuild, 1500);
            glib::Propagation::Proceed
        });
    }

    // Rescan
    {
        let rebuild = rebuild.clone();
        rescan_btn.connect_clicked(move |b| {
            b.set_sensitive(false);
            std::thread::spawn(|| network::rescan());
            let b = b.clone();
            let rebuild = rebuild.clone();
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                if let Some(f) = rebuild.borrow().as_ref() {
                    f();
                }
                b.set_sensitive(true);
            });
        });
    }

    // Initial population
    refresh_status();
    if let Some(f) = rebuild.borrow().as_ref() {
        f();
    }

    // Poll status only (list refreshes on action/rescan to avoid disrupting popovers)
    {
        let rs = refresh_status.clone();
        glib::timeout_add_local(
            Duration::from_millis(config.behavior.status_poll),
            move || {
                rs();
                glib::ControlFlow::Continue
            },
        );
    }

    // Escape closes
    if config.behavior.close_on_escape {
        let win = window.clone();
        let key = gtk4::EventControllerKey::new();
        key.connect_key_pressed(move |_, k, _, _| {
            if k == gtk4::gdk::Key::Escape {
                win.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(key);
    }

    // Optional click-away close (armed after a beat to skip the spurious initial leave)
    if config.behavior.close_on_unfocus {
        let armed = Rc::new(RefCell::new(false));
        {
            let armed = armed.clone();
            glib::timeout_add_local_once(Duration::from_millis(400), move || {
                *armed.borrow_mut() = true;
            });
        }
        let win = window.clone();
        let focus = gtk4::EventControllerFocus::new();
        focus.connect_leave(move |_| {
            if *armed.borrow() {
                win.close();
            }
        });
        window.add_controller(focus);
    }

    window.present();
}

fn build_network_row(net: &network::WifiNetwork, rebuild: &RebuildCell) -> Button {
    let btn = Button::new();
    btn.add_css_class("network-row");
    if net.active {
        btn.add_css_class("active");
    }

    let content = Box::new(Orientation::Horizontal, 10);

    let sig = Image::from_icon_name(network::signal_icon(net.signal));
    sig.add_css_class("network-signal");
    content.append(&sig);

    let info = Box::new(Orientation::Vertical, 0);
    info.set_hexpand(true);
    let name = Label::new(Some(&net.ssid));
    name.add_css_class("network-ssid");
    name.set_halign(Align::Start);
    name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    name.set_max_width_chars(24);
    info.append(&name);

    let sub_text = if net.active {
        Some("Connected")
    } else if net.known {
        Some("Saved")
    } else {
        None
    };
    if let Some(t) = sub_text {
        let sub = Label::new(Some(t));
        sub.add_css_class("network-row-sub");
        sub.set_halign(Align::Start);
        info.append(&sub);
    }
    content.append(&info);

    if net.secured {
        let lock = Image::from_icon_name("network-wireless-encrypted-symbolic");
        lock.add_css_class("network-lock");
        content.append(&lock);
    }

    btn.set_child(Some(&content));

    // Left-click: connect (or password prompt for secured + unknown)
    {
        let ssid = net.ssid.clone();
        let known = net.known;
        let secured = net.secured;
        let active = net.active;
        let rebuild = rebuild.clone();
        btn.connect_clicked(move |b| {
            if active {
                return; // already connected; right-click to disconnect
            }
            if known || !secured {
                spawn_connect(ssid.clone(), None, known);
                schedule_rebuild(&rebuild, 2000);
            } else {
                show_password_popover(b, &ssid, &rebuild);
            }
        });
    }

    // Right-click: options popover
    {
        let gesture = gtk4::GestureClick::new();
        gesture.set_button(3);
        let ssid = net.ssid.clone();
        let active = net.active;
        let known = net.known;
        let secured = net.secured;
        let rebuild = rebuild.clone();
        let btn_c = btn.clone();
        gesture.connect_released(move |_, _, _, _| {
            show_options_popover(&btn_c, &ssid, active, known, secured, &rebuild);
        });
        btn.add_controller(gesture);
    }

    btn
}

fn spawn_connect(ssid: String, password: Option<String>, known: bool) {
    std::thread::spawn(move || {
        if let Err(e) = network::connect(&ssid, password.as_deref(), known) {
            let _ = std::process::Command::new("notify-send")
                .args(["-u", "critical", &format!("Wi-Fi: {}", ssid), &e])
                .spawn();
        }
    });
}

fn schedule_rebuild(rebuild: &RebuildCell, delay_ms: u64) {
    let rebuild = rebuild.clone();
    glib::timeout_add_local_once(Duration::from_millis(delay_ms), move || {
        if let Some(f) = rebuild.borrow().as_ref() {
            f();
        }
    });
}

fn show_password_popover(anchor: &Button, ssid: &str, rebuild: &RebuildCell) {
    let pop = Popover::new();
    pop.set_parent(anchor);
    pop.set_position(PositionType::Bottom);
    pop.add_css_class("network-popover");

    let b = Box::new(Orientation::Vertical, 8);
    let lbl = Label::new(Some(&format!("Password for {}", ssid)));
    lbl.add_css_class("network-popover-title");
    lbl.set_halign(Align::Start);
    b.append(&lbl);

    let entry = PasswordEntry::new();
    entry.set_show_peek_icon(true);
    b.append(&entry);

    let connect = Button::with_label("Connect");
    connect.add_css_class("network-connect-btn");
    b.append(&connect);

    pop.set_child(Some(&b));

    {
        let ssid = ssid.to_string();
        let rebuild = rebuild.clone();
        let pop = pop.clone();
        let entry = entry.clone();
        connect.connect_clicked(move |_| {
            let pw = entry.text().to_string();
            spawn_connect(ssid.clone(), Some(pw), false);
            pop.popdown();
            schedule_rebuild(&rebuild, 2500);
        });
    }
    {
        let connect = connect.clone();
        entry.connect_activate(move |_| connect.emit_clicked());
    }

    pop.popup();
    entry.grab_focus();
}

fn show_options_popover(
    anchor: &Button,
    ssid: &str,
    active: bool,
    known: bool,
    secured: bool,
    rebuild: &RebuildCell,
) {
    let pop = Popover::new();
    pop.set_parent(anchor);
    pop.set_position(PositionType::Bottom);
    pop.add_css_class("network-popover");

    let b = Box::new(Orientation::Vertical, 2);

    if active {
        let item = menu_item("Disconnect", false);
        let ssid = ssid.to_string();
        let rebuild = rebuild.clone();
        let pop_c = pop.clone();
        item.connect_clicked(move |_| {
            let s = ssid.clone();
            std::thread::spawn(move || {
                let _ = network::disconnect(&s);
            });
            pop_c.popdown();
            schedule_rebuild(&rebuild, 1500);
        });
        b.append(&item);
    } else {
        let item = menu_item("Connect", false);
        let ssid = ssid.to_string();
        let rebuild = rebuild.clone();
        let pop_c = pop.clone();
        let anchor_c = anchor.clone();
        item.connect_clicked(move |_| {
            pop_c.popdown();
            if !known && secured {
                show_password_popover(&anchor_c, &ssid, &rebuild);
            } else {
                spawn_connect(ssid.clone(), None, known);
                schedule_rebuild(&rebuild, 2000);
            }
        });
        b.append(&item);
    }

    if known {
        let item = menu_item("Forget", true);
        let ssid = ssid.to_string();
        let rebuild = rebuild.clone();
        let pop_c = pop.clone();
        item.connect_clicked(move |_| {
            let s = ssid.clone();
            std::thread::spawn(move || {
                let _ = network::forget(&s);
            });
            pop_c.popdown();
            schedule_rebuild(&rebuild, 1500);
        });
        b.append(&item);
    }

    let settings = menu_item("Settings…", false);
    {
        let pop_c = pop.clone();
        settings.connect_clicked(move |_| {
            let _ = std::process::Command::new("nm-connection-editor").spawn();
            pop_c.popdown();
        });
    }
    b.append(&settings);

    pop.set_child(Some(&b));
    pop.popup();
}

fn menu_item(label: &str, destructive: bool) -> Button {
    let btn = Button::with_label(label);
    btn.add_css_class("network-menu-item");
    if destructive {
        btn.add_css_class("destructive");
    }
    btn
}

fn apply_position(window: &ApplicationWindow, config: &Config) {
    window.set_anchor(Edge::Top, false);
    window.set_anchor(Edge::Bottom, false);
    window.set_anchor(Edge::Left, false);
    window.set_anchor(Edge::Right, false);

    match config.position.anchor.as_str() {
        "top-left" => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Left, true);
        }
        "top-center" => {
            window.set_anchor(Edge::Top, true);
        }
        "top-right" => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Right, true);
        }
        "bottom-left" => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
        }
        "bottom-center" => {
            window.set_anchor(Edge::Bottom, true);
        }
        "bottom-right" => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Right, true);
        }
        "center" => {}
        _ => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Right, true);
        }
    }

    window.set_margin(Edge::Top, config.position.margin_top);
    window.set_margin(Edge::Right, config.position.margin_right);
    window.set_margin(Edge::Bottom, config.position.margin_bottom);
    window.set_margin(Edge::Left, config.position.margin_left);
}
