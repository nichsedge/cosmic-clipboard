use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gdk4::prelude::*;
use gtk4::prelude::*;
use libadwaita::prelude::*;

use crate::backend::{Backend, RingboardEntry};
use crate::theme::DesktopEnvironment;

#[derive(Clone)]
struct RowItem {
    entry: RingboardEntry,
    row: gtk4::ListBoxRow,
    search_text: String,
    is_fav: bool,
}

#[derive(Clone)]
struct HeaderItem {
    row: gtk4::ListBoxRow,
}

struct AppState {
    entries: Vec<RingboardEntry>,
    items: Vec<RowItem>,
    fav_header: Option<HeaderItem>,
    reg_header: Option<HeaderItem>,
    search_query: String,
    texture_cache: HashMap<u64, gdk4::Texture>,
}

pub struct ClipboardWindow {
    window: libadwaita::ApplicationWindow,
    list_box: gtk4::ListBox,
    search_entry: gtk4::SearchEntry,
    count_label: gtk4::Label,
    state: Rc<RefCell<AppState>>,
    _de: DesktopEnvironment,
}

impl ClipboardWindow {
    pub fn new(app: &libadwaita::Application) -> Rc<Self> {
        let de = DesktopEnvironment::current();

        let window = libadwaita::ApplicationWindow::builder()
            .application(app)
            .title("Clipboard History")
            .default_width(530)
            .default_height(620)
            .hide_on_close(true)
            .build();

        window.add_css_class("clipboard-window");

        // Wayland Layer Shell popover configuration (COSMIC / Niri)
        if gtk4_layer_shell::is_supported() {
            use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
            window.init_layer_shell();
            window.set_layer(Layer::Overlay);
            window.set_keyboard_mode(KeyboardMode::Exclusive);
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Bottom, false);
            window.set_anchor(Edge::Left, false);
            window.set_anchor(Edge::Right, false);
            window.set_margin(Edge::Top, 38);
            window.set_namespace(Some("cosmic-clipboard"));
        }

        let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        window.set_content(Some(&main_box));

        // 1. Header Area
        if de.is_cosmic() || gtk4_layer_shell::is_supported() {
            let header = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
            header.add_css_class("cosmic-header");

            let title_lbl = gtk4::Label::new(Some("Clipboard History"));
            title_lbl.add_css_class("cosmic-title");
            title_lbl.set_halign(gtk4::Align::Start);
            title_lbl.set_hexpand(true);
            header.append(&title_lbl);

            let refresh_btn = gtk4::Button::from_icon_name("view-refresh-symbolic");
            refresh_btn.set_tooltip_text(Some("Refresh (Ctrl+R)"));
            refresh_btn.add_css_class("flat");
            header.append(&refresh_btn);

            main_box.append(&header);
        } else {
            let header = libadwaita::HeaderBar::new();
            let title_lbl = gtk4::Label::new(Some("Clipboard History"));
            title_lbl.add_css_class("heading");
            header.set_title_widget(Some(&title_lbl));

            let refresh_btn = gtk4::Button::from_icon_name("view-refresh-symbolic");
            refresh_btn.set_tooltip_text(Some("Refresh (Ctrl+R)"));
            header.pack_end(&refresh_btn);

            main_box.append(&header);
        }

        // 2. Search Box
        let search_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        search_box.add_css_class("search-header");

        let search_entry = gtk4::SearchEntry::builder()
            .placeholder_text("Search clipboard history… (/)")
            .hexpand(true)
            .build();
        search_entry.add_css_class("search-box");
        search_box.append(&search_entry);
        main_box.append(&search_box);

        // 3. Scrolled List
        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .min_content_width(530)
            .min_content_height(480)
            .build();

        let list_box = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .build();
        scrolled.set_child(Some(&list_box));
        main_box.append(&scrolled);

        // 4. Footer Bar
        let bottom_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        bottom_box.add_css_class("bottom-bar");

        let count_label = gtk4::Label::new(Some("0 items"));
        count_label.add_css_class("footer-hint");
        bottom_box.append(&count_label);

