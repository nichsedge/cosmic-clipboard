#!/usr/bin/env python3
"""
Cosmic Clipboard - Premium Native GTK4/Libadwaita Clipboard History Manager
Clone of GNOME Clipboard History with full image/media support and zero-latency Rust backend.
"""

import sys
import os
import json
import base64
import subprocess
import time
import threading
import concurrent.futures
from typing import List, Dict, Optional, Any

import gi
gi.require_version('Gtk', '4.0')
gi.require_version('Gdk', '4.0')
gi.require_version('Adw', '1')
from gi.repository import Gtk, Gdk, Adw, GLib, Pango, Gio

APP_ID = "dev.nichsedge.cosmic.clipboard"
CSS = """
window.clipboard-window {
    background-color: @window_bg_color;
    border-radius: 16px;
    border: 1px solid alpha(@window_fg_color, 0.12);
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.45);
}

.search-header {
    padding: 12px 16px 6px 16px;
}

.search-box {
    border-radius: 12px;
    padding: 8px 12px;
    font-size: 14px;
}

.section-title {
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.8px;
    color: alpha(@window_fg_color, 0.55);
    margin: 10px 16px 4px 16px;
}

.history-card {
    border-radius: 10px;
    margin: 3px 8px;
    padding: 10px 12px;
    background-color: alpha(@window_fg_color, 0.03);
    border: 1px solid alpha(@window_fg_color, 0.05);
    transition: all 120ms ease;
}

.history-card:hover {
    background-color: alpha(@window_fg_color, 0.08);
    border-color: alpha(@window_fg_color, 0.12);
}

.history-card:selected {
    background-color: @accent_bg_color;
    color: @accent_fg_color;
}

.index-badge {
    background-color: alpha(@window_fg_color, 0.08);
    color: alpha(@window_fg_color, 0.7);
    border-radius: 6px;
    font-size: 11px;
    font-weight: 700;
    min-width: 22px;
    min-height: 22px;
    padding: 2px 6px;
}

.history-card:selected .index-badge {
    background-color: alpha(@accent_fg_color, 0.2);
    color: @accent_fg_color;
}

.item-text {
    font-size: 13.5px;
    line-height: 1.35;
}

.item-meta {
    font-size: 11px;
    color: alpha(@window_fg_color, 0.5);
    margin-top: 2px;
}

.history-card:selected .item-meta {
    color: alpha(@accent_fg_color, 0.7);
}

.thumbnail-frame {
    border-radius: 8px;
    border: 1px solid alpha(@window_fg_color, 0.15);
    background-color: alpha(@window_fg_color, 0.05);
}

.action-btn {
    opacity: 0.5;
    border-radius: 8px;
    padding: 6px;
    transition: opacity 120ms ease, background-color 120ms ease;
}

.action-btn:hover {
    opacity: 1.0;
    background-color: alpha(@window_fg_color, 0.15);
}

.history-card:selected .action-btn {
    opacity: 0.8;
}

.history-card:selected .action-btn:hover {
    opacity: 1.0;
    background-color: alpha(@accent_fg_color, 0.25);
}

.action-btn.starred {
    color: #e5a50a;
    opacity: 1.0;
}

.bottom-bar {
    padding: 8px 16px;
    background-color: alpha(@window_fg_color, 0.02);
    border-top: 1px solid alpha(@window_fg_color, 0.08);
}

.footer-hint {
    font-size: 11px;
    color: alpha(@window_fg_color, 0.4);
}
"""

FAVORITE_THRESHOLD = 1 << 32


