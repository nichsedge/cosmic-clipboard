use std::sync::OnceLock;

use gio::prelude::*;

use ksni::blocking::TrayMethods;
use ksni::{Category, MenuItem, Status, ToolTip, Tray};

use crate::backend::Backend;

static TRAY_HANDLE: OnceLock<Option<ksni::blocking::Handle<ClipboardTray>>> = OnceLock::new();

#[derive(Debug, Default)]
pub struct ClipboardTray;

impl Tray for ClipboardTray {
    fn id(&self) -> String {
        "cosmic-clipboard".into()
    }

    fn title(&self) -> String {
        "Clipboard History".into()
    }

    fn category(&self) -> Category {
        Category::ApplicationStatus
    }

    fn status(&self) -> Status {
        Status::Active
    }

    fn icon_name(&self) -> String {
        "edit-paste-symbolic".into()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Cosmic Clipboard".into(),
            description: "Clipboard History (Click to toggle)".into(),
            icon_name: "edit-paste-symbolic".into(),
            icon_pixmap: Vec::new(),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        // Toggle clipboard overlay on GTK main loop
        glib::idle_add_once(|| {
            if let Some(app) = gio::Application::default() {
                app.activate();
            }
        });
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Toggle Clipboard".into(),
                icon_name: "edit-paste-symbolic".into(),
                activate: Box::new(|_| {
                    glib::idle_add_once(|| {
                        if let Some(app) = gio::Application::default() {
                            app.activate();
                        }
                    });
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Clear History".into(),
                icon_name: "edit-clear-all-symbolic".into(),
                activate: Box::new(|_| {
                    glib::idle_add_once(|| {
                        Backend::clear_history();
                    });
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit-symbolic".into(),
                activate: Box::new(|_| {
                    glib::idle_add_once(|| {
                        if let Some(app) = gio::Application::default() {
                            app.quit();
                        }
                    });
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn start_tray() {
    TRAY_HANDLE.get_or_init(|| {
        let tray = ClipboardTray;
        match tray.spawn() {
            Ok(handle) => Some(handle),
            Err(e) => {
                eprintln!("Info: System tray / StatusNotifierItem watcher unavailable: {e}");
                None
            }
        }
    });
}
