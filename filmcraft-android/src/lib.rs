//! Native Android host for FilmCraft's Rust editor and video engine.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod bridge;
#[cfg(target_os = "android")]
mod control;
#[cfg(any(target_os = "android", test))]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod safe_area;
#[cfg(target_os = "android")]
mod services;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info));
    let Some(files) = app.internal_data_path() else {
        log::error!("Android data directory unavailable");
        return;
    };
    let bridge = match bridge::Bridge::new(&files) {
        Ok(b) => b,
        Err(e) => {
            log::error!("{e}");
            return;
        }
    };
    filmcraft_ui_egui::crash::install(Some(files.join("Logs")));
    let mut options = eframe::NativeOptions {
        android_app: Some(app),
        persistence_path: Some(files.join("ui.ron")),
        viewport: egui::ViewportBuilder::default().with_app_id("app.filmcraft.android").with_title("FilmCraft"),
        ..Default::default()
    };
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup {
        create.device_descriptor = std::sync::Arc::new(|adapter| eframe::wgpu::DeviceDescriptor {
            label: Some("FilmCraft Android renderer"),
            required_limits: adapter.limits(),
            ..Default::default()
        });
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        eframe::run_native(
            "FilmCraft",
            options,
            Box::new(move |cc| {
                let mut session = filmcraft_engine::Session::new(services::shared(bridge.clone(), &cc.egui_ctx));
                session.start_autosave(filmcraft_engine::autosave::AutosaveConfig::new(&files))?;
                if session.recovery_candidates().is_empty()
                    && let Ok(path) = std::fs::read_to_string(files.join("last-project.txt"))
                    && std::path::Path::new(&path).is_file()
                    && let Err(e) = session.execute("file.open", serde_json::json!({"path":path}))
                {
                    log::warn!("Cannot reopen last project: {e}");
                }
                let mut editor = filmcraft_ui_egui::FilmcraftApp::new(session);
                editor.mobile_host = true;
                if let Ok(bytes) = std::fs::read(files.join("ui-state.json"))
                    && let Ok(state) = serde_json::from_slice(&bytes)
                {
                    editor.ui = state;
                }
                editor.ui.program.res = filmcraft_ui_egui::state::PlaybackRes::Quarter;
                editor.frames.set_cache_budget(96 * 1024 * 1024);
                editor.hooks = services::hooks(bridge, &cc.egui_ctx);
                editor.audio = Some(Box::new(AndroidAudio::default()));
                if let Some(render) = cc.wgpu_render_state.clone() {
                    editor.set_wgpu(render);
                }
                if let Some(rx) = control::start(&files, &cc.egui_ctx) {
                    editor = editor.with_control(rx);
                }
                let safe_area = safe_area::SafeArea::new(files.join("bridge/safe-area.json"));
                Ok(Box::new(AndroidEditor { editor, files, safe_area, last_save: std::time::Instant::now(), saved_ui: Vec::new() }))
            }),
        )
    }));
    if !matches!(result, Ok(Ok(()))) {
        log::error!("FilmCraft could not run: {result:?}");
    }
}

#[cfg(target_os = "android")]
#[derive(Default)]
struct AndroidAudio(filmcraft_platform::android_audio::Output);
#[cfg(target_os = "android")]
impl filmcraft_ui_egui::AudioOut for AndroidAudio {
    fn start(&mut self, fill: filmcraft_platform::android_audio::Fill) -> Result<u32, String> {
        self.0.start(fill)
    }
    fn stop(&mut self) {
        self.0.stop();
    }
    fn sample_rate(&self) -> u32 {
        self.0.rate()
    }
    fn played_frames(&self) -> Option<u64> {
        self.0.played()
    }
}
#[cfg(target_os = "android")]
struct AndroidEditor {
    editor: filmcraft_ui_egui::FilmcraftApp,
    files: std::path::PathBuf,
    safe_area: safe_area::SafeArea,
    last_save: std::time::Instant,
    saved_ui: Vec<u8>,
}
#[cfg(target_os = "android")]
impl AndroidEditor {
    fn persist(&mut self) {
        if let Ok(bytes) = serde_json::to_vec(&self.editor.ui)
            && bytes != self.saved_ui
            && bridge::atomic_write(&self.files.join("ui-state.json"), &bytes).is_ok()
        {
            self.saved_ui = bytes;
        }
        if let Some(path) = &self.editor.session.path {
            let _ = bridge::atomic_write(&self.files.join("last-project.txt"), path.as_bytes());
        }
        self.editor.session.sync_persistence();
    }
}
#[cfg(target_os = "android")]
impl eframe::App for AndroidEditor {
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        self.safe_area.apply(ctx, input);
        if !input.focused && self.editor.playback.playing {
            self.editor.stop();
            self.persist();
        }
        self.editor.raw_input_hook(ctx, input);
    }
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.editor.logic(ctx, frame);
        let notice = self.files.join("bridge/notice.json");
        if let Ok(message) = std::fs::read_to_string(&notice) {
            let _ = std::fs::remove_file(notice);
            self.editor.status(message);
        }
        if self.last_save.elapsed() >= std::time::Duration::from_secs(2) {
            self.persist();
            self.last_save = std::time::Instant::now();
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.safe_area.prepare_ui(ui.ctx());
        let rect = ui.ctx().content_rect();
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        child.set_clip_rect(rect);
        self.editor.ui(&mut child, frame);
    }
    fn on_exit(&mut self) {
        self.persist();
        self.editor.stop();
        self.editor.on_exit();
    }
}
