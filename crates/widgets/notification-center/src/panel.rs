use crate::config::Config;
use crate::notification::{Notification, NotificationStore};
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Box, Button, Image, Label, ListBox, ListBoxRow, Orientation, PolicyType, ProgressBar,
    ScrolledWindow,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Actions from the panel
#[derive(Debug, Clone)]
pub enum PanelAction {
    Close,
    ClearAll,
    DismissOne(u32),
    ActionInvoked(u32, String),
    ToggleDnd(bool),
    OpenSettings,
}

/// The notification center panel
pub struct NotificationPanel {
    window: gtk4::Window,
    list_box: ListBox,
    header_subtitle: Label,
    empty_state: Box,
    footer: Box,
    action_tx: mpsc::UnboundedSender<PanelAction>,
    config: Arc<Config>,
}

/// Helper to load SF Symbol icons with fallback
fn sf_icon(name: &str, size: i32) -> Image {
    let icon_paths = [
        format!("/home/vib1240n/.local/share/rw/icons/{}.svg", name),
        format!("/usr/share/rw/icons/{}.svg", name),
    ];

    for path in &icon_paths {
        if std::path::Path::new(path).exists() {
            let img = Image::from_file(path);
            img.set_pixel_size(size);
            return img;
        }
    }

    // Fallback to GTK symbolic icons
    let fallback = match name {
        "bell.fill" => "notification-symbolic",
        "gearshape.fill" => "emblem-system-symbolic",
        "trash.fill" => "user-trash-symbolic",
        "checkmark.circle.fill" => "emblem-ok-symbolic",
        "exclamationmark.triangle.fill" => "dialog-warning-symbolic",
        "xmark.circle.fill" => "dialog-error-symbolic",
        "info.circle.fill" => "dialog-information-symbolic",
        "message.fill" => "mail-unread-symbolic",
        _ => "dialog-information-symbolic",
    };

    let img = Image::from_icon_name(fallback);
    img.set_pixel_size(size);
    img
}

