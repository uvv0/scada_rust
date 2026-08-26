use std::collections::BTreeSet;

use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::models::UiWindowBindingRow;

#[derive(Clone, Debug)]
pub(super) struct WebSafeWarningItem {
    pub reg_id: i32,
    pub message: String,
    pub labels: bool,
    pub size: bool,
    pub trend: bool,
    pub write: bool,
}

pub(super) struct WebSafeSummary {
    pub label: &'static str,
    pub fill: egui::Color32,
    pub stroke: egui::Color32,
    pub detail: String,
    pub labels: usize,
    pub size: usize,
    pub trend: usize,
    pub write: usize,
    pub breakdown: String,
    pub fixes: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct WebSafeProfile {
    pub version: u32,
    pub scope: String,
    pub muted_reg_ids: Vec<i32>,
    pub show_muted: bool,
    pub filter_labels: bool,
    pub filter_size: bool,
    pub filter_trend: bool,
    pub filter_write: bool,
}

pub(super) struct WebSafeWarningRow<'a> {
    pub message: &'a str,
    pub labels: bool,
    pub size: bool,
    pub trend: bool,
    pub write: bool,
    pub selected: bool,
    pub muted: bool,
}

pub(super) fn component_kind(binding: &UiWindowBindingRow) -> &str {
    binding.component_kind.as_deref().unwrap_or("auto")
}

pub(super) fn display_label(binding: &UiWindowBindingRow) -> &str {
    binding
        .label_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&binding.reg_name)
}

fn is_image_item(binding: &UiWindowBindingRow) -> bool {
    (binding.is_text || binding.reg_id <= 0) && component_kind(binding) == "image"
}

fn looks_like_absolute_path(path: &str) -> bool {
    path.starts_with('/') || path.starts_with('\\') || path.as_bytes().get(1) == Some(&b':')
}

pub(super) fn uses_external_label(binding: &UiWindowBindingRow) -> bool {
    if binding.is_text || binding.reg_id <= 0 {
        return false;
    }
    let is_tu = binding.reg_n_mb == 1 || binding.reg_tip == 1;
    let is_bool = binding.reg_tip == 0 && binding.reg_bits.is_some();
    if is_tu || is_bool {
        return false;
    }
    // Match ss6 web behavior: KP4 tit_ustavki has narrow min/max cells that still need left labels.
    matches!(
        component_kind(binding),
        "auto" | "numeric" | "setpoint" | "bar" | "gauge" | "trend"
    )
}

pub(super) fn prefers_internal_label(binding: &UiWindowBindingRow) -> bool {
    if binding.is_text || binding.reg_id <= 0 {
        return false;
    }
    if uses_external_label(binding) {
        return false;
    }
    match component_kind(binding) {
        "button" | "led" | "numeric" | "setpoint" | "bar" | "gauge" | "trend" => true,
        "auto" => true,
        _ => false,
    }
}

fn text_load(binding: &UiWindowBindingRow) -> usize {
    let label_len = display_label(binding).chars().count();
    let unit_len = binding
        .unit
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().count())
        .unwrap_or(0);
    let fmt_weight = binding
        .fmt
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|fmt| fmt.len().max(4))
        .unwrap_or(0);
    label_len + unit_len + fmt_weight
}

