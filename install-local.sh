#!/bin/bash
# install-local.sh - Build and install rust-widgets locally
# Usage:
#   ./install-local.sh              # Build and install everything
#   ./install-local.sh volume-control  # Build and install only volume-control
#   ./install-local.sh stats media     # Build and install multiple widgets
set -e

INSTALL_DIR="$HOME/.local/bin"
PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[0;33m'
NC='\033[0m'

# Widget mapping: name -> (binary, package, config-dir)
declare -A WIDGETS=(
    ["stats"]="rw-stats:stats-popup:stats-popup"
    ["control"]="rw-control:control-center:control-center"
    ["media"]="rw-media:media-player:media-player"
    ["volume"]="rw-volume:volume-control:volume-control"
    ["volume-osd"]="rw-volume-osd:volume-osd:volume-osd"
    ["notifications"]="rw-notifications:notification-center:notification-center"
)

# Parse arguments
SELECTED_WIDGETS=()
if [ $# -eq 0 ]; then
    # No args = install all
    SELECTED_WIDGETS=("stats" "control" "media" "volume" "volume-osd" "notifications")
    BUILD_ALL=true
else
    for arg in "$@"; do
        # Normalize names (allow volume-control or volume, etc)
        case "$arg" in
            stats|stats-popup) SELECTED_WIDGETS+=("stats") ;;
            control|control-center) SELECTED_WIDGETS+=("control") ;;
            media|media-player) SELECTED_WIDGETS+=("media") ;;
            volume|volume-control) SELECTED_WIDGETS+=("volume") ;;
            volume-osd|osd) SELECTED_WIDGETS+=("volume-osd") ;;
            notifications|notification-center|nc) SELECTED_WIDGETS+=("notifications") ;;
            *)
                echo -e "${YELLOW}Unknown widget: $arg${NC}"
                echo "Available: stats, control, media, volume, volume-osd, notifications"
                exit 1
                ;;
        esac
    done
    BUILD_ALL=false
fi

cd "$PROJECT_DIR"

# Build
if [ "$BUILD_ALL" = true ]; then
    echo -e "${BLUE}Building all widgets...${NC}"
    cargo build --release
else
    echo -e "${BLUE}Building selected widgets...${NC}"
    for widget in "${SELECTED_WIDGETS[@]}"; do
        IFS=':' read -r binary package config_dir <<< "${WIDGETS[$widget]}"
        echo -e "  Building ${GREEN}$package${NC}..."
        cargo build --release --package "$package"
    done
    # Always build CLI
    cargo build --release --package widget-cli 2>/dev/null || true
fi

# Install
echo -e "${BLUE}Installing binaries to $INSTALL_DIR...${NC}"
mkdir -p "$INSTALL_DIR"

# Install CLI if it exists
if [ -f "target/release/rw" ]; then
    cp target/release/rw "$INSTALL_DIR/"
    echo -e "  ${GREEN}rw${NC}"
fi

# Install selected widgets
for widget in "${SELECTED_WIDGETS[@]}"; do
    IFS=':' read -r binary package config_dir <<< "${WIDGETS[$widget]}"
    if [ -f "target/release/$binary" ]; then
        cp "target/release/$binary" "$INSTALL_DIR/"
        echo -e "  ${GREEN}$binary${NC}"
    fi
done

# Create configs for selected widgets
echo -e "${BLUE}Creating config directories...${NC}"

for widget in "${SELECTED_WIDGETS[@]}"; do
    IFS=':' read -r binary package config_dir <<< "${WIDGETS[$widget]}"
    mkdir -p "$HOME/.config/rw/$config_dir"
    
    # Create default config if it doesn't exist
    if [ ! -f "$HOME/.config/rw/$config_dir/config.toml" ]; then
        case "$widget" in
            stats)
                cat > "$HOME/.config/rw/$config_dir/config.toml" << 'EOFCONFIG'
[position]
anchor = "top-right"
margin_top = 50
margin_right = 10

[appearance]
width = 280

[behavior]
poll_interval = 2000
close_on_escape = true
close_on_unfocus = true

[sections]
cpu = true
memory = true
disk = true
battery = true
network = false
temperatures = true

[temperatures]
show_labels = ["package", "gpu", "core 0"]
max_display = 4
warning_threshold = 75
critical_threshold = 90

[animation]
enabled = true
type = "slide"
direction = "down"
duration = 250
EOFCONFIG
                echo -e "  ${GREEN}Created stats-popup config${NC}"
                ;;
            control)
                cat > "$HOME/.config/rw/$config_dir/config.toml" << 'EOFCONFIG'
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

[toggles]
wifi = true
bluetooth = true
dnd = true
caffeinate = true
night_light = true
vpn = true
airplane = false

[sliders]
volume = true
brightness = true
volume_output_selector = true

[animation]
enabled = true
type = "slide"
direction = "down"
duration = 250
EOFCONFIG
                echo -e "  ${GREEN}Created control-center config${NC}"
                ;;
            media)
                cat > "$HOME/.config/rw/$config_dir/config.toml" << 'EOFCONFIG'
[position]
anchor = "top-left"
margin_top = 50
margin_left = 10

[appearance]
width = 320

[behavior]
poll_interval = 500
close_on_escape = true
close_on_unfocus = false

[animation]
enabled = true
type = "slide"
direction = "left"
duration = 250
EOFCONFIG
                echo -e "  ${GREEN}Created media-player config${NC}"
                ;;
            volume)
                cat > "$HOME/.config/rw/$config_dir/config.toml" << 'EOFCONFIG'
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
poll_interval = 500
close_on_escape = true
close_on_unfocus = true

[animation]
enabled = true
type = "slide"
direction = "up"
duration = 250
EOFCONFIG
                echo -e "  ${GREEN}Created volume-control config${NC}"
                ;;
            volume-osd)
                cat > "$HOME/.config/rw/$config_dir/config.toml" << 'EOFCONFIG'
[position]
anchor = "top-center"
margin_top = 50

[appearance]
width = 200
height = 48
icon_size = 24
show_percentage = true

[behavior]
timeout = 1500
EOFCONFIG
                echo -e "  ${GREEN}Created volume-osd config${NC}"
                ;;
            notifications)
                cat > "$HOME/.config/rw/$config_dir/config.toml" << 'EOFCONFIG'
[position]
anchor = "top-right"
margin_top = 50
margin_right = 10

[appearance]
panel_width = 380
popup_width = 360
max_height = 600
max_history = 50

[behavior]
close_on_escape = true
close_on_unfocus = true
dnd_enabled = false

[popup]
anchor = "top-right"
margin = 10
gap = 8
timeout_low = 3000
timeout_normal = 5000
timeout_critical = 0
max_visible = 5

[animation]
enabled = true
type = "slide"
direction = "down"
duration = 200
EOFCONFIG
                echo -e "  ${GREEN}Created notification-center config${NC}"
                ;;
        esac
    fi
done

# Check PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo -e "${BLUE}NOTE:${NC} Add $INSTALL_DIR to your PATH:"
    echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc"
fi

echo -e "${GREEN}Done!${NC}"
echo ""
echo "Usage:"
echo "  rw toggle stats      # Toggle stats popup"
echo "  rw toggle control    # Toggle control center"
echo "  rw toggle media      # Toggle media player"
echo "  rw-volume            # Launch volume control"
echo "  rw-volume-osd        # Launch volume OSD daemon"
echo "  rw-notifications     # Launch notification daemon"
echo "  rw list              # List all widgets"
