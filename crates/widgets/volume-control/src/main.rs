mod audio;
mod config;
mod css;

use config::Config;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box, Button, Image, Label, ListBox, ListBoxRow, Orientation,
    Popover, Scale, ScrolledWindow, Separator,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const APP_ID: &str = "com.vib1240n.rust-widgets.volume-control";
const BINARY_NAME: &str = "rw-volume";

fn main() {
    // Enforce single instance - exit if already running
    if is_already_running() {
        eprintln!("rw-volume is already running");
        std::process::exit(0);
    }

    tracing_subscriber::fmt().with_env_filter("info").init();

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run();
}

/// Check if another instance of this widget is already running
fn is_already_running() -> bool {
    let my_pid = std::process::id();

    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if let Ok(pid) = name_str.parse::<u32>() {
                // Skip our own process
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

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Volume Control")
        .build();

    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_namespace("rust-widgets");
    window.set_keyboard_mode(KeyboardMode::OnDemand);

    apply_position(&window, &config);

    // Main container
    let container = Box::new(Orientation::Vertical, 0);
    container.add_css_class("volume-control");
    container.set_width_request(config.appearance.width);

    // ===== OUTPUT SECTION =====
    let output_section = Box::new(Orientation::Vertical, 8);

    // Output device selector
    if config.appearance.show_device_selector {
        let output_header = create_section_header("Output Device");
        output_section.append(&output_header);

        let output_selector = create_device_selector(true, config.appearance.device_name_max_chars);
        output_section.append(&output_selector);
    }

    // Master volume
    let master_volume = create_master_volume(config.behavior.volume_override);
    output_section.append(&master_volume);

    container.append(&output_section);

    // ===== APP VOLUMES SECTION =====
    if config.appearance.show_app_volumes {
        let separator = Separator::new(Orientation::Horizontal);
        separator.add_css_class("section-separator");
        container.append(&separator);

        let apps_header = create_section_header("Applications");
        container.append(&apps_header);

        let apps_container = Box::new(Orientation::Vertical, 0);
        apps_container.add_css_class("app-volumes");

        // Scrollable area for apps
        let scroll = ScrolledWindow::new();
        scroll.add_css_class("app-scroll");
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        scroll.set_propagate_natural_height(true);
        scroll.set_max_content_height(200);

        let apps_list = Box::new(Orientation::Vertical, 0);
        apps_list.add_css_class("apps-list");
        scroll.set_child(Some(&apps_list));
        apps_container.append(&scroll);

        container.append(&apps_container);

        // Store reference for updates
        let apps_list_ref = Rc::new(RefCell::new(apps_list));
        let volume_override = config.behavior.volume_override;
        let app_name_max_chars = config.appearance.app_name_max_chars;
        let stream_name_max_chars = config.appearance.stream_name_max_chars;
        update_app_streams(
            &apps_list_ref.borrow(),
            volume_override,
            app_name_max_chars,
            stream_name_max_chars,
        );

        // Poll for app changes
        let apps_list_poll = apps_list_ref.clone();
        let poll_interval = config.behavior.poll_interval;
        glib::timeout_add_local(Duration::from_millis(poll_interval), move || {
            update_app_streams(
                &apps_list_poll.borrow(),
                volume_override,
                app_name_max_chars,
                stream_name_max_chars,
            );
            glib::ControlFlow::Continue
        });
    }

    // ===== INPUT SECTION =====
    if config.appearance.show_input {
        let separator = Separator::new(Orientation::Horizontal);
        separator.add_css_class("section-separator");
        container.append(&separator);

        let input_section = Box::new(Orientation::Vertical, 8);
        input_section.add_css_class("input-section");

        // Input device selector
        if config.appearance.show_device_selector {
            let input_header = create_section_header("Input Device");
            input_section.append(&input_header);

            let input_selector =
                create_device_selector(false, config.appearance.device_name_max_chars);
            input_section.append(&input_selector);
        }

        // Input volume
        let input_volume = create_input_volume(config.behavior.volume_override);
        input_section.append(&input_volume);

        container.append(&input_section);
    }

    window.set_child(Some(&container));

    // Close on Escape
    let win_close = window.clone();
    let controller = gtk4::EventControllerKey::new();
    controller.connect_key_pressed(move |_, key, _, _| {
        if key == gtk4::gdk::Key::Escape {
            win_close.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(controller);

    // Poll for volume changes
    let master_vol_ref = Rc::new(RefCell::new(None::<(Scale, Label, Button)>));

    window.present();
}

fn create_section_header(title: &str) -> Box {
    let header = Box::new(Orientation::Horizontal, 0);
    header.add_css_class("section-header");

    let label = Label::new(Some(title));
    label.add_css_class("section-title");
    label.set_halign(gtk4::Align::Start);
    header.append(&label);

    header
}

fn create_device_selector(is_output: bool, device_name_max_chars: usize) -> Box {
    let container = Box::new(Orientation::Vertical, 4);
    container.add_css_class("device-selector");

    let devices = if is_output {
        audio::get_sinks()
    } else {
        audio::get_sources()
    };

    // Find current default
    let current_device = devices.iter().find(|d| d.is_default).cloned();
    let current_name = current_device
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "No device".to_string());

    // Main button showing current device
    let button = Button::new();
    button.add_css_class("device-button");
    button.add_css_class("active");

    let button_content = Box::new(Orientation::Horizontal, 8);
    button_content.set_hexpand(true);

    let icon_name = if is_output {
        "audio-speakers-symbolic"
    } else {
        "audio-input-microphone-symbolic"
    };
    let icon = Image::from_icon_name(icon_name);
    icon.add_css_class("device-icon");
    button_content.append(&icon);

    let name_label = Label::new(Some(&truncate_string(&current_name, device_name_max_chars)));
    name_label.add_css_class("device-name");
    name_label.set_hexpand(true);
    name_label.set_halign(gtk4::Align::Start);
    name_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    button_content.append(&name_label);

    let chevron = Image::from_icon_name("pan-down-symbolic");
    chevron.add_css_class("device-chevron");
    button_content.append(&chevron);

    button.set_child(Some(&button_content));

    // Create popover with device list
    let popover = Popover::new();
    popover.set_parent(&button);

    let list_box = ListBox::new();
    list_box.add_css_class("device-list");
    list_box.set_selection_mode(gtk4::SelectionMode::None);

    for device in &devices {
        let row = ListBoxRow::new();
        row.add_css_class("device-list-item");
        if device.is_default {
            row.add_css_class("selected");
        }

        let row_box = Box::new(Orientation::Horizontal, 8);

        let row_icon = Image::from_icon_name(icon_name);
        row_icon.add_css_class("device-icon");
        row_box.append(&row_icon);

        let row_name = Label::new(Some(&device.name));
        row_name.add_css_class("device-name");
        row_name.set_hexpand(true);
        row_name.set_halign(gtk4::Align::Start);
        row_name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        row_box.append(&row_name);

        if device.is_default {
            let check = Image::from_icon_name("object-select-symbolic");
            check.add_css_class("device-check");
            row_box.append(&check);
        }

        row.set_child(Some(&row_box));

        // Store device ID in row name for retrieval
        row.set_widget_name(&device.id.to_string());

        list_box.append(&row);
    }

    // Handle device selection
    let name_label_clone = name_label.clone();
    let popover_clone = popover.clone();
    let is_output_clone = is_output;
    list_box.connect_row_activated(move |_, row| {
        if let Ok(id) = row.widget_name().parse::<u32>() {
            if is_output_clone {
                audio::set_default_sink(id);
            } else {
                audio::set_default_source(id);
            }

            // Update button label
            if let Some(child) = row.child() {
                if let Some(row_box) = child.downcast_ref::<Box>() {
                    if let Some(label) = row_box.first_child().and_then(|w| w.next_sibling()) {
                        if let Some(l) = label.downcast_ref::<Label>() {
                            name_label_clone.set_text(l.text().as_str());
                        }
                    }
                }
            }

            popover_clone.popdown();
        }
    });

    popover.set_child(Some(&list_box));

    button.connect_clicked(move |_| {
        popover.popup();
    });

    container.append(&button);
    container
}

fn create_master_volume(volume_override: bool) -> Box {
    let container = Box::new(Orientation::Vertical, 8);
    container.add_css_class("master-volume");

    let row = Box::new(Orientation::Horizontal, 8);
    row.add_css_class("volume-row");

    // Mute button with icon
    let mute_btn = Button::new();
    mute_btn.add_css_class("mute-button");

    let vol = audio::get_sink_volume();
    let muted = audio::is_sink_muted();

    let icon_name = get_volume_icon(vol, muted);
    let icon = Image::from_icon_name(icon_name);
    icon.add_css_class("volume-icon");
    if muted {
        icon.add_css_class("muted");
        mute_btn.add_css_class("muted");
    }
    mute_btn.set_child(Some(&icon));

    let icon_ref = Rc::new(RefCell::new(icon.clone()));
    let mute_btn_ref = Rc::new(RefCell::new(mute_btn.clone()));

    mute_btn.connect_clicked({
        let icon_ref = icon_ref.clone();
        let mute_btn_ref = mute_btn_ref.clone();
        move |_| {
            audio::toggle_sink_mute();
            // Update icon after toggle
            glib::timeout_add_local_once(Duration::from_millis(50), {
                let icon_ref = icon_ref.clone();
                let mute_btn_ref = mute_btn_ref.clone();
                move || {
                    let muted = audio::is_sink_muted();
                    let vol = audio::get_sink_volume();
                    let icon = icon_ref.borrow();
                    let mute_btn = mute_btn_ref.borrow();
                    icon.set_icon_name(Some(get_volume_icon(vol, muted)));
                    if muted {
                        icon.add_css_class("muted");
                        mute_btn.add_css_class("muted");
                    } else {
                        icon.remove_css_class("muted");
                        mute_btn.remove_css_class("muted");
                    }
                }
            });
        }
    });

    row.append(&mute_btn);

    // Volume slider - max 100% unless volume_override is enabled
    let max_vol = if volume_override { 1.5 } else { 1.0 };
    let scale = Scale::with_range(Orientation::Horizontal, 0.0, max_vol, 0.01);
    scale.add_css_class("volume-slider");
    scale.set_hexpand(true);
    scale.set_value(vol.min(max_vol as f32) as f64);
    if muted {
        scale.add_css_class("muted");
    }

    // Value label
    let value_label = Label::new(Some(&format!("{}%", (vol * 100.0) as u32)));
    value_label.add_css_class("volume-value");
    if muted {
        value_label.add_css_class("muted");
    }

    let value_label_clone = value_label.clone();
    let icon_ref_scale = icon_ref.clone();
    scale.connect_value_changed(move |s| {
        let vol = s.value() as f32;
        audio::set_sink_volume(vol);
        value_label_clone.set_text(&format!("{}%", (vol * 100.0) as u32));

        // Update icon based on volume
        let muted = audio::is_sink_muted();
        icon_ref_scale
            .borrow()
            .set_icon_name(Some(get_volume_icon(vol, muted)));
    });

    row.append(&scale);
    row.append(&value_label);

    container.append(&row);
    container
}

fn create_input_volume(volume_override: bool) -> Box {
    let container = Box::new(Orientation::Vertical, 8);
    container.add_css_class("input-volume");

    let row = Box::new(Orientation::Horizontal, 8);
    row.add_css_class("volume-row");

    // Mute button
    let mute_btn = Button::new();
    mute_btn.add_css_class("mute-button");

    let vol = audio::get_source_volume();
    let muted = audio::is_source_muted();

    let icon_name = if muted {
        "microphone-disabled-symbolic"
    } else {
        "audio-input-microphone-symbolic"
    };
    let icon = Image::from_icon_name(icon_name);
    icon.add_css_class("input-icon");
    if muted {
        icon.add_css_class("muted");
        mute_btn.add_css_class("muted");
    }
    mute_btn.set_child(Some(&icon));

    let icon_ref = Rc::new(RefCell::new(icon.clone()));
    let mute_btn_ref = Rc::new(RefCell::new(mute_btn.clone()));

    mute_btn.connect_clicked({
        let icon_ref = icon_ref.clone();
        let mute_btn_ref = mute_btn_ref.clone();
        move |_| {
            audio::toggle_source_mute();
            glib::timeout_add_local_once(Duration::from_millis(50), {
                let icon_ref = icon_ref.clone();
                let mute_btn_ref = mute_btn_ref.clone();
                move || {
                    let muted = audio::is_source_muted();
                    let icon = icon_ref.borrow();
                    let mute_btn = mute_btn_ref.borrow();
                    let icon_name = if muted {
                        "microphone-disabled-symbolic"
                    } else {
                        "audio-input-microphone-symbolic"
                    };
                    icon.set_icon_name(Some(icon_name));
                    if muted {
                        icon.add_css_class("muted");
                        mute_btn.add_css_class("muted");
                    } else {
                        icon.remove_css_class("muted");
                        mute_btn.remove_css_class("muted");
                    }
                }
            });
        }
    });

    row.append(&mute_btn);

    // Volume slider - max 100% unless volume_override is enabled
    let max_vol = if volume_override { 1.5 } else { 1.0 };
    let scale = Scale::with_range(Orientation::Horizontal, 0.0, max_vol, 0.01);
    scale.add_css_class("volume-slider");
    scale.set_hexpand(true);
    scale.set_value(vol.min(max_vol as f32) as f64);
    if muted {
        scale.add_css_class("muted");
    }

    let value_label = Label::new(Some(&format!("{}%", (vol * 100.0) as u32)));
    value_label.add_css_class("volume-value");
    if muted {
        value_label.add_css_class("muted");
    }

    let value_label_clone = value_label.clone();
    scale.connect_value_changed(move |s| {
        let vol = s.value() as f32;
        audio::set_source_volume(vol);
        value_label_clone.set_text(&format!("{}%", (vol * 100.0) as u32));
    });

    row.append(&scale);
    row.append(&value_label);

    container.append(&row);
    container
}

fn update_app_streams(
    apps_list: &Box,
    volume_override: bool,
    app_name_max_chars: i32,
    stream_name_max_chars: usize,
) {
    // Clear existing
    while let Some(child) = apps_list.first_child() {
        apps_list.remove(&child);
    }

    let streams = audio::get_streams();

    if streams.is_empty() {
        let empty = Box::new(Orientation::Horizontal, 0);
        empty.add_css_class("empty-state");
        empty.set_halign(gtk4::Align::Center);

        let label = Label::new(Some("No applications playing audio"));
        label.add_css_class("empty-label");
        empty.append(&label);

        apps_list.append(&empty);
        return;
    }

    for stream in streams {
        let row = create_app_row(
            &stream,
            volume_override,
            app_name_max_chars,
            stream_name_max_chars,
        );
        apps_list.append(&row);
    }
}

fn create_app_row(
    stream: &audio::AudioStream,
    volume_override: bool,
    app_name_max_chars: i32,
    stream_name_max_chars: usize,
) -> Box {
    let row = Box::new(Orientation::Horizontal, 8);
    row.add_css_class("app-row");

    // App icon
    let icon_name = audio::get_app_icon(&stream.app_name);
    let icon = Image::from_icon_name(icon_name);
    icon.add_css_class("app-icon");
    row.append(&icon);

    // App info
    let info = Box::new(Orientation::Vertical, 0);
    info.add_css_class("app-info");

    let name = Label::new(Some(&stream.app_name));
    name.add_css_class("app-name");
    name.set_halign(gtk4::Align::Start);
    name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    name.set_max_width_chars(app_name_max_chars);
    info.append(&name);

    if !stream.name.is_empty() && stream.name != stream.app_name {
        let stream_name = Label::new(Some(&truncate_string(&stream.name, stream_name_max_chars)));
        stream_name.add_css_class("app-stream");
        stream_name.set_halign(gtk4::Align::Start);
        stream_name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        info.append(&stream_name);
    }

    row.append(&info);

    // Volume slider - max 100% unless volume_override is enabled
    let max_vol = if volume_override { 1.5 } else { 1.0 };
    let scale = Scale::with_range(Orientation::Horizontal, 0.0, max_vol, 0.01);
    scale.add_css_class("app-slider");
    scale.set_hexpand(true);
    scale.set_value(stream.volume.min(max_vol as f32) as f64);
    if stream.muted {
        scale.add_css_class("muted");
    }

    let stream_id = stream.id;
    let value_label = Label::new(Some(&format!("{}%", (stream.volume * 100.0) as u32)));
    value_label.add_css_class("app-volume-value");

    let value_label_clone = value_label.clone();
    scale.connect_value_changed(move |s| {
        let vol = s.value() as f32;
        audio::set_stream_volume(stream_id, vol);
        value_label_clone.set_text(&format!("{}%", (vol * 100.0) as u32));
    });

    row.append(&scale);
    row.append(&value_label);

    // Mute button
    let mute_btn = Button::new();
    mute_btn.add_css_class("app-mute-btn");
    let mute_icon = if stream.muted {
        "audio-volume-muted-symbolic"
    } else {
        "audio-volume-high-symbolic"
    };
    mute_btn.set_child(Some(&Image::from_icon_name(mute_icon)));
    if stream.muted {
        mute_btn.add_css_class("muted");
    }

    let mute_stream_id = stream.id;
    mute_btn.connect_clicked(move |_| {
        audio::toggle_stream_mute(mute_stream_id);
    });

    row.append(&mute_btn);

    row
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

fn get_volume_icon(vol: f32, muted: bool) -> &'static str {
    if muted {
        "audio-volume-muted-symbolic"
    } else if vol <= 0.0 {
        "audio-volume-muted-symbolic"
    } else if vol < 0.33 {
        "audio-volume-low-symbolic"
    } else if vol < 0.66 {
        "audio-volume-medium-symbolic"
    } else {
        "audio-volume-high-symbolic"
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