pub(super) fn issues_for_binding(binding: &UiWindowBindingRow) -> Vec<String> {
    if !binding.visible {
        return Vec::new();
    }

    let kind = component_kind(binding);
    let label = display_label(binding);
    let mut issues: Vec<String> = Vec::new();
    let text_load = text_load(binding);

    if is_image_item(binding) {
        let path = label.trim().replace('\\', "/");
        if path.is_empty() {
            issues.push("image path is empty".to_string());
        }
        if looks_like_absolute_path(&path) {
            issues.push("image path is absolute; web expects ui_images/name.png".to_string());
        }
        if path.split('/').any(|part| part == "..") {
            issues.push("image path contains .. and is blocked in web".to_string());
        }
        if binding.w < 160 || binding.h < 100 {
            issues.push("image tile is small for a readable technical scheme".to_string());
        }
        return issues;
    }

    let (min_w, min_h) = match kind {
        "button" | "numeric" | "setpoint" => (72, 32),
        "led" => (24, 24),
        "bar" => (80, 20),
        "gauge" | "trend" => (96, 48),
        _ => (0, 0),
    };
    if min_w > 0 && (binding.w < min_w || binding.h < min_h) {
        issues.push(format!("size {}x{} < {}x{}", binding.w, binding.h, min_w, min_h));
    }

    if binding.writable && (binding.w < 72 || binding.h < 32) {
        issues.push("small click target for web".to_string());
    }

    let external_label = uses_external_label(binding);
    if external_label && binding.x < 90 {
        issues.push("external label may clip near left edge".to_string());
    }
    if external_label && label.chars().count() > 24 {
        issues.push("external label will likely truncate in web".to_string());
    }
    if !external_label && !binding.is_text && label.chars().count() > 18 && binding.w < 110 {
        issues.push("long label may clip inside tile".to_string());
    }
    if prefers_internal_label(binding) && label.chars().count() > 12 && binding.h < 34 {
        issues.push("internal label cramped for web card header".to_string());
    }

    match kind {
        "numeric" | "setpoint" | "auto" => {
            if text_load > 16 && binding.w < 120 {
                issues.push("value + unit text likely crowded".to_string());
            }
            if binding.w < 96 {
                issues.push("narrow numeric card for web readability".to_string());
            }
        }
        "button" => {
            if binding.writable && binding.w < 88 {
                issues.push("write button may feel cramped on web".to_string());
            }
        }
        "bar" => {
            if binding.h < 28 {
                issues.push("bar too short for readable value overlay".to_string());
            }
            if binding.w < 96 {
                issues.push("bar too narrow for web label/value balance".to_string());
            }
        }
        "gauge" => {
            if binding.w < 120 || binding.h < 70 {
                issues.push("gauge face/value likely cramped in web".to_string());
            }
        }
        "trend" => {
            if binding.w < 120 || binding.h < 60 {
                issues.push("trend too small for readable history line".to_string());
            }
            if binding.w < 150 {
                issues.push("trend width may look sparse at web scale".to_string());
            }
        }
        _ => {}
    }

    issues
}

pub(super) fn classify_issue(issue: &str) -> &'static str {
    if issue.contains("label") || issue.contains("text") || issue.contains("path") {
        "labels"
    } else if issue.contains("write") || issue.contains("writable") {
        "write"
    } else if issue.contains("trend") || issue.contains("sparse") || issue.contains("history line") {
        "trend"
    } else {
        "size"
    }
}

fn push_fix(hints: &mut Vec<String>, text: &str) {
    if !hints.iter().any(|hint| hint == text) {
        hints.push(text.to_string());
    }
}

pub(super) fn fix_hints_for_binding(binding: &UiWindowBindingRow) -> Vec<String> {
    let issues = issues_for_binding(binding);
    let mut hints = Vec::new();
    for issue in issues {
        if issue.contains("left edge") {
            push_fix(&mut hints, "move from left edge");
        }
        if issue.contains("truncate") || issue.contains("clip") || issue.contains("cramped") {
            push_fix(&mut hints, "shorten label");
        }
        if issue.contains("external label") {
            push_fix(&mut hints, "switch to internal label");
        }
        if issue.contains("size")
            || issue.contains("narrow")
            || issue.contains("small")
            || issue.contains("short")
            || issue.contains("crowded")
            || issue.contains("gauge face")
        {
            push_fix(&mut hints, "make wider");
        }
        if issue.contains("size")
            || issue.contains("small click target")
            || issue.contains("short")
            || issue.contains("cramped")
            || issue.contains("history line")
            || issue.contains("gauge face")
        {
            push_fix(&mut hints, "make taller");
        }
        if issue.contains("sparse") || issue.contains("trend") || issue.contains("history line") {
            push_fix(&mut hints, "grow trend width");
        }
        if issue.contains("write") || issue.contains("writable") || issue.contains("click target") {
            push_fix(&mut hints, "enlarge write target");
        }
        if issue.contains("value + unit text") {
            push_fix(&mut hints, "shorten unit or format");
        }
        if issue.contains("image path") {
            push_fix(&mut hints, "use relative ui_images/name.png");
        }
    }
    hints
}

