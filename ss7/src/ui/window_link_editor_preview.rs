use eframe::egui;

use crate::models::AlarmRuleRow;

#[derive(Clone, Copy, PartialEq, Eq)]
enum AlarmVisualLevel {
    Normal,
    Prewarn,
    Alarm,
}

fn eval_alarm_level(value: f64, rules: &[AlarmRuleRow]) -> AlarmVisualLevel {
    let mut level = AlarmVisualLevel::Normal;
    for r in rules {
        let mut alarm = false;
        let mut prewarn = false;
        if let Some(hi) = r.set_hi {
            if value >= hi {
                alarm = true;
            }
        }
        if let Some(lo) = r.set_lo {
            if value <= lo {
                alarm = true;
            }
        }
        if let Some(hi_1) = r.set_hi_1 {
            if value >= hi_1 {
                prewarn = true;
            }
        }
        if let Some(lo_1) = r.set_lo_1 {
            if value <= lo_1 {
                prewarn = true;
            }
        }
        if alarm {
            return AlarmVisualLevel::Alarm;
        }
        if prewarn {
            level = AlarmVisualLevel::Prewarn;
        }
    }
    level
}

pub(super) fn alarm_color(
    live: Option<f64>,
    rules: Option<&Vec<AlarmRuleRow>>,
    fallback: egui::Color32,
) -> egui::Color32 {
    if let (Some(value), Some(rules)) = (live, rules) {
        match eval_alarm_level(value, rules) {
            AlarmVisualLevel::Alarm => egui::Color32::from_rgb(210, 54, 54),
            AlarmVisualLevel::Prewarn => egui::Color32::from_rgb(230, 200, 60),
            AlarmVisualLevel::Normal => egui::Color32::from_rgb(40, 190, 70),
        }
    } else {
        fallback
    }
}

pub(super) fn draw_bar_alarm_markers(
    painter: &egui::Painter,
    rect: egui::Rect,
    scale_max: f64,
    rules: Option<&Vec<AlarmRuleRow>>,
) {
    let Some(rules) = rules else {
        return;
    };
    if !(scale_max.is_finite() && scale_max > 0.0) {
        return;
    }
    let is_vertical = rect.height() > rect.width();
    for rule in rules {
        if !rule.enabled {
            continue;
        }
        let thresholds = [
            (rule.set_lo, egui::Color32::from_rgb(210, 54, 54)),
            (rule.set_hi, egui::Color32::from_rgb(210, 54, 54)),
            (rule.set_lo_1, egui::Color32::from_rgb(230, 200, 60)),
            (rule.set_hi_1, egui::Color32::from_rgb(230, 200, 60)),
        ];
        for (value, color) in thresholds {
            let Some(value) = value else {
                continue;
            };
            let ratio = (value / scale_max).clamp(0.0, 1.0) as f32;
            if is_vertical {
                let y = rect.max.y - rect.height() * ratio;
                painter.line_segment(
                    [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                    egui::Stroke::new(1.5, color),
                );
            } else {
                let x = rect.min.x + rect.width() * ratio;
                painter.line_segment(
                    [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                    egui::Stroke::new(1.5, color),
                );
            }
        }
    }
}

pub(super) fn draw_gauge_alarm_markers(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    scale_max: f64,
    rules: Option<&Vec<AlarmRuleRow>>,
) {
    let Some(rules) = rules else {
        return;
    };
    if !(scale_max.is_finite() && scale_max > 0.0) {
        return;
    }
    let start_angle = std::f32::consts::PI;
    let end_angle = std::f32::consts::TAU;
    for rule in rules {
        if !rule.enabled {
            continue;
        }
        let thresholds = [
            (rule.set_lo, egui::Color32::from_rgb(210, 54, 54)),
            (rule.set_hi, egui::Color32::from_rgb(210, 54, 54)),
            (rule.set_lo_1, egui::Color32::from_rgb(230, 200, 60)),
            (rule.set_hi_1, egui::Color32::from_rgb(230, 200, 60)),
        ];
        for (value, color) in thresholds {
            let Some(value) = value else {
                continue;
            };
            let ratio = (value / scale_max).clamp(0.0, 1.0) as f32;
            let angle = start_angle + (end_angle - start_angle) * ratio;
            let outer = center + egui::vec2(angle.cos() * radius, angle.sin() * radius);
            let inner = center + egui::vec2(angle.cos() * radius * 0.8, angle.sin() * radius * 0.8);
            painter.line_segment([inner, outer], egui::Stroke::new(2.0, color));
        }
    }
}

pub(super) fn draw_trend_tile(
    painter: &egui::Painter,
    rect: egui::Rect,
    web_safe: bool,
    history: Option<&Vec<f64>>,
    live: Option<f64>,
    rules: Option<&Vec<AlarmRuleRow>>,
) {
    painter.rect_filled(
        rect,
        if web_safe { 10.0 } else { 4.0 },
        egui::Color32::from_rgb(22, 28, 40),
    );
    let inner = if web_safe {
        egui::Rect::from_min_max(
            egui::pos2(rect.min.x + 6.0, rect.min.y + 18.0),
            egui::pos2(rect.max.x - 6.0, rect.max.y - 14.0),
        )
    } else {
        rect.shrink2(egui::vec2(6.0, 6.0))
    };
    painter.rect_stroke(
        inner,
        2.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 82, 104)),
    );

    let Some(history) = history else {
        return;
    };
    if history.len() < 2 {
        return;
    }

    let min_v = history.iter().copied().fold(f64::INFINITY, f64::min);
    let max_v = history.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = (max_v - min_v).max(1e-9);
    let mut pts = Vec::with_capacity(history.len());
    for (idx, sample) in history.iter().enumerate() {
        let t = idx as f32 / (history.len().saturating_sub(1)) as f32;
        let x = inner.left() + inner.width() * t;
        let norm = ((*sample - min_v) / span) as f32;
        let y = inner.bottom() - inner.height() * norm;
        pts.push(egui::pos2(x, y));
    }
    painter.add(egui::Shape::line(
        pts,
        egui::Stroke::new(
            2.0,
            alarm_color(live, rules, egui::Color32::from_rgb(110, 200, 255)),
        ),
    ));
}