impl NotificationPanel {
    pub fn new(
        app: &gtk4::Application,
        config: Arc<Config>,
        action_tx: mpsc::UnboundedSender<PanelAction>,
    ) -> Self {
        let window = gtk4::Window::builder()
            .application(app)
            .title("Notifications")
            .decorated(false)
            .resizable(false)
            .build();

        // Layer shell setup
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_namespace("rust-widgets");
        window.set_keyboard_mode(KeyboardMode::OnDemand);

        // Position from config
        apply_position(&window, &config);

        // ==================== OUTER WRAPPER (for blur corner fix) ====================
        let outer_wrapper = Box::new(Orientation::Vertical, 0);
        outer_wrapper.set_margin_start(16);
        outer_wrapper.set_margin_end(16);
        outer_wrapper.set_margin_top(16);
        outer_wrapper.set_margin_bottom(16);

        // ==================== MAIN CONTAINER ====================
        let container = Box::new(Orientation::Vertical, 0);
        container.add_css_class("nc-container");
        container.set_width_request(config.appearance.panel_width);
        // Let the container expand vertically to fit content
        container.set_vexpand(true);

        // ==================== HEADER ====================
        let header = Box::new(Orientation::Horizontal, 12);
        header.add_css_class("nc-header");

        // Bell icon wrapper (rounded box like Figma)
        let bell_wrapper = Box::new(Orientation::Vertical, 0);
        bell_wrapper.add_css_class("nc-bell-wrapper");
        bell_wrapper.set_valign(gtk4::Align::Center);
        let bell_icon = sf_icon("bell.fill", 20);
        bell_icon.add_css_class("nc-bell-icon");
        bell_wrapper.append(&bell_icon);
        header.append(&bell_wrapper);

        // Title and subtitle column
        let title_box = Box::new(Orientation::Vertical, 2);
        title_box.set_hexpand(true);
        title_box.set_valign(gtk4::Align::Center);

        let title = Label::new(Some("Notifications"));
        title.add_css_class("nc-title");
        title.set_halign(gtk4::Align::Start);
        title_box.append(&title);

        let header_subtitle = Label::new(Some("0 unread"));
        header_subtitle.add_css_class("nc-subtitle");
        header_subtitle.set_halign(gtk4::Align::Start);
        title_box.append(&header_subtitle);

        header.append(&title_box);

        // Settings button
        let settings_btn = Button::new();
        settings_btn.add_css_class("nc-settings-btn");
        let settings_icon = sf_icon("gearshape.fill", 20);
        settings_icon.add_css_class("nc-settings-icon");
        settings_btn.set_child(Some(&settings_icon));
        let action_tx_clone = action_tx.clone();
        settings_btn.connect_clicked(move |_| {
            let _ = action_tx_clone.send(PanelAction::OpenSettings);
        });
        header.append(&settings_btn);

        container.append(&header);

        // ==================== SCROLL AREA ====================
        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
        // Set both min and max content height for proper expansion
        scroll.set_min_content_height(150);
        scroll.set_max_content_height(600); // Match Figma's max-h-[600px]
        scroll.add_css_class("nc-scroll");

        let list_box = ListBox::new();
        list_box.add_css_class("nc-list");
        list_box.set_selection_mode(gtk4::SelectionMode::None);
        scroll.set_child(Some(&list_box));

        container.append(&scroll);

        // ==================== EMPTY STATE ====================
        let empty_state = Box::new(Orientation::Vertical, 12);
        empty_state.add_css_class("nc-empty");
        empty_state.set_vexpand(true);
        empty_state.set_valign(gtk4::Align::Center);
        empty_state.set_margin_top(48);
        empty_state.set_margin_bottom(48);

        let empty_icon = sf_icon("bell.fill", 48);
        empty_icon.add_css_class("nc-empty-icon");
        empty_state.append(&empty_icon);

        let empty_label = Label::new(Some("No notifications"));
        empty_label.add_css_class("nc-empty-label");
        empty_state.append(&empty_label);

        // ==================== FOOTER ====================
        let footer = Box::new(Orientation::Horizontal, 0);
        footer.add_css_class("nc-footer");

        let clear_btn = Button::new();
        clear_btn.add_css_class("nc-clear-btn");
        clear_btn.set_hexpand(true);

        let clear_content = Box::new(Orientation::Horizontal, 8);
        clear_content.set_halign(gtk4::Align::Center);

        let trash_icon = sf_icon("trash.fill", 16);
        trash_icon.add_css_class("nc-trash-icon");
        clear_content.append(&trash_icon);

        let clear_label = Label::new(Some("Clear All"));
        clear_label.add_css_class("nc-clear-label");
        clear_content.append(&clear_label);

        clear_btn.set_child(Some(&clear_content));

        let action_tx_clone = action_tx.clone();
        clear_btn.connect_clicked(move |_| {
            let _ = action_tx_clone.send(PanelAction::ClearAll);
        });

        footer.append(&clear_btn);
        container.append(&footer);

        // ==================== ASSEMBLE ====================
        outer_wrapper.append(&container);
        window.set_child(Some(&outer_wrapper));

        // Escape to close
        if config.behavior.close_on_escape {
            let controller = gtk4::EventControllerKey::new();
            let action_tx_clone = action_tx.clone();
            controller.connect_key_pressed(move |_, key, _, _| {
                if key == gtk4::gdk::Key::Escape {
                    let _ = action_tx_clone.send(PanelAction::Close);
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
            window.add_controller(controller);
        }

        // Click outside to close
        if config.behavior.close_on_unfocus {
            let focus_controller = gtk4::EventControllerFocus::new();
            let action_tx_clone = action_tx.clone();
            focus_controller.connect_leave(move |_| {
                let _ = action_tx_clone.send(PanelAction::Close);
            });
            window.add_controller(focus_controller);
        }

        Self {
            window,
            list_box,
            header_subtitle,
            empty_state,
            footer,
            action_tx,
            config,
        }
    }

    /// Show the panel
    pub fn show(&self) {
        self.window.present();

        if self.config.animation.enabled {
            animate_panel_in(&self.window, &self.config);
        }
    }

    /// Hide the panel
    pub fn hide(&self) {
        if self.config.animation.enabled {
            let w = self.window.clone();
            animate_panel_out(&self.window, &self.config, move || {
                w.set_visible(false);
            });
        } else {
            self.window.set_visible(false);
        }
    }

    /// Toggle visibility
    pub fn toggle(&self) {
        if self.window.is_visible() {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    /// Update DND switch state (no switch in this design, kept for compatibility)
    pub fn set_dnd(&self, _enabled: bool) {
        // No DND switch in Figma design
    }

    /// Update the panel with current notifications
    pub fn update(&self, store: &NotificationStore) {
        // Clear existing
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let notifications = store.all();
        let count = notifications.len();

        // Update subtitle
        let subtitle_text = if count == 1 {
            "1 unread".to_string()
        } else {
            format!("{} unread", count)
        };
        self.header_subtitle.set_text(&subtitle_text);

        if count == 0 {
            // Show empty state, hide footer
            self.list_box.append(&self.empty_state);
            self.footer.set_visible(false);
        } else {
            // Add notification rows, show footer
            for notif in notifications {
                let row = self.build_notification_row(notif);
                self.list_box.append(&row);
            }
            self.footer.set_visible(true);
        }
    }

    /// Build a notification row matching Figma design
    fn build_notification_row(&self, notification: &Notification) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.add_css_class("nc-row");
        row.set_activatable(false);
        row.set_selectable(false);

        // Determine notification type for icon/color
        let notif_type = get_notification_type(notification);

        // Main card container
        let card = Box::new(Orientation::Horizontal, 12);
        card.add_css_class("nc-card");
        card.add_css_class(&format!("nc-card-{}", notif_type));

        // Icon wrapper with colored background
        let icon_wrapper = Box::new(Orientation::Vertical, 0);
        icon_wrapper.add_css_class("nc-icon-wrapper");
        icon_wrapper.add_css_class(&format!("nc-icon-wrapper-{}", notif_type));
        icon_wrapper.set_valign(gtk4::Align::Start);

        let type_icon = sf_icon(get_type_icon(&notif_type), 20);
        type_icon.add_css_class("nc-type-icon");
        type_icon.add_css_class(&format!("nc-type-icon-{}", notif_type));
        icon_wrapper.append(&type_icon);

        card.append(&icon_wrapper);

        // Content area
        let content = Box::new(Orientation::Vertical, 4);
        content.add_css_class("nc-card-content");
        content.set_hexpand(true);

        // Top row: title + time
        let top_row = Box::new(Orientation::Horizontal, 8);

        let title_col = Box::new(Orientation::Vertical, 2);
        title_col.set_hexpand(true);

        let title = Label::new(Some(&notification.summary));
        title.add_css_class("nc-card-title");
        title.set_halign(gtk4::Align::Start);
        title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        title.set_max_width_chars(30);
        title_col.append(&title);

        let app_label = Label::new(Some(&notification.app_name));
        app_label.add_css_class("nc-card-app");
        app_label.set_halign(gtk4::Align::Start);
        title_col.append(&app_label);

        top_row.append(&title_col);

        let time_label = Label::new(Some(&notification.time_ago()));
        time_label.add_css_class("nc-card-time");
        time_label.set_valign(gtk4::Align::Start);
        top_row.append(&time_label);

        content.append(&top_row);

        // Body text
        if !notification.body.is_empty() {
            let body = Label::new(Some(&strip_markup(&notification.body)));
            body.add_css_class("nc-card-body");
            body.set_halign(gtk4::Align::Start);
            body.set_wrap(true);
            body.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            body.set_max_width_chars(40);
            body.set_lines(2);
            body.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            content.append(&body);
        }

        // Progress bar (if present)
        if let Some(progress) = notification.progress {
            let progress_bar = ProgressBar::new();
            progress_bar.set_fraction(progress as f64 / 100.0);
            progress_bar.add_css_class("nc-card-progress");
            progress_bar.set_margin_top(8);
            content.append(&progress_bar);
        }

        // Actions (if present)
        let visible_actions: Vec<_> = notification
            .actions
            .iter()
            .filter(|(id, _)| id != "default")
            .collect();

        if !visible_actions.is_empty() {
            let actions_box = Box::new(Orientation::Horizontal, 8);
            actions_box.add_css_class("nc-card-actions");
            actions_box.set_margin_top(8);

            for (action_id, label) in visible_actions {
                let btn = Button::with_label(label);
                btn.add_css_class("nc-action-btn");
                let action_tx = self.action_tx.clone();
                let notif_id = notification.id;
                let action_key = action_id.clone();
                btn.connect_clicked(move |_| {
                    let _ =
                        action_tx.send(PanelAction::ActionInvoked(notif_id, action_key.clone()));
                });
                actions_box.append(&btn);
            }

            content.append(&actions_box);
        }

        card.append(&content);

        // Dismiss button (shows on hover via CSS)
        let dismiss_btn = Button::new();
        dismiss_btn.set_icon_name("window-close-symbolic");
        dismiss_btn.add_css_class("nc-dismiss-btn");
        dismiss_btn.set_valign(gtk4::Align::Start);
        let action_tx = self.action_tx.clone();
        let notif_id = notification.id;
        dismiss_btn.connect_clicked(move |_| {
            let _ = action_tx.send(PanelAction::DismissOne(notif_id));
        });
        card.append(&dismiss_btn);

        // Click on card to activate source app
        let gesture = gtk4::GestureClick::new();
        let action_tx = self.action_tx.clone();
        let notif_id = notification.id;
        let has_default = notification.actions.iter().any(|(a, _)| a == "default");
        let desktop_entry = notification.desktop_entry.clone();
        let app_name = notification.app_name.clone();
        gesture.connect_released(move |gesture, _, _, _| {
            if has_default {
                let _ = action_tx.send(PanelAction::ActionInvoked(notif_id, "default".to_string()));
            } else {
                // Fallback: try to launch by desktop entry or app name
                let entry = desktop_entry.as_deref().unwrap_or(&app_name);
                let desktop_id = if entry.ends_with(".desktop") {
                    entry.to_string()
                } else {
                    format!("{}.desktop", entry.to_lowercase())
                };
                if let Some(app_info) = gtk4::gio::DesktopAppInfo::new(&desktop_id) {
                    let _ = app_info.launch(&[], gtk4::gio::AppLaunchContext::NONE);
                }
            }
        });
        content.add_controller(gesture);

        row.set_child(Some(&card));
        row
    }
}

/// Determine notification type based on app name and content
fn get_notification_type(notification: &Notification) -> String {
    let app_lower = notification.app_name.to_lowercase();
    let summary_lower = notification.summary.to_lowercase();

    // Check urgency first
    if matches!(notification.urgency, crate::notification::Urgency::Critical) {
        return "error".to_string();
    }

    // Success indicators
    if app_lower.contains("vscode") || app_lower.contains("code") || app_lower.contains("build") {
        if summary_lower.contains("success")
            || summary_lower.contains("completed")
            || summary_lower.contains("done")
        {
            return "success".to_string();
        }
    }

    // Warning indicators
    if app_lower.contains("battery") || app_lower.contains("power") {
        if summary_lower.contains("low") || summary_lower.contains("warning") {
            return "warning".to_string();
        }
    }

    // Error indicators
    if summary_lower.contains("error")
        || summary_lower.contains("failed")
        || summary_lower.contains("failure")
    {
        return "error".to_string();
    }
    if app_lower.contains("network") && summary_lower.contains("disconnect") {
        return "error".to_string();
    }

    // Default to info
    "info".to_string()
}

/// Get SF Symbol icon name for notification type
fn get_type_icon(notif_type: &str) -> &'static str {
    match notif_type {
        "success" => "checkmark.circle.fill",
        "warning" => "exclamationmark.triangle.fill",
        "error" => "xmark.circle.fill",
        _ => "info.circle.fill",
    }
}

fn apply_position(window: &gtk4::Window, config: &Config) {
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

fn animate_panel_in(window: &gtk4::Window, config: &Config) {
    let steps = 20u32;
    let step_duration = config.animation.duration / steps as u64;
    let step_count = Rc::new(std::cell::Cell::new(0u32));
    let start_margin = -200;
    let final_margin = config.position.margin_top;

    window.set_margin(Edge::Top, start_margin);
    window.set_opacity(0.0);

    let w = window.clone();
    glib::timeout_add_local(Duration::from_millis(step_duration), move || {
        let current = step_count.get();
        let progress = (current + 1) as f64 / steps as f64;
        let eased = 1.0 - (1.0 - progress).powi(3);

        let margin = start_margin + ((final_margin - start_margin) as f64 * eased) as i32;
        w.set_margin(Edge::Top, margin);
        w.set_opacity(eased);

        step_count.set(current + 1);

        if current + 1 >= steps {
            w.set_margin(Edge::Top, final_margin);
            w.set_opacity(1.0);
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

fn animate_panel_out<F>(window: &gtk4::Window, config: &Config, on_complete: F)
where
    F: Fn() + 'static,
{
    let steps = 15u32;
    let step_duration = config.animation.duration / steps as u64;
    let step_count = Rc::new(std::cell::Cell::new(0u32));
    let start_margin = window.margin(Edge::Top);
    let end_margin = -200;
    let on_complete = Rc::new(on_complete);

    let w = window.clone();
    glib::timeout_add_local(Duration::from_millis(step_duration), move || {
        let current = step_count.get();
        let progress = (current + 1) as f64 / steps as f64;
        let eased = progress.powi(3);

        let margin = start_margin + ((end_margin - start_margin) as f64 * eased) as i32;
        w.set_margin(Edge::Top, margin);
        w.set_opacity(1.0 - eased);

        step_count.set(current + 1);

        if current + 1 >= steps {
            on_complete();
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

/// Strip basic HTML/pango markup
fn strip_markup(text: &str) -> String {
    let mut result = text.to_string();
    for tag in &["<b>", "</b>", "<i>", "</i>", "<u>", "</u>", "<br>", "<br/>"] {
        result = result.replace(tag, "");
    }
    result = result
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"");
    result
}
