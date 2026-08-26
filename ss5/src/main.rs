#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod db;
mod modbus;
mod modbus_service;
mod models;
mod script;
mod ui;
mod utils;

use app::Ss5App;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

fn exe_neighbor(name: &str) -> PathBuf {
    let mut path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("ss5.exe"));
    path.set_file_name(name);
    path
}

fn append_log(file_name: &str, msg: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(exe_neighbor(file_name)) {
        let _ = writeln!(f, "{msg}");
    }
}

struct InitErrorApp {
    message: String,
}

impl eframe::App for InitErrorApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("ss5 не смог запуститься");
            ui.add_space(8.0);
            ui.label("Ошибка произошла до открытия основного интерфейса.");
            ui.add_space(8.0);
            ui.add(
                eframe::egui::TextEdit::multiline(&mut self.message)
                    .font(eframe::egui::TextStyle::Monospace)
                    .desired_rows(14)
                    .desired_width(f32::INFINITY)
                    .interactive(false),
            );
            ui.add_space(8.0);
            ui.label("Проверьте PostgreSQL и файл ss5.toml рядом с ss5.exe.");
        });
    }
}

/// Function: $name.
fn main() -> eframe::Result<()> {
    std::panic::set_hook(Box::new(|info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            *s
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.as_str()
        } else {
            "<non-string panic payload>"
        };
        append_log("ss5_panic.log", &format!("panic at {}: {}", loc, msg));
    }));

    let app = match Ss5App::try_new() {
        Ok(v) => v,
        Err(e) => {
            let msg = format!("ss5 init error: {e:?}");
            append_log("ss5_init_error.log", &msg);
            eprintln!("ss5 init error: {e}");
            let options = eframe::NativeOptions {
                viewport: eframe::egui::ViewportBuilder::default()
                    .with_inner_size([760.0, 360.0])
                    .with_min_inner_size([620.0, 280.0]),
                ..Default::default()
            };
            return eframe::run_native(
                "ss5 init error",
                options,
                Box::new(move |_| Ok(Box::new(InitErrorApp { message: msg }))),
            );
        }
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1460.0, 860.0])
            .with_min_inner_size([1180.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "ss5",
        options,
        Box::new(move |_| Ok(Box::new(app))),
    )
}
