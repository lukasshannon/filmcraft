//! Phone arrangement of the existing editor panels and commands.
use crate::{FilmcraftApp, dock::PanelKind, native_dialogs::FileDialog, panels, screen_edges};
use egui::{Rect, Ui, pos2, vec2};
use serde_json::json;

fn button(app: &mut FilmcraftApp, ui: &mut Ui, id: &str, label: &str) -> bool {
    let response = ui.add_sized([0.0, 36.0], egui::Button::new(label));
    app.auto.add_response(id, &response, label);
    response.clicked()
}
fn invoke(app: &mut FilmcraftApp, ctx: &egui::Context, id: &str) {
    if let Err(error) = crate::menus::invoke(app, ctx, id, json!({})) {
        app.status(error);
    }
}
fn child(ui: &mut Ui, id: &str, rect: Rect, contents: impl FnOnce(&mut Ui)) {
    let mut region = ui.new_child(egui::UiBuilder::new().id_salt(id).max_rect(rect));
    region.set_clip_rect(rect.intersect(ui.clip_rect()));
    contents(&mut region);
}
pub fn show(app: &mut FilmcraftApp, ui: &mut Ui, full: Rect) {
    let ctx = ui.ctx().clone();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("mobile-host"), true));
    let mut panel =
        app.pending_panel_raise.take().or_else(|| ctx.data(|d| d.get_temp::<PanelKind>(egui::Id::new("mobile-panel")))).unwrap_or(PanelKind::Project);
    // Only the row ends receive curved-glass padding. Preview backgrounds retain
    // the complete viewport, and rotations use the same project and panel state.
    let header = Rect::from_min_max(full.min, pos2(full.right(), (full.top() + 44.0).min(full.bottom())));
    ui.painter().rect_filled(header, 0.0, app.tokens.header_bg);
    child(ui, "phone-header", header, |ui| {
        screen_edges::row(ui, |ui| {
            ui.horizontal(|ui| {
                if button(app, ui, "mobile.menu", "Menu") {
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("mobile-menu"), true));
                }
                if button(app, ui, "mobile.import", "+ Media") {
                    invoke(app, &ctx, "file.import");
                }
                if button(app, ui, "mobile.save", "Save") {
                    invoke(app, &ctx, "file.save");
                }
                if button(app, ui, "mobile.export", "Export") {
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("mobile-export"), true));
                }
            });
        })
    });
    let bottom = Rect::from_min_max(pos2(full.left(), (full.bottom() - 44.0).max(header.bottom())), full.max);
    ui.painter().rect_filled(bottom, 0.0, app.tokens.header_bg);
    child(ui, "phone-tabs", bottom, |ui| {
        screen_edges::row(ui, |ui| {
            egui::ScrollArea::horizontal().id_salt("mobile-tabs-scroll").show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (kind, label) in
                        [(PanelKind::Project, "Media"), (PanelKind::Timeline, "Timeline"), (PanelKind::Properties, "Edit"), (PanelKind::Effects, "Effects")]
                    {
                        let response = ui.add_sized([0.0, 36.0], egui::Button::selectable(panel == kind, label));
                        app.auto.add_response(&format!("mobile.panel.{kind:?}"), &response, label);
                        if response.clicked() {
                            panel = kind;
                            app.ui.focused = kind;
                        }
                    }
                    let response = ui.menu_button("More", |ui| {
                        egui::ScrollArea::vertical().max_height((full.height() - 110.0).max(60.0)).show(ui, |ui| {
                            for kind in PanelKind::ALL {
                                if button(app, ui, &format!("mobile.more.{kind:?}"), kind.title()) {
                                    panel = kind;
                                    app.ui.focused = kind;
                                    ui.close();
                                }
                            }
                        });
                    });
                    app.auto.add_response("mobile.more", &response.response, "More panels");
                });
            });
        })
    });
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("mobile-panel"), panel));
    let status = Rect::from_min_max(pos2(full.left(), (bottom.top() - 22.0).max(header.bottom())), bottom.left_top() + vec2(full.width(), 0.0));
    child(ui, "phone-status", status, |ui| {
        screen_edges::row(ui, |ui| {
            let text = if app.ui.status.is_empty() { app.session.project.name.clone() } else { app.ui.status.clone() };
            ui.add(egui::Label::new(text).truncate());
            let tokens = app.tokens;
            app.job_status(ui, status, &tokens);
        })
    });
    let body = Rect::from_min_max(pos2(full.left(), header.bottom()), pos2(full.right(), status.top()));
    let (preview, pane) = panel_rects(body);
    panels::show(app, ui, PanelKind::Program, preview);
    let title = Rect::from_min_size(pane.min, vec2(pane.width(), 40.0_f32.min(pane.height())));
    child(ui, "phone-panel-title", title, |ui| {
        ui.horizontal(|ui| {
            ui.label(panel.title());
            if panel == PanelKind::Project && button(app, ui, "mobile.useClip", "Make sequence") {
                invoke(app, &ctx, "file.newSequenceFromClip");
            }
            if panel == PanelKind::Timeline {
                if button(app, ui, "mobile.undo", "Undo") {
                    invoke(app, &ctx, "edit.undo");
                }
                if button(app, ui, "mobile.razor", "Split") {
                    invoke(app, &ctx, "sequence.addEdit");
                }
            }
        });
    });
    let pane_body = Rect::from_min_max(pos2(pane.left(), title.bottom()), pane.max);
    if pane_body.height() > 1.0 {
        panels::show(app, ui, panel, pane_body);
    }
    menu(app, &ctx, full);
    export(app, &ctx, full);
}
pub fn panel_rects(body: Rect) -> (Rect, Rect) {
    if body.width() < body.height() {
        let y = body.top() + body.height() * 0.39;
        (Rect::from_min_max(body.min, pos2(body.right(), y)), Rect::from_min_max(pos2(body.left(), y + 2.0), body.max))
    } else {
        let x = body.left() + body.width() * 0.47;
        (Rect::from_min_max(body.min, pos2(x, body.bottom())), Rect::from_min_max(pos2(x + 2.0, body.top()), body.max))
    }
}
fn menu(app: &mut FilmcraftApp, ctx: &egui::Context, full: Rect) {
    let key = egui::Id::new("mobile-menu");
    if !ctx.data(|d| d.get_temp::<bool>(key)).unwrap_or(false) {
        return;
    }
    let mut open = true;
    let mut search = ctx.data(|d| d.get_temp::<String>(key.with("search"))).unwrap_or_default();
    let mut command = None;
    egui::Window::new("FilmCraft menu")
        .id(key)
        .collapsible(false)
        .resizable(false)
        .default_width((full.width() - 40.0).clamp(160.0, 430.0))
        .max_width((full.width() - 32.0).max(160.0))
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            let response = ui.add(egui::TextEdit::singleline(&mut search).hint_text("Find a command").desired_width(ui.available_width()));
            app.auto.add_response("mobile.menu.search", &response, "Find a command");
            egui::ScrollArea::vertical().max_height((full.height() - 140.0).max(60.0)).show(ui, |ui| {
                for item in crate::menus::menu_items(app) {
                    let label = format!("{}  /  {}", item.path.join(" / "), item.label);
                    if !search.is_empty() && !label.to_lowercase().contains(&search.to_lowercase()) {
                        continue;
                    }
                    let r = ui.add_enabled(item.enabled, egui::Button::new(label).wrap());
                    app.auto.add_response(&format!("mobile.command.{}", item.id), &r, &item.label);
                    if r.clicked() {
                        command = Some(item.id);
                    }
                }
            });
            if button(app, ui, "mobile.menu.close", "Close") {
                open = false;
            }
        });
    if let Some(id) = command {
        open = false;
        invoke(app, ctx, &id);
    }
    ctx.data_mut(|d| {
        d.insert_temp(key, open);
        d.insert_temp(key.with("search"), search);
    });
}
fn export(app: &mut FilmcraftApp, ctx: &egui::Context, full: Rect) {
    let key = egui::Id::new("mobile-export");
    if !ctx.data(|d| d.get_temp::<bool>(key)).unwrap_or(false) {
        return;
    }
    let mut open = true;
    let mut format = ctx.data(|d| d.get_temp::<String>(key.with("format"))).unwrap_or_else(|| "mp4".into());
    let mut start = false;
    egui::Window::new("Export video")
        .id(key)
        .collapsible(false)
        .resizable(false)
        .default_width((full.width() - 48.0).clamp(160.0, 400.0))
        .max_width((full.width() - 40.0).max(160.0))
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height((full.height() - 155.0).max(50.0)).show(ui, |ui| {
                ui.label("Export the active sequence at its original resolution.");
                for (value, label) in [("mp4", "H.264 video (.mp4)"), ("wav", "Audio (.wav)")] {
                    let r = ui.radio_value(&mut format, value.into(), label);
                    app.auto.add_response(&format!("mobile.export.{value}"), &r, label);
                }
                ui.label("Choose where to save in the Android picker. Encoding runs in the background.");
            });
            ui.horizontal(|ui| {
                if button(app, ui, "mobile.export.cancel", "Cancel") {
                    open = false;
                }
                if button(app, ui, "mobile.export.start", "Choose file & export") {
                    start = true;
                }
            });
        });
    if start {
        open = false;
        let name = format!("FilmCraft.{format}");
        let selected = format.clone();
        let result = app.pick(FileDialog::save_as("Export", &[&format], &name), move |app, paths| {
            let Some(path) = paths.into_iter().next() else { return Ok(serde_json::Value::Null) };
            let result = app.session.execute("file.exportMedia", json!({"path":path,"format":selected,"range":"entire"})).map_err(|e| e.to_string());
            if result.is_ok() {
                app.status("Exporting…");
            }
            result
        });
        if let Err(error) = result {
            app.status(error);
        }
    }
    ctx.data_mut(|d| {
        d.insert_temp(key, open);
        d.insert_temp(key.with("format"), format);
    });
}
