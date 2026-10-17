use std::collections::HashSet;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

pub const FAVORITE_THRESHOLD: u64 = 1 << 32; // 4294967296
pub const MAX_HISTORY_ITEMS: usize = 60;

static WAYLAND_WATCHER_ACTIVE: AtomicBool = AtomicBool::new(false);
static WATCHER_CHECKED: OnceLock<()> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RingboardEntry {
    pub id: u64,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub favorite: bool,
}

fn default_kind() -> String {
    "Human".to_string()
}

pub struct Backend;

impl Backend {
    pub fn is_watcher_active() -> bool {
        WATCHER_CHECKED.get_or_init(|| {
            let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
            if desktop.eq_ignore_ascii_case("cosmic") || desktop.to_lowercase().contains("niri") {
                WAYLAND_WATCHER_ACTIVE.store(true, Ordering::Relaxed);
            } else {
                let status = Command::new("pgrep")
                    .arg("-f")
                    .arg("ringboard-wayland")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                WAYLAND_WATCHER_ACTIVE.store(status, Ordering::Relaxed);
            }
        });
        WAYLAND_WATCHER_ACTIVE.load(Ordering::Relaxed)
    }

    pub fn sync_active_clipboard() {
        if Self::is_watcher_active() {
            return;
        }

        // Image fallback
        if let Ok(img_out) = Command::new("wl-paste")
            .arg("--type")
            .arg("image")
            .output()
        {
            if img_out.status.success() && !img_out.stdout.is_empty() {
                if let Ok(mut child) = Command::new("ringboard")
                    .args(["add", "-m", "image/png", "-"])
                    .stdin(Stdio::piped())
                    .spawn()
                {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(&img_out.stdout);
                    }
                    let _ = child.wait();
                    return;
                }
            }
        }

