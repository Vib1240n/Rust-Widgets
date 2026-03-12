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
    Left,
    Right,
}

fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

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

    let (edge, start_margin) = match direction {
        Direction::Down => (Edge::Top, -400),
        Direction::Up => (Edge::Bottom, -400),
        Direction::Left => (Edge::Left, -400),
        Direction::Right => (Edge::Right, -400),
    };

    window.set_margin(edge, start_margin);
    window.set_opacity(0.0);

    let window_clone = window.clone();
    let step_count_clone = step_count.clone();

    glib::timeout_add_local(Duration::from_millis(step_duration), move || {
        let current = step_count_clone.get();
        let progress = (current + 1) as f64 / ANIMATION_STEPS as f64;
        let eased = ease_out_cubic(progress);

        let margin = start_margin + ((final_margin - start_margin) as f64 * eased) as i32;
        window_clone.set_margin(edge, margin);
        window_clone.set_opacity(eased);

        step_count_clone.set(current + 1);

        if current + 1 >= ANIMATION_STEPS {
            window_clone.set_margin(edge, final_margin);
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

    let edge = match direction {
        Direction::Down | Direction::Up => Edge::Top,
        Direction::Left | Direction::Right => Edge::Left,
    };

    let start_margin = window.margin(edge);
    let end_margin = match direction {
        Direction::Up | Direction::Left => -400,
        Direction::Down | Direction::Right => 400,
    };

    let window_clone = window.clone();
    let step_count_clone = step_count.clone();
    let on_complete = Rc::new(on_complete);

    glib::timeout_add_local(Duration::from_millis(step_duration), move || {
        let current = step_count_clone.get();
        let progress = (current + 1) as f64 / ANIMATION_STEPS as f64;
        let eased = ease_in_cubic(progress);

        let margin = start_margin + ((end_margin - start_margin) as f64 * eased) as i32;
        window_clone.set_margin(edge, margin);
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
