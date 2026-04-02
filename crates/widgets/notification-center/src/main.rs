mod config;
mod css;
mod dbus;
mod notification;
mod panel;
mod popup;

use config::Config;
use dbus::NotificationEvent;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::Application;
use notification::NotificationStore;
use panel::{NotificationPanel, PanelAction};
use popup::{PopupAction, PopupManager};
use std::cell::RefCell;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc;

const APP_ID: &str = "com.vib1240n.rust-widgets.notification-center";
const BINARY_NAME: &str = "rw-notifications";
const SOCKET_PATH: &str = "/tmp/rw-notifications.sock";

fn main() {
    // Check for commands via socket if daemon already running
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "toggle" | "show" | "hide" | "dnd" | "clear" => {
                send_command(&args[1]);
                return;
            }
            _ => {}
        }
    }

    // Enforce single instance
    if is_already_running() {
        eprintln!("{} is already running", BINARY_NAME);
        // If no command, try toggle
        send_command("toggle");
        return;
    }

    tracing_subscriber::fmt().with_env_filter("info").init();

    // Setup socket for IPC
    let _ = std::fs::remove_file(SOCKET_PATH);
    let listener = UnixListener::bind(SOCKET_PATH).expect("Failed to bind socket");
    listener
        .set_nonblocking(true)
        .expect("Failed to set non-blocking");

    tracing::info!("Started notification daemon, socket at {}", SOCKET_PATH);

    let app = Application::builder().application_id(APP_ID).build();

    // Pass listener to app
    let listener = Rc::new(RefCell::new(listener));
    let listener_clone = listener.clone();

    app.connect_activate(move |app| {
        build_ui(app, listener_clone.clone());
    });

    app.run_with_args::<&str>(&[]);

    // Cleanup socket
    let _ = std::fs::remove_file(SOCKET_PATH);
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

fn send_command(cmd: &str) {
    match UnixStream::connect(SOCKET_PATH) {
        Ok(mut stream) => {
            let _ = stream.write_all(cmd.as_bytes());
        }
        Err(_) => {
            eprintln!("Notification daemon not running");
        }
    }
}

