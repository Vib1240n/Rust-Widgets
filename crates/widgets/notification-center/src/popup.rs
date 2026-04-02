use crate::config::Config;
use crate::notification::Notification;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Box, Button, GestureClick, Image, Label, Orientation, ProgressBar, Widget,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Actions the popup can trigger
#[derive(Debug, Clone)]
pub enum PopupAction {
    Dismissed(u32),
    ActionInvoked(u32, String),
    Clicked(u32),
}

/// Manages popup notification windows
pub struct PopupManager {
    app: gtk4::Application,
    config: Arc<Config>,
    /// Active popup windows by notification ID
    popups: Rc<RefCell<HashMap<u32, PopupWindow>>>,
    /// Channel to send actions back to main
    action_tx: mpsc::UnboundedSender<PopupAction>,
    /// Current total height of popups (for stacking)
    stack_offset: Rc<Cell<i32>>,
}

struct PopupWindow {
    window: gtk4::Window,
    height: i32,
    timeout_source: Option<glib::SourceId>,
}

impl PopupManager {
    pub fn new(
        app: gtk4::Application,
        config: Arc<Config>,
        action_tx: mpsc::UnboundedSender<PopupAction>,
    ) -> Self {
        Self {
            app,
            config,
            popups: Rc::new(RefCell::new(HashMap::new())),
            action_tx,
            stack_offset: Rc::new(Cell::new(0)),
        }
    }

    /// Show a popup for a notification
    pub fn show(&self, notification: &Notification) {
        let id = notification.id;

        // Check if we're at max popups
        if self.popups.borrow().len() >= self.config.popup.max_visible {
            // Remove oldest popup
            if let Some(oldest_id) = self.get_oldest_popup_id() {
                self.dismiss(oldest_id);
            }
        }

        // Calculate position
        let margin_top = self.config.popup.margin + self.stack_offset.get();

        // Create window
        let window = gtk4::Window::builder()
            .application(&self.app)
            .decorated(false)
            .resizable(false)
            .build();

        // Layer shell setup
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_namespace("rust-widgets-popup");
        window.set_keyboard_mode(KeyboardMode::None);

        // Position
        window.set_anchor(Edge::Top, true);
        window.set_anchor(Edge::Right, true);
        window.set_margin(Edge::Top, margin_top);
        window.set_margin(Edge::Right, self.config.popup.margin);

        // Build content
        let content = self.build_popup_content(notification);
        window.set_child(Some(&content));

        // Click to open notification center or invoke default action
        let gesture = GestureClick::new();
        let action_tx = self.action_tx.clone();
        let notif_id = id;
        let has_default = notification
            .actions
            .iter()
            .any(|(a, _)| a == "default");
        gesture.connect_released(move |_, _, _, _| {
            if has_default {
                let _ = action_tx.send(PopupAction::ActionInvoked(
                    notif_id,
                    "default".to_string(),
                ));
            } else {
                let _ = action_tx.send(PopupAction::Clicked(notif_id));
            }
        });
        window.add_controller(gesture);

        window.present();

        // Get actual height after present
        let height = window.height().max(80);

        // Update stack offset
        self.stack_offset
            .set(self.stack_offset.get() + height + self.config.popup.gap);

        // Setup auto-dismiss timeout
        let timeout = notification.get_timeout(&self.config.popup);
        let timeout_source = if timeout > 0 {
            let popups = self.popups.clone();
            let stack_offset = self.stack_offset.clone();
            let action_tx = self.action_tx.clone();
            let gap = self.config.popup.gap;
            Some(glib::timeout_add_local_once(
                Duration::from_millis(timeout),
                move || {
                    if let Some(popup) = popups.borrow_mut().remove(&id) {
                        // Update stack offset
                        stack_offset.set(
                            (stack_offset.get() - popup.height - gap).max(0),
                        );
                        popup.window.close();
                        let _ = action_tx.send(PopupAction::Dismissed(id));
                    }
                },
            ))
        } else {
            None
        };

        // Animate in
        if self.config.animation.enabled {
            animate_slide_in(&window, self.config.animation.duration);
        }

        self.popups.borrow_mut().insert(
            id,
            PopupWindow {
                window,
                height,
                timeout_source,
            },
        );
    }