pub(super) fn draw_led_tile(
    painter: &egui::Painter,
    rect: egui::Rect,
    web_safe: bool,
    live: Option<f64>,
    rules: Option<&Vec<AlarmRuleRow>>,
) {
    if web_safe {
        painter.rect_filled(rect, 10.0, egui::Color32::from_rgb(21, 28, 41));
        painter.rect_stroke(
            rect,
            10.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 64, 82)),
        );
    }
    let led_center = if web_safe {
        egui::pos2(rect.center().x, rect.center().y + 2.0)
    } else {
        rect.center()
    };
    let led_radius = (rect.width().min(rect.height()) * 0.35).max(4.0);
    let on = live.unwrap_or(0.0) >= 0.5;
    let led_color = if on {
        alarm_color(live, rules, egui::Color32::from_rgb(40, 190, 70))
    } else {
        egui::Color32::from_rgb(24, 28, 34)
    };
    painter.circle_filled(led_center, led_radius, led_color);
    painter.circle_stroke(
        led_center,
        led_radius,
        egui::Stroke::new(1.0, egui::Color32::WHITE),
    );
}

pub(super) fn draw_numeric_tile(painter: &egui::Painter, rect: egui::Rect, web_safe: bool) {
    painter.rect_filled(
        rect,
        if web_safe { 10.0 } else { 3.0 },
        egui::Color32::from_rgb(26, 32, 46),
    );
}

pub(super) fn draw_button_tile(
    painter: &egui::Painter,
    rect: egui::Rect,
    web_safe: bool,
    writable: bool,
) {
    let base = if writable {
        egui::Color32::from_rgb(38, 88, 132)
    } else {
        egui::Color32::from_rgb(52, 58, 72)
    };
    painter.rect_filled(rect, 6.0, base);
    let highlight = egui::Rect::from_min_max(
        rect.min,
        egui::pos2(rect.max.x, rect.min.y + rect.height() * 0.4),
    );
    painter.rect_filled(
        highlight,
        6.0,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 26),
    );
    let bottom_shadow = egui::Rect::from_min_max(
        egui::pos2(rect.min.x, rect.max.y - rect.height() * 0.22),
        rect.max,
    );
    painter.rect_filled(
        bottom_shadow,
        6.0,
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 28),
    );
    painter.rect_stroke(
        rect,
        if web_safe { 10.0 } else { 6.0 },
        egui::Stroke::new(
            1.0,
            if writable {
                egui::Color32::from_rgb(120, 190, 255)
            } else {
                egui::Color32::from_gray(120)
            },
        ),
    );
}