fn warning_item_for_binding(binding: &UiWindowBindingRow) -> Option<WebSafeWarningItem> {
    let kind = component_kind(binding);
    let issues = issues_for_binding(binding);

    if issues.is_empty() {
        return None;
    }

    let id_text = if is_image_item(binding) {
        "IMAGE".to_string()
    } else if binding.is_text || binding.reg_id <= 0 {
        "TEXT".to_string()
    } else {
        format!("reg {}", binding.reg_id)
    };
    let mut labels = false;
    let mut size = false;
    let mut trend = false;
    let mut write = false;
    for issue in &issues {
        match classify_issue(issue) {
            "labels" => labels = true,
            "write" => write = true,
            "trend" => trend = true,
            _ => size = true,
        }
    }
    let fixes = fix_hints_for_binding(binding);
    let fix_text = if fixes.is_empty() {
        String::new()
    } else {
        format!(" Fix: {}.", fixes.join(", "))
    };
    Some(WebSafeWarningItem {
        reg_id: binding.reg_id,
        message: format!("{id_text} [{kind}]: {}.{fix_text}", issues.join("; ")),
        labels,
        size,
        trend,
        write,
    })
}

pub(super) fn collect_warning_items(
    bindings: &[UiWindowBindingRow],
    muted_reg_ids: &BTreeSet<i32>,
    want_muted: bool,
) -> Vec<WebSafeWarningItem> {
    bindings
        .iter()
        .filter(|binding| muted_reg_ids.contains(&binding.reg_id) == want_muted)
        .filter_map(warning_item_for_binding)
        .collect()
}

pub(super) fn summarize(bindings: &[UiWindowBindingRow], muted_reg_ids: &BTreeSet<i32>) -> WebSafeSummary {
    let total_visible = bindings.iter().filter(|b| b.visible).count();
    if total_visible == 0 {
        return WebSafeSummary {
            label: "none",
            fill: egui::Color32::from_rgb(18, 25, 38),
            stroke: egui::Color32::from_rgb(50, 64, 85),
            detail: "Web-safe summary: no visible bindings.".to_string(),
            labels: 0,
            size: 0,
            trend: 0,
            write: 0,
            breakdown: String::new(),
            fixes: String::new(),
        };
    }

    let mut warning_bindings = 0_usize;
    let mut severe_count = 0_usize;
    let mut sparse_count = 0_usize;
    let mut label_count = 0_usize;
    let mut size_count = 0_usize;
    let mut trend_count = 0_usize;
    let mut write_count = 0_usize;
    for binding in bindings
        .iter()
        .filter(|b| b.visible && !muted_reg_ids.contains(&b.reg_id))
    {
        let issues = issues_for_binding(binding);
        if issues.is_empty() {
            continue;
        }
        warning_bindings += 1;
        for issue in issues {
            match classify_issue(&issue) {
                "labels" => label_count += 1,
                "write" => write_count += 1,
                "trend" => trend_count += 1,
                _ => size_count += 1,
            }
            if issue.contains("sparse") {
                sparse_count += 1;
            } else {
                severe_count += 1;
            }
        }
    }

    let mut breakdown_parts = Vec::new();
    if label_count > 0 {
        breakdown_parts.push(format!("Labels {label_count}"));
    }
    if size_count > 0 {
        breakdown_parts.push(format!("Size {size_count}"));
    }
    if trend_count > 0 {
        breakdown_parts.push(format!("Trend {trend_count}"));
    }
    if write_count > 0 {
        breakdown_parts.push(format!("Write {write_count}"));
    }
    let breakdown = breakdown_parts.join(", ");
    let mut suggested_fixes = Vec::new();
    if label_count > 0 {
        suggested_fixes.push("shorten labels / switch labels inside");
    }
    if size_count > 0 {
        suggested_fixes.push("make tiles wider or taller");
    }
    if trend_count > 0 {
        suggested_fixes.push("grow trend width");
    }
    if write_count > 0 {
        suggested_fixes.push("enlarge write targets");
    }
    let fixes = suggested_fixes.join("; ");

    if warning_bindings == 0 {
        return WebSafeSummary {
            label: "clean",
            fill: egui::Color32::from_rgb(24, 50, 37),
            stroke: egui::Color32::from_rgb(47, 122, 83),
            detail: format!("Web-safe summary: clean (0/{} bindings flagged).", total_visible),
            labels: 0,
            size: 0,
            trend: 0,
            write: 0,
            breakdown: String::new(),
            fixes: String::new(),
        };
    }

    let all_sparse = severe_count == 0 && sparse_count > 0;
    let mostly_flagged = warning_bindings * 2 >= total_visible.max(1);
    let (label, fill, stroke) = if all_sparse {
        (
            "sparse",
            egui::Color32::from_rgb(58, 46, 21),
            egui::Color32::from_rgb(142, 107, 33),
        )
    } else if mostly_flagged || severe_count >= 3 {
        (
            "risky",
            egui::Color32::from_rgb(61, 30, 37),
            egui::Color32::from_rgb(140, 60, 75),
        )
    } else {
        (
            "partial",
            egui::Color32::from_rgb(58, 46, 21),
            egui::Color32::from_rgb(142, 107, 33),
        )
    };

    WebSafeSummary {
        label,
        fill,
        stroke,
        detail: format!(
            "Web-safe summary: {} ({} / {} bindings flagged).",
            label, warning_bindings, total_visible
        ),
        labels: label_count,
        size: size_count,
        trend: trend_count,
        write: write_count,
        breakdown,
        fixes,
    }
}

