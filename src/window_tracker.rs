use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Duration;

use cosmic_protocols::toplevel_info::v1::client::{
    zcosmic_toplevel_handle_v1, zcosmic_toplevel_info_v1,
};
use wayland_client::{
    event_created_child, protocol::wl_registry, Connection, Dispatch, QueueHandle,
};

static TRACKER: OnceLock<WindowTracker> = OnceLock::new();

#[derive(Clone, Debug, Default)]
pub struct ActiveWindow {
    pub app_id: String,
    pub title: String,
}

#[derive(Clone, Debug)]
struct Toplevel {
    handle: zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
    title: Option<String>,
    app_id: Option<String>,
    state: Vec<State>,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Maximized = 0,
    Minimized = 1,
    Activated = 2,
    Fullscreen = 3,
}

impl TryFrom<u32> for State {
    type Error = ();
    fn try_from(val: u32) -> Result<State, ()> {
        match val {
            0 => Ok(State::Maximized),
            1 => Ok(State::Minimized),
            2 => Ok(State::Activated),
            3 => Ok(State::Fullscreen),
            _ => Err(()),
        }
    }
}

struct AppData {
    toplevel_info: Option<zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1>,
    toplevels: Vec<Toplevel>,
    target_window: Arc<RwLock<Option<ActiveWindow>>>,
}

impl AppData {
    fn update_active(&mut self) {
        let active = self
            .toplevels
            .iter()
            .find(|t| t.state.contains(&State::Activated));

        if let Some(t) = active {
            let app_id = t.app_id.clone().unwrap_or_default();
            // Ignore our own clipboard window so we preserve the application that opened it
            if !app_id.contains("cosmic.clipboard") && !app_id.is_empty() {
                let win = ActiveWindow {
                    app_id,
                    title: t.title.clone().unwrap_or_default(),
                };
                if let Ok(mut lock) = self.target_window.write() {
                    *lock = Some(win);
                }
            }
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for AppData {
    fn event(
        app_data: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<AppData>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version: _,
        } = event
        {
            if interface == "zcosmic_toplevel_info_v1" {
                app_data.toplevel_info = Some(registry.bind::<zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, _, _>(
                    name,
                    1,
                    qh,
                    (),
                ));
            }
        }
    }
}

impl Dispatch<zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, ()> for AppData {
    fn event(
        app_data: &mut Self,
        _: &zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
        event: zcosmic_toplevel_info_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
        if let zcosmic_toplevel_info_v1::Event::Toplevel { toplevel } = event {
            app_data.toplevels.push(Toplevel {
                handle: toplevel,
                title: None,
                app_id: None,
                state: Vec::new(),
            });
        }
    }

    event_created_child!(
        AppData,
        zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
        [
            zcosmic_toplevel_info_v1::EVT_TOPLEVEL_OPCODE => (zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1, ()),
        ]
    );
}

impl Dispatch<zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1, ()> for AppData {
    fn event(
        app_data: &mut Self,
        toplevel: &zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
        event: zcosmic_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
        match event {
            zcosmic_toplevel_handle_v1::Event::Title { title } => {
                if let Some(info) = app_data.toplevels.iter_mut().find(|t| &t.handle == toplevel) {
                    info.title = Some(title);
                }
                app_data.update_active();
            }
            zcosmic_toplevel_handle_v1::Event::AppId { app_id } => {
                if let Some(info) = app_data.toplevels.iter_mut().find(|t| &t.handle == toplevel) {
                    info.app_id = Some(app_id);
                }
                app_data.update_active();
            }
            zcosmic_toplevel_handle_v1::Event::State { state } => {
                if let Some(info) = app_data.toplevels.iter_mut().find(|t| &t.handle == toplevel) {
                    info.state = state
                        .chunks_exact(4)
                        .map(|chunk| u32::from_ne_bytes(chunk.try_into().unwrap()))
                        .flat_map(|val| State::try_from(val).ok())
                        .collect::<Vec<_>>();
                }
                app_data.update_active();
            }
            zcosmic_toplevel_handle_v1::Event::Closed => {
                app_data.toplevels.retain(|t| &t.handle != toplevel);
                app_data.update_active();
            }
            _ => {}
        }
    }
}

pub struct WindowTracker {
    target_window: Arc<RwLock<Option<ActiveWindow>>>,
    saved_target: Mutex<Option<ActiveWindow>>,
}

impl WindowTracker {
    pub fn global() -> &'static Self {
        TRACKER.get_or_init(|| {
            let target_window = Arc::new(RwLock::new(None));
            let target_clone = target_window.clone();

            std::thread::Builder::new()
                .name("cosmic-window-tracker".to_string())
                .spawn(move || {
                    loop {
                        Self::run_connection(&target_clone);
                        std::thread::sleep(Duration::from_secs(2));
                    }
                })
                .expect("failed to spawn window tracker thread");

            Self {
                target_window,
                saved_target: Mutex::new(None),
            }
        })
    }

    /// Snapshot the active window right before cosmic-clipboard is presented
    pub fn snapshot_active_window(&self) {
        // 1. Try Niri IPC if in Niri
        if let Ok(out) = Command::new("niri").args(["msg", "-j", "focused-window"]).output() {
            if out.status.success() {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                    if let Some(app_id) = json.get("app_id").and_then(|v| v.as_str()) {
                        if !app_id.contains("cosmic.clipboard") {
                            let win = ActiveWindow {
                                app_id: app_id.to_string(),
                                title: json.get("title").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                            };
                            if let Ok(mut lock) = self.saved_target.lock() {
                                *lock = Some(win);
                                return;
                            }
                        }
                    }
                }
            }
        }

        // 2. Use COSMIC protocol tracked window
        if let Ok(read) = self.target_window.read() {
            if let Some(ref win) = *read {
                if let Ok(mut lock) = self.saved_target.lock() {
                    *lock = Some(win.clone());
                }
            }
        }
    }

    pub fn is_terminal_focused(&self) -> bool {
        let app_id = if let Ok(lock) = self.saved_target.lock() {
            lock.as_ref().map(|w| w.app_id.to_lowercase()).unwrap_or_default()
        } else {
            String::new()
        };

        if app_id.is_empty() {
            return false;
        }

        is_terminal_app(&app_id)
    }

    fn run_connection(target_window: &Arc<RwLock<Option<ActiveWindow>>>) {
        let conn = match Connection::connect_to_env() {
            Ok(c) => c,
            Err(_) => return,
        };

        let display = conn.display();
        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();
        let _registry = display.get_registry(&qh, ());

        let mut app_data = AppData {
            toplevel_info: None,
            toplevels: Vec::new(),
            target_window: target_window.clone(),
        };

        if event_queue.roundtrip(&mut app_data).is_err() {
            return;
        }

        if app_data.toplevel_info.is_none() {
            return;
        }

        if event_queue.roundtrip(&mut app_data).is_err() {
            return;
        }
        if event_queue.roundtrip(&mut app_data).is_err() {
            return;
        }

        app_data.update_active();

        loop {
            if event_queue.blocking_dispatch(&mut app_data).is_err() {
                break;
            }
        }
    }
}

pub fn is_terminal_app(app_id: &str) -> bool {
    let lower = app_id.to_lowercase();
    lower.contains("terminal")
        || lower.contains("term")
        || lower.contains("ghostty")
        || lower.contains("alacritty")
        || lower.contains("kitty")
        || lower.contains("foot")
        || lower.contains("ptyxis")
        || lower.contains("wezterm")
        || lower.contains("xterm")
        || lower.contains("rxvt")
}
