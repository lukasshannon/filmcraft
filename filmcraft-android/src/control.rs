//! Private file transport for the existing FilmCraft control protocol.
//! Disabled unless an app-private marker exists. No socket or Android permission.
use filmcraft_ui_egui::ControlRequest;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

pub fn start(files: &Path, ctx: &egui::Context) -> Option<Receiver<ControlRequest>> {
    let root = files.join("control");
    if !root.join("enabled").exists() {
        return None;
    }
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    std::thread::Builder::new()
        .name("private control transport".into())
        .spawn(move || {
            let inbox = root.join("requests");
            let outbox = root.join("replies");
            if std::fs::create_dir_all(&inbox).and_then(|()| std::fs::create_dir_all(&outbox)).is_err() {
                return;
            }
            while root.join("enabled").exists() {
                if let Ok(entries) = std::fs::read_dir(&inbox) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().is_none_or(|e| e != "json") {
                            continue;
                        }
                        let response = (|| -> Result<Value, String> {
                            if entry.metadata().map_err(|e| e.to_string())?.len() > 1024 * 1024 {
                                return Err("Request too large".into());
                            }
                            let value: Value = serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                            let method = value["method"].as_str().ok_or("method required")?;
                            let (request, response) = ControlRequest::new(method, value.get("params").cloned().unwrap_or_else(|| json!({})));
                            tx.send(request).map_err(|e| e.to_string())?;
                            ctx.request_repaint();
                            match response.recv_timeout(Duration::from_secs(45)) {
                                Ok(value) => Ok(value),
                                Err(e) => Err(e.to_string()),
                            }
                        })()
                        .unwrap_or_else(|e| json!({"ok":false,"error":e}));
                        if let Ok(bytes) = serde_json::to_vec(&response) {
                            let _ = crate::bridge::atomic_write(&outbox.join(entry.file_name()), &bytes);
                        }
                        let _ = std::fs::remove_file(path);
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        })
        .ok()?;
    Some(rx)
}
