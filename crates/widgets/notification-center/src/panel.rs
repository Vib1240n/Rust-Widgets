use crate::config::Config;
use crate::notification::{Notification, NotificationStore};
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Box, Button, Image, Label, ListBox, ListBoxRow, Orientation, PolicyType,
    ProgressBar, ScrolledWindow, Switch,
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
}

/// The notification center panel
pub struct NotificationPanel {
    window: gtk4::Window,
    list_box: ListBox,
    header_count: Label,
    empty_state: Box,
    dnd_switch: Switch,
    action_tx: mpsc::UnboundedSender<PanelAction>,
    config: Arc<Config>,
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

        // Main container
        let container = Box::new(Orientation::Vertical, 0);
        container.add_css_class("widget-container");
        container.set_width_request(config.appearance.panel_width);

        // Header
        let header = Box::new(Orientation::Horizontal, 12);
        header.add_css_class("nc-header");

        let title = Label::new(Some("Notifications"));
        title.add_css_class("nc-title");
        title.set_hexpand(true);
        title.set_halign(gtk4::Align::Start);
        header.append(&title);

        let header_count = Label::new(Some("0"));
        header_count.add_css_class("nc-count");
        header.append(&header_count);

        container.append(&header);

        // DND row
        let dnd_row = Box::new(Orientation::Horizontal, 8);
        dnd_row.add_css_class("nc-dnd-row");

        let dnd_icon = Image::from_icon_name("notifications-disabled-symbolic");
        dnd_icon.add_css_class("nc-dnd-icon");
        dnd_row.append(&dnd_icon);

        let dnd_label = Label::new(Some("Do Not Disturb"));
        dnd_label.add_css_class("nc-dnd-label");
        dnd_label.set_hexpand(true);
        dnd_label.set_halign(gtk4::Align::Start);
        dnd_row.append(&dnd_label);

        let dnd_switch = Switch::new();
        dnd_switch.set_active(config.behavior.dnd_enabled);
        dnd_switch.add_css_class("nc-dnd-switch");
        let action_tx_clone = action_tx.clone();
        dnd_switch.connect_state_set(move |_, state| {
            let _ = action_tx_clone.send(PanelAction::ToggleDnd(state));
            glib::Propagation::Proceed
        });
        dnd_row.append(&dnd_switch);

        container.append(&dnd_row);

        // Scrolled list
        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
        scroll.set_max_content_height(config.appearance.max_height - 120);
        scroll.add_css_class("nc-scroll");

        let list_box = ListBox::new();
        list_box.add_css_class("nc-list");
        list_box.set_selection_mode(gtk4::SelectionMode::None);
        scroll.set_child(Some(&list_box));

        container.append(&scroll);

        // Empty state (shown when no notifications)
        let empty_state = Box::new(Orientation::Vertical, 12);
        empty_state.add_css_class("nc-empty");
        empty_state.set_vexpand(true);
        empty_state.set_valign(gtk4::Align::Center);

        let empty_icon = Image::from_icon_name("notifications-symbolic");
        empty_icon.set_pixel_size(48);
        empty_icon.add_css_class("nc-empty-icon");
        empty_state.append(&empty_icon);

        let empty_label = Label::new(Some("No notifications"));
        empty_label.add_css_class("nc-empty-label");
        empty_state.append(&empty_label);

        // Footer with clear button
        let footer = Box::new(Orientation::Horizontal, 8);
        footer.add_css_class("nc-footer");
        footer.set_halign(gtk4::Align::End);

        let clear_btn = Button::with_label("Clear All");
        clear_btn.add_css_class("nc-clear-btn");
        let action_tx_clone = action_tx.clone();
        clear_btn.connect_clicked(move |_| {
            let _ = action_tx_clone.send(PanelAction::ClearAll);
        });
        footer.append(&clear_btn);

        container.append(&footer);