fn warning_severity(row: &WebSafeWarningRow<'_>) -> (&'static str, egui::Color32, egui::Color32) {
    let category_count = row.labels as u8 + row.size as u8 + row.trend as u8 + row.write as u8;
    if row.write || category_count >= 3 {
        (
            "HI",
            egui::Color32::from_rgba_unmultiplied(88, 34, 38, 180),
            egui::Color32::from_rgb(232, 162, 168),
        )
    } else if row.size || row.labels {
        (
            "MD",
            egui::Color32::from_rgba_unmultiplied(88, 64, 28, 168),
            egui::Color32::from_rgb(235, 201, 146),
        )
    } else {
        (
            "LO",
            egui::Color32::from_rgba_unmultiplied(46, 66, 72, 160),
            egui::Color32::from_rgb(166, 208, 221),
        )
    }
}

fn warning_chip(
    ui: &mut egui::Ui,
    text: &str,
    fill: egui::Color32,
    stroke: egui::Color32,
    text_color: egui::Color32,
) {
    egui::Frame::none()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke))
        .rounding(egui::Rounding::same(5.0))
        .inner_margin(egui::Margin::symmetric(5.0, 2.0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .size(10.0)
                    .color(text_color)
                    .strong(),
            );
        });
}

pub(super) fn render_warning_row(ui: &mut egui::Ui, row: WebSafeWarningRow<'_>) -> egui::Response {
    let (severity, sev_fill_base, sev_text) = warning_severity(&row);
    let sev_fill = if row.muted {
        egui::Color32::from_rgba_unmultiplied(
            sev_fill_base.r(),
            sev_fill_base.g(),
            sev_fill_base.b(),
            sev_fill_base.a() / 2,
        )
    } else {
        sev_fill_base
    };
    let sev_stroke = if row.muted {
        egui::Color32::from_rgb(108, 118, 130)
    } else {
        sev_text
    };
    let msg_color = if row.muted {
        egui::Color32::from_rgb(154, 168, 184)
    } else {
        egui::Color32::from_rgb(221, 228, 236)
    };

    let inner = ui.horizontal_wrapped(|ui| {
        warning_chip(ui, severity, sev_fill, sev_stroke, sev_text);
        if row.labels {
            warning_chip(
                ui,
                "LBL",
                egui::Color32::from_rgba_unmultiplied(48, 58, 84, if row.muted { 72 } else { 132 }),
                egui::Color32::from_rgb(96, 122, 168),
                egui::Color32::from_rgb(184, 202, 236),
            );
        }
        if row.size {
            warning_chip(
                ui,
                "SIZE",
                egui::Color32::from_rgba_unmultiplied(72, 54, 28, if row.muted { 72 } else { 132 }),
                egui::Color32::from_rgb(156, 122, 82),
                egui::Color32::from_rgb(230, 204, 166),
            );
        }
        if row.trend {
            warning_chip(
                ui,
                "TRND",
                egui::Color32::from_rgba_unmultiplied(28, 70, 72, if row.muted { 72 } else { 132 }),
                egui::Color32::from_rgb(84, 154, 160),
                egui::Color32::from_rgb(178, 222, 225),
            );
        }
        if row.write {
            warning_chip(
                ui,
                "WRITE",
                egui::Color32::from_rgba_unmultiplied(84, 34, 40, if row.muted { 72 } else { 136 }),
                egui::Color32::from_rgb(174, 82, 92),
                egui::Color32::from_rgb(238, 188, 194),
            );
        }
        ui.selectable_label(
            row.selected,
            egui::RichText::new(row.message)
                .size(11.0)
                .color(msg_color),
        )
    });
    inner.inner
}

