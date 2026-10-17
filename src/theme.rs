use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEnvironment {
    Cosmic,
    Gnome,
    Niri,
    Other,
}

impl DesktopEnvironment {
    pub fn current() -> Self {
        let desktop = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        if desktop.eq_ignore_ascii_case("cosmic") {
            DesktopEnvironment::Cosmic
        } else if desktop.to_lowercase().contains("gnome") {
            DesktopEnvironment::Gnome
        } else if desktop.to_lowercase().contains("niri") {
            DesktopEnvironment::Niri
        } else {
            DesktopEnvironment::Other
        }
    }

    pub fn is_cosmic(&self) -> bool {
        matches!(self, DesktopEnvironment::Cosmic)
    }
}

pub fn get_css(de: DesktopEnvironment) -> String {
    match de {
        DesktopEnvironment::Cosmic => get_cosmic_css(),
        _ => get_adwaita_css(),
    }
}

fn get_cosmic_css() -> String {
    r#"
window.clipboard-window {
    background-color: #1B1B1B;
    color: #F5F5F5;
    border-radius: 12px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.65);
}

.cosmic-header {
    padding: 10px 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.cosmic-title {
    font-size: 14px;
    font-weight: 700;
    color: #F5F5F5;
}

.search-header {
    padding: 12px 16px 8px 16px;
}

.search-box {
    border-radius: 8px;
    padding: 8px 12px;
    font-size: 14px;
    background-color: #242424;
    color: #ECECEC;
    border: 1px solid #363636;
}

.search-box:focus-within {
    border-color: #63D0DF;
}

.section-title {
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.8px;
    color: #63D0DF;
    margin: 10px 16px 4px 16px;
}

.history-card {
    border-radius: 8px;
    margin: 3px 8px;
    padding: 10px 12px;
    background-color: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    transition: background-color 80ms ease;
}

.history-card:hover {
    background-color: rgba(255, 255, 255, 0.09);
    border-color: rgba(255, 255, 255, 0.14);
}

.history-card:selected {
    background-color: #63D0DF;
    color: #000000;
    border-color: #63D0DF;
}

.index-badge {
    background-color: rgba(255, 255, 255, 0.10);
    color: #ECECEC;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 700;
    min-width: 22px;
    min-height: 22px;
    padding: 2px 6px;
}

.history-card:selected .index-badge {
    background-color: rgba(0, 0, 0, 0.25);
    color: #000000;
}

.item-text {
    font-size: 13.5px;
    line-height: 1.35;
}

.item-meta {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.55);
    margin-top: 2px;
}

.history-card:selected .item-meta {
    color: rgba(0, 0, 0, 0.70);
}

.thumbnail-frame {
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background-color: rgba(255, 255, 255, 0.05);
}

.action-btn {
    opacity: 0.55;
    border-radius: 6px;
    padding: 6px;
    transition: opacity 80ms ease;
}

.action-btn:hover {
    opacity: 1.0;
    background-color: rgba(255, 255, 255, 0.15);
}

.history-card:selected .action-btn {
    opacity: 0.85;
}

.history-card:selected .action-btn:hover {
    opacity: 1.0;
    background-color: rgba(0, 0, 0, 0.20);
}

.action-btn.starred {
    color: #F7E062;
    opacity: 1.0;
}

.history-card:selected .action-btn.starred {
    color: #945B00;
}

.bottom-bar {
    padding: 8px 16px;
    background-color: rgba(255, 255, 255, 0.02);
    border-top: 1px solid rgba(255, 255, 255, 0.08);
}

.footer-hint {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.45);
}
"#
    .to_string()
}

fn get_adwaita_css() -> String {
    r#"
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
    transition: background-color 80ms ease;
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
    transition: opacity 80ms ease;
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
"#
    .to_string()
}
