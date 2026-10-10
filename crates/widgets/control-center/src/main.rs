//! rw-control: control center (Liquid Glass Control Center design).
//!
//!   heading (date, clock)
//!   connectivity card    Wi-Fi / Bluetooth rows (switch + details)
//!   quick tiles          [tiles] items, two per row
//!   levels card          display + sound sliders, output row
//!   now playing card
//!   footer               Sleep · Lock · Power
//!
//! Sub-views slide in from the right: Bluetooth devices, sound outputs.

mod animation;
mod bluetooth;
mod config;
mod connectivity;
mod css;
mod footer;
mod header;
mod icons;
mod levels;
mod media;
mod sys;
mod tiles;

use config::Config;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Orientation, Stack, StackTransitionType};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

const APP_ID: &str = "com.vib1240n.rust-widgets.control-center";
const BINARY_NAME: &str = "rw-control";

thread_local! {
    /// Close (with animation) from anywhere: footer buttons, Wi-Fi details...
    static CLOSER: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

pub fn close_panel() {
    if let Some(f) = CLOSER.with(|c| c.borrow().clone()) {
        f();
    }
}

fn main() {
    if is_already_running() {
        eprintln!("rw-control is already running");
        std::process::exit(0);
    }
    tracing_subscriber::fmt().with_env_filter("info").init();
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run();
}

fn is_already_running() -> bool {
    let me = std::process::id();
    let Ok(rd) = std::fs::read_dir("/proc") else { return false };
    rd.flatten().any(|e| {
        e.file_name().to_string_lossy().parse::<u32>().is_ok_and(|pid| pid != me)
            && std::fs::read_to_string(e.path().join("comm")).is_ok_and(|c| c.trim() == BINARY_NAME)
    })
}

fn panel(width: i32) -> Box {
    let b = Box::new(Orientation::Vertical, 0);
    b.add_css_class("widget-container");
    b.add_css_class("cc-panel");
    b.set_width_request(width);
    b
}

fn build_ui(app: &Application) {
    let config = Rc::new(Config::load());
    css::load();

    let window = ApplicationWindow::builder().application(app).title("Control Center").build();
    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_namespace("rust-widgets");
    window.set_keyboard_mode(KeyboardMode::OnDemand);
    apply_position(&window, &config);

    let stack = Stack::new();
    stack.set_transition_type(StackTransitionType::SlideLeftRight);
    stack.set_transition_duration(200);
    stack.set_vhomogeneous(false);
    stack.set_interpolate_size(true);

    // ---- close (shared by Esc, unfocus, footer, details) ----
    let is_closing = Rc::new(Cell::new(false));
    {
        let w = window.clone();
        let anim = config.animation.clone();
        let closing = is_closing.clone();
        let f: Rc<dyn Fn()> = Rc::new(move || {
            if !closing.replace(true) {
                close_with_animation(&w, &anim);
            }
        });
        CLOSER.with(|c| *c.borrow_mut() = Some(f));
    }

    // ---- sub-views ----
    let s = stack.clone();
    let bt_panel = bluetooth::BluetoothPanel::new(move || s.set_visible_child_name("main"));
    let bt_view = panel(config.appearance.width);
    bt_view.append(&bt_panel.container);
    let bt_panel = Rc::new(bt_panel);

    let s = stack.clone();
    let (out_box, outputs) = levels::outputs_view(move || s.set_visible_child_name("main"));
    let out_view = panel(config.appearance.width);
    out_view.append(&out_box);

    // ---- main view ----
    let main = panel(config.appearance.width);
    main.append(&header::build());

    let conn = if config.sections.connectivity {
        let net_cmd = config.commands.network.clone();
        let (s, btp) = (stack.clone(), bt_panel.clone());
        let (card, c) = connectivity::build(
            move || {
                close_panel();
                sys::shell(&net_cmd);
            },
            move || {
                s.set_visible_child_name("bluetooth");
                btp.refresh();
            },
        );
        main.append(&card);
        Some(c)
    } else {
        None
    };

    let tiles = if config.sections.tiles {
        tiles::build(&config).map(|(grid, t)| {
            main.append(&grid);
            t
        })
    } else {
        None
    };

    let levels = if config.sections.levels {
        let (s, ov) = (stack.clone(), outputs.clone());
        let (card, l) = levels::build(move || {
            ov.refresh();
            s.set_visible_child_name("outputs");
        });
        main.append(&card);
        Some(l)
    } else {
        None
    };

    let media = if config.sections.media {
        let (card, m) = media::build();
        main.append(&card);
        Some(m)
    } else {
        None
    };

    if config.sections.footer {
        main.append(&footer::build(config.commands.power.clone(), close_panel));
    }

    stack.add_named(&main, Some("main"));
    stack.add_named(&bt_view, Some("bluetooth"));
    stack.add_named(&out_view, Some("outputs"));
    stack.set_visible_child_name("main");
    window.set_child(Some(&stack));

    // ---- polling while open (the process only lives while shown) ----
    let tick = Rc::new(Cell::new(0u32));
    let (st, btp) = (stack.clone(), bt_panel.clone());
    glib::timeout_add_local(Duration::from_millis(config.behavior.poll_interval.max(250)), move || {
        let n = tick.get().wrapping_add(1);
        tick.set(n);
        if let Some(l) = &levels {
            l.poll();
        }
        if let Some(m) = &media {
            m.poll();
        }
        if n % 3 == 0 {
            if let Some(c) = &conn {
                c.poll();
            }
            if let Some(t) = &tiles {
                t.poll();
            }
        }
        if st.visible_child_name().as_deref() == Some("bluetooth") {
            btp.refresh();
        }
        glib::ControlFlow::Continue
    });

    // ---- keys / focus ----
    if config.behavior.close_on_escape {
        let keys = gtk4::EventControllerKey::new();
        let st = stack.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key != gtk4::gdk::Key::Escape {
                return glib::Propagation::Proceed;
            }
            if st.visible_child_name().as_deref() != Some("main") {
                st.set_visible_child_name("main");
            } else {
                close_panel();
            }
            glib::Propagation::Stop
        });
        window.add_controller(keys);
    }
    if config.behavior.close_on_unfocus {
        let focus = gtk4::EventControllerFocus::new();
        focus.connect_leave(|_| close_panel());
        window.add_controller(focus);
    }

    window.present();
    if config.animation.enabled {
        let direction = match config.animation.direction.as_str() {
            "up" => animation::Direction::Up,
            _ => animation::Direction::Down,
        };
        animation::slide_in(&window, direction, config.animation.duration, config.position.margin_top);
    }
}

fn apply_position(window: &ApplicationWindow, config: &Config) {
    for e in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        window.set_anchor(e, false);
    }
    let edges: &[Edge] = match config.position.anchor.as_str() {
        "top-left" => &[Edge::Top, Edge::Left],
        "top-center" => &[Edge::Top],
        "bottom-left" => &[Edge::Bottom, Edge::Left],
        "bottom-center" => &[Edge::Bottom],
        "bottom-right" => &[Edge::Bottom, Edge::Right],
        _ => &[Edge::Top, Edge::Right],
    };
    for e in edges {
        window.set_anchor(*e, true);
    }
    window.set_margin(Edge::Top, config.position.margin_top);
    window.set_margin(Edge::Right, config.position.margin_right);
    window.set_margin(Edge::Bottom, config.position.margin_bottom);
    window.set_margin(Edge::Left, config.position.margin_left);
}

fn close_with_animation(window: &ApplicationWindow, anim: &config::AnimationConfig) {
    if anim.enabled {
        let direction = match anim.direction.as_str() {
            "down" => animation::Direction::Up,
            _ => animation::Direction::Down,
        };
        let w = window.clone();
        animation::slide_out(window, direction, anim.duration, move || w.close());
    } else {
        window.close();
    }
}
