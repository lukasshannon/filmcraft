use crate::bridge::Bridge;
use filmcraft_engine::{FsServices, Services};
use std::{io, sync::Arc};
pub struct AndroidServices {
    pub bridge: Bridge,
    pub ctx: egui::Context,
}
impl Services for AndroidServices {
    fn read_file(&self, p: &str) -> io::Result<Vec<u8>> {
        FsServices.read_file(p)
    }
    fn write_file(&self, p: &str, data: &[u8]) -> io::Result<()> {
        FsServices.write_file(p, data)?;
        if std::path::Path::new(p).starts_with(self.bridge.files.join("exports")) {
            let bridge = self.bridge.clone();
            let ctx = self.ctx.clone();
            let p = p.to_string();
            // Project serialization is already durable locally. Provider writes must
            // never block the frame loop, even when a cloud drive is disconnected.
            std::thread::Builder::new().name("Android save document".into()).spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.publish(&p)));
                bridge.notice(&match result {
                    Ok(Ok(())) => "Saved to Android storage".to_string(),
                    Ok(Err(e)) => format!("Saved on device; Android storage copy failed: {e}. Use Save As to retry."),
                    Err(_) => "Saved on device; Android storage copy failed. Use Save As to retry.".into(),
                });
                ctx.request_repaint();
            })?;
        }
        Ok(())
    }
    fn write_file_new(&self, p: &str, data: &[u8]) -> io::Result<()> {
        FsServices.write_file_new(p, data)
    }
    fn file_size(&self, p: &str) -> io::Result<u64> {
        FsServices.file_size(p)
    }
    fn read_range(&self, p: &str, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        FsServices.read_range(p, offset, len)
    }
    fn reader(&self, p: &str) -> Option<io::Result<filmcraft_media::SharedReader>> {
        FsServices.reader(p)
    }
    fn list_dir(&self, p: &str) -> Option<io::Result<Vec<String>>> {
        FsServices.list_dir(p)
    }
    fn file_loader(&self) -> Option<filmcraft_media::sequence::FrameLoader> {
        FsServices.file_loader()
    }
    fn publish_export(&self, report: &filmcraft_engine::export::Report) -> io::Result<()> {
        for path in std::iter::once(&report.path).chain(report.extra_files.iter()) {
            self.bridge.publish(path).map_err(io::Error::other)?;
        }
        self.bridge.notice("Export saved to Android storage");
        self.ctx.request_repaint();
        Ok(())
    }
    fn home_dir(&self) -> Option<String> {
        Some(self.bridge.files.join("imports").to_string_lossy().into_owned())
    }
}
pub fn hooks(bridge: Bridge, ctx: &egui::Context) -> filmcraft_ui_egui::HostHooks {
    let ctx = ctx.clone();
    filmcraft_ui_egui::HostHooks {
        run_dialog: Some(Box::new(move |dialog| {
            let (tx, rx) = std::sync::mpsc::channel();
            let bridge = bridge.clone();
            let ctx = ctx.clone();
            let sender = tx.clone();
            if let Err(e) = std::thread::Builder::new().name("Android document picker".into()).spawn(move || {
                let answer = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.pick(dialog))) {
                    Ok(Ok(paths)) => paths,
                    Ok(Err(e)) => {
                        bridge.notice(&e);
                        Vec::new()
                    }
                    Err(_) => {
                        bridge.notice("Android document picker failed");
                        Vec::new()
                    }
                };
                let _ = sender.send(answer);
                ctx.request_repaint();
            }) {
                log::error!("Cannot start picker: {e}");
                let _ = tx.send(Vec::new());
            }
            rx
        })),
        ..Default::default()
    }
}
pub fn shared(bridge: Bridge, ctx: &egui::Context) -> Arc<dyn Services> {
    Arc::new(AndroidServices { bridge, ctx: ctx.clone() })
}
