use std::collections::HashMap;

use crate::modbus;

use super::{PostDeviceCmd, POST_CMD_ADDR, POST_CMD_EN, POST_CMD_FUNC, POST_CMD_VAL};

/// Проверяет, что ключ относится к служебным post-командам (`920..923`) и не должен архивироваться как обычный регистр.
/// # Параметры
/// - `key`: логический/служебный ключ регистра или индекса.
/// # Возвращает
/// - `bool`: `true`, если ключ относится к диапазону служебной post-команды.
/// # Пример
/// - `if is_post_command_key(key) { continue; }`
pub(super) fn is_post_command_key(key: i32) -> bool {
    key == POST_CMD_EN || key == POST_CMD_FUNC || key == POST_CMD_ADDR || key == POST_CMD_VAL
}

/// Преобразует человекочитаемый 1-based адрес в wire-адрес Modbus (0-based).
/// # Параметры
/// - `addr`: человекочитаемый адрес регистра Modbus (обычно 1-based).
/// # Возвращает
/// - `i32`: wire-адрес Modbus (0-based).
/// # Пример
/// - `let wire_addr = post_addr_to_wire(addr);`
pub(super) fn post_addr_to_wire(addr: i32) -> i32 {
    if addr > 0 {
        addr - 1
    } else {
        0
    }
}

/// Читает из output-регистров структуру команды на устройство; возвращает `None`, если флаг включения команды не установлен.
/// # Параметры
/// - `out_regs`: карта вычисленных регистров скрипта (ключ -> значение).
/// # Возвращает
/// - `Option<PostDeviceCmd>`: распарсенная команда или `None` при выключенном флаге.
/// # Пример
/// - `let cmd = extract_post_device_command(&out_regs);`
pub(super) fn extract_post_device_command(out_regs: &HashMap<i32, f64>) -> Option<PostDeviceCmd> {
    let en = out_regs.get(&POST_CMD_EN).copied().unwrap_or(0.0) as i32;
    if en == 0 {
        return None;
    }
    Some(PostDeviceCmd {
        func: out_regs.get(&POST_CMD_FUNC).copied().unwrap_or(0.0) as i32,
        addr: out_regs.get(&POST_CMD_ADDR).copied().unwrap_or(0.0) as i32,
        value: out_regs.get(&POST_CMD_VAL).copied().unwrap_or(0.0),
    })
}

/// Строит Modbus PDU для post-команды (FC5/FC6), валидирует адрес и возвращает `(addr_wire, frame)`.
/// # Параметры
/// - `rtu`: адрес устройства RTU (unit id) в Modbus сети.
/// - `cmd`: параметры post-команды устройства (func/addr/value).
/// # Возвращает
/// - `Option<(i32, Vec<u8>)>`: wire-адрес и PDU команды, либо `None` при невалидных входах.
/// # Пример
/// - `let frame = build_post_device_mb(rtu, cmd);`
pub(super) fn build_post_device_mb(rtu: u16, cmd: PostDeviceCmd) -> Option<(i32, Vec<u8>)> {
    if !(0..=65535).contains(&cmd.addr) {
        return None;
    }
    let (size_words, data): (u16, Vec<u8>) = match cmd.func {
        5 => {
            let on = cmd.value >= 0.5;
            let d = if on {
                vec![0xFF, 0x00]
            } else {
                vec![0x00, 0x00]
            };
            (1u16, d)
        }
        6 => {
            let w = (cmd.value.round() as i32).clamp(0, 0xFFFF) as u16;
            (1u16, vec![((w >> 8) & 0xFF) as u8, (w & 0xFF) as u8])
        }
        _ => return None,
    };
    let addr_wire = post_addr_to_wire(cmd.addr);
    let mb = modbus::sout_mb_only(rtu, cmd.func as u8, addr_wire, size_words, Some(&data)).ok()?;
    Some((addr_wire, mb))
}