pub(super) fn profile_preview_status(scope: &str, selected_count: Option<usize>) -> String {
    match selected_count {
        Some(selected) => format!(
            "web-safe selection profile preview loaded: {}; selected {}",
            scope, selected
        ),
        None => format!("web-safe profile preview loaded: {}", scope),
    }
}

pub(super) fn profile_export_status(path: &str, selected_count: Option<usize>) -> String {
    match selected_count {
        Some(selected) => format!(
            "web-safe selection profile exported: {}; selected {}",
            path, selected
        ),
        None => format!("web-safe profile exported: {}", path),
    }
}

fn import_suffix_parts(muted_changed: Option<bool>, filters_changed: Option<bool>) -> String {
    let mut parts = Vec::new();
    if muted_changed.unwrap_or(false) {
        parts.push("muted state pending save");
    }
    if let Some(changed) = filters_changed {
        parts.push(if changed {
            "filters updated"
        } else {
            "filters unchanged"
        });
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("; {}", parts.join("; "))
    }
}

pub(super) fn profile_import_status(
    label: &str,
    scope: &str,
    applied: Option<usize>,
    skipped: usize,
    selected_count: Option<usize>,
    muted_changed: Option<bool>,
    filters_changed: Option<bool>,
) -> String {
    let mut text = match applied {
        Some(applied_count) => format!(
            "{}: {}; applied {}, skipped {}",
            label, scope, applied_count, skipped
        ),
        None => format!("{}: {}; skipped {}", label, scope, skipped),
    };
    if let Some(selected) = selected_count {
        text.push_str(&format!("; selected {}", selected));
    }
    text.push_str(&import_suffix_parts(muted_changed, filters_changed));
    text
}

pub(super) fn action_error(action: &str, err: impl std::fmt::Display) -> String {
    format!("{action} failed: {err}")
}

pub(super) fn profiles_dir() -> Result<std::path::PathBuf, String> {
    let dir = std::env::current_dir()
        .map_err(|e| format!("current dir failed: {e}"))?
        .join("web_safe_profiles");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create profile dir failed: {e}"))?;
    Ok(dir)
}

pub(super) fn export_named_profile(profile: &WebSafeProfile) -> Result<String, String> {
    let dir = profiles_dir()?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("clock failed: {e}"))?
        .as_secs();
    let json = serde_json::to_string_pretty(&profile)
        .map_err(|e| format!("serialize profile failed: {e}"))?;
    let named_path = dir.join(format!("{}_{}.json", profile.scope, ts));
    std::fs::write(&named_path, &json).map_err(|e| format!("write profile failed: {e}"))?;
    let latest_path = dir.join("last_profile.json");
    std::fs::write(&latest_path, json).map_err(|e| format!("write latest profile failed: {e}"))?;
    Ok(named_path.display().to_string())
}

pub(super) fn import_profile() -> Result<WebSafeProfile, String> {
    let path = profiles_dir()?.join("last_profile.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read profile failed: {e}"))?;
    serde_json::from_str::<WebSafeProfile>(&text).map_err(|e| format!("parse profile failed: {e}"))
}

