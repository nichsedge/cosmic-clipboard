pub mod backend;
pub mod config;
pub mod theme;
pub mod tray;
pub mod ui;
pub mod window_tracker;

use std::cell::RefCell;
use std::rc::Rc;

use gio::prelude::*;

use theme::DesktopEnvironment;
use ui::ClipboardWindow;
use window_tracker::WindowTracker;

const APP_ID: &str = "dev.nichsedge.cosmic.clipboard";

fn main() -> glib::ExitCode {
    let app = libadwaita::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    let window_holder: Rc<RefCell<Option<Rc<ClipboardWindow>>>> = Rc::new(RefCell::new(None));
    let hold_guard: Rc<RefCell<Option<gio::ApplicationHoldGuard>>> = Rc::new(RefCell::new(None));

    {
        let window_holder = window_holder.clone();
        let hold_guard = hold_guard.clone();
        app.connect_startup(move |app| {
            // Keep process resident so hotkey activations are instant (< 2ms)
            *hold_guard.borrow_mut() = Some(app.hold());

            let cfg = config::Config::global();

            // Initialize system tray / top panel indicator if enabled
            if cfg.general.enable_tray {
                tray::start_tray();
            }

            // Initialize active window tracking for terminal auto-paste detection
            WindowTracker::global();

            let de = DesktopEnvironment::current();
            let css = theme::get_css(de);
            let provider = gtk4::CssProvider::new();
            provider.load_from_string(&css);

            if let Some(display) = gdk4::Display::default() {
                gtk4::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }

            // Pre-instantiate window and render initial history in background
            let win = ClipboardWindow::new(app);
            win.refresh_data();
            *window_holder.borrow_mut() = Some(win);
        });
    }

    {
        let window_holder = window_holder.clone();
        app.connect_activate(move |_| {
            if let Some(ref win) = *window_holder.borrow() {
                if win.is_visible() {
                    win.hide();
                } else {
                    win.present_and_focus();
                }
            }
        });
    }

    {
        let window_holder = window_holder.clone();
        app.connect_command_line(move |app, cmdline| {
            let args = cmdline.arguments();
            let is_daemon = args.iter().any(|arg| arg == "--daemon");

            if !is_daemon {
                if let Some(ref win) = *window_holder.borrow() {
                    if win.is_visible() {
                        win.hide();
                    } else {
                        win.present_and_focus();
                    }
                } else {
                    app.activate();
                }
            }
            0.into()
        });
    }

    app.run()
}
