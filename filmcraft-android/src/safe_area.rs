//! Android reports physical window insets; egui lays out in logical points.
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub(crate) struct SafeArea {
    path: PathBuf,
    checked: Option<Instant>,
    value: serde_json::Value,
}

impl SafeArea {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path, checked: None, value: serde_json::Value::Null }
    }

    fn refresh(&mut self) {
        if self.checked.is_none_or(|last| last.elapsed() >= Duration::from_millis(120)) {
            self.checked = Some(Instant::now());
            if let Ok(bytes) = std::fs::read(&self.path)
                && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
            {
                self.value = value;
            }
        }
    }

    pub(crate) fn apply(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        self.refresh();
        let screen = input.screen_rect.unwrap_or_else(|| ctx.viewport_rect());
        input.safe_area_insets = Some(convert(&self.value, screen.size()));
        ctx.request_repaint_after(Duration::from_millis(120));
    }

    pub(crate) fn prepare_ui(&mut self, ctx: &egui::Context) {
        self.refresh();
        // Publish view data in the UI pass that consumes it, including after a
        // system picker resumes the activity. Input collection can run separately.
        filmcraft_ui_egui::screen_edges::set(ctx, geometry(&self.value, ctx.viewport_rect().size()));
        ctx.request_repaint_after(Duration::from_millis(120));
    }
}

fn convert(value: &serde_json::Value, size: egui::Vec2) -> egui::SafeAreaInsets {
    let number = |key: &str| value.get(key).and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
    let (width, height) = (number("width"), number("height"));
    // Do not reuse landscape insets during the rotation frame before Java publishes portrait.
    if width <= 0.0 || height <= 0.0 || (width / height - size.x / size.y).abs() > 0.05 {
        return egui::SafeAreaInsets::default();
    }
    let inset = |key, scale: f32, limit| (number(key) * scale).max(0.0).min(limit);
    egui::SafeAreaInsets(egui::epaint::MarginF32 { bottom: inset("bottom", size.y / height, size.y * 0.8), ..Default::default() })
}

fn geometry(value: &serde_json::Value, size: egui::Vec2) -> filmcraft_ui_egui::screen_edges::Geometry {
    use filmcraft_ui_egui::screen_edges::{Corner, Geometry};
    let number = |v: &serde_json::Value, key| v.get(key).and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
    let (width, height) = (number(value, "width"), number(value, "height"));
    if width <= 0.0 || height <= 0.0 || (width / height - size.x / size.y).abs() > 0.05 {
        return Geometry::default();
    }
    let (sx, sy) = (size.x / width, size.y / height);
    let mut result = Geometry::default();
    if let Some(corners) = value.get("corners").and_then(serde_json::Value::as_array) {
        for corner in corners.iter().take(4) {
            let Some(position) = corner.get("position").and_then(serde_json::Value::as_u64).filter(|p| *p <= 3) else {
                continue;
            };
            let radius = number(corner, "radius");
            if radius <= 0.0 || radius > width.min(height) * 0.5 {
                continue;
            }
            result.corners.push(Corner {
                center: egui::pos2(number(corner, "x") * sx, number(corner, "y") * sy),
                radius: radius * sx.min(sy),
                position: position as u8,
            });
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_the_keyboard_reserves_a_screen_margin() {
        let value = json!({"width":2400,"height":1080,"left":96,"right":36,"top":36,"bottom":48});
        let safe = convert(&value, egui::vec2(800.0, 360.0)).0;
        assert_eq!(safe, egui::epaint::MarginF32 { bottom: 16.0, ..Default::default() });
    }

    #[test]
    fn rotation_discards_stale_insets_and_portrait_keyboard_is_respected() {
        let landscape = json!({"width":2400,"height":1080,"left":96});
        assert_eq!(convert(&landscape, egui::vec2(360.0, 800.0)).0, egui::epaint::MarginF32::ZERO);
        let portrait = json!({"width":1080,"height":2400,"top":90,"bottom":900});
        let safe = convert(&portrait, egui::vec2(360.0, 800.0)).0;
        assert_eq!(safe.top, 0.0);
        assert_eq!(safe.bottom, 300.0);
    }

    #[test]
    fn corners_stay_local_instead_of_shrinking_the_editor() {
        let value = json!({"width":1080,"height":2400,"bottom":0,
            "corners":[{"position":0,"radius":120,"x":120,"y":120}], "cutouts":[[480,0,600,84]]});
        let size = egui::vec2(360.0, 800.0);
        assert_eq!(convert(&value, size).0, egui::epaint::MarginF32::ZERO);
        let local = geometry(&value, size);
        assert_eq!(local.corners[0].radius, 40.0);
        assert_eq!(local.corners[0].center, egui::pos2(40.0, 40.0));
        // Stale build-3 cutout fields have no effect on the screen or menu geometry.
        let corners_only = json!({"width":1080,"height":2400,"bottom":0,
            "corners":value.get("corners")});
        assert_eq!(local, geometry(&corners_only, size));
        assert!(geometry(&value, egui::vec2(800.0, 360.0)).corners.is_empty());
    }
}
