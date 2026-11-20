//! Pairing from the installer window. Runs the agent's own pairing mode,
//! `vitals.exe pair --ui <dir> --skip-if-paired <config>` (agent/src/pair.rs), which writes
//! `status.txt` into <dir> and stops when a `cancel` file appears. This module reads that file
//! and hands each new code to the page, which draws it as the Vitals QR code.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::json;

use crate::{ops, payload, text};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct Pairing {
    folder: PathBuf,
    done: Arc<AtomicBool>,
}

fn folder() -> PathBuf {
    ops::data_dir().join("pair")
}

/// Starts pairing. `emit` receives page events: `pair` (code, link, seconds left), `pairInfo`
/// (a status message), `paired`. On success the service is started, so monitoring begins.
pub fn start(
    install_dir: &Path,
    emit: impl Fn(serde_json::Value) + Send + 'static,
) -> Result<Pairing, String> {
    let folder = folder();
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let exe = install_dir.join("vitals.exe");
    Command::new(&exe)
        .arg("pair")
        .arg("--ui")
        .arg(&folder)
        .arg("--skip-if-paired")
        .arg(ops::config_path())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("{}: {e}", exe.display()))?;

    let done = Arc::new(AtomicBool::new(false));
    let (status_file, stop) = (folder.join("status.txt"), done.clone());
    std::thread::spawn(move || {
        let mut last = text::PairStatus::default();
        while !stop.load(Ordering::SeqCst) {
            if let Ok(body) = std::fs::read_to_string(&status_file) {
                let now = text::parse_pair_status(&body);
                if now.state == "paired" {
                    stop.store(true, Ordering::SeqCst);
                    let _ = ops::start_service(false);
                    emit(json!({ "event": "paired" }));
                    return;
                }
                if now.state == "pending" && !now.code.is_empty() {
                    if now.code != last.code || now.left != last.left {
                        emit(json!({
                            "event": "pair",
                            "code": now.code,
                            "url": text::pairing_url(payload::CONVEX_URL, &now.code),
                            "left": now.left,
                        }));
                    }
                } else if !now.message.is_empty() && now.message != last.message {
                    emit(json!({ "event": "pairInfo", "message": now.message }));
                }
                last = now;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
    Ok(Pairing { folder, done })
}

impl Pairing {
    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::SeqCst)
    }

    /// Stops the agent's pairing mode (it exits when it sees the `cancel` file).
    pub fn cancel(&self) {
        self.done.store(true, Ordering::SeqCst);
        let _ = std::fs::write(self.folder.join("cancel"), b"");
    }
}