class ClipboardBackend:
    """Interfaces with Ringboard daemon and system clipboard."""

    @staticmethod
    def is_wayland_watcher_active() -> bool:
        """Checks if ringboard-wayland daemon is running."""
        try:
            res = subprocess.run(["pgrep", "-f", "ringboard-wayland"], capture_output=True)
            return res.returncode == 0
        except Exception:
            return False

    @staticmethod
    def sync_active_clipboard():
        """Only sync active clipboard if ringboard-wayland is NOT actively watching."""
        if ClipboardBackend.is_wayland_watcher_active():
            return

        # Fallback for desktop sessions without active Wayland data-control watcher (e.g. GNOME Wayland)
        try:
            img_res = subprocess.run(
                ["wl-paste", "--type", "image"],
                capture_output=True,
                timeout=0.3
            )
            if img_res.returncode == 0 and len(img_res.stdout) > 0:
                subprocess.run(
                    ["ringboard", "add", "-m", "image/png", "-"],
                    input=img_res.stdout,
                    capture_output=True,
                    timeout=0.5
                )
                return
        except Exception:
            pass

        try:
            txt_res = subprocess.run(
                ["wl-paste", "--type", "text"],
                capture_output=True,
                timeout=0.3
            )
            if txt_res.returncode == 0 and len(txt_res.stdout) > 0:
                subprocess.run(
                    ["ringboard", "add", "-"],
                    input=txt_res.stdout,
                    capture_output=True,
                    timeout=0.5
                )
        except Exception:
            pass

    @staticmethod
    def get_entries() -> List[Dict[str, Any]]:
        """Dumps all entries from ringboard, deduplicated by content, with favorites prioritized."""
        ClipboardBackend.sync_active_clipboard()
        try:
            p = subprocess.run(
                ["ringboard", "debug", "dump"],
                capture_output=True,
                text=True,
                check=True,
                timeout=2.0
            )
            raw_entries = json.loads(p.stdout)

            # Separate into favorites and regular
            favs_raw = []
            reg_raw = []
            for e in raw_entries:
                is_fav = (e.get("id", 0) < FAVORITE_THRESHOLD)
                e["favorite"] = is_fav
                if is_fav:
                    favs_raw.append(e)
                else:
                    reg_raw.append(e)

            # Sort both descending by ID (newest first)
            favs_raw.sort(key=lambda x: x.get("id", 0), reverse=True)
            reg_raw.sort(key=lambda x: x.get("id", 0), reverse=True)

            # Deduplicate favorites (keep newest instance of unique content)
            unique_favs = []
            fav_content_set = set()
            for e in favs_raw:
                content_key = (e.get("kind"), e.get("mime_type", ""), e.get("data"))
                if content_key not in fav_content_set:
                    fav_content_set.add(content_key)
                    unique_favs.append(e)

            # Deduplicate regular entries and exclude items already in favorites
            unique_reg = []
            reg_content_set = set()
            for e in reg_raw:
                content_key = (e.get("kind"), e.get("mime_type", ""), e.get("data"))
                if content_key in fav_content_set:
                    continue  # Pinned items belong exclusively in Favorites section
                if content_key not in reg_content_set:
                    reg_content_set.add(content_key)
                    unique_reg.append(e)

            return unique_favs + unique_reg
        except Exception as err:
            print(f"[ClipboardBackend] Error dumping entries: {err}", file=sys.stderr)
            return []

    @staticmethod
    def toggle_favorite(entry: Dict[str, Any]) -> bool:
        """Favorites or unfavorites an entry with ID fallback and duplicate cleanup."""
        entry_id = entry.get("id")
        is_favorite = entry.get("favorite", False)
        data = entry.get("data")
        action = "unfavorite" if is_favorite else "favorite"
        cmd = ["ringboard", action, str(entry_id)]

        try:
            res = subprocess.run(cmd, capture_output=True, text=True, timeout=1.0)
            success = (res.returncode == 0)

            # If ID was not found, attempt to locate current ID by data
            if not success and data:
                dump = subprocess.run(["ringboard", "debug", "dump"], capture_output=True, text=True, timeout=1.0)
                if dump.returncode == 0:
                    for ce in json.loads(dump.stdout):
                        if ce.get("data") == data:
                            new_id = ce.get("id")
                            if new_id and new_id != entry_id:
                                retry = subprocess.run(["ringboard", action, str(new_id)], capture_output=True, text=True, timeout=1.0)
                                if retry.returncode == 0:
                                    success = True
                                    break

            # If unfavoriting, remove any leftover duplicate favorites with the same content
            if is_favorite and data:
                dump = subprocess.run(["ringboard", "debug", "dump"], capture_output=True, text=True, timeout=1.0)
                if dump.returncode == 0:
                    for ce in json.loads(dump.stdout):
                        if ce.get("id", 0) < FAVORITE_THRESHOLD and ce.get("data") == data:
                            subprocess.run(["ringboard", "remove", str(ce.get("id"))], capture_output=True)

            return success
        except Exception as err:
            print(f"[ClipboardBackend] Error toggling favorite: {err}", file=sys.stderr)
            return False

    @staticmethod
    def remove(entry: Dict[str, Any]) -> bool:
        """Removes an entry from database with fallback."""
        entry_id = entry.get("id")
        data = entry.get("data")
        try:
            res = subprocess.run(["ringboard", "remove", str(entry_id)], capture_output=True, timeout=1.0)
            if res.returncode == 0:
                return True
            if data:
                dump = subprocess.run(["ringboard", "debug", "dump"], capture_output=True, text=True, timeout=1.0)
                if dump.returncode == 0:
                    for ce in json.loads(dump.stdout):
                        if ce.get("data") == data:
                            subprocess.run(["ringboard", "remove", str(ce.get("id"))], capture_output=True)
                            return True
            return False
        except Exception as err:
            print(f"[ClipboardBackend] Error removing entry: {err}", file=sys.stderr)
            return False

    @staticmethod
    def clear_history(entries: Optional[List[Dict[str, Any]]] = None):
        """Wipes ONLY non-favorite / unpinned history, keeping pinned entries completely safe."""
        try:
            if entries is None:
                entries = ClipboardBackend.get_entries()
            unpinned_ids = [
                e.get("id") for e in entries
                if not e.get("favorite") and e.get("id") is not None
            ]
            if not unpinned_ids:
                return

            def remove_entry(eid):
                try:
                    subprocess.run(["ringboard", "remove", str(eid)], capture_output=True, timeout=1.0)
                except Exception:
                    pass

            with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
                list(executor.map(remove_entry, unpinned_ids))
        except Exception as err:
            print(f"[ClipboardBackend] Error wiping unpinned history: {err}", file=sys.stderr)

    @staticmethod
    def copy_to_clipboard(entry: Dict[str, Any]):
        """Sets the system clipboard and simulates Ctrl+V paste."""
        kind = entry.get("kind", "Human")
        data = entry.get("data", "")
        entry_id = entry.get("id")

        if entry_id is not None:
            try:
                subprocess.run(["ringboard", "move-to-front", str(entry_id)], capture_output=True)
            except Exception:
                pass

        if kind == "Bytes" and entry.get("mime_type", "").startswith("image/"):
            # Binary image copy via wl-copy
            try:
                raw_bytes = base64.b64decode(data + '=' * (-len(data) % 4))
                mime = entry.get("mime_type", "image/png")
                subprocess.run(["wl-copy", "--type", mime], input=raw_bytes, check=False)
            except Exception as e:
                print(f"[ClipboardBackend] Image copy failed: {e}", file=sys.stderr)
        else:
            # Text copy via wl-copy
            try:
                subprocess.run(["wl-copy"], input=str(data).encode("utf-8"), check=False)
            except Exception as e:
                print(f"[ClipboardBackend] Text copy failed: {e}", file=sys.stderr)

        # Trigger auto-paste after window hides and focus returns
        def do_paste():
            time.sleep(0.15)
            env = os.environ.copy()
            if "YDOTOOL_SOCKET" not in env and os.path.exists("/run/ydotool.socket"):
                env["YDOTOOL_SOCKET"] = "/run/ydotool.socket"

            # 1. Try ydotool (Universal kernel uinput for GNOME & COSMIC)
            try:
                # Keycodes: 29 = KEY_LEFTCTRL, 47 = KEY_V
                res = subprocess.run(
                    ["ydotool", "key", "29:1", "47:1", "47:0", "29:0"],
                    env=env,
                    capture_output=True,
                    timeout=1.0
                )
                if res.returncode == 0:
                    return
            except Exception:
                pass

            # 2. Try wtype (Wayland virtual keyboard for COSMIC / Niri)
            try:
                res = subprocess.run(
                    ["wtype", "-M", "ctrl", "-k", "v", "-m", "ctrl"],
                    capture_output=True,
                    timeout=1.0
                )
                if res.returncode == 0:
                    return
            except Exception:
                pass

        threading.Thread(target=do_paste, daemon=True).start()


