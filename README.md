# rust-widgets

Clean, minimal desktop widgets for Wayland. Built with Rust, GTK4, and Layer Shell.

I got tired of bloated widget systems and electron apps eating my RAM. So I made this. It's fast, it looks good, and it stays out of your way.

---

## What's in the box

- **rw-control** - Control center with quick toggles, volume/brightness sliders, media controls, and system stats
- **rw-volume** - Volume control with per-app volume, device switching, and input control
- **rw-media** - Media player widget with album art and playback controls (MPRIS)
- **rw-stats** - System stats popup showing CPU, RAM, disk, battery, and temps

All widgets use a consistent glassmorphic design that plays nicely with blur effects.

---

## Install

### From releases (recommended)

```bash
curl -L https://github.com/vib1240n/rust-widgets/releases/download/latest/rust-widgets-x86_64-linux.tar.gz | tar -xz
./install-local.sh
```

### Build from source

You'll need GTK4, libadwaita, and gtk4-layer-shell dev packages.

```bash
# Arch
sudo pacman -S gtk4 libadwaita gtk4-layer-shell

# Ubuntu/Debian
sudo apt install libgtk-4-dev libadwaita-1-dev libgtk4-layer-shell-dev

# Then build
git clone https://github.com/vib1240n/rust-widgets.git
cd rust-widgets
./install-local.sh
```

Binaries go to `~/.local/bin`. Make sure that's in your PATH.

---

## Usage

```bash
rw toggle control    # Toggle control center
rw toggle volume     # Toggle volume control
rw toggle media      # Toggle media player
rw toggle stats      # Toggle stats popup

rw show <widget>     # Show a widget
rw hide <widget>     # Hide a widget
rw list              # List all widgets and their status
```

### Hyprland keybinds

Add these to your hyprland config:

```ini
# Control center
bind = $mainMod, C, exec, rw toggle control

# Volume (with media keys)
bind = , XF86AudioRaiseVolume, exec, wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+
bind = , XF86AudioLowerVolume, exec, wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-
bind = , XF86AudioMute, exec, wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle

# Dedicated volume widget toggle
bind = $mainMod, V, exec, rw toggle volume

# Media player
bind = $mainMod, M, exec, rw toggle media

# Stats
bind = $mainMod, S, exec, rw toggle stats
```

### Waybar integration

```json
"custom/control": {
  "format": "",
  "on-click": "rw toggle control"
},
"pulseaudio": {
  "on-click": "rw toggle volume"
}
```

### Layer rules for blur

```ini
layerrule = blur, namespace:rust-widgets
layerrule = ignorealpha 0.5, namespace:rust-widgets
```

---

## Configuration

Configs live in `~/.config/rw/<widget>/config.toml`. They're created automatically on first run.

### Example: control center

```toml
# ~/.config/rw/control-center/config.toml

[position]
anchor = "top-right"
margin_top = 50
margin_right = 10

[appearance]
width = 360

[behavior]
poll_interval = 1000
close_on_escape = true
close_on_unfocus = true

[sections]
toggles = true
sliders = true
media = true
stats = true

[animation]
enabled = true
direction = "down"
duration = 250
```

### Example: volume control

```toml
# ~/.config/rw/volume-control/config.toml

[position]
anchor = "top-right"
margin_top = 50
margin_right = 10

[appearance]
width = 320
show_app_volumes = true
show_input = true
show_device_selector = true

[behavior]
volume_override = false  # Set true to allow >100% volume
close_on_escape = true
close_on_unfocus = true
```

---

## Design

The aesthetic is inspired by macOS Control Center and iStats Menu, but adapted for Linux desktops. Think glassmorphism without being obnoxious about it.

- Semi-transparent backgrounds
- Subtle blur (compositor-dependent)
- Smooth slide animations
- Consistent spacing and typography

It's meant to complement tiling window managers, not fight with them.

---

## Tech stack

- **Rust** - Because life's too short for segfaults
- **GTK4** - Modern, native, fast
- **gtk4-layer-shell** - Proper Wayland popups
- **sysinfo** - System metrics
- **PulseAudio/PipeWire** - Audio control via pactl

---

## Roadmap

- [x] Control center
- [x] Volume control with per-app volumes
- [x] Media player (MPRIS)
- [x] System stats
- [ ] Brightness OSD
- [ ] Power menu
- [ ] Calendar widget
- [ ] Bluetooth menu
- [ ] Network menu

---

## Contributing

PRs welcome. If you're adding a new widget, follow the existing structure in `crates/widgets/`. Each widget is self-contained with its own config, CSS, and main.rs.

---

## License

MIT. Do whatever you want with it.

---

Made by [@vib1240n](https://github.com/vib1240n) because I wanted nice widgets on Linux.
