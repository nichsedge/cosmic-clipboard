use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gdk4::prelude::*;
use gtk4::prelude::*;
use libadwaita::prelude::*;

use crate::backend::{Backend, RingboardEntry};
use crate::theme::DesktopEnvironment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterCategory {
    All,
    Favorites,
    Text,
    Code,
    Links,
    Images,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    Image,
    Color,
    Url,
    Code,
    Text,
}

#[derive(Clone)]
struct RowItem {
    entry: RingboardEntry,
    row: gtk4::ListBoxRow,
    search_text: String,
    is_fav: bool,
    kind: ContentKind,
}

struct AppState {
    entries: Vec<RingboardEntry>,
    items: Vec<RowItem>,
    search_query: String,
    active_category: FilterCategory,
    texture_cache: HashMap<u64, gdk4::Texture>,
}

pub struct ClipboardWindow {
    window: libadwaita::ApplicationWindow,
    list_box: gtk4::ListBox,
    search_entry: gtk4::SearchEntry,
    count_label: gtk4::Label,
    filter_buttons: Vec<(FilterCategory, gtk4::Button)>,
    state: Rc<RefCell<AppState>>,
    _de: DesktopEnvironment,
}

impl ClipboardWindow {
    pub fn new(app: &libadwaita::Application) -> Rc<Self> {
        let de = DesktopEnvironment::current();
        let cfg = crate::config::Config::global();

        let window = libadwaita::ApplicationWindow::builder()
            .application(app)
            .title("Clipboard History")
            .default_width(cfg.window.width)
            .default_height(cfg.window.height)
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
            window.set_margin(Edge::Top, cfg.window.margin_top);
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

        // 3. Category Filter Pills
        let filter_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        filter_bar.add_css_class("filter-bar");

        let categories = [
            (FilterCategory::All, "All"),
            (FilterCategory::Favorites, "⭐ Pinned"),
            (FilterCategory::Text, "📝 Text"),
            (FilterCategory::Code, "💻 Code"),
            (FilterCategory::Links, "🔗 Links"),
            (FilterCategory::Images, "🖼️ Images"),
        ];

        let initial_category = match cfg.general.default_category.to_lowercase().as_str() {
            "favorites" | "pinned" | "favs" => FilterCategory::Favorites,
            "text" => FilterCategory::Text,
            "code" => FilterCategory::Code,
            "links" | "urls" => FilterCategory::Links,
            "images" | "pics" => FilterCategory::Images,
            _ => FilterCategory::All,
        };

        let mut filter_buttons = Vec::new();
        for (cat, label) in categories {
            let btn = gtk4::Button::builder()
                .label(label)
                .build();
            btn.add_css_class("filter-pill");
            if cat == initial_category {
                btn.add_css_class("active");
            }
            filter_bar.append(&btn);
            filter_buttons.push((cat, btn));
        }
        main_box.append(&filter_bar);

        // 4. Scrolled List
        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .min_content_width(540)
            .min_content_height(480)
            .build();

        let list_box = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .build();
        scrolled.set_child(Some(&list_box));
        main_box.append(&scrolled);

        // 5. Footer Bar
        let bottom_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        bottom_box.add_css_class("bottom-bar");

        let count_label = gtk4::Label::new(Some("0 items"));
        count_label.add_css_class("footer-hint");
        bottom_box.append(&count_label);

        let hint_lbl = gtk4::Label::new(Some("↵ Paste  •  1-9 Pick  •  F Pin  •  Alt+1-6 Tabs  •  Esc Close"));
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
            search_query: String::new(),
            active_category: initial_category,
            texture_cache: HashMap::new(),
        }));

        let win_ref = Rc::new(Self {
            window,
            list_box,
            search_entry,
            count_label,
            filter_buttons,
            state,
            _de: de,
        });

        win_ref.setup_headers();
        win_ref.setup_filtering();
        win_ref.setup_category_clicks();
        win_ref.setup_keybindings();
        win_ref.setup_actions(clear_btn);

        win_ref
    }

    fn setup_headers(&self) {
        self.list_box.set_header_func(|row, before| {
            let is_fav = row.widget_name() == "fav";
            let prev_is_fav = before.map(|b| b.widget_name() == "fav");

            let show_header = match prev_is_fav {
                None => true, // First visible row in list
                Some(prev_fav) => is_fav != prev_fav, // Transition boundary between fav and reg
            };

            if show_header {
                let title = if is_fav { "⭐ FAVORITES" } else { "🕒 RECENT HISTORY" };
                let need_new = match row.header() {
                    Some(ref w) => {
                        if let Some(lbl) = w.downcast_ref::<gtk4::Label>() {
                            lbl.set_label(title);
                            false
                        } else {
                            true
                        }
                    }
                    None => true,
                };
                if need_new {
                    let lbl = gtk4::Label::builder()
                        .label(title)
                        .xalign(0.0)
                        .build();
                    lbl.add_css_class("section-title");
                    row.set_header(Some(&lbl));
                }
            } else {
                row.set_header(gtk4::Widget::NONE);
            }
        });
    }

    fn setup_filtering(&self) {
        let state_clone = self.state.clone();

        self.list_box.set_filter_func(move |row| {
            let state = state_clone.borrow();
            let query = &state.search_query;
            let active_cat = state.active_category;

            for item in &state.items {
                if item.row == *row {
                    let cat_match = item_matches_category(item, active_cat);
                    let query_match = query.is_empty() || item.search_text.contains(query);
                    return cat_match && query_match;
                }
            }

            false
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
            let active_cat = state.active_category;
            let visible_count = state
                .items
                .iter()
                .filter(|it| item_matches_category(it, active_cat) && (text.is_empty() || it.search_text.contains(&text)))
                .count();

            // Select first visible row
            if let Some(first_visible) = state
                .items
                .iter()
                .find(|it| item_matches_category(it, active_cat) && (text.is_empty() || it.search_text.contains(&text)))
            {
                list_box.select_row(Some(&first_visible.row));
            }

            let msg = if text.is_empty() {
                format!("{} items", visible_count)
            } else {
                format!("{} items (filtered from {})", visible_count, state.items.len())
            };
            count_label.set_text(&msg);
        });
    }

    fn setup_category_clicks(self: &Rc<Self>) {
        for (cat, btn) in &self.filter_buttons {
            let this = Rc::clone(self);
            let target_cat = *cat;
            btn.connect_clicked(move |_| {
                this.select_category(target_cat);
            });
        }
    }

    pub fn select_category(self: &Rc<Self>, target_cat: FilterCategory) {
        self.state.borrow_mut().active_category = target_cat;

        for (cat, btn) in &self.filter_buttons {
            if *cat == target_cat {
                btn.add_css_class("active");
            } else {
                btn.remove_css_class("active");
            }
        }

        self.list_box.invalidate_filter();

        let state = self.state.borrow();
        let query = &state.search_query;
        let visible_count = state
            .items
            .iter()
            .filter(|it| item_matches_category(it, target_cat) && (query.is_empty() || it.search_text.contains(query)))
            .count();

        if let Some(first_visible) = state
            .items
            .iter()
            .find(|it| item_matches_category(it, target_cat) && (query.is_empty() || it.search_text.contains(query)))
        {
            self.list_box.select_row(Some(&first_visible.row));
        }

        let msg = format!("{} items", visible_count);
        self.count_label.set_text(&msg);
    }

    fn setup_keybindings(self: &Rc<Self>) {
        let key_controller = gtk4::EventControllerKey::new();
        // CAPTURE phase intercepts key events at the window root before any child widget consumes them
        key_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);

        let this = Rc::clone(self);
        key_controller.connect_key_pressed(move |_, keyval, _, state| {
            let key_name = keyval.name().map(|n| n.to_string()).unwrap_or_default();
            let ctrl = (state & gdk4::ModifierType::CONTROL_MASK) == gdk4::ModifierType::CONTROL_MASK;
            let alt = (state & gdk4::ModifierType::ALT_MASK) == gdk4::ModifierType::ALT_MASK;

            // 1. ESCAPE: Immediately close window from anywhere
            if key_name == "Escape" {
                this.window.set_visible(false);
                return glib::Propagation::Stop;
            }

            // 2. Ctrl + R: Reload history
            if ctrl && (key_name == "r" || key_name == "R") {
                this.refresh_data();
                return glib::Propagation::Stop;
            }

            // 3. Alt + 1..6: Category tab switching
            if alt {
                let cat_opt = match key_name.as_str() {
                    "1" => Some(FilterCategory::All),
                    "2" => Some(FilterCategory::Favorites),
                    "3" => Some(FilterCategory::Text),
                    "4" => Some(FilterCategory::Code),
                    "5" => Some(FilterCategory::Links),
                    "6" => Some(FilterCategory::Images),
                    _ => None,
                };
                if let Some(cat) = cat_opt {
                    this.select_category(cat);
                    return glib::Propagation::Stop;
                }
            }

            // 4. 1..9 Quick pick (with Ctrl or when search entry is not focused)
            if let Some(digit) = key_name.chars().next() {
                if digit.is_ascii_digit() && digit != '0' && (ctrl || !this.search_entry.has_focus()) {
                    let idx = (digit as usize) - ('1' as usize);
                    let query = this.state.borrow().search_query.clone();
                    let active_cat = this.state.borrow().active_category;
                    let state = this.state.borrow();
                    let visible_items: Vec<_> = state
                        .items
                        .iter()
                        .filter(|it| item_matches_category(it, active_cat) && (query.is_empty() || it.search_text.contains(&query)))
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

            // 5. '/' focuses search entry
            if key_name == "slash" && !this.search_entry.has_focus() {
                this.search_entry.grab_focus();
                return glib::Propagation::Stop;
            }

            // 6. Enter activates
            if key_name == "Return" || key_name == "KP_Enter" {
                let selected = this.list_box.selected_row();
                let state = this.state.borrow();
                let query = state.search_query.clone();
                let active_cat = state.active_category;

                let to_activate = if let Some(ref sel) = selected {
                    state.items.iter().find(|it| it.row == *sel).map(|it| it.entry.clone())
                } else {
                    state
                        .items
                        .iter()
                        .find(|it| item_matches_category(it, active_cat) && (query.is_empty() || it.search_text.contains(&query)))
                        .map(|it| it.entry.clone())
                };

                drop(state);
                if let Some(entry) = to_activate {
                    this.window.set_visible(false);
                    Backend::copy_to_clipboard(&entry);
                    return glib::Propagation::Stop;
                }
            }

            // 7. Delete / Backspace removes item (when search entry is not focused)
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

            // 8. F favorites (when search entry is not focused)
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

            // 9. Down navigation from search entry
            if this.search_entry.has_focus() && key_name == "Down" {
                let state = this.state.borrow();
                let query = state.search_query.clone();
                let active_cat = state.active_category;
                if let Some(first_visible) = state
                    .items
                    .iter()
                    .find(|it| item_matches_category(it, active_cat) && (query.is_empty() || it.search_text.contains(&query)))
                {
                    this.list_box.select_row(Some(&first_visible.row));
                    first_visible.row.grab_focus();
                    return glib::Propagation::Stop;
                }
            }

            // 10. Up navigation returns focus to search entry if at first visible item
            if !this.search_entry.has_focus() && key_name == "Up" {
                if let Some(selected) = this.list_box.selected_row() {
                    let state = this.state.borrow();
                    let query = &state.search_query;
                    let active_cat = state.active_category;
                    let is_first_visible = state
                        .items
                        .iter()
                        .find(|it| item_matches_category(it, active_cat) && (query.is_empty() || it.search_text.contains(query)))
                        .map(|it| it.row == selected)
                        .unwrap_or(false);
                    drop(state);
                    if is_first_visible {
                        this.search_entry.grab_focus();
                        return glib::Propagation::Stop;
                    }
                }
            }

            glib::Propagation::Proceed
        });

        self.window.add_controller(key_controller);

        // Also connect stop-search on the SearchEntry to dismiss window immediately on Escape
        let win_clone = self.window.clone();
        self.search_entry.connect_stop_search(move |_| {
            win_clone.set_visible(false);
        });
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

        for (i, entry) in entries.iter().enumerate() {
            let is_fav = entry.favorite;
            let (row, search_text, kind) = self.create_entry_row(i + 1, entry);
            row.set_widget_name(if is_fav { "fav" } else { "reg" });
            self.list_box.append(&row);

            items.push(RowItem {
                entry: entry.clone(),
                row,
                search_text,
                is_fav,
                kind,
            });
        }

        {
            let mut state = self.state.borrow_mut();
            state.entries = entries;
            state.items = items;
        }

        let query = self.search_entry.text().trim().to_lowercase();
        self.state.borrow_mut().search_query = query.clone();
        self.list_box.invalidate_filter();

        let state = self.state.borrow();
        let active_cat = state.active_category;
        let visible_count = state
            .items
            .iter()
            .filter(|it| item_matches_category(it, active_cat) && (query.is_empty() || it.search_text.contains(&query)))
            .count();

        if let Some(first_visible) = state
            .items
            .iter()
            .find(|it| item_matches_category(it, active_cat) && (query.is_empty() || it.search_text.contains(&query)))
        {
            self.list_box.select_row(Some(&first_visible.row));
        }

        let msg = if query.is_empty() {
            format!("{} items", visible_count)
        } else {
            format!("{} items (filtered from {})", visible_count, state.items.len())
        };
        self.count_label.set_text(&msg);
    }

    fn create_entry_row(self: &Rc<Self>, index: usize, entry: &RingboardEntry) -> (gtk4::ListBoxRow, String, ContentKind) {
        let row = gtk4::ListBoxRow::builder().build();
        row.add_css_class("history-card");

        let box_widget = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);

        // Number Badge
        let badge_text = format!("{index}");
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
        let content_kind: ContentKind;

        if is_image {
            content_kind = ContentKind::Image;
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

                let subtype = mime.split('/').next_back().unwrap_or("png").to_uppercase();
                let meta_lbl = gtk4::Label::builder()
                    .label(format!("Image ({subtype}) • {w}×{h}"))
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

            let (detected_kind, extra_info) = detect_kind(text_str);
            content_kind = detected_kind;

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

            if content_kind == ContentKind::Code {
                text_lbl.add_css_class("item-code");
            }
            content_box.append(&text_lbl);

            // Metadata row with optional type badge & color swatch
            let meta_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);

            if let Some(ref badge_text) = extra_info {
                if content_kind == ContentKind::Color {
                    let swatch = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
                    swatch.add_css_class("color-swatch");
                    swatch.set_valign(gtk4::Align::Center);
                    let provider = gtk4::CssProvider::new();
                    provider.load_from_string(&format!(".swatch-{} {{ background-color: {}; }}", entry.id, text_str));
                    if let Some(display) = gdk4::Display::default() {
                        gtk4::style_context_add_provider_for_display(&display, &provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
                    }
                    swatch.add_css_class(&format!("swatch-{}", entry.id));
                    meta_box.append(&swatch);
                }

                let badge_lbl = gtk4::Label::builder()
                    .label(badge_text)
                    .valign(gtk4::Align::Center)
                    .build();
                badge_lbl.add_css_class("type-badge");
                meta_box.append(&badge_lbl);
            }

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
                .valign(gtk4::Align::Center)
                .build();
            meta_lbl.add_css_class("item-meta");
            meta_box.append(&meta_lbl);

            content_box.append(&meta_box);
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

        (row, search_text, content_kind)
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

fn item_matches_category(item: &RowItem, cat: FilterCategory) -> bool {
    match cat {
        FilterCategory::All => true,
        FilterCategory::Favorites => item.is_fav,
        FilterCategory::Text => item.kind == ContentKind::Text || item.kind == ContentKind::Color,
        FilterCategory::Code => item.kind == ContentKind::Code,
        FilterCategory::Links => item.kind == ContentKind::Url,
        FilterCategory::Images => item.kind == ContentKind::Image,
    }
}

fn detect_kind(s: &str) -> (ContentKind, Option<String>) {
    let trimmed = s.trim();

    // 1. Hex Color
    if is_hex_color(trimmed) {
        return (ContentKind::Color, Some(trimmed.to_uppercase()));
    }

    // 2. URL
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let domain = extract_domain(trimmed);
        let badge = if domain.is_empty() {
            "🌐 Link".to_string()
        } else {
            format!("🌐 {domain}")
        };
        return (ContentKind::Url, Some(badge));
    }

    // 3. Code snippet
    if looks_like_code(trimmed) {
        return (ContentKind::Code, Some("💻 Code".to_string()));
    }

    (ContentKind::Text, None)
}

fn is_hex_color(s: &str) -> bool {
    let s = s.trim();
    if !s.starts_with('#') {
        return false;
    }
    let hex = &s[1..];
    (hex.len() == 3 || hex.len() == 4 || hex.len() == 6 || hex.len() == 8)
        && hex.chars().all(|c| c.is_ascii_hexdigit())
}

fn extract_domain(url: &str) -> String {
    let s = url.trim();
    let without_proto = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    without_proto
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_string()
}

fn looks_like_code(s: &str) -> bool {
    let t = s.trim();
    if t.contains("fn ")
        || t.contains("def ")
        || t.contains("const ")
        || t.contains("let ")
        || t.contains("import ")
        || t.contains("export ")
        || t.contains("class ")
        || t.contains("struct ")
        || t.contains("impl ")
        || t.contains("SELECT ")
        || t.contains("FROM ")
        || t.contains("WHERE ")
        || t.starts_with("curl ")
        || t.starts_with("sudo ")
        || t.starts_with("git ")
        || t.starts_with("docker ")
        || t.starts_with("cargo ")
    {
        return true;
    }
    let lines: Vec<&str> = t.lines().collect();
    if lines.len() > 1 && (t.contains('{') && t.contains('}')) {
        return true;
    }
    false
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