        let hint_lbl = gtk4::Label::new(Some("↵ Paste  •  1-9 Quick Pick  •  F Favorite  •  ⌫ Del  •  Esc Close"));
        hint_lbl.add_css_class("footer-hint");
        hint_lbl.set_hexpand(true);
        hint_lbl.set_halign(gtk4::Align::Center);
        bottom_box.append(&hint_lbl);

        let clear_btn = gtk4::Button::builder()
            .label("Clear History")
            .tooltip_text("Wipe unpinned entries")
            .build();
        clear_btn.add_css_class("flat");
        clear_btn.add_css_class("destructive-action");
        bottom_box.append(&clear_btn);

        main_box.append(&bottom_box);

        let state = Rc::new(RefCell::new(AppState {
            entries: Vec::new(),
            items: Vec::new(),
            fav_header: None,
            reg_header: None,
            search_query: String::new(),
            texture_cache: HashMap::new(),
        }));

        let win_ref = Rc::new(Self {
            window,
            list_box,
            search_entry,
            count_label,
            state,
            _de: de,
        });

        win_ref.setup_filtering();
        win_ref.setup_keybindings();
        win_ref.setup_actions(clear_btn);

        win_ref
    }

    fn setup_filtering(&self) {
        let state_clone = self.state.clone();

        self.list_box.set_filter_func(move |row| {
            let state = state_clone.borrow();
            let query = &state.search_query;
            if query.is_empty() {
                return true;
            }

            // Check if it's the favorites header
            if let Some(ref fh) = state.fav_header {
                if fh.row == *row {
                    return state.items.iter().any(|item| item.is_fav && item.search_text.contains(query));
                }
            }

            // Check if it's the regular header
            if let Some(ref rh) = state.reg_header {
                if rh.row == *row {
                    return state.items.iter().any(|item| !item.is_fav && item.search_text.contains(query));
                }
            }

            // Check item rows
            for item in &state.items {
                if item.row == *row {
                    return item.search_text.contains(query);
                }
            }

            true
        });

        let this = self.state.clone();
        let list_box = self.list_box.clone();
        let search_entry = self.search_entry.clone();
        let count_label = self.count_label.clone();

        search_entry.connect_search_changed(move |entry| {
            let text = entry.text().trim().to_lowercase();
            this.borrow_mut().search_query = text.clone();
            list_box.invalidate_filter();

            let state = this.borrow();
            let visible_count = state
                .items
                .iter()
                .filter(|it| text.is_empty() || it.search_text.contains(&text))
                .count();

            // Select first visible row
            if let Some(first_visible) = state
                .items
                .iter()
                .find(|it| text.is_empty() || it.search_text.contains(&text))
            {
                list_box.select_row(Some(&first_visible.row));
            }

            let msg = if text.is_empty() {
                format!("{} items", state.items.len())
            } else {
                format!("{} items (filtered from {})", visible_count, state.items.len())
            };
            count_label.set_text(&msg);
        });
    }

    fn setup_keybindings(self: &Rc<Self>) {
        let key_controller = gtk4::EventControllerKey::new();

        let this = Rc::clone(self);
        key_controller.connect_key_pressed(move |_, keyval, _, state| {
            let key_name = keyval.name().map(|n| n.to_string()).unwrap_or_default();
            let ctrl = (state & gdk4::ModifierType::CONTROL_MASK) == gdk4::ModifierType::CONTROL_MASK;

            if key_name == "Escape" {
                this.window.set_visible(false);
                return glib::Propagation::Stop;
            }

            if ctrl && (key_name == "r" || key_name == "R") {
                this.refresh_data();
                return glib::Propagation::Stop;
            }

            // 1..9 Quick pick (with Ctrl or when search entry is not focused)
            if let Some(digit) = key_name.chars().next() {
                if digit.is_ascii_digit() && digit != '0' {
                    if ctrl || !this.search_entry.has_focus() {
                        let idx = (digit as usize) - ('1' as usize);
                        let query = this.state.borrow().search_query.clone();
                        let state = this.state.borrow();
                        let visible_items: Vec<_> = state
                            .items
                            .iter()
                            .filter(|it| query.is_empty() || it.search_text.contains(&query))
                            .collect();

                        if idx < visible_items.len() {
                            let entry = visible_items[idx].entry.clone();
                            drop(state);
                            this.window.set_visible(false);
                            Backend::copy_to_clipboard(&entry);
                            return glib::Propagation::Stop;
                        }
                    }
                }
            }

            // Focus search with '/'
            if key_name == "slash" && !this.search_entry.has_focus() {
                this.search_entry.grab_focus();
                return glib::Propagation::Stop;
            }

            // Enter activates
            if key_name == "Return" || key_name == "KP_Enter" {
                let selected = this.list_box.selected_row();
                let state = this.state.borrow();
                let query = state.search_query.clone();

                let to_activate = if let Some(ref sel) = selected {
                    state.items.iter().find(|it| it.row == *sel).map(|it| it.entry.clone())
                } else {
                    state
                        .items
                        .iter()
                        .find(|it| query.is_empty() || it.search_text.contains(&query))
                        .map(|it| it.entry.clone())
                };

                drop(state);
                if let Some(entry) = to_activate {
                    this.window.set_visible(false);
                    Backend::copy_to_clipboard(&entry);
                    return glib::Propagation::Stop;
                }
            }

            // Delete / Backspace removes item (when search entry is not focused)
            if (key_name == "Delete" || key_name == "BackSpace") && !this.search_entry.has_focus() {
                if let Some(selected) = this.list_box.selected_row() {
                    let state = this.state.borrow();
                    let to_del = state.items.iter().find(|it| it.row == selected).map(|it| it.entry.clone());
                    drop(state);
                    if let Some(entry) = to_del {
                        Backend::remove(&entry);
                        this.refresh_data();
                        return glib::Propagation::Stop;
                    }
                }
            }

            // F favorites (when search entry is not focused)
            if (key_name == "f" || key_name == "F") && !this.search_entry.has_focus() {
                if let Some(selected) = this.list_box.selected_row() {
                    let state = this.state.borrow();
                    let to_fav = state.items.iter().find(|it| it.row == selected).map(|it| it.entry.clone());
                    drop(state);
                    if let Some(entry) = to_fav {
                        Backend::toggle_favorite(&entry);
                        this.refresh_data();
                        return glib::Propagation::Stop;
                    }
                }
            }

            // Down navigation from search entry
            if this.search_entry.has_focus() && key_name == "Down" {
                let state = this.state.borrow();
                let query = state.search_query.clone();
                if let Some(first_visible) = state
                    .items
                    .iter()
                    .find(|it| query.is_empty() || it.search_text.contains(&query))
                {
                    this.list_box.select_row(Some(&first_visible.row));
                    first_visible.row.grab_focus();
                    return glib::Propagation::Stop;
                }
            }

            glib::Propagation::Proceed
        });

        self.window.add_controller(key_controller);
    }

    fn setup_actions(self: &Rc<Self>, clear_btn: gtk4::Button) {
        let this = Rc::clone(self);
        self.list_box.connect_row_activated(move |_, row| {
            let state = this.state.borrow();
            let entry = state.items.iter().find(|it| it.row == *row).map(|it| it.entry.clone());
            drop(state);

            if let Some(e) = entry {
                this.window.set_visible(false);
                Backend::copy_to_clipboard(&e);
            }
        });

        let this = Rc::clone(self);
        clear_btn.connect_clicked(move |_| {
            let dialog = libadwaita::AlertDialog::builder()
                .heading("Clear Clipboard History?")
                .body("This will remove all non-favorited entries from history. Pinned entries will be kept.")
                .build();

            dialog.add_response("cancel", "Cancel");
            dialog.add_response("clear", "Clear History");
            dialog.set_response_appearance("clear", libadwaita::ResponseAppearance::Destructive);

            let win_for_dlg = Rc::clone(&this);
            dialog.choose(Some(&this.window), gio::Cancellable::NONE, move |response| {
                if response == "clear" {
                    Backend::clear_history();
                    win_for_dlg.search_entry.set_text("");
                    win_for_dlg.refresh_data();
                }
            });
        });
    }

    pub fn refresh_data(self: &Rc<Self>) {
        let entries = Backend::get_entries();

        // Clear list_box
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let mut items = Vec::new();
        let mut fav_header = None;
        let mut reg_header = None;

        let has_favs = entries.iter().any(|e| e.favorite);
        let _has_reg = entries.iter().any(|e| !e.favorite);

        let mut displayed_fav_h = false;
        let mut displayed_reg_h = false;

        for (i, entry) in entries.iter().enumerate() {
            let is_fav = entry.favorite;

            if is_fav && !displayed_fav_h {
                let h_row = self.create_header_row("⭐ FAVORITES", "favs");
                self.list_box.append(&h_row);
                fav_header = Some(HeaderItem {
                    row: h_row,
                });
                displayed_fav_h = true;
            } else if !is_fav && has_favs && !displayed_reg_h {
                let h_row = self.create_header_row("🕒 RECENT HISTORY", "regular");
                self.list_box.append(&h_row);
                reg_header = Some(HeaderItem {
                    row: h_row,
                });
                displayed_reg_h = true;
            }

            let (row, search_text) = self.create_entry_row(i + 1, entry);
            self.list_box.append(&row);

            items.push(RowItem {
                entry: entry.clone(),
                row,
                search_text,
                is_fav,
            });
        }

        {
            let mut state = self.state.borrow_mut();
            state.entries = entries;
            state.items = items;
            state.fav_header = fav_header;
            state.reg_header = reg_header;
        }

        let query = self.search_entry.text().trim().to_lowercase();
        self.state.borrow_mut().search_query = query.clone();
        self.list_box.invalidate_filter();

        let state = self.state.borrow();
        let visible_count = state
            .items
            .iter()
            .filter(|it| query.is_empty() || it.search_text.contains(&query))
            .count();

        if let Some(first_visible) = state
            .items
            .iter()
            .find(|it| query.is_empty() || it.search_text.contains(&query))
        {
            self.list_box.select_row(Some(&first_visible.row));
        }

        let msg = if query.is_empty() {
            format!("{} items", state.items.len())
        } else {
            format!("{} items (filtered from {})", visible_count, state.items.len())
        };
        self.count_label.set_text(&msg);
    }

    fn create_header_row(&self, title: &str, _section: &str) -> gtk4::ListBoxRow {
        let row = gtk4::ListBoxRow::builder()
            .selectable(false)
            .activatable(false)
            .build();
        let lbl = gtk4::Label::builder()
            .label(title)
            .xalign(0.0)
            .build();
        lbl.add_css_class("section-title");
        row.set_child(Some(&lbl));
        row
    }

    fn create_entry_row(self: &Rc<Self>, index: usize, entry: &RingboardEntry) -> (gtk4::ListBoxRow, String) {
        let row = gtk4::ListBoxRow::builder().build();
        row.add_css_class("history-card");

        let box_widget = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);

        // Number Badge
        let badge_text = if index <= 9 {
            format!("{index}")
        } else {
            format!("{index}")
        };
        let badge = gtk4::Label::builder()
            .label(&badge_text)
            .valign(gtk4::Align::Center)
            .build();
        badge.add_css_class("index-badge");
        box_widget.append(&badge);

        // Content Area
        let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        content_box.set_hexpand(true);

        let is_image = entry.kind == "Bytes"
            && entry
                .mime_type
                .as_deref()
                .unwrap_or("")
                .starts_with("image/");

        let search_text: String;

        if is_image {
            let mime = entry.mime_type.as_deref().unwrap_or("image/png");
            search_text = format!("image {mime}").to_lowercase();

            let img_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);

            // Texture caching
            let mut state = self.state.borrow_mut();
            let texture_opt = if let Some(cached) = state.texture_cache.get(&entry.id) {
                Some(cached.clone())
            } else {
                // Decode base64
                if let Ok(raw_bytes) = base64_decode_tolerant(entry.data.trim()) {
                    let glib_bytes = glib::Bytes::from(&raw_bytes);
                    if let Ok(texture) = gdk4::Texture::from_bytes(&glib_bytes) {
                        state.texture_cache.insert(entry.id, texture.clone());
                        Some(texture)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            drop(state);

            if let Some(texture) = texture_opt {
                let w = texture.width();
                let h = texture.height();
                let picture = gtk4::Picture::for_paintable(&texture);
                picture.set_content_fit(gtk4::ContentFit::Contain);
                picture.set_size_request(80, 52);
                picture.add_css_class("thumbnail-frame");
                img_box.append(&picture);

                let subtype = mime.split('/').last().unwrap_or("png").to_uppercase();
                let meta_lbl = gtk4::Label::builder()
                    .label(&format!("Image ({subtype}) • {w}×{h}"))
                    .xalign(0.0)
                    .valign(gtk4::Align::Center)
                    .build();
                meta_lbl.add_css_class("item-meta");
                img_box.append(&meta_lbl);
            } else {
                let lbl = gtk4::Label::builder()
                    .label("[Image Preview Unavailable]")
                    .xalign(0.0)
                    .build();
                img_box.append(&lbl);
            }

            content_box.append(&img_box);
        } else {
            let text_str = entry.data.trim();
            search_text = text_str.to_lowercase();

            let lines: Vec<&str> = text_str.lines().collect();
            let mut preview_words = Vec::new();
            for l in lines.iter().take(3) {
                let tr = l.trim();
                if !tr.is_empty() {
                    preview_words.push(tr);
                }
            }
            let mut preview = preview_words.join(" ");
            if preview.len() > 180 {
                preview.truncate(180);
                preview.push('…');
            }

            let text_lbl = gtk4::Label::builder()
                .label(&preview)
                .xalign(0.0)
                .ellipsize(pango::EllipsizeMode::End)
                .lines(2)
                .wrap(true)
                .wrap_mode(pango::WrapMode::Char)
                .build();
            text_lbl.add_css_class("item-text");
            content_box.append(&text_lbl);

            let char_count = text_str.chars().count();
            let line_count = lines.len();
            let meta_str = if line_count > 1 {
                format!("{char_count} chars • {line_count} lines")
            } else {
                format!("{char_count} chars")
            };
            let meta_lbl = gtk4::Label::builder()
                .label(&meta_str)
                .xalign(0.0)
                .build();
            meta_lbl.add_css_class("item-meta");
            content_box.append(&meta_lbl);
        }

        box_widget.append(&content_box);

        // Actions
        let actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        actions.set_valign(gtk4::Align::Center);

        // Star / Favorite Button
        let is_fav = entry.favorite;
        let star_icon = if is_fav {
            "starred-symbolic"
        } else {
            "non-starred-symbolic"
        };
        let star_btn = gtk4::Button::from_icon_name(star_icon);
        star_btn.add_css_class("action-btn");
        if is_fav {
            star_btn.add_css_class("starred");
        }
        star_btn.set_tooltip_text(Some(if is_fav { "Unfavorite" } else { "Favorite" }));

        let this = Rc::clone(self);
        let entry_for_star = entry.clone();
        star_btn.connect_clicked(move |_| {
            Backend::toggle_favorite(&entry_for_star);
            this.refresh_data();
        });
        actions.append(&star_btn);

        // Delete Button
        let del_btn = gtk4::Button::from_icon_name("user-trash-symbolic");
        del_btn.add_css_class("action-btn");
        del_btn.set_tooltip_text(Some("Remove"));

        let this = Rc::clone(self);
        let entry_for_del = entry.clone();
        del_btn.connect_clicked(move |_| {
            Backend::remove(&entry_for_del);
            this.refresh_data();
        });
        actions.append(&del_btn);

        box_widget.append(&actions);
        row.set_child(Some(&box_widget));

        (row, search_text)
    }

    pub fn present_and_focus(self: &Rc<Self>) {
        crate::window_tracker::WindowTracker::global().snapshot_active_window();
        self.search_entry.set_text("");
        self.refresh_data();
        self.window.present();
        self.search_entry.grab_focus();
    }

    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }
}

fn base64_decode_tolerant(input: &str) -> Result<Vec<u8>, ()> {
    use base64::Engine;
    let mut s = input.to_string();
    let rem = s.len() % 4;
    if rem > 0 {
        s.push_str(&"=".repeat(4 - rem));
    }
    base64::engine::general_purpose::STANDARD
        .decode(&s)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(input))
        .map_err(|_| ())
}
