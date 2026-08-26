use std::collections::BTreeSet;

/// Декодирует битовую маску групп (`64 bytes`) в список номеров групп `1..=512`.
///
/// # Parameters
/// - `grups`: байтовое поле групп длиной 64.
///
/// # Returns
/// - `Vec<i32>` с включенными группами по возрастанию.
/// - Пустой вектор, если размер входа не равен 64 байтам.
pub fn decode_groups(grups: &[u8]) -> Vec<i32> {
    if grups.len() != 64 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for bit in 0..(64 * 8) {
        let byte_index = bit >> 3;
        let bit_index = bit & 7;
        if (grups[byte_index] & (1 << bit_index)) != 0 {
            out.push((bit + 1) as i32);
        }
    }
    out
}

/// Кодирует множество номеров групп `1..=512` в битовую маску длиной 64 байта.
///
/// # Parameters
/// - `groups`: множество выбранных групп.
///
/// # Returns
/// - Вектор из 64 байт с установленными битами для валидных групп.
pub fn encode_groups(groups: &BTreeSet<i32>) -> Vec<u8> {
    let mut out = vec![0u8; 64];
    for &g in groups {
        if g < 1 || g > 512 {
            continue;
        }
        let bit = (g - 1) as usize;
        let byte_index = bit >> 3;
        let bit_index = bit & 7;
        out[byte_index] |= 1 << bit_index;
    }
    out
}

/// Формирует полную hex-строку для буфера байт.
///
/// # Parameters
/// - `data`: исходный буфер.
///
/// # Returns
/// - Полная строка `xx xx xx ...`.
/// - `"<empty>"`, если буфер пуст.
pub fn hex_full(data: &[u8]) -> String {
    if data.is_empty() {
        return "<empty>".to_string();
    }
    let mut s = String::new();
    for (i, b) in data.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{:02x}", b));
    }
    s
}
