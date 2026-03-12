mod animation;
mod config;
mod css;

use config::Config;
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::gio::Cancellable;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box, Button, Image, Label, Orientation, Picture, Scale,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

const APP_ID: &str = "com.vib1240n.rust-widgets.media-player";
const BINARY_NAME: &str = "rw-media";

fn main() {
    // Enforce single instance - exit if already running
    if is_already_running() {
        eprintln!("rw-media is already running");
        std::process::exit(0);
    }

    tracing_subscriber::fmt().with_env_filter("info").init();

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run();
}

/// Check if another instance of this widget is already running
fn is_already_running() -> bool {
    let my_pid = std::process::id();
    
    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            
            if let Ok(pid) = name_str.parse::<u32>() {
                // Skip our own process
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

#[derive(Clone, Debug)]
struct Player {
    name: String,
    status: String,
}

fn get_players() -> Vec<Player> {
    let output = Command::new("playerctl")
        .args(["-l"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut players: Vec<Player> = output
        .lines()
        .filter(|l| !l.is_empty())
        .map(|name| {
            let status = Command::new("playerctl")
                .args(["-p", name, "status"])
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            Player {
                name: name.to_string(),
                status,
            }
        })
        .collect();

    // Sort: Playing first, then Spotify preference
    players.sort_by(|a, b| {
        let a_playing = a.status == "Playing";
        let b_playing = b.status == "Playing";
        let a_spotify = a.name.to_lowercase().contains("spotify");
        let b_spotify = b.name.to_lowercase().contains("spotify");

        match (a_playing, b_playing) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                // Both playing or both paused - prefer spotify
                match (a_spotify, b_spotify) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                }
            }
        }
    });

    players
}

fn get_active_player(players: &[Player]) -> Option<String> {
    players.first().map(|p| p.name.clone())
}

fn build_ui(app: &Application) {
    let config = Rc::new(Config::load());
    css::load();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Media Player")
        .build();

    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_namespace("rust-widgets");
    window.set_keyboard_mode(KeyboardMode::OnDemand);

    apply_position(&window, &config);

    // Track current player index
    let players = Rc::new(RefCell::new(get_players()));
    let current_idx = Rc::new(RefCell::new(0usize));

    // Main container
    let container = Box::new(Orientation::Vertical, 0);
    container.add_css_class("media-player");
    container.set_width_request(config.appearance.width);

    // Album art
    let art_frame = Box::new(Orientation::Vertical, 0);
    art_frame.add_css_class("album-art-frame");
    art_frame.set_size_request(config.appearance.width, config.appearance.width);

    let album_art = Picture::new();
    album_art.add_css_class("album-art");
    album_art.set_hexpand(true);
    album_art.set_vexpand(true);
    art_frame.append(&album_art);

    container.append(&art_frame);

    // Controls container
    let controls = Box::new(Orientation::Vertical, 8);
    controls.add_css_class("controls-container");

    // Player selector (dots)
    let selector_box = Box::new(Orientation::Horizontal, 8);
    selector_box.add_css_class("player-selector");
    selector_box.set_halign(gtk4::Align::Center);
    selector_box.set_margin_bottom(8);

    controls.append(&selector_box);

    // Track info
    let info_box = Box::new(Orientation::Vertical, 2);
    info_box.add_css_class("track-info");

    let title_label = Label::new(Some("No media playing"));
    title_label.add_css_class("track-title");
    title_label.set_halign(gtk4::Align::Start);
    title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    title_label.set_max_width_chars(30);
    info_box.append(&title_label);

    let artist_label = Label::new(Some(""));
    artist_label.add_css_class("track-artist");
    artist_label.set_halign(gtk4::Align::Start);
    artist_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    artist_label.set_max_width_chars(35);
    info_box.append(&artist_label);

    controls.append(&info_box);

    // Progress bar
    let progress_box = Box::new(Orientation::Vertical, 4);
    progress_box.add_css_class("progress-section");

    let progress_scale = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
    progress_scale.add_css_class("progress-bar");
    progress_scale.set_hexpand(true);
    progress_scale.set_draw_value(false);

    let current_idx_seek = current_idx.clone();
    let players_seek = players.clone();
    progress_scale.connect_change_value(move |_, _, value| {
        let players = players_seek.borrow();
        let idx = *current_idx_seek.borrow();
        if let Some(player) = players.get(idx) {
            seek_to_position(&player.name, value);
        }
        glib::Propagation::Proceed
    });

    progress_box.append(&progress_scale);

    // Time labels
    let time_box = Box::new(Orientation::Horizontal, 0);
    time_box.add_css_class("time-labels");

    let position_label = Label::new(Some("0:00"));
    position_label.add_css_class("time-position");
    position_label.set_halign(gtk4::Align::Start);
    position_label.set_hexpand(true);
    time_box.append(&position_label);

    let duration_label = Label::new(Some("0:00"));
    duration_label.add_css_class("time-duration");
    duration_label.set_halign(gtk4::Align::End);
    time_box.append(&duration_label);

    progress_box.append(&time_box);
    controls.append(&progress_box);

    // Playback controls
    let playback_box = Box::new(Orientation::Horizontal, 12);
    playback_box.add_css_class("playback-controls");
    playback_box.set_halign(gtk4::Align::Center);

    let prev_btn = Button::new();
    prev_btn.add_css_class("control-btn");
    prev_btn.set_child(Some(&Image::from_icon_name("media-skip-backward-symbolic")));
    let current_idx_prev = current_idx.clone();
    let players_prev = players.clone();
    prev_btn.connect_clicked(move |_| {
        let players = players_prev.borrow();
        let idx = *current_idx_prev.borrow();
        if let Some(player) = players.get(idx) {
            media_prev(&player.name);
        }
    });
    playback_box.append(&prev_btn);

    let play_btn = Button::new();
    play_btn.add_css_class("control-btn");
    play_btn.add_css_class("play-btn");
    play_btn.set_child(Some(&Image::from_icon_name(
        "media-playback-start-symbolic",
    )));
    let current_idx_play = current_idx.clone();
    let players_play = players.clone();
    play_btn.connect_clicked(move |_| {
        let players = players_play.borrow();
        let idx = *current_idx_play.borrow();
        if let Some(player) = players.get(idx) {
            media_play_pause(&player.name);
        }
    });
    playback_box.append(&play_btn);

    let next_btn = Button::new();
    next_btn.add_css_class("control-btn");
    next_btn.set_child(Some(&Image::from_icon_name("media-skip-forward-symbolic")));
    let current_idx_next = current_idx.clone();
    let players_next = players.clone();
    next_btn.connect_clicked(move |_| {
        let players = players_next.borrow();
        let idx = *current_idx_next.borrow();
        if let Some(player) = players.get(idx) {
            media_next(&player.name);
        }
    });
    playback_box.append(&next_btn);

    controls.append(&playback_box);

    // Volume control
    let volume_box = Box::new(Orientation::Horizontal, 8);
    volume_box.add_css_class("volume-section");

    let volume_icon = Image::from_icon_name("audio-volume-high-symbolic");
    volume_icon.add_css_class("volume-icon");
    volume_box.append(&volume_icon);

    let volume_scale = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
    volume_scale.add_css_class("volume-slider");
    volume_scale.set_hexpand(true);
    volume_scale.set_draw_value(false);
    volume_scale.set_value(get_volume(
        &players
            .borrow()
            .first()
            .map(|p| p.name.clone())
            .unwrap_or_default(),
    ) as f64);

    let current_idx_vol = current_idx.clone();
    let players_vol = players.clone();
    volume_scale.connect_value_changed(move |s| {
        let players = players_vol.borrow();
        let idx = *current_idx_vol.borrow();
        if let Some(player) = players.get(idx) {
            set_volume(&player.name, s.value() as u32);
        }
    });

    volume_box.append(&volume_scale);

    let volume_label = Label::new(Some("100%"));
    volume_label.add_css_class("volume-label");
    volume_box.append(&volume_label);

    controls.append(&volume_box);
    container.append(&controls);
    window.set_child(Some(&container));

    // Clones for update loop
    let album_art_clone = album_art.clone();
    let title_clone = title_label.clone();
    let artist_clone = artist_label.clone();
    let play_btn_clone = play_btn.clone();
    let progress_clone = progress_scale.clone();
    let position_clone = position_label.clone();
    let duration_clone = duration_label.clone();
    let volume_label_clone = volume_label.clone();
    let volume_scale_clone = volume_scale.clone();
    let selector_box_clone = selector_box.clone();
    let players_update = players.clone();
    let current_idx_update = current_idx.clone();

    // Initial update
    update_player_selector(
        &selector_box_clone,
        &players_update.borrow(),
        *current_idx_update.borrow(),
        current_idx.clone(),
        players.clone(),
    );

    if let Some(player) = players.borrow().first() {
        update_media_info(
            &player.name,
            &album_art_clone,
            &title_clone,
            &artist_clone,
            &play_btn_clone,
            &progress_clone,
            &position_clone,
            &duration_clone,
        );
    }

    // Poll for updates
    let poll_interval = config.behavior.poll_interval;
    let last_art_url = Rc::new(RefCell::new(String::new()));

    glib::timeout_add_local(Duration::from_millis(poll_interval), move || {
        // Refresh player list
        let new_players = get_players();
        let mut current = current_idx_update.borrow_mut();

        // Keep current selection if still valid, otherwise reset
        if *current >= new_players.len() {
            *current = 0;
        }

        *players_update.borrow_mut() = new_players.clone();
        drop(current);

        // Update selector dots
        update_player_selector(
            &selector_box_clone,
            &new_players,
            *current_idx_update.borrow(),
            current_idx_update.clone(),
            players_update.clone(),
        );

        // Update media info for current player
        let idx = *current_idx_update.borrow();
        if let Some(player) = new_players.get(idx) {
            update_media_info(
                &player.name,
                &album_art_clone,
                &title_clone,
                &artist_clone,
                &play_btn_clone,
                &progress_clone,
                &position_clone,
                &duration_clone,
            );

            let vol = get_volume(&player.name);
            volume_label_clone.set_text(&format!("{}%", vol));
            if (volume_scale_clone.value() - vol as f64).abs() > 2.0 {
                volume_scale_clone.set_value(vol as f64);
            }
        }

        glib::ControlFlow::Continue
    });

    // Track closing state
    let is_closing = Rc::new(RefCell::new(false));

    if config.behavior.close_on_escape {
        let controller = gtk4::EventControllerKey::new();
        let window_clone = window.clone();
        let is_closing_clone = is_closing.clone();
        let anim_config = config.animation.clone();
        controller.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                if !*is_closing_clone.borrow() {
                    *is_closing_clone.borrow_mut() = true;
                    close_with_animation(&window_clone, &anim_config);
                }
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(controller);
    }

    if config.behavior.close_on_unfocus {
        let focus_controller = gtk4::EventControllerFocus::new();
        let window_clone = window.clone();
        let is_closing_clone = is_closing.clone();
        let anim_config = config.animation.clone();
        focus_controller.connect_leave(move |_| {
            if !*is_closing_clone.borrow() {
                *is_closing_clone.borrow_mut() = true;
                close_with_animation(&window_clone, &anim_config);
            }
        });
        window.add_controller(focus_controller);
    }

    window.present();

    if config.animation.enabled {
        let direction = match config.animation.direction.as_str() {
            "up" => animation::Direction::Up,
            "left" => animation::Direction::Left,
            _ => animation::Direction::Down,
        };
        animation::slide_in(
            &window,
            direction,
            config.animation.duration,
            config.position.margin_left,
        );
    }
}

fn update_player_selector(
    selector_box: &Box,
    players: &[Player],
    current_idx: usize,
    idx_rc: Rc<RefCell<usize>>,
    players_rc: Rc<RefCell<Vec<Player>>>,
) {
    // Clear existing dots
    while let Some(child) = selector_box.first_child() {
        selector_box.remove(&child);
    }

    if players.len() <= 1 {
        return; // No selector needed for single player
    }

    for (i, player) in players.iter().enumerate() {
        let dot = Button::new();
        dot.add_css_class("player-dot");
        if i == current_idx {
            dot.add_css_class("active");
        }

        // Show player icon based on name
        let icon_name = if player.name.to_lowercase().contains("spotify") {
            "emblem-music-symbolic"
        } else if player.name.to_lowercase().contains("firefox") {
            "firefox-symbolic"
        } else if player.name.to_lowercase().contains("chrom") {
            "web-browser-symbolic"
        } else {
            "multimedia-player-symbolic"
        };

        dot.set_child(Some(&Image::from_icon_name(icon_name)));
        dot.set_tooltip_text(Some(&player.name));

        let idx_clone = idx_rc.clone();
        let idx = i;
        dot.connect_clicked(move |_| {
            *idx_clone.borrow_mut() = idx;
        });

        selector_box.append(&dot);
    }
}

fn apply_position(window: &gtk4::ApplicationWindow, config: &Config) {
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
        "center" => {}
        _ => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Left, true);
        }
    }

    window.set_margin(Edge::Top, config.position.margin_top);
    window.set_margin(Edge::Right, config.position.margin_right);
    window.set_margin(Edge::Bottom, config.position.margin_bottom);
    window.set_margin(Edge::Left, config.position.margin_left);
}

