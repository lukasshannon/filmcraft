//! Local control padding for curved screen corners.
//! Backgrounds and the canvas still extend to the display edges.
use egui::{Context, Pos2, Rect, Ui};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corner {
    pub center: Pos2,
    pub radius: f32,
    /// Android order: top left, top right, bottom right, bottom left.
    pub position: u8,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geometry {
    pub corners: Vec<Corner>,
}

fn key() -> egui::Id {
    egui::Id::new("display-edge-geometry")
}

pub fn set(ctx: &Context, geometry: Geometry) {
    ctx.data_mut(|data| data.insert_temp(key(), geometry));
}

fn geometry(ctx: &Context) -> Geometry {
    ctx.data(|data| data.get_temp(key())).unwrap_or_default()
}

fn circle_inset(radius: f32, distance: f32) -> f32 {
    (radius * radius - distance * distance).max(0.0).sqrt()
}

impl Geometry {
    /// Curves only pad the ends of control strips that cross a screen corner.
    fn clear_strip(&self, bounds: Rect, horizontal: bool) -> Rect {
        let mut clear = bounds;
        for corner in &self.corners {
            if !corner.center.is_finite() || !corner.radius.is_finite() || corner.radius <= 0.0 || corner.position > 3 {
                continue;
            }
            let left = matches!(corner.position, 0 | 3);
            let top = corner.position < 2;
            if horizontal {
                let y = if top { bounds.top() } else { bounds.bottom() };
                let distance = (y - corner.center.y).abs();
                if (top && y < corner.center.y) || (!top && y > corner.center.y) {
                    let reach = circle_inset(corner.radius, distance.min(corner.radius));
                    if left {
                        clear.min.x = clear.min.x.max(corner.center.x - reach);
                    } else {
                        clear.max.x = clear.max.x.min(corner.center.x + reach);
                    }
                }
            } else {
                let x = if left { bounds.left() } else { bounds.right() };
                let distance = (x - corner.center.x).abs();
                if (left && x < corner.center.x) || (!left && x > corner.center.x) {
                    let reach = circle_inset(corner.radius, distance.min(corner.radius));
                    if top {
                        clear.min.y = clear.min.y.max(corner.center.y - reach);
                    } else {
                        clear.max.y = clear.max.y.min(corner.center.y + reach);
                    }
                }
            }
        }
        clear.intersect(bounds)
    }
}

fn strip<R>(ui: &mut Ui, horizontal: bool, contents: impl FnOnce(&mut Ui) -> R) -> R {
    let clear = geometry(ui.ctx()).clear_strip(ui.max_rect(), horizontal);
    if clear == ui.max_rect() {
        return contents(ui);
    }
    ui.scope_builder(egui::UiBuilder::new().max_rect(clear), |ui| {
        let mut clip = ui.clip_rect();
        if horizontal {
            clip.min.x = clip.min.x.max(clear.left());
            clip.max.x = clip.max.x.min(clear.right());
        } else {
            clip.min.y = clip.min.y.max(clear.top());
            clip.max.y = clip.max.y.min(clear.bottom());
        }
        ui.set_clip_rect(clip);
        contents(ui)
    })
    .inner
}

pub(crate) fn row<R>(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> R {
    strip(ui, true, contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    fn phone() -> Geometry {
        Geometry {
            corners: vec![
                Corner { center: pos2(40.0, 40.0), radius: 40.0, position: 0 },
                Corner { center: pos2(320.0, 40.0), radius: 40.0, position: 1 },
                Corner { center: pos2(320.0, 760.0), radius: 40.0, position: 2 },
                Corner { center: pos2(40.0, 760.0), radius: 40.0, position: 3 },
            ],
        }
    }

    #[test]
    fn only_control_strips_at_the_corners_receive_padding() {
        let geometry = phone();
        let top = Rect::from_min_size(Pos2::ZERO, vec2(360.0, 36.0));
        let clear = geometry.clear_strip(top, true);
        assert_eq!((clear.top(), clear.bottom()), (0.0, 36.0));
        assert_eq!((clear.left(), clear.right()), (40.0, 320.0));
        let middle = top.translate(vec2(0.0, 100.0));
        assert_eq!(geometry.clear_strip(middle, true), middle);
        let bottom = top.translate(vec2(0.0, 764.0));
        assert_eq!(geometry.clear_strip(bottom, true).top(), 764.0);
        let toolbar = Rect::from_min_max(pos2(0.0, 80.0), pos2(40.0, 776.0));
        let clear = geometry.clear_strip(toolbar, false);
        assert_eq!(clear.left(), 0.0);
        assert_eq!(clear.top(), 80.0);
        assert_eq!(clear.bottom(), 760.0);
    }

    #[test]
    fn the_menu_row_keeps_its_full_width_between_the_corners() {
        let geometry = phone();
        let top = Rect::from_min_size(Pos2::ZERO, vec2(360.0, 36.0));
        let clear = geometry.clear_strip(top, true);
        assert_eq!((clear.left(), clear.right()), (40.0, 320.0));
        assert!(clear.contains(pos2(180.0, 10.0)));
        let options = top.translate(vec2(0.0, 40.0));
        assert_eq!(geometry.clear_strip(options, true), options);
    }

    #[test]
    fn corner_clearance_keeps_bars_at_the_edges_and_pads_their_controls() {
        use egui_kittest::kittest::Queryable;
        let mut h = egui_kittest::Harness::builder().with_size(vec2(360.0, 800.0)).build_ui(|ui| {
            set(ui.ctx(), phone());
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("original"), ui.max_rect()));
            egui::Panel::top("top").exact_size(36.0).frame(egui::Frame::NONE).show(ui, |ui| {
                row(ui, |ui| {
                    let _ = ui.button("File");
                })
            });
            egui::Panel::bottom("bottom").exact_size(24.0).frame(egui::Frame::NONE).show(ui, |ui| {
                row(ui, |ui| {
                    let _ = ui.button("Zoom");
                })
            });
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("remaining"), ui.available_rect_before_wrap()));
        });
        h.run_steps(3);
        assert_eq!(h.ctx.content_rect(), Rect::from_min_size(Pos2::ZERO, vec2(360.0, 800.0)));
        // Panel separator strokes can move the content by a fraction of a point.
        // Test the curve itself instead of requiring a rounded integer margin.
        let file = h.get_by_label("File").rect().left_top();
        let zoom = h.get_by_label("Zoom").rect().left_bottom();
        assert!(file.x > 0.0 && (file - pos2(40.0, 40.0)).length_sq() <= 1600.5, "File: {file:?}");
        assert!(zoom.x > 0.0 && (zoom - pos2(40.0, 760.0)).length_sq() <= 1600.5, "Zoom: {zoom:?}");
        let canvas: Rect = h.ctx.data(|d| d.get_temp(egui::Id::new("remaining"))).unwrap();
        // The test harness supplies its own frame; the edge helper must not add
        // any margins to that original content area.
        let original: Rect = h.ctx.data(|d| d.get_temp(egui::Id::new("original"))).unwrap();
        assert_eq!((canvas.left(), canvas.right()), (original.left(), original.right()));
        assert!(canvas.top() <= original.top() + 37.0 && canvas.bottom() >= original.bottom() - 25.0, "canvas: {canvas:?}");
    }
}
