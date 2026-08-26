use crate::reg::Reg;
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub(super) struct ReadBlock {
    pub adr: i32,
    pub cnt_words: i32,
    pub func: u8,
}

/// Строит порядок обхода групп: сначала `start_group_id` (если включена), затем остальные включенные группы.
/// # Параметры
/// - `enabled`: список включенных групп КПЗ.
/// - `start_group_id`: группа, с которой начинается приоритетный обход в цикле опроса.
/// # Возвращает
/// - `Vec<i32>`: упорядоченный список групп для обхода.
/// # Пример
/// - `let ordered = ordered_groups(&enabled_groups, start_group_id);`
pub(super) fn ordered_groups(enabled: &[i32], start_group_id: i32) -> Vec<i32> {
    let mut out = Vec::with_capacity(enabled.len());
    if enabled.contains(&start_group_id) {
        out.push(start_group_id);
    }
    for g in enabled {
        if *g != start_group_id {
            out.push(*g);
        }
    }
    out
}

/// Определяет Modbus функцию чтения для регистра: `4` для TIT, `3` для REG, иначе `None`.
/// # Параметры
/// - `reg`: описание регистра (адрес, тип, флаги), по которому выбирается функция чтения.
/// - `n_mb_tit_id`: идентификатор типа регистра TIT (чтение FC4).
/// - `n_mb_reg_id`: идентификатор типа регистра REG (чтение FC3).
/// # Возвращает
/// - `Option<u8>`: Modbus функция чтения (`3`/`4`) или `None`.
/// # Пример
/// - `let func = read_func_for_reg(reg, n_mb_tit_id, n_mb_reg_id);`
pub(super) fn read_func_for_reg(
    reg: &Reg,
    n_mb_tit_id: Option<i32>,
    n_mb_reg_id: Option<i32>,
) -> Option<u8> {
    if reg.n_mb == n_mb_tit_id {
        return Some(4);
    }
    if reg.n_mb == n_mb_reg_id {
        return Some(3);
    }
    None
}

/// Оставляет только валидные для опроса регистры: адрес в диапазоне `0..65535` и поддерживаемый тип чтения.
/// # Параметры
/// - `regs`: набор регистров, участвующих в планировании/чтении.
/// - `n_mb_tit_id`: идентификатор типа регистра TIT (чтение FC4).
/// - `n_mb_reg_id`: идентификатор типа регистра REG (чтение FC3).
/// # Возвращает
/// - `Vec<Reg>`: список регистров, допущенных к опросу.
/// # Пример
/// - `let regs_poll = filter_regs_for_polling(&regs, n_mb_tit_id, n_mb_reg_id);`
pub(super) fn filter_regs_for_polling(
    regs: &[Reg],
    n_mb_tit_id: Option<i32>,
    n_mb_reg_id: Option<i32>,
) -> Vec<Reg> {
    let mut out = Vec::new();
    for r in regs {
        let a = r.addr;
        if !(0..=65535).contains(&a) {
            continue;
        }
        if read_func_for_reg(r, n_mb_tit_id, n_mb_reg_id).is_none() {
            continue;
        }
        out.push(r.clone());
    }
    out
}