fn close_with_animation(window: &gtk4::ApplicationWindow, anim_config: &config::AnimationConfig) {
    if anim_config.enabled {
        let direction = match anim_config.direction.as_str() {
            "down" => animation::Direction::Up,
            "left" => animation::Direction::Left,
            _ => animation::Direction::Down,
        };
        let w = window.clone();
        animation::slide_out(window, direction, anim_config.duration, move || w.close());
    } else {
        window.close();
    }
}

fn update_media_info(
    player: &str,
    album_art: &Picture,
    title: &Label,
    artist: &Label,
    play_btn: &Button,
    progress: &Scale,
    position: &Label,
    duration: &Label,
) {
    let status = Command::new("playerctl")
        .args(["-p", player, "status"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    if status.is_empty() || status == "No players found" {
        title.set_text("No media playing");
        artist.set_text("");
        return;
    }

    let icon_name = if status == "Playing" {
        "media-playback-pause-symbolic"
    } else {
        "media-playback-start-symbolic"
    };
    play_btn.set_child(Some(&Image::from_icon_name(icon_name)));

    let title_text = Command::new("playerctl")
        .args(["-p", player, "metadata", "title"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    let artist_text = Command::new("playerctl")
        .args(["-p", player, "metadata", "artist"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    title.set_text(if title_text.is_empty() {
        "Unknown"
    } else {
        &title_text
    });
    artist.set_text(&artist_text);

    let pos_us: i64 = Command::new("playerctl")
        .args(["-p", player, "position"])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<f64>()
                .ok()
        })
        .map(|s| (s * 1_000_000.0) as i64)
        .unwrap_or(0);

    let dur_us: i64 = Command::new("playerctl")
        .args(["-p", player, "metadata", "mpris:length"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
        .unwrap_or(0);

    if dur_us > 0 {
        let progress_pct = (pos_us as f64 / dur_us as f64) * 100.0;
        progress.set_value(progress_pct);
    }

    let pos_secs = pos_us / 1_000_000;
    let dur_secs = dur_us / 1_000_000;
    position.set_text(&format!("{}:{:02}", pos_secs / 60, pos_secs % 60));
    duration.set_text(&format!("{}:{:02}", dur_secs / 60, dur_secs % 60));

    let art_url = Command::new("playerctl")
        .args(["-p", player, "metadata", "mpris:artUrl"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    if !art_url.is_empty() {
        load_album_art(album_art, &art_url);
    }
}

fn load_album_art(picture: &Picture, url: &str) {
    if url.starts_with("file://") {
        let path = url.strip_prefix("file://").unwrap_or(url);
        if let Ok(pixbuf) = Pixbuf::from_file(path) {
            picture.set_pixbuf(Some(&pixbuf));
        }
    } else if url.starts_with("http://") || url.starts_with("https://") {
        let file = gtk4::gio::File::for_uri(url);
        let picture = picture.clone();
        file.read_async(
            glib::Priority::DEFAULT,
            None::<&Cancellable>,
            move |result| {
                if let Ok(stream) = result {
                    if let Ok(pixbuf) = Pixbuf::from_stream(&stream, None::<&Cancellable>) {
                        picture.set_pixbuf(Some(&pixbuf));
                    }
                }
            },
        );
    }
}

fn media_play_pause(player: &str) {
    let _ = Command::new("playerctl")
        .args(["-p", player, "play-pause"])
        .spawn();
}

fn media_next(player: &str) {
    let _ = Command::new("playerctl")
        .args(["-p", player, "next"])
        .spawn();
}

fn media_prev(player: &str) {
    let _ = Command::new("playerctl")
        .args(["-p", player, "previous"])
        .spawn();
}

fn seek_to_position(player: &str, percent: f64) {
    let dur_us: i64 = Command::new("playerctl")
        .args(["-p", player, "metadata", "mpris:length"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
        .unwrap_or(0);

    if dur_us > 0 {
        let target_secs = (dur_us as f64 * percent / 100.0) / 1_000_000.0;
        let _ = Command::new("playerctl")
            .args(["-p", player, "position", &format!("{:.2}", target_secs)])
            .spawn();
    }
}

fn get_volume(player: &str) -> u32 {
    Command::new("playerctl")
        .args(["-p", player, "volume"])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<f64>()
                .ok()
        })
        .map(|v| (v * 100.0) as u32)
        .unwrap_or(100)
}

fn set_volume(player: &str, vol: u32) {
    let vol_float = vol as f64 / 100.0;
    let _ = Command::new("playerctl")
        .args(["-p", player, "volume", &format!("{:.2}", vol_float)])
        .spawn();
}
