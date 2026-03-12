use gtk4::glib;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, LayerShell};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

const ANIMATION_STEPS: u32 = 30;

#[derive(Clone, Copy)]
pub enum Direction {
    Down,
    Up,
}

/// Ease-out cubic: fast start, slow end
fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

/// Ease-in cubic: slow start, fast end
fn ease_in_cubic(t: f64) -> f64 {
    t.powi(3)
}

pub fn slide_in(
    window: &gtk4::ApplicationWindow,
    direction: Direction,
    duration_ms: u64,
    final_margin: i32,
) {
    let step_duration = duration_ms / ANIMATION_STEPS as u64;
    let step_count = Rc::new(Cell::new(0u32));

    // Start position (off-screen)
    let start_margin = match direction {
        Direction::Down => -300,
        Direction::Up => 300,
    };

    window.set_margin(Edge::Top, start_margin);
    window.set_opacity(0.0);

    let window_clone = window.clone();
    let step_count_clone = step_count.clone();

    glib::timeout_add_local(Duration::from_millis(step_duration), move || {
        let current = step_count_clone.get();
        let progress = (current + 1) as f64 / ANIMATION_STEPS as f64;
        let eased = ease_out_cubic(progress);

        let margin = start_margin + ((final_margin - start_margin) as f64 * eased) as i32;
        window_clone.set_margin(Edge::Top, margin);
        window_clone.set_opacity(eased);

        step_count_clone.set(current + 1);

        if current + 1 >= ANIMATION_STEPS {
            // Ensure final values are exact
            window_clone.set_margin(Edge::Top, final_margin);
            window_clone.set_opacity(1.0);
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

pub fn slide_out<F>(
    window: &gtk4::ApplicationWindow,
    direction: Direction,
    duration_ms: u64,
    on_complete: F,
) where
    F: Fn() + 'static,
{
    let step_duration = duration_ms / ANIMATION_STEPS as u64;
    let step_count = Rc::new(Cell::new(0u32));

    let start_margin = window.margin(Edge::Top);
    let end_margin = match direction {
        Direction::Up => -300,
        Direction::Down => 300,
    };

    let window_clone = window.clone();
    let step_count_clone = step_count.clone();
    let on_complete = Rc::new(on_complete);

    glib::timeout_add_local(Duration::from_millis(step_duration), move || {
        let current = step_count_clone.get();
        let progress = (current + 1) as f64 / ANIMATION_STEPS as f64;
        let eased = ease_in_cubic(progress);

        let margin = start_margin + ((end_margin - start_margin) as f64 * eased) as i32;
        window_clone.set_margin(Edge::Top, margin);
        window_clone.set_opacity(1.0 - eased);

        step_count_clone.set(current + 1);

        if current + 1 >= ANIMATION_STEPS {
            on_complete();
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}
