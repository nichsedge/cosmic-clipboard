# Cosmic Clipboard (Clipboard History for COSMIC, GNOME & Niri)

A native, high-performance GTK4 / Libadwaita / Wayland clipboard history manager written in Rust. It combines GNOME Clipboard History parity with native COSMIC desktop integration and adaptive Wayland layer-shell popover positioning. Backed by `ringboard-server`.

## Features

- ⚡ **100% Native Rust Client**: Ultra-low memory footprint (~32 MB RAM), zero startup latency (< 8ms toggle via resident D-Bus IPC daemon), and parallel Rayon history pruning.
- 🎨 **Adaptive Desktop Theming (Combining A & B)**:
  - **COSMIC Desktop (B)**: Automatically loads COSMIC palette tokens (`#1B1B1B` background, `#63D0DF` accent, `#F7E062` star yellow) with 12px/8px corners matching COSMIC applets and panels.
  - **GNOME & Niri (A)**: Native Libadwaita dark styling, 16px corner radii, and Adwaita design guidelines.
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

## Architecture & Integration

- Binary: `~/.local/bin/cosmic-clipboard`
- CLI / Launcher: `~/.local/bin/cosmic-clipboard`
- Systemd Resident Daemon: `~/.config/systemd/user/cosmic-clipboard.service`
- Config File: `~/.config/cosmic-clipboard/config.toml`
- 1st-Class COSMIC Panel Applet: `~/.local/share/applications/dev.nichsedge.CosmicPanelClipboardButton.desktop` (`X-CosmicApplet=true`)
- Keybindings:
  - COSMIC: `Super + V` (`com.system76.CosmicSettings.Shortcuts`)
  - GNOME: `Super + V` or `Super + Shift + V`
  - Niri: `Super + V`

