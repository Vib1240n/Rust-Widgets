use crate::notification::{ImageData, Notification, Urgency};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedValue, Value};
use zbus::{interface, Connection};

/// Events sent from DBus to the UI
#[derive(Debug, Clone)]
pub enum NotificationEvent {
    /// New notification received
    Notify(Notification),
    /// Close notification request
    Close(u32),
    /// Get capabilities query
    GetCapabilities,
}

/// Events sent from UI back to DBus for signals
#[derive(Debug, Clone)]
pub enum NotificationResponse {
    /// Notification was closed
    Closed { id: u32, reason: u32 },
    /// Action was invoked
    ActionInvoked { id: u32, action_key: String },
}

/// The DBus interface implementation
pub struct NotificationServer {
    event_tx: mpsc::UnboundedSender<NotificationEvent>,
    next_id: std::sync::atomic::AtomicU32,
}

impl NotificationServer {
    pub fn new(event_tx: mpsc::UnboundedSender<NotificationEvent>) -> Self {
        Self {
            event_tx,
            next_id: std::sync::atomic::AtomicU32::new(1),
        }
    }

    fn next_id(&self) -> u32 {
        self.next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    /// Get server capabilities
    fn get_capabilities(&self) -> Vec<String> {
        vec![
            "actions".to_string(),
            "body".to_string(),
            "body-hyperlinks".to_string(),
            "body-markup".to_string(),
            "icon-static".to_string(),
            "persistence".to_string(),
            "action-icons".to_string(),
        ]
    }

    /// Send a notification
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        // Use replaces_id if > 0, otherwise generate new
        let id = if replaces_id > 0 {
            replaces_id
        } else {
            self.next_id()
        };

        let mut notification = Notification::new(id, app_name, app_icon, summary, body);

        // Parse actions (pairs of id, label)
        let mut action_pairs = Vec::new();
        let mut iter = actions.into_iter();
        while let (Some(action_id), Some(label)) = (iter.next(), iter.next()) {
            action_pairs.push((action_id, label));
        }
        notification.actions = action_pairs;

        // Parse hints
        for (key, value) in hints {
            match key.as_str() {
                "urgency" => {
                    if let Ok(u) = <u8>::try_from(&*value) {
                        notification.urgency = Urgency::from(u);
                    }
                }
                "category" => {
                    if let Ok(s) = <String>::try_from(&*value) {
                        notification.category = Some(s);
                    }
                }
                "desktop-entry" => {
                    if let Ok(s) = <String>::try_from(&*value) {
                        notification.desktop_entry = Some(s);
                    }
                }
                "transient" => {
                    if let Ok(b) = <bool>::try_from(&*value) {
                        notification.transient = b;
                    }
                }
                "resident" => {
                    if let Ok(b) = <bool>::try_from(&*value) {
                        notification.resident = b;
                    }
                }
                "value" | "x-canonical-private-synchronous" => {
                    // Progress value
                    if let Ok(v) = <i32>::try_from(&*value) {
                        notification.progress = Some(v.clamp(0, 100));
                    }
                }
                "image-path" | "image_path" => {
                    if let Ok(s) = <String>::try_from(&*value) {
                        notification.image_path = Some(s);
                    }
                }
                "icon_data" | "image-data" | "image_data" => {
                    // Parse image data structure (iiibiiay)
                    if let Some(img) = parse_image_data(&value) {
                        notification.image_data = Some(img);
                    }
                }
                _ => {}
            }
        }

        // Set timeout
        if expire_timeout != 0 {
            notification.expire_timeout = Some(expire_timeout);
        }

        // Send to UI
        let _ = self.event_tx.send(NotificationEvent::Notify(notification));

        id
    }

    /// Close a notification
    fn close_notification(&self, id: u32) {
        let _ = self.event_tx.send(NotificationEvent::Close(id));
    }

    /// Get server information
    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "rust-widgets".to_string(),
            "vib1240n".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
            "1.2".to_string(), // Spec version
        )
    }

    /// Signal: Notification closed
    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    /// Signal: Action invoked
    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;
}

/// Parse image data from DBus variant
fn parse_image_data(value: &OwnedValue) -> Option<ImageData> {
    // Image data is a struct: (iiibiiay)
    // width, height, rowstride, has_alpha, bits_per_sample, channels, data
    let v: &Value = value.downcast_ref().ok()?;

    if let Value::Structure(s) = v {
        let fields = s.fields();
        if fields.len() >= 7 {
            let width = i32::try_from(&fields[0]).ok()?;
            let height = i32::try_from(&fields[1]).ok()?;
            let rowstride = i32::try_from(&fields[2]).ok()?;
            let has_alpha = bool::try_from(&fields[3]).ok()?;
            let bits_per_sample = i32::try_from(&fields[4]).ok()?;
            let channels = i32::try_from(&fields[5]).ok()?;

            // Get the byte array
            let data = if let Value::Array(arr) = &fields[6] {
                arr.iter()
                    .filter_map(|v| u8::try_from(v).ok())
                    .collect::<Vec<u8>>()
            } else {
                return None;
            };

            return Some(ImageData {
                width,
                height,
                rowstride,
                has_alpha,
                bits_per_sample,
                channels,
                data,
            });
        }
    }

    None
}

/// Close reasons per spec
pub mod close_reason {
    pub const EXPIRED: u32 = 1;
    pub const DISMISSED: u32 = 2;
    pub const CLOSED_BY_CALL: u32 = 3;
    pub const UNDEFINED: u32 = 4;
}

/// Start the DBus server
pub async fn start_server(
    event_tx: mpsc::UnboundedSender<NotificationEvent>,
) -> zbus::Result<(Connection, Arc<NotificationServer>)> {
    let server = Arc::new(NotificationServer::new(event_tx));

    let conn = Connection::session().await?;

    // Request the well-known name
    conn.request_name("org.freedesktop.Notifications").await?;

    // Register the interface
    let server_clone = server.clone();
    conn.object_server()
        .at("/org/freedesktop/Notifications", NotificationServer::new(server.event_tx.clone()))
        .await?;

    tracing::info!("DBus notification server started");

    Ok((conn, server))
}

/// Send closed signal
pub async fn emit_closed(conn: &Connection, id: u32, reason: u32) -> zbus::Result<()> {
    let iface_ref = conn
        .object_server()
        .interface::<_, NotificationServer>("/org/freedesktop/Notifications")
        .await?;

    NotificationServer::notification_closed(iface_ref.signal_emitter(), id, reason).await
}

/// Send action invoked signal
pub async fn emit_action_invoked(conn: &Connection, id: u32, action_key: &str) -> zbus::Result<()> {
    let iface_ref = conn
        .object_server()
        .interface::<_, NotificationServer>("/org/freedesktop/Notifications")
        .await?;

    NotificationServer::action_invoked(iface_ref.signal_emitter(), id, action_key).await
}