fn build_ui(app: &Application, listener: Rc<RefCell<UnixListener>>) {
    let config = Arc::new(Config::load());
    css::load();

    // Channels for communication
    let (dbus_tx, mut dbus_rx) = mpsc::unbounded_channel::<NotificationEvent>();
    let (popup_action_tx, mut popup_action_rx) = mpsc::unbounded_channel::<PopupAction>();
    let (panel_action_tx, mut panel_action_rx) = mpsc::unbounded_channel::<PanelAction>();

    // Notification store
    let store = Rc::new(RefCell::new(NotificationStore::new(
        config.appearance.max_history,
    )));

    // DND state
    let dnd_enabled = Rc::new(RefCell::new(config.behavior.dnd_enabled));

    // Create popup manager
    let popup_manager = Rc::new(PopupManager::new(
        app.clone(),
        config.clone(),
        popup_action_tx,
    ));

    // Create panel (hidden initially)
    let panel = Rc::new(NotificationPanel::new(
        app,
        config.clone(),
        panel_action_tx,
    ));

    // Start DBus server in tokio runtime
    let dbus_tx_clone = dbus_tx.clone();
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            match dbus::start_server(dbus_tx_clone).await {
                Ok((conn, _server)) => {
                    tracing::info!("DBus server running");
                    // Keep connection alive
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to start DBus server: {}", e);
                }
            }
        });
    });

    // Process DBus events
    let store_clone = store.clone();
    let popup_manager_clone = popup_manager.clone();
    let panel_clone = panel.clone();
    let dnd_clone = dnd_enabled.clone();
    let config_clone = config.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        // Process DBus notifications
        while let Ok(event) = dbus_rx.try_recv() {
            match event {
                NotificationEvent::Notify(mut notif) => {
                    let id = notif.id;

                    // Store notification
                    store_clone.borrow_mut().insert(notif.clone());

                    // Show popup if not DND (unless critical)
                    let is_dnd = *dnd_clone.borrow();
                    let is_critical =
                        notif.urgency == notification::Urgency::Critical;

                    if !is_dnd || is_critical {
                        popup_manager_clone.show(&notif);
                    }

                    // Update panel if visible
                    if panel_clone.is_visible() {
                        panel_clone.update(&store_clone.borrow());
                    }
                }
                NotificationEvent::Close(id) => {
                    popup_manager_clone.dismiss(id);
                    store_clone.borrow_mut().remove(id);
                    if panel_clone.is_visible() {
                        panel_clone.update(&store_clone.borrow());
                    }
                }
                NotificationEvent::GetCapabilities => {
                    // Handled by DBus interface directly
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // Process popup actions
    let store_clone = store.clone();
    let panel_clone = panel.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        while let Ok(action) = popup_action_rx.try_recv() {
            match action {
                PopupAction::Dismissed(id) => {
                    // Mark as read in store
                    if let Some(notif) = store_clone.borrow_mut().get_mut(id) {
                        notif.dismissed = true;
                    }
                    // TODO: emit closed signal
                }
                PopupAction::ActionInvoked(id, action_key) => {
                    // TODO: emit action invoked signal
                    tracing::info!("Action invoked: {} -> {}", id, action_key);
                }
                PopupAction::Clicked(id) => {
                    // Open notification center and scroll to notification
                    panel_clone.show();
                    panel_clone.update(&store_clone.borrow());
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // Process panel actions
    let store_clone = store.clone();
    let panel_clone = panel.clone();
    let dnd_clone = dnd_enabled.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        while let Ok(action) = panel_action_rx.try_recv() {
            match action {
                PanelAction::Close => {
                    panel_clone.hide();
                }
                PanelAction::ClearAll => {
                    store_clone.borrow_mut().clear_all();
                    panel_clone.update(&store_clone.borrow());
                }
                PanelAction::DismissOne(id) => {
                    store_clone.borrow_mut().remove(id);
                    panel_clone.update(&store_clone.borrow());
                }
                PanelAction::ActionInvoked(id, action_key) => {
                    tracing::info!("Panel action: {} -> {}", id, action_key);
                    // TODO: emit DBus signal
                }
                PanelAction::ToggleDnd(enabled) => {
                    *dnd_clone.borrow_mut() = enabled;
                    tracing::info!("DND: {}", enabled);
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // Process IPC commands
    let panel_clone = panel.clone();
    let store_clone = store.clone();
    let popup_manager_clone = popup_manager.clone();
    let dnd_clone = dnd_enabled.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        // Check for incoming connections
        if let Ok((mut stream, _)) = listener.borrow().accept() {
            let mut buf = [0u8; 64];
            if let Ok(n) = stream.read(&mut buf) {
                let cmd = String::from_utf8_lossy(&buf[..n]);
                match cmd.trim() {
                    "toggle" => {
                        panel_clone.toggle();
                        if panel_clone.is_visible() {
                            panel_clone.update(&store_clone.borrow());
                        }
                    }
                    "show" => {
                        panel_clone.show();
                        panel_clone.update(&store_clone.borrow());
                    }
                    "hide" => {
                        panel_clone.hide();
                    }
                    "dnd" => {
                        let new_state = !*dnd_clone.borrow();
                        *dnd_clone.borrow_mut() = new_state;
                        panel_clone.set_dnd(new_state);
                        tracing::info!("DND toggled: {}", new_state);
                    }
                    "clear" => {
                        store_clone.borrow_mut().clear_all();
                        popup_manager_clone.dismiss_all();
                        if panel_clone.is_visible() {
                            panel_clone.update(&store_clone.borrow());
                        }
                    }
                    _ => {}
                }
            }
        }
        glib::ControlFlow::Continue
    });

    tracing::info!("Notification center ready");
}