pub(super) fn apply_profile(
    bindings: &mut [UiWindowBindingRow],
    muted_reg_ids: &mut BTreeSet<i32>,
    dirty: &mut bool,
    show_muted: &mut bool,
    filter_labels: &mut bool,
    filter_size: &mut bool,
    filter_trend: &mut bool,
    filter_write: &mut bool,
    profile: &WebSafeProfile,
    apply_muted: bool,
    apply_filters: bool,
) -> (usize, usize, bool, bool) {
    let available: BTreeSet<i32> = bindings.iter().map(|b| b.reg_id).collect();
    let target_muted: BTreeSet<i32> = profile
        .muted_reg_ids
        .iter()
        .copied()
        .filter(|reg_id| available.contains(reg_id))
        .collect();
    let skipped = profile
        .muted_reg_ids
        .iter()
        .filter(|reg_id| !available.contains(reg_id))
        .count();

    let previous_muted = muted_reg_ids.clone();
    if apply_muted {
        for binding in bindings {
            binding.web_safe_muted = target_muted.contains(&binding.reg_id);
        }
        *muted_reg_ids = target_muted.clone();
        if previous_muted != target_muted {
            *dirty = true;
        }
    }
    let filters_changed = *show_muted != profile.show_muted
        || *filter_labels != profile.filter_labels
        || *filter_size != profile.filter_size
        || *filter_trend != profile.filter_trend
        || *filter_write != profile.filter_write;
    if apply_filters {
        *show_muted = profile.show_muted;
        *filter_labels = profile.filter_labels;
        *filter_size = profile.filter_size;
        *filter_trend = profile.filter_trend;
        *filter_write = profile.filter_write;
    }

    (
        target_muted.len(),
        skipped,
        apply_muted && previous_muted != target_muted,
        apply_filters && filters_changed,
    )
}

pub(super) fn apply_profile_to_selection(
    bindings: &mut [UiWindowBindingRow],
    selected_ids: &BTreeSet<i32>,
    muted_reg_ids: &mut BTreeSet<i32>,
    dirty: &mut bool,
    profile: &WebSafeProfile,
) -> (usize, usize, bool, Vec<i32>) {
    let selected_available: BTreeSet<i32> = bindings
        .iter()
        .filter(|b| selected_ids.contains(&b.reg_id))
        .map(|b| b.reg_id)
        .collect();
    let target_muted: BTreeSet<i32> = profile
        .muted_reg_ids
        .iter()
        .copied()
        .filter(|reg_id| selected_available.contains(reg_id))
        .collect();
    let skipped = profile
        .muted_reg_ids
        .iter()
        .filter(|reg_id| !selected_available.contains(reg_id))
        .count();

    let previous_muted = muted_reg_ids.clone();
    let mut changed_reg_ids = Vec::new();
    for binding in bindings.iter_mut() {
        if selected_ids.contains(&binding.reg_id) {
            let new_muted = target_muted.contains(&binding.reg_id);
            if binding.web_safe_muted != new_muted {
                changed_reg_ids.push(binding.reg_id);
            }
            binding.web_safe_muted = new_muted;
        }
    }
    *muted_reg_ids = bindings
        .iter()
        .filter(|b| b.web_safe_muted)
        .map(|b| b.reg_id)
        .collect();
    let changed = previous_muted != *muted_reg_ids;
    if changed {
        *dirty = true;
    }
    (target_muted.len(), skipped, changed, changed_reg_ids)
}

pub(super) fn preview_profile_import(
    bindings: &[UiWindowBindingRow],
    current_muted: &BTreeSet<i32>,
    show_muted: bool,
    filter_labels: bool,
    filter_size: bool,
    filter_trend: bool,
    filter_write: bool,
    profile: &WebSafeProfile,
) -> String {
    let available: BTreeSet<i32> = bindings.iter().map(|b| b.reg_id).collect();
    let target_muted: BTreeSet<i32> = profile
        .muted_reg_ids
        .iter()
        .copied()
        .filter(|reg_id| available.contains(reg_id))
        .collect();
    let skipped = profile
        .muted_reg_ids
        .iter()
        .filter(|reg_id| !available.contains(reg_id))
        .count();
    let would_add = target_muted.difference(current_muted).count();
    let would_remove = current_muted.difference(&target_muted).count();
    let unchanged = target_muted.intersection(current_muted).count();

    let mut filter_changes = Vec::new();
    if show_muted != profile.show_muted {
        filter_changes.push(format!("show_muted {}->{}", show_muted, profile.show_muted));
    }
    if filter_labels != profile.filter_labels {
        filter_changes.push(format!("labels {}->{}", filter_labels, profile.filter_labels));
    }
    if filter_size != profile.filter_size {
        filter_changes.push(format!("size {}->{}", filter_size, profile.filter_size));
    }
    if filter_trend != profile.filter_trend {
        filter_changes.push(format!("trend {}->{}", filter_trend, profile.filter_trend));
    }
    if filter_write != profile.filter_write {
        filter_changes.push(format!("write {}->{}", filter_write, profile.filter_write));
    }

    let filter_text = if filter_changes.is_empty() {
        "filters unchanged".to_string()
    } else {
        format!("filter changes: {}", filter_changes.join(", "))
    };

    format!(
        "Profile preview from {}: apply {}, skipped {}, add {}, remove {}, unchanged {}; {}.",
        profile.scope,
        target_muted.len(),
        skipped,
        would_add,
        would_remove,
        unchanged,
        filter_text
    )
}

