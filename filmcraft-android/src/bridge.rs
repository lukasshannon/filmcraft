//! App-private requests; Android document provider work is performed off UI threads.
use filmcraft_ui_egui::native_dialogs::FileDialog;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(1);
#[derive(Clone)]
pub struct Bridge {
    pub files: PathBuf,
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension(format!("tmp-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
    let result = (|| {
        let mut file = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
        file.write_all(bytes).and_then(|()| file.sync_all()).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
impl Bridge {
    pub fn new(files: &Path) -> Result<Self, String> {
        for relative in ["bridge/requests", "bridge/replies", "imports", "exports"] {
            std::fs::create_dir_all(files.join(relative)).map_err(|e| e.to_string())?;
        }
        Ok(Self { files: files.to_path_buf() })
    }
    pub fn call(&self, value: Value, timeout: Duration) -> Result<Value, String> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
        let name = format!("{now}-{}.json", NEXT.fetch_add(1, Ordering::Relaxed));
        let request = self.files.join("bridge/requests").join(&name);
        let reply = self.files.join("bridge/replies").join(&name);
        atomic_write(&request, &serde_json::to_vec(&value).map_err(|e| e.to_string())?)?;
        let start = Instant::now();
        while start.elapsed() < timeout {
            match std::fs::read(&reply) {
                Ok(bytes) => {
                    let _ = std::fs::remove_file(reply);
                    let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                    if value["kind"] == "error" {
                        return Err(value["message"].as_str().unwrap_or("Android storage failed").into());
                    }
                    return Ok(value);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => std::thread::sleep(Duration::from_millis(80)),
                Err(e) => return Err(e.to_string()),
            }
        }
        let _ = std::fs::remove_file(request);
        Err("Android storage timed out; the local copy is retained".into())
    }
    pub fn pick(&self, dialog: FileDialog) -> Result<Vec<String>, String> {
        let request = match dialog {
            FileDialog::OpenFiles { .. } => json!({"kind":"open", "multiple":true}),
            FileDialog::OpenFile { .. } | FileDialog::OpenProject | FileDialog::Relink { .. } => {
                json!({"kind":"open", "multiple":false})
            }
            FileDialog::SaveProject { name } | FileDialog::SaveAs { name, .. } => {
                json!({"kind":"save", "suggested":name})
            }
            FileDialog::Folder { .. } => json!({"kind":"folder", "export":true}),
        };
        paths(self.call(request, Duration::from_secs(1800))?)
    }
    pub fn publish(&self, path: &str) -> Result<(), String> {
        let p = Path::new(path);
        if !p.starts_with(self.files.join("exports")) || p.components().any(|c| c == std::path::Component::ParentDir) {
            return Ok(()); // Internal recovery, proxies and previews stay on device.
        }
        self.call(json!({"kind":"write", "path":path}), Duration::from_secs(1800))?;
        Ok(())
    }
    pub fn notice(&self, message: &str) {
        let _ = atomic_write(&self.files.join("bridge/notice.json"), message.as_bytes());
    }
}
fn paths(value: Value) -> Result<Vec<String>, String> {
    match value["kind"].as_str() {
        Some("cancelled") => Ok(Vec::new()),
        Some("save") => value["path"].as_str().map(|p| vec![p.into()]).ok_or_else(|| "No save path returned".into()),
        Some("paths") => value["paths"]
            .as_array()
            .ok_or("No file paths returned")?
            .iter()
            .map(|v| v.as_str().map(str::to_owned).ok_or_else(|| "Invalid file path".into()))
            .collect(),
        _ => Err("Unknown Android picker response".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_and_invalid_results() {
        assert_eq!(paths(json!({"kind":"cancelled"})).unwrap(), Vec::<String>::new());
        assert!(paths(json!({"kind":"paths", "paths":[7]})).is_err());
        assert!(paths(json!({"kind":"save"})).is_err());
    }
}
