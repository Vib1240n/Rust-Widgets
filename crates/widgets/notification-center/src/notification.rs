use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

/// Notification urgency level per freedesktop spec
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Urgency {
    Low = 0,
    Normal = 1,
    Critical = 2,
}

impl Default for Urgency {
    fn default() -> Self {
        Urgency::Normal
    }
}

impl From<u8> for Urgency {
    fn from(v: u8) -> Self {
        match v {
            0 => Urgency::Low,
            2 => Urgency::Critical,
            _ => Urgency::Normal,
        }
    }
}

/// A stored notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<(String, String)>, // (action_id, label)
    pub urgency: Urgency,
    pub timestamp: DateTime<Local>,
    /// Progress value if hint provided (0-100)
    pub progress: Option<i32>,
    /// Whether notification has been read/dismissed
    pub dismissed: bool,
    /// Image path from hints
    pub image_path: Option<String>,
    /// Raw image data (icon_data hint) - base64 encoded for serialization
    pub image_data: Option<ImageData>,
    /// Category hint
    pub category: Option<String>,
    /// Desktop entry hint
    pub desktop_entry: Option<String>,
    /// Whether this notification should be transient (no history)
    pub transient: bool,
    /// Resident - don't remove when action invoked
    pub resident: bool,
    /// Custom timeout override from notify call
    pub expire_timeout: Option<i32>,
}

/// Image data from notification hints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    pub width: i32,
    pub height: i32,
    pub rowstride: i32,
    pub has_alpha: bool,
    pub bits_per_sample: i32,
    pub channels: i32,
    pub data: Vec<u8>,
}

impl Notification {
    pub fn new(id: u32, app_name: String, app_icon: String, summary: String, body: String) -> Self {
        Self {
            id,
            app_name,
            app_icon,
            summary,
            body,
            actions: Vec::new(),
            urgency: Urgency::Normal,
            timestamp: Local::now(),
            progress: None,
            dismissed: false,
            image_path: None,
            image_data: None,
            category: None,
            desktop_entry: None,
            transient: false,
            resident: false,
            expire_timeout: None,
        }
    }

    /// Format timestamp for display
    pub fn time_ago(&self) -> String {
        let now = Local::now();
        let diff = now.signed_duration_since(self.timestamp);

        if diff.num_seconds() < 60 {
            "Just now".to_string()
        } else if diff.num_minutes() < 60 {
            let m = diff.num_minutes();
            if m == 1 {
                "1 min ago".to_string()
            } else {
                format!("{} mins ago", m)
            }
        } else if diff.num_hours() < 24 {
            let h = diff.num_hours();
            if h == 1 {
                "1 hour ago".to_string()
            } else {
                format!("{} hours ago", h)
            }
        } else {
            self.timestamp.format("%b %d, %H:%M").to_string()
        }
    }

    /// Get effective timeout in ms
    pub fn get_timeout(&self, config: &crate::config::PopupConfig) -> u64 {
        // If notification specified a timeout, use it (unless 0 = server decides)
        if let Some(timeout) = self.expire_timeout {
            if timeout > 0 {
                return timeout as u64;
            }
            // timeout == 0 means server decides, timeout == -1 means never expire
            if timeout == -1 {
                return 0; // Never auto-dismiss
            }
        }

        // Use config based on urgency
        match self.urgency {
            Urgency::Low => config.timeout_low,
            Urgency::Normal => config.timeout_normal,
            Urgency::Critical => config.timeout_critical,
        }
    }
}

/// Notification storage and ID management
pub struct NotificationStore {
    /// All notifications, keyed by ID
    notifications: HashMap<u32, Notification>,
    /// Next notification ID
    next_id: AtomicU32,
    /// Max notifications to keep
    max_history: usize,
}

impl NotificationStore {
    pub fn new(max_history: usize) -> Self {
        Self {
            notifications: HashMap::new(),
            next_id: AtomicU32::new(1),
            max_history,
        }
    }

    /// Generate next notification ID
    pub fn next_id(&self) -> u32 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Add or replace a notification
    pub fn insert(&mut self, notification: Notification) {
        // Don't store transient notifications
        if notification.transient {
            return;
        }

        self.notifications.insert(notification.id, notification);

        // Prune old notifications if over limit
        self.prune();
    }

    /// Remove a notification by ID
    pub fn remove(&mut self, id: u32) -> Option<Notification> {
        self.notifications.remove(&id)
    }

    /// Get a notification by ID
    pub fn get(&self, id: u32) -> Option<&Notification> {
        self.notifications.get(&id)
    }

    /// Get mutable notification by ID
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Notification> {
        self.notifications.get_mut(&id)
    }

    /// Get all notifications sorted by timestamp (newest first)
    pub fn all(&self) -> Vec<&Notification> {
        let mut notifs: Vec<_> = self.notifications.values().collect();
        notifs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        notifs
    }

    /// Get unread count
    pub fn unread_count(&self) -> usize {
        self.notifications.values().filter(|n| !n.dismissed).count()
    }

    /// Mark all as read
    pub fn mark_all_read(&mut self) {
        for n in self.notifications.values_mut() {
            n.dismissed = true;
        }
    }

    /// Clear all notifications
    pub fn clear_all(&mut self) {
        self.notifications.clear();
    }

    /// Clear dismissed notifications only
    pub fn clear_dismissed(&mut self) {
        self.notifications.retain(|_, n| !n.dismissed);
    }

    /// Prune old notifications to stay under max_history
    fn prune(&mut self) {
        if self.notifications.len() <= self.max_history {
            return;
        }

        // Get IDs sorted by timestamp (oldest first)
        let mut entries: Vec<_> = self
            .notifications
            .iter()
            .map(|(id, n)| (*id, n.timestamp))
            .collect();
        entries.sort_by(|a, b| a.1.cmp(&b.1));

        // Collect IDs to remove
        let to_remove = entries.len() - self.max_history;
        let ids_to_remove: Vec<u32> = entries
            .into_iter()
            .take(to_remove)
            .map(|(id, _)| id)
            .collect();

        // Remove them
        for id in ids_to_remove {
            self.notifications.remove(&id);
        }
    }
}
