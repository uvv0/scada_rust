#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod app_io;
mod app_windows;
mod db;
mod db_config;
mod db_schema;
mod modbus;
mod modbus_service;
mod models;
mod theme;
mod ui;

use app::Ss7App;
use theme::apply_im1_visuals;

fn main() -> eframe::Result<()> {
    let app = match Ss7App::try_new() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ss7 init error: {e}");
            return Ok(());
        }
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 840.0])
            .with_min_inner_size([1120.0, 680.0]),
        ..Default::default()
    };

    eframe::run_native(
        "SS7 UI Designer",
        options,
        Box::new(move |cc| {
            apply_im1_visuals(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}
