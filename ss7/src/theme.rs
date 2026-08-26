use eframe::egui;

pub fn apply_im1_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(egui::Color32::from_rgb(205, 248, 234));
    visuals.panel_fill = egui::Color32::from_rgb(6, 10, 16);
    visuals.window_fill = egui::Color32::from_rgb(9, 15, 24);
    visuals.extreme_bg_color = egui::Color32::from_rgb(4, 8, 13);
    visuals.faint_bg_color = egui::Color32::from_rgb(13, 21, 34);
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(10, 16, 26);
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(34, 74, 112));
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(12, 24, 38);
    visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 102, 151));
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(18, 44, 69);
    visuals.widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 188, 226));
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(10, 62, 88);
    visuals.widgets.active.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(44, 217, 184));
    visuals.selection.bg_fill = egui::Color32::from_rgb(24, 128, 112);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(66, 230, 200));
    visuals.hyperlink_color = egui::Color32::from_rgb(80, 206, 255);
    visuals.warn_fg_color = egui::Color32::from_rgb(255, 201, 94);
    visuals.error_fg_color = egui::Color32::from_rgb(255, 111, 122);
    ctx.set_visuals(visuals);

    ctx.style_mut(|s| {
        s.spacing.scroll = egui::style::ScrollStyle::solid();
    });
}