pub(super) fn plan_group_reads(
    regs: &[Reg],
    n_mb_tit_id: Option<i32>,
    n_mb_reg_id: Option<i32>,
    max_words_per_block: i32,
) -> Option<(Vec<Reg>, HashSet<i32>, Vec<ReadBlock>)> {
    let mut regs_poll_sorted = filter_regs_for_polling(regs, n_mb_tit_id, n_mb_reg_id);
    if regs_poll_sorted.is_empty() {
        return None;
    }
    regs_poll_sorted.sort_by_key(|r| r.addr);

    let mut write_ids: HashSet<i32> = HashSet::new();
    let mut blocks: Vec<ReadBlock> = Vec::new();

    let mut fc4_start: Option<i32> = None;
    let mut fc4_end: i32 = 0;
    let mut fc3_start: Option<i32> = None;
    let mut fc3_end: i32 = 0;

    let push_block = |out: &mut Vec<ReadBlock>, start: i32, end: i32, func: u8| {
        let cnt = end - start + 1;
        if cnt > 0 {
            out.push(ReadBlock {
                adr: start,
                cnt_words: cnt,
                func,
            });
        }
    };

    for r in &regs_poll_sorted {
        if r.a_en && r.a_no_write == 0 {
            write_ids.insert(r.id);
        }

        let Some(func) = read_func_for_reg(r, n_mb_tit_id, n_mb_reg_id) else {
            continue;
        };
        let r_start = r.addr;
        let r_end = r.addr + if r.is_32() { 2 } else { 1 } - 1;

        let (start_slot, end_slot) = if func == 4 {
            (&mut fc4_start, &mut fc4_end)
        } else {
            (&mut fc3_start, &mut fc3_end)
        };

        match *start_slot {
            None => {
                *start_slot = Some(r_start);
                *end_slot = r_end;
            }
            Some(start) => {
                let gap = r_start > *end_slot + 1;
                let would_cnt = r_end - start + 1;
                if gap || would_cnt > max_words_per_block {
                    push_block(&mut blocks, start, *end_slot, func);
                    *start_slot = Some(r_start);
                    *end_slot = r_end;
                } else if r_end > *end_slot {
                    *end_slot = r_end;
                }
            }
        }
    }

    if let Some(start) = fc4_start {
        push_block(&mut blocks, start, fc4_end, 4);
    }
    if let Some(start) = fc3_start {
        push_block(&mut blocks, start, fc3_end, 3);
    }
    blocks.sort_by_key(|b| b.adr);

    if blocks.is_empty() {
        return None;
    }

    Some((regs_poll_sorted, write_ids, blocks))
}

/// Группирует отсортированные регистры в непрерывные блоки чтения одной функции, разрывая блок по gap или превышению `max_words_per_block`.
/// # Параметры
/// - `regs`: набор регистров, участвующих в планировании/чтении.
/// - `max_words_per_block`: лимит слов в одном сформированном блоке чтения.
/// - `func`: код Modbus-функции для блока/команды.
/// # Возвращает
/// - `Vec<ReadBlock>`: сформированные блоки чтения для одной функции.
/// # Пример
/// - `let blocks = build_blocks_with_func(&regs_f4, 120, 4);`
#[cfg(test)]
pub(super) fn build_blocks_with_func(
    regs: &[Reg],
    max_words_per_block: i32,
    func: u8,
) -> Vec<ReadBlock> {
    if regs.is_empty() {
        return Vec::new();
    }
    let mut sorted = regs.to_vec();
    sorted.sort_by_key(|r| r.addr);

    let mut out: Vec<ReadBlock> = Vec::new();

    let mut start = sorted[0].addr;
    let mut end = sorted[0].addr + if sorted[0].is_32() { 2 } else { 1 } - 1;

    let mut push_block = |s: i32, e: i32| {
        let cnt = e - s + 1;
        if cnt > 0 {
            out.push(ReadBlock {
                adr: s,
                cnt_words: cnt,
                func,
            });
        }
    };

    for r in sorted.iter().skip(1) {
        let r_start = r.addr;
        let r_end = r.addr + if r.is_32() { 2 } else { 1 } - 1;

        let gap = r_start > end + 1;
        let would_cnt = r_end - start + 1;

        if gap || would_cnt > max_words_per_block {
            push_block(start, end);
            start = r_start;
            end = r_end;
        } else if r_end > end {
            end = r_end;
        }
    }

    push_block(start, end);
    out
}

