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
- ⌨️ **Keyboard Navigation**:
  - `Enter` / `Return`: Copy selected item and auto-paste into active window via `ydotool` / `wtype` (40ms).
  - `1..9` / `Ctrl + 1..9`: Instantly pick and paste the N-th visible entry.
  - `Down` / `Up`: Navigate rows.
  - `F`: Toggle favorite / pin.
  - `Delete` / `Backspace`: Remove entry.
  - `Esc`: Close window.
  - `/`: Focus search input.
  - `Ctrl + R`: Refresh history.

## Architecture & Integration

- Binary: `~/.local/bin/cosmic-clipboard`
- CLI / Launcher: `~/.local/bin/cosmic-clipboard`
- Systemd Resident Daemon: `~/.config/systemd/user/cosmic-clipboard.service`
- Keybindings:
  - COSMIC: `Super + V` (`com.system76.CosmicSettings.Shortcuts`)
  - GNOME: `Super + V` or `Super + Shift + V`
  - Niri: `Super + V`
