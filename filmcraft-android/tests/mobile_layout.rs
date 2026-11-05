use filmcraft_ui_egui::mobile::panel_rects;
#[test]
fn both_orientations_keep_preview_and_panel_inside_viewport() {
    for (w, h) in [(380.0, 853.0), (853.0, 380.0), (320.0, 568.0), (568.0, 320.0)] {
        let body = egui::Rect::from_min_max(egui::pos2(0.0, 44.0), egui::pos2(w, h - 66.0));
        let (preview, panel) = panel_rects(body);
        assert!(body.contains_rect(preview) && body.contains_rect(panel));
        assert!(preview.width() > 100.0 && preview.height() > 70.0);
        assert!(panel.width() > 100.0 && panel.height() > 70.0);
        assert!(!preview.intersects(panel));
    }
}
