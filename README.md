# Cosmic Clipboard

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)
[![GTK4](https://img.shields.io/badge/GTK4-Libadwaita-blue.svg)](https://gtk.org/)
[![Wayland](https://img.shields.io/badge/Wayland-Layer--Shell-brightgreen.svg)](https://wayland.freedesktop.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A native, high-performance GTK4 / Libadwaita / Wayland clipboard history manager written in Rust. It combines GNOME Clipboard History parity with native COSMIC desktop integration and adaptive Wayland layer-shell popover positioning. Backed by `ringboard-server`.

## Features

- ⚡ **100% Native Rust Client**: Ultra-low memory footprint (~32 MB RAM), zero startup latency (< 8ms toggle via resident D-Bus IPC daemon), and parallel Rayon history pruning.
- 🎨 **Adaptive Desktop Theming**:
  - **COSMIC Desktop**: Automatically loads COSMIC palette tokens (`#1B1B1B` background, `#63D0DF` accent, `#F7E062` star yellow) with 12px/8px corners matching COSMIC applets and panels.
  - **GNOME & Niri**: Native Libadwaita dark styling, 16px corner radii, and Adwaita design guidelines.
- 🪟 **Wayland Layer-Shell Popover Positioning**:
  - Anchors cleanly beneath the top panel (`margin-top: 38px`) via `zwlr_layer_shell_v1` on COSMIC (`cosmic-comp`) and Niri (`niri`).
  - Seamlessly falls back to native modal application window placement on GNOME Wayland (`mutter`).
- 🖼️ **Full Media & Image Support**: Captures screenshots and copied images (PNG, JPEG, etc.) with rounded visual thumbnail cards, resolution badges, and in-memory GPU texture caching.
- ⭐ **Pinned / Favorites**: Pin frequently used snippets or credentials to a dedicated "⭐ FAVORITES" section at the top.
- 🧹 **Selective Clear History**: Cleans all unpinned history items via parallel deletion, keeping pinned entries completely safe.
- 🔍 **Live Search**: Instant in-memory C-level filtering without widget reallocation.
- 🏷️ **Category Filter Pills**:
  - Horizontal filter tabs: `All`, `⭐ Pinned`, `📝 Text`, `💻 Code`, `🔗 Links`, `🖼️ Images`.
  - Instant keyboard shortcuts: `Alt + 1..6` for lightning-fast tab switching.
- 🎨 **Rich Previews & Visual Highlighting**:
  - **Live Color Swatches**: Renders a live color pill next to any hex color code (`#RRGGBB` / `#RGB`).
  - **URL & Domain Badges**: Identifies links and displays clean domain badges (`🌐 github.com`).
  - **Code Highlighting**: Auto-detects code snippets/commands and renders them in monospace (`💻 Code`).
  - **Metadata Summary**: Character counts and line indicators for multi-line snippets.
- 📟 **Smart Terminal Auto-Paste**:
  - Dynamic Wayland toplevel tracker via `cosmic-protocols` (`zcosmic_toplevel_info_v1`) and `niri msg -j focused-window`.
  - Automatically detects terminal emulators (`cosmic-term`, `ghostty`, `alacritty`, `kitty`, `ptyxis`, `foot`, `wezterm`) and injects `Ctrl + Shift + V` with an unmap focus delay, while using `Ctrl + V` for regular GUI apps.
- 🎛️ **StatusNotifierItem (SNI) Top Panel Tray Applet**:
  - Native FreeDesktop / KDE `StatusNotifierItem` integration via D-Bus (`com.system76.CosmicStatusNotifierWatcher` on COSMIC, AppIndicator on GNOME/KDE).
  - Left-click toggles the clipboard history overlay instantly.
  - Right-click context menu with Quick Toggle, Clear History, and Quit.
- ⚙️ **User Configuration File (`~/.config/cosmic-clipboard/config.toml`)**:
  - Automatically created with helpful documentation on first launch.
  - Tune history retention (`history_limit = 200`), popup dimensions (`width`, `height`, `margin_top`), auto-paste delays (`paste_delay_ms`, `key_hold_delay_ms`), initial filter category (`default_category = "all"`), and tray visibility (`enable_tray = true`).
- ⌨️ **Keyboard Navigation**:
  - `Enter` / `Return`: Copy selected item and auto-paste into active window via `ydotool` / `wtype`.
  - `1..9` / `Ctrl + 1..9`: Instantly pick and paste the N-th visible entry.
  - `Alt + 1..6`: Instant category filter switching (`All`, `Pinned`, `Text`, `Code`, `Links`, `Images`).
  - `Down` / `Up`: Navigate rows (or jump from search bar to list).
  - `F`: Toggle favorite / pin.
  - `Delete` / `Backspace`: Remove entry.
  - `Esc`: Close window (window-level capture phase intercepting search bar focus).
  - `/`: Focus search input.
  - `Ctrl + R`: Refresh history.

## Installation & Build

### Prerequisites

- **Rust toolchain** (1.80+)
- **System libraries**:
  - Fedora: `sudo dnf install gtk4-devel libadwaita-devel gtk4-layer-shell-devel clang`
  - Ubuntu / Debian: `sudo apt install libgtk-4-dev libadwaita-1-dev libgtk4-layer-shell-dev clang`
  - Arch Linux: `sudo pacman -S gtk4 libadwaita gtk4-layer-shell clang`
- **Backend & Auto-Paste Tools**:
  - `ringboard`: Clipboard history daemon backend.
  - `ydotool` or `wtype`: For simulated paste injection into target windows.

### Build from Source

```bash
git clone https://github.com/nichsedge/cosmic-clipboard.git
cd cosmic-clipboard
cargo build --release
install -Dm755 target/release/cosmic-clipboard ~/.local/bin/cosmic-clipboard
```

## Configuration (`~/.config/cosmic-clipboard/config.toml`)

```toml
[general]
history_limit = 200          # 0 = unlimited, auto-prunes oldest unpinned entries
enable_tray = true           # Show top panel / system tray indicator icon
default_category = "all"     # "all", "favorites", "text", "code", "links", "images"

[window]
width = 540
height = 580
margin_top = 38              # Top panel offset for layer-shell compositors

[paste]
paste_delay_ms = 120         # Delay before injecting paste keys after window unmap
key_hold_delay_ms = 15       # Key press/release duration in ydotool / wtype
smart_terminal_paste = true  # Automatically switch to Ctrl+Shift+V for terminals
```

## System Integration

### Systemd User Service

Run `cosmic-clipboard` as a resident background service for instant (< 8ms) hotkey activation:

`~/.config/systemd/user/cosmic-clipboard.service`:

```ini
[Unit]
Description=Cosmic Clipboard Daemon
PartOf=graphical-session.target
After=graphical-session.target

[Service]
ExecStart=%h/.local/bin/cosmic-clipboard
Restart=on-failure
RestartSec=3

[Install]
WantedBy=graphical-session.target
```

Enable and start the service:
```bash
systemctl --user daemon-reload
systemctl --user enable --now cosmic-clipboard.service
```

### Desktop Shortcuts

Map a global shortcut to execute `cosmic-clipboard` (or trigger its toggle command):

- **COSMIC**: Go to **Settings → Keyboard → Shortcuts** → Add custom shortcut: `cosmic-clipboard` on `Super + V`.
- **GNOME**: Go to **Settings → Keyboard → View and Customize Shortcuts → Custom Shortcuts** → Add `cosmic-clipboard` on `Super + V` or `Super + Shift + V`.
- **Niri**: Add `binds { Mod+V { spawn "cosmic-clipboard"; } }` to `~/.config/niri/config.kdl`.

## License

This project is licensed under the [MIT License](LICENSE).

