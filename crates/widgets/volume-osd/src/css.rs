//! Styling: theme (palette + glass design system) + rw component CSS +
//! user overrides, applied live. See widget_core::theme.

const COMPONENT_CSS: &str = include_str!("../../style.css");

pub fn load() {
    widget_core::theme::load_and_watch(COMPONENT_CSS);
}