class ClipboardWindow(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app)
        self.set_title("Clipboard History")
        self.set_default_size(530, 620)
        self.add_css_class("clipboard-window")

        self.backend = ClipboardBackend()
        self.entries: List[Dict[str, Any]] = []
        self.filtered_entries: List[Dict[str, Any]] = []
        self.visible_rows: List[Gtk.ListBoxRow] = []

        self._build_ui()
        self._setup_keybindings()

    def _build_ui(self):
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.set_content(main_box)

        # 1. Header Bar
        header = Adw.HeaderBar()
        title_label = Gtk.Label(label="Clipboard History")
        title_label.add_css_class("heading")
        header.set_title_widget(title_label)

        # Reload button in header
        reload_btn = Gtk.Button(icon_name="view-refresh-symbolic", tooltip_text="Refresh (Ctrl+R)")
        reload_btn.connect("clicked", lambda _: self.refresh_data())
        header.pack_end(reload_btn)

        main_box.append(header)

        # 2. Search Box
        search_container = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        search_container.add_css_class("search-header")

        self.search_entry = Gtk.SearchEntry(
            placeholder_text="Search clipboard history… (/)",
            hexpand=True
        )
        self.search_entry.add_css_class("search-box")
        self.search_entry.connect("search-changed", self._on_search_changed)
        search_container.append(self.search_entry)
        main_box.append(search_container)

        # 3. Scrolled List
        self.scrolled_window = Gtk.ScrolledWindow(hexpand=True, vexpand=True)
        self.scrolled_window.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        self.list_box = Gtk.ListBox()
        self.list_box.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self.list_box.connect("row-activated", self._on_row_activated)
        self.scrolled_window.set_child(self.list_box)
        main_box.append(self.scrolled_window)

        # 4. Bottom Footer Bar
        bottom_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        bottom_box.add_css_class("bottom-bar")

        self.count_label = Gtk.Label(label="0 items")
        self.count_label.add_css_class("footer-hint")
        bottom_box.append(self.count_label)

        hint_label = Gtk.Label(label="↵ Paste  •  1-9 Quick Pick  •  F Favorite  •  ⌫ Del  •  Esc Close")
        hint_label.add_css_class("footer-hint")
        hint_label.set_hexpand(True)
        hint_label.set_halign(Gtk.Align.CENTER)
        bottom_box.append(hint_label)

        clear_btn = Gtk.Button(label="Clear History", tooltip_text="Wipe unpinned entries")
        clear_btn.add_css_class("flat")
        clear_btn.add_css_class("destructive-action")
        clear_btn.connect("clicked", self._on_clear_clicked)
        bottom_box.append(clear_btn)

        main_box.append(bottom_box)

    def _setup_keybindings(self):
        controller = Gtk.EventControllerKey.new()
        controller.connect("key-pressed", self._on_key_pressed)
        self.add_controller(controller)

    def _on_key_pressed(self, controller, keyval, keycode, state):
        key_name = Gdk.keyval_name(keyval)

        # Escape closes window
        if key_name == "Escape":
            self.set_visible(False)
            return True

        # Quick pick 1..9 (with Ctrl or when search entry is not focused)
        ctrl_pressed = (state & Gdk.ModifierType.CONTROL_MASK) != 0
        if key_name in ["1", "2", "3", "4", "5", "6", "7", "8", "9"]:
            if ctrl_pressed or not self.search_entry.has_focus():
                idx = int(key_name) - 1
                if idx < len(self.visible_rows):
                    self._on_row_activated(self.list_box, self.visible_rows[idx])
                    return True

        # Ctrl + R reloads
        if ctrl_pressed and key_name in ["r", "R"]:
            self.refresh_data()
            return True

        # Focus search on '/'
        if key_name == "slash" and not self.search_entry.has_focus():
            self.search_entry.grab_focus()
            return True

        selected_row = self.list_box.get_selected_row()

        # Enter activates
        if key_name in ["Return", "KP_Enter"]:
            if selected_row:
                self._on_row_activated(self.list_box, selected_row)
                return True
            elif self.visible_rows:
                self._on_row_activated(self.list_box, self.visible_rows[0])
                return True

        # Delete / Backspace removes item (when search entry is not focused)
        if key_name in ["Delete", "BackSpace"] and not self.search_entry.has_focus():
            if selected_row and hasattr(selected_row, "entry_data"):
                self._delete_entry(selected_row.entry_data)
                return True

        # F favorites (when search entry is not focused)
        if key_name in ["f", "F"] and not self.search_entry.has_focus():
            if selected_row and hasattr(selected_row, "entry_data"):
                self._toggle_favorite(selected_row.entry_data)
                return True

        # Down/Up navigation from search box
        if self.search_entry.has_focus():
            if key_name == "Down" and self.visible_rows:
                self.list_box.select_row(self.visible_rows[0])
                self.visible_rows[0].grab_focus()
                return True

        return False

    def refresh_data(self):
        """Reloads entries from Ringboard backend."""
        self.entries = self.backend.get_entries()
        self._filter_and_render()

    def _on_search_changed(self, entry):
        self._filter_and_render()

    def _filter_and_render(self):
        query = self.search_entry.get_text().strip().lower()

        if not query:
            self.filtered_entries = self.entries
        else:
            filtered = []
            for e in self.entries:
                kind = e.get("kind", "Human")
                data = str(e.get("data", "")).lower()
                mime = str(e.get("mime_type", "")).lower()
                if query in data or query in mime or (query == "image" and kind == "Bytes"):
                    filtered.append(e)
            self.filtered_entries = filtered

        # Clear existing rows
        while True:
            row = self.list_box.get_row_at_index(0)
            if not row:
                break
            self.list_box.remove(row)

        self.visible_rows.clear()
        has_favs = any(e.get("favorite") for e in self.filtered_entries)
        has_regular = any(not e.get("favorite") for e in self.filtered_entries)

        displayed_fav_header = False
        displayed_reg_header = False

        for i, entry in enumerate(self.filtered_entries[:100]):  # Cap visible at 100
            is_fav = entry.get("favorite", False)

            # Section header
            if is_fav and not displayed_fav_header:
                header_row = self._create_header_row("⭐ FAVORITES")
                self.list_box.append(header_row)
                displayed_fav_header = True
            elif not is_fav and has_favs and not displayed_reg_header:
                header_row = self._create_header_row("🕒 RECENT HISTORY")
                self.list_box.append(header_row)
                displayed_reg_header = True

            row = self._create_entry_row(i + 1, entry)
            self.list_box.append(row)
            self.visible_rows.append(row)

        # Select first item
        if self.visible_rows:
            self.list_box.select_row(self.visible_rows[0])

        count_text = f"{len(self.filtered_entries)} items"
        if len(self.filtered_entries) != len(self.entries):
            count_text += f" (filtered from {len(self.entries)})"
        self.count_label.set_text(count_text)

    def _create_header_row(self, title: str) -> Gtk.ListBoxRow:
        row = Gtk.ListBoxRow(selectable=False, activatable=False)
        lbl = Gtk.Label(label=title, xalign=0)
        lbl.add_css_class("section-title")
        row.set_child(lbl)
        return row

    def _create_entry_row(self, index: int, entry: Dict[str, Any]) -> Gtk.ListBoxRow:
        row = Gtk.ListBoxRow()
        row.entry_data = entry
        row.add_css_class("history-card")

        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)

        # Number badge (1..9 or index)
        num_str = str(index) if index <= 9 else f"{index}"
        badge = Gtk.Label(label=num_str)
        badge.add_css_class("index-badge")
        badge.set_valign(Gtk.Align.CENTER)
        box.append(badge)

        # Content area
        content_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2, hexpand=True)
        kind = entry.get("kind", "Human")
        data = entry.get("data", "")
        mime = entry.get("mime_type", "")

        if kind == "Bytes" and mime.startswith("image/"):
            # Render Image thumbnail
            img_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
            try:
                raw_bytes = base64.b64decode(data + '=' * (-len(data) % 4))
                texture = Gdk.Texture.new_from_bytes(GLib.Bytes.new(raw_bytes))
                w, h = texture.get_width(), texture.get_height()

                picture = Gtk.Picture.new_for_paintable(texture)
                picture.set_content_fit(Gtk.ContentFit.CONTAIN)
                picture.set_size_request(80, 52)
                picture.add_css_class("thumbnail-frame")
                img_box.append(picture)

                meta_lbl = Gtk.Label(label=f"Image ({mime.split('/')[-1].upper()}) • {w}×{h} • {len(raw_bytes)//1024 + 1} KB", xalign=0)
                meta_lbl.add_css_class("item-meta")
                meta_lbl.set_valign(Gtk.Align.CENTER)
                img_box.append(meta_lbl)
            except Exception:
                lbl = Gtk.Label(label="[Image Preview Unavailable]", xalign=0)
                img_box.append(lbl)

            content_box.append(img_box)
        else:
            # Render Text
            text_str = str(data).strip()
            # Clean first 200 chars for preview
            lines = text_str.splitlines()
            preview = " ".join([l.strip() for l in lines[:3] if l.strip()])
            if len(preview) > 180:
                preview = preview[:180] + "…"

            text_lbl = Gtk.Label(label=preview, xalign=0)
            text_lbl.add_css_class("item-text")
            text_lbl.set_ellipsize(Pango.EllipsizeMode.END)
            text_lbl.set_lines(2)
            text_lbl.set_wrap(True)
            text_lbl.set_wrap_mode(Pango.WrapMode.CHAR)
            content_box.append(text_lbl)

            char_count = len(text_str)
            line_count = len(lines)
            meta_str = f"{char_count} chars"
            if line_count > 1:
                meta_str += f" • {line_count} lines"
            meta_lbl = Gtk.Label(label=meta_str, xalign=0)
            meta_lbl.add_css_class("item-meta")
            content_box.append(meta_lbl)

        box.append(content_box)

        # Quick Actions
        actions = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        actions.set_valign(Gtk.Align.CENTER)

        # Star / Favorite Button
        is_fav = entry.get("favorite", False)
        star_btn = Gtk.Button(icon_name="starred-symbolic" if is_fav else "non-starred-symbolic")
        star_btn.add_css_class("action-btn")
        if is_fav:
            star_btn.add_css_class("starred")
        star_btn.set_tooltip_text("Unfavorite" if is_fav else "Favorite")
        star_btn.connect("clicked", lambda _, e=entry: self._toggle_favorite(e))
        actions.append(star_btn)

        # Delete Button
        del_btn = Gtk.Button(icon_name="user-trash-symbolic")
        del_btn.add_css_class("action-btn")
        del_btn.set_tooltip_text("Remove")
        del_btn.connect("clicked", lambda _, e=entry: self._delete_entry(e))
        actions.append(del_btn)

        box.append(actions)
        row.set_child(box)
        return row

    def _on_row_activated(self, list_box, row):
        if not hasattr(row, "entry_data"):
            return
        entry = row.entry_data
        self.set_visible(False)
        self.backend.copy_to_clipboard(entry)

    def _toggle_favorite(self, entry: Dict[str, Any]):
        self.backend.toggle_favorite(entry)
        self.refresh_data()

    def _delete_entry(self, entry: Dict[str, Any]):
        self.backend.remove(entry)
        self.refresh_data()

    def _on_clear_clicked(self, btn):
        dialog = Adw.MessageDialog(
            transient_for=self,
            heading="Clear Clipboard History?",
            body="This will remove all non-favorited entries from history. Pinned entries will be kept."
        )
        dialog.add_response("cancel", "Cancel")
        dialog.add_response("clear", "Clear History")
        dialog.set_response_appearance("clear", Adw.ResponseAppearance.DESTRUCTIVE)

        def on_response(dlg, response_id):
            if response_id == "clear":
                self.backend.clear_history(self.entries)
                self.refresh_data()

        dialog.connect("response", on_response)
        dialog.present()


class CosmicClipboardApp(Adw.Application):
    def __init__(self):
        super().__init__(
            application_id=APP_ID,
            flags=Gio.ApplicationFlags.HANDLES_COMMAND_LINE
        )
        self.window: Optional[ClipboardWindow] = None

    def do_startup(self):
        Adw.Application.do_startup(self)
        display = Gdk.Display.get_default()
        if display:
            provider = Gtk.CssProvider()
            provider.load_from_string(CSS)
            Gtk.StyleContext.add_provider_for_display(
                display,
                provider,
                Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
            )

    def do_activate(self):
        if not self.window:
            self.window = ClipboardWindow(self)

        if self.window.is_visible():
            self.window.set_visible(False)
        else:
            self.window.search_entry.set_text("")
            self.window.refresh_data()
            self.window.present()
            self.window.search_entry.grab_focus()

    def do_command_line(self, command_line):
        args = command_line.get_arguments()
        # Toggle window
        self.activate()
        return 0


def main():
    app = CosmicClipboardApp()
    return app.run(sys.argv)


if __name__ == "__main__":
    sys.exit(main())
