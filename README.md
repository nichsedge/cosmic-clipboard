# Cosmic Clipboard (Clipboard History for COSMIC & Wayland)

A sleek, modern, native GTK4 / Libadwaita clipboard history manager designed to look and feel 1-to-1 identical to the GNOME Clipboard History extension, backed by a high-performance zero-latency Rust daemon (`ringboard-server`).

## Features

- 🖼️ **Full Media & Image Support**: Captures screenshots and copied images (PNG, JPEG, etc.) with rounded visual thumbnail cards, resolution badges, and file sizes.
- 🎨 **Pixel-Perfect Adwaita Design**: Native dark theme, rounded corners, subtle glassmorphism borders, and smooth transitions matching GNOME and COSMIC design guidelines.
- ⚡ **Zero-Latency Rust Engine**: Backed by `ringboard-server` using `io_uring` and memory-mapped logs.
- ⭐ **Pinned / Favorites**: Pin frequently used snippets or credentials to a dedicated "Favorites" section at the top.
- 🔍 **Type-Ahead Live Search**: Fast instant filtering across text, code, and image MIME types.
- ⌨️ **Keyboard Navigation**:
  - `Enter` / `Return`: Copy selected item and auto-paste into active window via `wtype`
  - `Ctrl + 1..9`: Instantly pick and paste the N-th visible entry
  - `Down` / `Up`: Navigate rows
  - `F`: Toggle favorite / pin
  - `Delete` / `Backspace`: Remove entry
  - `Esc`: Close window
  - `/`: Focus search input

## Architecture & Integration

- Binary: `~/.local/bin/cosmic-clipboard`
- Desktop Entry: `~/.local/share/applications/cosmic-clipboard.desktop`
- COSMIC Shortcut: `Super + V` (`com.system76.CosmicSettings.Shortcuts`)
- GNOME Shortcut: `Super + Shift + V` (custom keybinding)