        window.set_child(Some(&container));

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
            header_count,
            empty_state,
            dnd_switch,
            action_tx,
            config,
        }
    }

    /// Show the panel
    pub fn show(&self) {
        self.window.present();

        // Animate in if enabled
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

    /// Update the panel with current notifications
    pub fn update(&self, store: &NotificationStore) {
        // Clear existing
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let notifications = store.all();
        let count = notifications.len();

        // Update count
        self.header_count.set_text(&count.to_string());

        if count == 0 {
            // Show empty state
            self.list_box.append(&self.empty_state);
        } else {
            // Add notification rows
            for notif in notifications {
                let row = self.build_notification_row(notif);
                self.list_box.append(&row);
            }
        }
    }

    /// Update DND switch state
    pub fn set_dnd(&self, enabled: bool) {
        self.dnd_switch.set_active(enabled);
    }

    fn build_notification_row(&self, notification: &Notification) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.add_css_class("nc-notification-row");

        if notification.dismissed {
            row.add_css_class("dismissed");
        }

        // Add urgency class
        match notification.urgency {
            crate::notification::Urgency::Low => row.add_css_class("urgency-low"),
            crate::notification::Urgency::Critical => row.add_css_class("urgency-critical"),
            _ => {}
        }

        let container = Box::new(Orientation::Horizontal, 12);
        container.add_css_class("nc-notification");

        // App icon
        let icon_box = Box::new(Orientation::Vertical, 0);
        icon_box.set_valign(gtk4::Align::Start);

        if !notification.app_icon.is_empty() {
            let icon = if notification.app_icon.starts_with('/') {
                Image::from_file(&notification.app_icon)
            } else {
                Image::from_icon_name(&notification.app_icon)
            };
            icon.set_pixel_size(32);
            icon.add_css_class("nc-app-icon");
            icon_box.append(&icon);
        }

        container.append(&icon_box);

        // Content
        let content = Box::new(Orientation::Vertical, 4);
        content.set_hexpand(true);

        // Header: app name + time
        let header = Box::new(Orientation::Horizontal, 8);

        let app_name = Label::new(Some(&notification.app_name));
        app_name.add_css_class("nc-app-name");
        app_name.set_halign(gtk4::Align::Start);
        header.append(&app_name);

        let spacer = Box::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        header.append(&spacer);

        let time_label = Label::new(Some(&notification.time_ago()));
        time_label.add_css_class("nc-time");
        header.append(&time_label);

        content.append(&header);

        // Summary
        let summary = Label::new(Some(&notification.summary));
        summary.add_css_class("nc-summary");
        summary.set_halign(gtk4::Align::Start);
        summary.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        summary.set_max_width_chars(40);
        content.append(&summary);

        // Body
        if !notification.body.is_empty() {
            let body = Label::new(Some(&strip_markup(&notification.body)));
            body.add_css_class("nc-body");
            body.set_halign(gtk4::Align::Start);
            body.set_wrap(true);
            body.set_max_width_chars(45);
            body.set_lines(2);
            body.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            content.append(&body);
        }

        // Progress
        if let Some(progress) = notification.progress {
            let progress_bar = ProgressBar::new();
            progress_bar.set_fraction(progress as f64 / 100.0);
            progress_bar.add_css_class("nc-progress");
            content.append(&progress_bar);
        }

        // Actions
        let visible_actions: Vec<_> = notification
            .actions
            .iter()
            .filter(|(id, _)| id != "default")
            .collect();

        if !visible_actions.is_empty() {
            let actions_box = Box::new(Orientation::Horizontal, 8);
            actions_box.add_css_class("nc-actions");

            for (action_id, label) in visible_actions {
                let btn = Button::with_label(label);
                btn.add_css_class("nc-action-btn");
                let action_tx = self.action_tx.clone();
                let notif_id = notification.id;
                let action_key = action_id.clone();
                btn.connect_clicked(move |_| {
                    let _ = action_tx.send(PanelAction::ActionInvoked(
                        notif_id,
                        action_key.clone(),
                    ));
                });
                actions_box.append(&btn);
            }

            content.append(&actions_box);
        }

        container.append(&content);

        // Dismiss button
        let dismiss_btn = Button::new();
        dismiss_btn.set_icon_name("window-close-symbolic");
        dismiss_btn.add_css_class("nc-dismiss-btn");
        dismiss_btn.set_valign(gtk4::Align::Start);
        let action_tx = self.action_tx.clone();
        let notif_id = notification.id;
        dismiss_btn.connect_clicked(move |_| {
            let _ = action_tx.send(PanelAction::DismissOne(notif_id));
        });
        container.append(&dismiss_btn);

        row.set_child(Some(&container));
        row
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