        // Text fallback
        if let Ok(txt_out) = Command::new("wl-paste")
            .arg("--type")
            .arg("text")
            .output()
        {
            if txt_out.status.success() && !txt_out.stdout.is_empty() {
                if let Ok(mut child) = Command::new("ringboard")
                    .args(["add", "-"])
                    .stdin(Stdio::piped())
                    .spawn()
                {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(&txt_out.stdout);
                    }
                    let _ = child.wait();
                }
            }
        }
    }

    pub fn get_entries() -> Vec<RingboardEntry> {
        Self::sync_active_clipboard();

        let output = match Command::new("ringboard")
            .args(["debug", "dump"])
            .output()
        {
            Ok(o) if o.status.success() => o.stdout,
            _ => return Vec::new(),
        };

        let raw_entries: Vec<RingboardEntry> = match serde_json::from_slice(&output) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("[Backend] Failed to parse ringboard dump: {e}");
                return Vec::new();
            }
        };

        let mut favs_raw = Vec::new();
        let mut reg_raw = Vec::new();

        for mut e in raw_entries {
            let is_fav = e.id < FAVORITE_THRESHOLD;
            e.favorite = is_fav;
            if is_fav {
                favs_raw.push(e);
            } else {
                reg_raw.push(e);
            }
        }

        // Sort descending by ID (newest first)
        favs_raw.sort_by(|a, b| b.id.cmp(&a.id));
        reg_raw.sort_by(|a, b| b.id.cmp(&a.id));

        // Deduplicate favorites
        let mut unique_favs = Vec::new();
        let mut fav_content_set = HashSet::new();
        for e in favs_raw {
            let key = (e.kind.clone(), e.mime_type.clone(), e.data.clone());
            if !fav_content_set.contains(&key) {
                fav_content_set.insert(key);
                unique_favs.push(e);
            }
        }

        // Deduplicate regular entries (excluding items already favorited)
        let mut unique_reg = Vec::new();
        let mut reg_content_set = HashSet::new();
        for e in reg_raw {
            let key = (e.kind.clone(), e.mime_type.clone(), e.data.clone());
            if fav_content_set.contains(&key) {
                continue;
            }
            if !reg_content_set.contains(&key) {
                reg_content_set.insert(key);
                unique_reg.push(e);
                if unique_reg.len() >= MAX_HISTORY_ITEMS {
                    break;
                }
            }
        }

        let mut result = unique_favs;
        result.extend(unique_reg);
        result
    }

    pub fn toggle_favorite(entry: &RingboardEntry) -> bool {
        let action = if entry.favorite {
            "unfavorite"
        } else {
            "favorite"
        };
        let status = Command::new("ringboard")
            .args([action, &entry.id.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        // If unfavorited, also clean up any duplicate favorite entries with same data
        if entry.favorite && !entry.data.is_empty() {
            if let Ok(dump_out) = Command::new("ringboard").args(["debug", "dump"]).output() {
                if dump_out.status.success() {
                    if let Ok(entries) = serde_json::from_slice::<Vec<RingboardEntry>>(&dump_out.stdout) {
                        for ce in entries {
                            if ce.id < FAVORITE_THRESHOLD && ce.data == entry.data {
                                let _ = Command::new("ringboard")
                                    .args(["remove", &ce.id.to_string()])
                                    .status();
                            }
                        }
                    }
                }
            }
        }

        status
    }

    pub fn remove(entry: &RingboardEntry) -> bool {
        let status = Command::new("ringboard")
            .args(["remove", &entry.id.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !status && !entry.data.is_empty() {
            if let Ok(dump_out) = Command::new("ringboard").args(["debug", "dump"]).output() {
                if dump_out.status.success() {
                    if let Ok(entries) = serde_json::from_slice::<Vec<RingboardEntry>>(&dump_out.stdout) {
                        for ce in entries {
                            if ce.data == entry.data {
                                let _ = Command::new("ringboard")
                                    .args(["remove", &ce.id.to_string()])
                                    .status();
                                return true;
                            }
                        }
                    }
                }
            }
        }

        status
    }

    pub fn clear_history() {
        let dump_out = match Command::new("ringboard").args(["debug", "dump"]).output() {
            Ok(o) if o.status.success() => o.stdout,
            _ => return,
        };

        let entries: Vec<RingboardEntry> = match serde_json::from_slice(&dump_out) {
            Ok(e) => e,
            Err(_) => return,
        };

        let unpinned_ids: Vec<u64> = entries
            .into_iter()
            .filter(|e| e.id >= FAVORITE_THRESHOLD)
            .map(|e| e.id)
            .collect();

        if unpinned_ids.is_empty() {
            return;
        }

        // Concurrently remove all unpinned items
        unpinned_ids.par_iter().for_each(|id| {
            let _ = Command::new("ringboard")
                .args(["remove", &id.to_string()])
                .output();
        });

        // Run garbage collection
        let _ = Command::new("ringboard")
            .args(["garbage-collect", "-m", "0"])
            .output();
    }

    pub fn copy_to_clipboard(entry: &RingboardEntry) {
        // Move to front in history
        let _ = Command::new("ringboard")
            .args(["move-to-front", &entry.id.to_string()])
            .output();

        let is_image = entry.kind == "Bytes"
            && entry
                .mime_type
                .as_deref()
                .unwrap_or("")
                .starts_with("image/");

        if is_image {
            let mime = entry.mime_type.as_deref().unwrap_or("image/png");
            // Standard base64 decode with padding tolerance
            let clean_data = entry.data.trim();
            if let Ok(raw_bytes) = base64_decode_tolerant(clean_data) {
                if let Ok(mut child) = Command::new("wl-copy")
                    .args(["--type", mime])
                    .stdin(Stdio::piped())
                    .spawn()
                {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(&raw_bytes);
                    }
                    let _ = child.wait();
                }
            }
        } else {
            if let Ok(mut child) = Command::new("wl-copy")
                .stdin(Stdio::piped())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(entry.data.as_bytes());
                }
                let _ = child.wait();
            }
        }

        // Auto-paste trigger after focus restoration in background
        thread::spawn(|| {
            // Give compositor 120ms to unmap clipboard window and return active focus to target app
            thread::sleep(Duration::from_millis(120));

            let mut ydotool_env = std::env::vars().collect::<Vec<_>>();
            if !ydotool_env.iter().any(|(k, _)| k == "YDOTOOL_SOCKET")
                && std::path::Path::new("/run/ydotool.socket").exists()
            {
                ydotool_env.push(("YDOTOOL_SOCKET".to_string(), "/run/ydotool.socket".to_string()));
            }

            // 1. ydotool with key delay -d 15 so key-down and key-up are properly processed by event loops
            let mut ydo_cmd = Command::new("ydotool");
            ydo_cmd.args(["key", "-d", "15", "29:1", "47:1", "47:0", "29:0"]);
            for (k, v) in &ydotool_env {
                ydo_cmd.env(k, v);
            }
            if let Ok(res) = ydo_cmd.output() {
                if res.status.success() {
                    return;
                }
            }

            // 2. wtype fallback with delay
            let _ = Command::new("wtype")
                .args(["-M", "ctrl", "-d", "20", "-k", "v", "-m", "ctrl"])
                .output();
        });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_decode_tolerant() {
        let original = b"Hello, World!";
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(original);
        let decoded = base64_decode_tolerant(&encoded).unwrap();
        assert_eq!(decoded, original);

        // Test without padding
        let encoded_no_pad = base64::engine::general_purpose::STANDARD_NO_PAD.encode(original);
        let decoded_no_pad = base64_decode_tolerant(&encoded_no_pad).unwrap();
        assert_eq!(decoded_no_pad, original);
    }

    #[test]
    fn test_favorite_threshold() {
        assert_eq!(FAVORITE_THRESHOLD, 4294967296);
        let pinned_id = 60u64;
        let regular_id = 4294971030u64;
        assert!(pinned_id < FAVORITE_THRESHOLD);
        assert!(regular_id >= FAVORITE_THRESHOLD);
    }

    #[test]
    fn test_entry_deserialization() {
        let json = r#"[{"id":60,"kind":"Human","data":"test text"},{"id":4294971030,"kind":"Bytes","mime_type":"image/png","data":"iVBORw=="}]"#;
        let entries: Vec<RingboardEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, 60);
        assert_eq!(entries[0].kind, "Human");
        assert_eq!(entries[0].data, "test text");
        assert_eq!(entries[1].id, 4294971030);
        assert_eq!(entries[1].mime_type.as_deref(), Some("image/png"));
    }
}

