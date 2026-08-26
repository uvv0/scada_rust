//! Хелперы для окон: парсинг привязок, описание по умолчанию, код окна.

use std::collections::BTreeSet;

use crate::models::UiWindowBindingRow;

/// Парсит значение привязки из слов Modbus по типу регистра.
pub fn parse_binding_value_from_words_static(
    b: &UiWindowBindingRow,
    words: &[u16],
) -> Result<f64, String> {
    if words.is_empty() {
        return Err("empty words".to_string());
    }
    if matches!(b.reg_tip, 2 | 4 | 5) {
        if words.len() < 2 {
            return Err("not enough words for 32-bit value".to_string());
        }
        let hi = words[0];
        let lo = words[1];
        let bytes = [
            ((hi >> 8) & 0xFF) as u8,
            (hi & 0xFF) as u8,
            ((lo >> 8) & 0xFF) as u8,
            (lo & 0xFF) as u8,
        ];
        let v = match b.reg_tip {
            5 => f32::from_be_bytes(bytes) as f64,
            4 => u32::from_be_bytes(bytes) as f64,
            2 => i32::from_be_bytes(bytes) as f64,
            _ => u32::from_be_bytes(bytes) as f64,
        };
        return Ok(v);
    }
    if b.reg_tip == 0
        && let Some(bit) = b.reg_bits
        && (0..=15).contains(&bit)
    {
        return Ok(((words[0] >> bit) & 1) as f64);
    }
    let w = words[0];
    let v = match b.reg_tip {
        1 => (w as i16) as f64,
        _ => w as f64,
    };
    Ok(v)
}

/// Описание окна по умолчанию из шаблона/КП-шаблона.
pub fn default_window_description(
    template_description: Option<&str>,
    template_title: &str,
    template_code: &str,
    kp_template_title: Option<&str>,
) -> Option<String> {
    let from_template = template_description
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);
    if from_template.is_some() {
        return from_template;
    }
    let template_title = template_title.trim();
    let template_code = template_code.trim();
    let kp_template_title = kp_template_title
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    let desc = if !kp_template_title.is_empty() && !template_title.is_empty() {
        format!("{kp_template_title}: {template_title}")
    } else if !template_title.is_empty() {
        template_title.to_string()
    } else if !template_code.is_empty() {
        format!("window {template_code}")
    } else {
        "window".to_string()
    };
    Some(desc)
}

/// Следующий свободный код окна (base, base_2, base_3, ...).
pub fn next_available_window_code(existing_codes: &mut BTreeSet<String>, base_code: &str) -> String {
    let base = base_code.trim();
    let base = if base.is_empty() { "window" } else { base };
    if existing_codes.insert(base.to_string()) {
        return base.to_string();
    }
    let mut suffix = 2usize;
    loop {
        let candidate = format!("{base}_{suffix}");
        if existing_codes.insert(candidate.clone()) {
            return candidate;
        }
        suffix += 1;
    }
}