    /// Dismiss a popup
    pub fn dismiss(&self, id: u32) {
        if let Some(popup) = self.popups.borrow_mut().remove(&id) {
            // Cancel timeout if any
            if let Some(source) = popup.timeout_source {
                source.remove();
            }

            // Update stack offset
            self.stack_offset.set(
                (self.stack_offset.get() - popup.height - self.config.popup.gap).max(0),
            );

            // Animate out if enabled
            if self.config.animation.enabled {
                let w = popup.window.clone();
                animate_slide_out(&popup.window, self.config.animation.duration, move || {
                    w.close();
                });
            } else {
                popup.window.close();
            }
        }
    }

    /// Dismiss all popups
    pub fn dismiss_all(&self) {
        let ids: Vec<u32> = self.popups.borrow().keys().copied().collect();
        for id in ids {
            self.dismiss(id);
        }
    }

    /// Replace a popup (for replaces_id)
    pub fn replace(&self, notification: &Notification) {
        self.dismiss(notification.id);
        self.show(notification);
    }

    fn get_oldest_popup_id(&self) -> Option<u32> {
        self.popups.borrow().keys().copied().next()
    }

    fn build_popup_content(&self, notification: &Notification) -> Widget {
        let container = Box::new(Orientation::Vertical, 0);
        container.add_css_class("popup-container");
        container.set_width_request(self.config.appearance.popup_width);

        // Add urgency class
        match notification.urgency {
            crate::notification::Urgency::Low => container.add_css_class("urgency-low"),
            crate::notification::Urgency::Critical => container.add_css_class("urgency-critical"),
            _ => {}
        }

        // Header row: icon, app name, time, close button
        let header = Box::new(Orientation::Horizontal, 8);
        header.add_css_class("popup-header");

        // App icon
        if !notification.app_icon.is_empty() {
            let icon = if notification.app_icon.starts_with('/') {
                Image::from_file(&notification.app_icon)
            } else {
                Image::from_icon_name(&notification.app_icon)
            };
            icon.set_pixel_size(16);
            icon.add_css_class("popup-app-icon");
            header.append(&icon);
        }

        // App name
        let app_label = Label::new(Some(&notification.app_name));
        app_label.add_css_class("popup-app-name");
        app_label.set_hexpand(true);
        app_label.set_halign(gtk4::Align::Start);
        header.append(&app_label);

        // Close button
        let close_btn = Button::new();
        close_btn.set_icon_name("window-close-symbolic");
        close_btn.add_css_class("popup-close");
        let popups = self.popups.clone();
        let stack_offset = self.stack_offset.clone();
        let action_tx = self.action_tx.clone();
        let gap = self.config.popup.gap;
        let id = notification.id;
        close_btn.connect_clicked(move |_| {
            if let Some(popup) = popups.borrow_mut().remove(&id) {
                if let Some(source) = popup.timeout_source {
                    source.remove();
                }
                stack_offset.set((stack_offset.get() - popup.height - gap).max(0));
                popup.window.close();
                let _ = action_tx.send(PopupAction::Dismissed(id));
            }
        });
        header.append(&close_btn);

        container.append(&header);

        // Content row: optional image, summary, body
        let content = Box::new(Orientation::Horizontal, 12);
        content.add_css_class("popup-content");

        // Image if present
        if let Some(ref path) = notification.image_path {
            let img = Image::from_file(path);
            img.set_pixel_size(48);
            img.add_css_class("popup-image");
            content.append(&img);
        } else if let Some(ref img_data) = notification.image_data {
            if let Some(pixbuf) = image_data_to_pixbuf(img_data) {
                let img = Image::from_pixbuf(Some(&pixbuf));
                img.set_pixel_size(48);
                img.add_css_class("popup-image");
                content.append(&img);
            }
        }

        // Text content
        let text_box = Box::new(Orientation::Vertical, 4);
        text_box.set_hexpand(true);

        let summary = Label::new(Some(&notification.summary));
        summary.add_css_class("popup-summary");
        summary.set_halign(gtk4::Align::Start);
        summary.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        summary.set_max_width_chars(40);
        text_box.append(&summary);

        if !notification.body.is_empty() {
            let body = Label::new(Some(&strip_markup(&notification.body)));
            body.add_css_class("popup-body");
            body.set_halign(gtk4::Align::Start);
            body.set_wrap(true);
            body.set_max_width_chars(45);
            body.set_lines(3);
            body.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            text_box.append(&body);
        }

        content.append(&text_box);
        container.append(&content);

        // Progress bar if present
        if let Some(progress) = notification.progress {
            let progress_bar = ProgressBar::new();
            progress_bar.set_fraction(progress as f64 / 100.0);
            progress_bar.add_css_class("popup-progress");
            container.append(&progress_bar);
        }

        // Actions if present (and not just "default")
        let visible_actions: Vec<_> = notification
            .actions
            .iter()
            .filter(|(id, _)| id != "default")
            .collect();

        if !visible_actions.is_empty() {
            let actions_box = Box::new(Orientation::Horizontal, 8);
            actions_box.add_css_class("popup-actions");
            actions_box.set_halign(gtk4::Align::End);

            for (action_id, label) in visible_actions {
                let btn = Button::with_label(label);
                btn.add_css_class("popup-action-btn");
                let action_tx = self.action_tx.clone();
                let notif_id = notification.id;
                let action_key = action_id.clone();
                btn.connect_clicked(move |_| {
                    let _ = action_tx.send(PopupAction::ActionInvoked(
                        notif_id,
                        action_key.clone(),
                    ));
                });
                actions_box.append(&btn);
            }

            container.append(&actions_box);
        }

        container.upcast()
    }
}