pub(super) fn preview_profile_diff(
    bindings: &[UiWindowBindingRow],
    current_muted: &BTreeSet<i32>,
    show_muted: bool,
    filter_labels: bool,
    filter_size: bool,
    filter_trend: bool,
    filter_write: bool,
    profile: &WebSafeProfile,
) -> (String, Vec<String>, Vec<String>) {
    let summary = preview_profile_import(
        bindings,
        current_muted,
        show_muted,
        filter_labels,
        filter_size,
        filter_trend,
        filter_write,
        profile,
    );
    let available: BTreeSet<i32> = bindings.iter().map(|b| b.reg_id).collect();
    let target_muted: BTreeSet<i32> = profile
        .muted_reg_ids
        .iter()
        .copied()
        .filter(|reg_id| available.contains(reg_id))
        .collect();

    let mut will_mute = Vec::new();
    let mut will_unmute = Vec::new();
    for binding in bindings {
        let label = format!("reg {} [{}]", binding.reg_id, display_label(binding));
        if target_muted.contains(&binding.reg_id) && !current_muted.contains(&binding.reg_id) {
            will_mute.push(label);
        } else if current_muted.contains(&binding.reg_id) && !target_muted.contains(&binding.reg_id) {
            will_unmute.push(label);
        }
    }

    (summary, will_mute, will_unmute)
}

pub(super) fn preview_profile_diff_for_selection(
    bindings: &[UiWindowBindingRow],
    selected_ids: &BTreeSet<i32>,
    profile: &WebSafeProfile,
) -> (String, Vec<String>, Vec<String>) {
    let selected_available: BTreeSet<i32> = bindings
        .iter()
        .filter(|b| selected_ids.contains(&b.reg_id))
        .map(|b| b.reg_id)
        .collect();
    let target_muted: BTreeSet<i32> = profile
        .muted_reg_ids
        .iter()
        .copied()
        .filter(|reg_id| selected_available.contains(reg_id))
        .collect();
    let skipped = profile
        .muted_reg_ids
        .iter()
        .filter(|reg_id| !selected_available.contains(reg_id))
        .count();
    let current_muted: BTreeSet<i32> = bindings
        .iter()
        .filter(|b| selected_ids.contains(&b.reg_id) && b.web_safe_muted)
        .map(|b| b.reg_id)
        .collect();
    let would_add = target_muted.difference(&current_muted).count();
    let would_remove = current_muted.difference(&target_muted).count();
    let unchanged = target_muted.intersection(&current_muted).count();

    let summary = format!(
        "Selection profile preview from {}: selected {}, apply {}, skipped {}, add {}, remove {}, unchanged {}.",
        profile.scope,
        selected_available.len(),
        target_muted.len(),
        skipped,
        would_add,
        would_remove,
        unchanged
    );

    let mut will_mute = Vec::new();
    let mut will_unmute = Vec::new();
    for binding in bindings {
        if !selected_ids.contains(&binding.reg_id) {
            continue;
        }
        let label = format!("reg {} [{}]", binding.reg_id, display_label(binding));
        if target_muted.contains(&binding.reg_id) && !current_muted.contains(&binding.reg_id) {
            will_mute.push(label);
        } else if current_muted.contains(&binding.reg_id) && !target_muted.contains(&binding.reg_id) {
            will_unmute.push(label);
        }
    }

    (summary, will_mute, will_unmute)
}