/// Извлекает слова (`u16`) из Modbus-кадра ответа, проверяя формат, byte_count, отсутствие exception и достаточную длину данных.
/// # Параметры
/// - `mb`: извлеченный Modbus-фрейм ответа.
/// - `expected_count`: ожидаемое число 16-бит слов в ответе.
/// # Возвращает
/// - `Vec<u16>`: декодированные слова ответа; пусто при невалидном фрейме.
/// # Пример
/// - `let words = words_from_modbus_frame(mb, cnt_words);`
pub(super) fn words_from_modbus_frame(mb: &[u8], expected_count: i32) -> Vec<u16> {
    if mb.is_empty() || expected_count <= 0 {
        return Vec::new();
    }
    let ulen = if mb[0] >= 0xF8 { 2 } else { 1 };
    let func_index = ulen;
    if mb.len() <= func_index {
        return Vec::new();
    }
    let func = mb[func_index];
    if (func & 0x80) != 0 {
        return Vec::new();
    }
    let bc_index = func_index + 1;

    let (byte_count, data_start) = if ulen == 2 {
        if mb.len() < bc_index + 2 {
            return Vec::new();
        }
        (
            ((mb[bc_index] as usize) << 8) | (mb[bc_index + 1] as usize),
            bc_index + 2,
        )
    } else {
        if mb.len() <= bc_index {
            return Vec::new();
        }
        (mb[bc_index] as usize, bc_index + 1)
    };

    let need_bytes = expected_count as usize * 2;
    if byte_count < need_bytes {
        return Vec::new();
    }
    if mb.len() < data_start + byte_count {
        return Vec::new();
    }

    let mut out: Vec<u16> = Vec::with_capacity(expected_count as usize);
    for i in 0..expected_count as usize {
        let hi = mb[data_start + i * 2] as u16;
        let lo = mb[data_start + i * 2 + 1] as u16;
        out.push((hi << 8) | lo);
    }
    out
}

/// Декодирует значение регистра из массива слов с учетом типа (bit/16/32), знака и порядка слов (`hi_lo`).
/// # Параметры
/// - `r`: описание регистра (тип, адрес, битность).
/// - `words`: массив прочитанных 16-бит слов блока.
/// - `base_addr`: базовый адрес прочитанного блока.
/// - `count_words`: длина прочитанного блока в словах.
/// - `hi_lo`: порядок слов для 32-бит декодирования (hi/lo).
/// # Возвращает
/// - `Option<f64>`: декодированное значение регистра или `None` при несоответствии.
/// # Пример
/// - `let v = decode_numeric(reg, &words, block_addr, block_cnt, true);`
pub(super) fn decode_numeric(
    r: &Reg,
    words: &[u16],
    base_addr: i32,
    count_words: i32,
    hi_lo: bool,
) -> Option<f64> {
    let off = r.addr - base_addr;
    if off < 0 || off >= count_words {
        return None;
    }

    if r.is_bit() {
        let bit = r.bits.unwrap_or(0);
        if !(0..=15).contains(&bit) {
            return None;
        }
        let w = *words.get(off as usize).unwrap_or(&0);
        return Some(((w >> bit) & 1) as f64);
    }

    if r.is_32() {
        if off + 1 >= count_words {
            return None;
        }
        let w0 = words.get(off as usize).copied().unwrap_or(0);
        let w1 = words.get((off + 1) as usize).copied().unwrap_or(0);
        let (hi, lo) = if hi_lo { (w0, w1) } else { (w1, w0) };
        let bytes = [
            ((hi >> 8) & 0xFF) as u8,
            (hi & 0xFF) as u8,
            ((lo >> 8) & 0xFF) as u8,
            (lo & 0xFF) as u8,
        ];
        let v = match r.tip {
            5 => f32::from_be_bytes(bytes) as f64,
            4 => u32::from_be_bytes(bytes) as f64,
            2 => i32::from_be_bytes(bytes) as f64,
            _ => u32::from_be_bytes(bytes) as f64,
        };
        return Some(v);
    }

    let w = *words.get(off as usize).unwrap_or(&0);
    let v = match r.tip {
        1 => (w as i16) as i32 as f64,
        3 => w as f64,
        _ => w as f64,
    };
    Some(v)
}
