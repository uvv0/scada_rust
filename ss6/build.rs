use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-changed=src/web.rs");
    println!("cargo:rerun-if-changed=assets/app.css");
    println!("cargo:rerun-if-changed=assets/app.js");
    println!("cargo:rerun-if-changed=assets/api.js");
    println!("cargo:rerun-if-changed=assets/chart_controller.js");
    println!("cargo:rerun-if-changed=assets/meta_state.js");
    println!("cargo:rerun-if-changed=assets/preview_scale.js");
    println!("cargo:rerun-if-changed=assets/preview_poll.js");
    println!("cargo:rerun-if-changed=assets/preview_modals.js");
    println!("cargo:rerun-if-changed=assets/preview_scene.js");

    let build_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string());

    println!("cargo:rustc-env=SS6_BUILD_ID={build_id}");
}