/// Convert notification image data to GDK pixbuf
fn image_data_to_pixbuf(data: &crate::notification::ImageData) -> Option<gtk4::gdk_pixbuf::Pixbuf> {
    // Pixbuf::from_bytes can panic on invalid data, so we validate first
    if data.width <= 0 || data.height <= 0 || data.rowstride <= 0 || data.data.is_empty() {
        return None;
    }
    
    // Check if data size is reasonable
    let expected_size = (data.height * data.rowstride) as usize;
    if data.data.len() < expected_size {
        return None;
    }

    Some(gtk4::gdk_pixbuf::Pixbuf::from_bytes(
        &glib::Bytes::from(&data.data),
        gtk4::gdk_pixbuf::Colorspace::Rgb,
        data.has_alpha,
        data.bits_per_sample,
        data.width,
        data.height,
        data.rowstride,
    ))
}

/// Strip basic HTML/pango markup from notification body
fn strip_markup(text: &str) -> String {
    let mut result = text.to_string();
    
    // Remove common formatting tags
    for tag in &["<b>", "</b>", "<i>", "</i>", "<u>", "</u>", "<br>", "<br/>", "<br />"] {
        result = result.replace(tag, "");
    }
    
    // Simple <a> tag removal - find <a...>content</a> and keep content
    while let Some(start) = result.find("<a") {
        if let Some(tag_end) = result[start..].find('>') {
            let tag_end_abs = start + tag_end + 1;
            if let Some(close) = result[tag_end_abs..].find("</a>") {
                let close_abs = tag_end_abs + close;
                let content = &result[tag_end_abs..close_abs];
                result = format!("{}{}{}", &result[..start], content, &result[close_abs + 4..]);
            } else {
                break;
            }
        } else {
            break;
        }
    }
    
    // Remove any remaining tags
    while let Some(start) = result.find('<') {
        if let Some(end) = result[start..].find('>') {
            result = format!("{}{}", &result[..start], &result[start + end + 1..]);
        } else {
            break;
        }
    }
    
    // Decode basic entities
    result = result
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'");
    
    result
}

/// Slide-in animation
fn animate_slide_in(window: &gtk4::Window, duration_ms: u64) {
    let steps = 20u32;
    let step_duration = duration_ms / steps as u64;
    let step_count = Rc::new(Cell::new(0u32));
    let start_margin = -100;
    let final_margin = window.margin(Edge::Top);

    window.set_margin(Edge::Top, start_margin);
    window.set_opacity(0.0);

    let w = window.clone();
    glib::timeout_add_local(Duration::from_millis(step_duration), move || {
        let current = step_count.get();
        let progress = (current + 1) as f64 / steps as f64;
        let eased = 1.0 - (1.0 - progress).powi(3); // ease-out-cubic

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

/// Slide-out animation
fn animate_slide_out<F>(window: &gtk4::Window, duration_ms: u64, on_complete: F)
where
    F: Fn() + 'static,
{
    let steps = 15u32;
    let step_duration = duration_ms / steps as u64;
    let step_count = Rc::new(Cell::new(0u32));
    let start_margin = window.margin(Edge::Top);
    let end_margin = start_margin + 50;
    let on_complete = Rc::new(on_complete);

    let w = window.clone();
    glib::timeout_add_local(Duration::from_millis(step_duration), move || {
        let current = step_count.get();
        let progress = (current + 1) as f64 / steps as f64;
        let eased = progress.powi(3); // ease-in-cubic

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
