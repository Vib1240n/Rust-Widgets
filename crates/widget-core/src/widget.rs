use crate::Position;
use gtk4::ApplicationWindow;

/// Core trait that all widgets must implement
pub trait Widget {
    /// Unique identifier for this widget (e.g., "control-center", "volume-osd")
    fn name(&self) -> &'static str;

    /// Build and return the widget's window
    fn build(&self, app: &gtk4::Application) -> ApplicationWindow;

    /// Where to position the widget on screen
    fn position(&self) -> Position;

    /// Optional fixed size (width, height). None = auto-size
    fn size(&self) -> Option<(i32, i32)> {
        None
    }

    /// Margins from screen edge (top, right, bottom, left)
    fn margins(&self) -> (i32, i32, i32, i32) {
        (10, 10, 10, 10)
    }

    /// Layer shell namespace (for hyprland rules)
    fn namespace(&self) -> &'static str {
        "rust-widgets"
    }

    /// Called when widget becomes visible
    fn on_show(&self) {}

    /// Called when widget is hidden
    fn on_hide(&self) {}

    /// Auto-hide timeout in milliseconds. None = no auto-hide
    fn timeout(&self) -> Option<u64> {
        None
    }
}
