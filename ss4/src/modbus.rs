//! Модуль низкоуровневой сборки/разбора Modbus RTU кадров и упаковки в UDP-пакеты.

use bytes::BytesMut;

#[derive(Clone, Debug)]
/// Параметры UDP-заголовка для инкапсуляции Modbus RTU кадра.
pub struct UdpParams {
    pub kan: u8,
    pub speed: u8,
    pub stop: u8,
    pub par: u8,
    #[allow(dead_code)]
    pub data: u8,
    #[allow(dead_code)]
    pub rtu: u16,
    #[allow(dead_code)]
    pub out_max: u16,
    pub packet_id: u8,
    pub pkt_type: u8, // 0=req, 1=resp
    pub dsr: u16,
    pub modem: u16,
    #[allow(dead_code)]
    pub port: u16,
    #[allow(dead_code)]
    pub ip: String,
}

impl Default for UdpParams {
    fn default() -> Self {
        Self {
            kan: 3,
            speed: 8,
            stop: 0,
            par: 2,
            data: 8,
            rtu: 301,
            out_max: 256,
            packet_id: 0,
            pkt_type: 0,
            dsr: 1,
            modem: 50002,
            port: 5100,
            ip: "192.168.0.10".to_string(),
        }
    }
}

/// Вычисляет Modbus CRC16 для переданного байтового среза.
///
/// # Parameters
/// - `data`: данные кадра без CRC-хвоста.
///
/// # Returns
/// - 16-битное значение CRC (little-endian при записи в wire).
pub fn crc16(data: &[u8]) -> u16 {
    const TABLE: [u16; 256] = [
        0x0000, 0xC0C1, 0xC181, 0x0140, 0xC301, 0x03C0, 0x0280, 0xC241, 0xC601, 0x06C0, 0x0780,
        0xC741, 0x0500, 0xC5C1, 0xC481, 0x0440, 0xCC01, 0x0CC0, 0x0D80, 0xCD41, 0x0F00, 0xCFC1,
        0xCE81, 0x0E40, 0x0A00, 0xCAC1, 0xCB81, 0x0B40, 0xC901, 0x09C0, 0x0880, 0xC841, 0xD801,
        0x18C0, 0x1980, 0xD941, 0x1B00, 0xDBC1, 0xDA81, 0x1A40, 0x1E00, 0xDEC1, 0xDF81, 0x1F40,
        0xDD01, 0x1DC0, 0x1C80, 0xDC41, 0x1400, 0xD4C1, 0xD581, 0x1540, 0xD701, 0x17C0, 0x1680,
        0xD641, 0xD201, 0x12C0, 0x1380, 0xD341, 0x1100, 0xD1C1, 0xD081, 0x1040, 0xF001, 0x30C0,
        0x3180, 0xF141, 0x3300, 0xF3C1, 0xF281, 0x3240, 0x3600, 0xF6C1, 0xF781, 0x3740, 0xF501,
        0x35C0, 0x3480, 0xF441, 0x3C00, 0xFCC1, 0xFD81, 0x3D40, 0xFF01, 0x3FC0, 0x3E80, 0xFE41,
        0xFA01, 0x3AC0, 0x3B80, 0xFB41, 0x3900, 0xF9C1, 0xF881, 0x3840, 0x2800, 0xE8C1, 0xE981,
        0x2940, 0xEB01, 0x2BC0, 0x2A80, 0xEA41, 0xEE01, 0x2EC0, 0x2F80, 0xEF41, 0x2D00, 0xEDC1,
        0xEC81, 0x2C40, 0xE401, 0x24C0, 0x2580, 0xE541, 0x2700, 0xE7C1, 0xE681, 0x2640, 0x2200,
        0xE2C1, 0xE381, 0x2340, 0xE101, 0x21C0, 0x2080, 0xE041, 0xA001, 0x60C0, 0x6180, 0xA141,
        0x6300, 0xA3C1, 0xA281, 0x6240, 0x6600, 0xA6C1, 0xA781, 0x6740, 0xA501, 0x65C0, 0x6480,
        0xA441, 0x6C00, 0xACC1, 0xAD81, 0x6D40, 0xAF01, 0x6FC0, 0x6E80, 0xAE41, 0xAA01, 0x6AC0,
        0x6B80, 0xAB41, 0x6900, 0xA9C1, 0xA881, 0x6840, 0x7800, 0xB8C1, 0xB981, 0x7940, 0xBB01,
        0x7BC0, 0x7A80, 0xBA41, 0xBE01, 0x7EC0, 0x7F80, 0xBF41, 0x7D00, 0xBDC1, 0xBC81, 0x7C40,
        0xB401, 0x74C0, 0x7580, 0xB541, 0x7700, 0xB7C1, 0xB681, 0x7640, 0x7200, 0xB2C1, 0xB381,
        0x7340, 0xB101, 0x71C0, 0x7080, 0xB041, 0x5000, 0x90C1, 0x9181, 0x5140, 0x9301, 0x53C0,
        0x5280, 0x9241, 0x9601, 0x56C0, 0x5780, 0x9741, 0x5500, 0x95C1, 0x9481, 0x5440, 0x9C01,
        0x5CC0, 0x5D80, 0x9D41, 0x5F00, 0x9FC1, 0x9E81, 0x5E40, 0x5A00, 0x9AC1, 0x9B81, 0x5B40,
        0x9901, 0x59C0, 0x5880, 0x9841, 0x8801, 0x48C0, 0x4980, 0x8941, 0x4B00, 0x8BC1, 0x8A81,
        0x4A40, 0x4E00, 0x8EC1, 0x8F81, 0x4F40, 0x8D01, 0x4DC0, 0x4C80, 0x8C41, 0x4400, 0x84C1,
        0x8581, 0x4540, 0x8701, 0x47C0, 0x4680, 0x8641, 0x8201, 0x42C0, 0x4380, 0x8341, 0x4100,
        0x81C1, 0x8081, 0x4040,
    ];

    let mut crc: u16 = 0xFFFF;
    for &b in data {
        let xor = (b as u16) ^ (crc & 0x00FF);
        crc = (crc >> 8) ^ TABLE[xor as usize];
    }
    crc
}

fn encode_rtu_address(rtu: u16) -> Result<Vec<u8>, String> {
    const PLAIN_MAX: u16 = 247;
    const EXT_BASE: u16 = 248;
    const EXT_BLOCK: u16 = 250;
    const EXT_PREFIX_MIN: u8 = 0xF8;
    const EXT_PREFIX_MAX: u8 = 0xFE;
    const EXT_MAX_RTU: u16 = 1997; // FE 249

    if rtu == 0 {
        return Err("rtu must be in range 1..=1997".to_string());
    }
    if rtu <= PLAIN_MAX {
        return Ok(vec![rtu as u8]);
    }
    if rtu > EXT_MAX_RTU {
        return Err(format!("rtu out of protocol range: {} (max 1997)", rtu));
    }

    let ext = rtu - EXT_BASE;
    let hi = EXT_PREFIX_MIN + (ext / EXT_BLOCK) as u8;
    let lo = (ext % EXT_BLOCK) as u8;
    if hi > EXT_PREFIX_MAX {
        return Err(format!("rtu out of protocol range: {} (max 1997)", rtu));
    }
    Ok(vec![hi, lo])
}

/// Формирует 22-байтный UDP-заголовок протокола устройства.
///
/// # Parameters
/// - `par`: параметры канала/модема/идентификаторов.
/// - `out_max_bytes`: полный размер отправляемого UDP-пакета.
///
/// # Returns
/// - Готовый бинарный заголовок длиной 22 байта.
pub fn shab(par: &UdpParams, out_max_bytes: usize) -> Vec<u8> {
    let mut fbuf = vec![0u8; 22];
    fbuf[0] = 1;
    fbuf[1] = (out_max_bytes & 0x00FF) as u8;
    fbuf[2] = ((out_max_bytes & 0xFF00) >> 8) as u8;
    fbuf[3] = par.packet_id;
    fbuf[4] = par.pkt_type;
    fbuf[5] = (par.dsr & 0x00FF) as u8;
    fbuf[6] = ((par.dsr & 0xFF00) >> 8) as u8;
    fbuf[7] = (par.modem & 0x00FF) as u8;
    fbuf[8] = ((par.modem & 0xFF00) >> 8) as u8;
    fbuf[9] = par.kan;
    fbuf[10] = par.speed;
    fbuf[11] = par.stop;
    fbuf[12] = par.par;
    fbuf[13] = 3;
    fbuf[14] = 3;
    fbuf[15] = 0;
    fbuf[16] = 30;
    fbuf[17] = 0;
    fbuf[18] = 255;
    fbuf[19] = 0;
    fbuf[20] = 255;
    fbuf[21] = 0;
    fbuf
}

/// Собирает Modbus RTU-команду (без UDP-заголовка) и добавляет CRC.
///
/// # Parameters
/// - `rtu`: адрес устройства.
/// - `tip`: функция Modbus (`3/4` чтение, `5/6/16` запись).
/// - `adr`: стартовый адрес регистра.
/// - `size`: количество слов.
/// - `dat`: payload для write-функций.
///
/// # Returns
/// - `Ok(Vec<u8>)`: валидный RTU-кадр.
/// - `Err(String)`: неподдерживаемая функция/невалидные аргументы.
pub fn sout_mb_only(
    rtu: u16,
    tip: u8,
    adr: i32,
    size: u16,
    dat: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    let is_write = dat.is_some();
    let adr_human = adr;
    let size_words = size;
    let func: u8 = if is_write {
        match tip {
            5 | 6 | 16 => tip,
            _ => return Err(format!("unsupported write func tip={}", tip)),
        }
    } else {
        match tip {
            3 | 4 => tip,
            _ => return Err(format!("unsupported read func tip={}", tip)),
        }
    };

    let adr0: i32 = adr_human;

    if !(0..=0xFFFF).contains(&adr0) {
        return Err(format!(
            "adr0 out of range: adrHuman={} adr0={}",
            adr_human, adr0
        ));
    }

    let mut outb = BytesMut::with_capacity(16 + dat.map(|d| d.len()).unwrap_or(0));

    outb.extend_from_slice(&encode_rtu_address(rtu)?);

    outb.extend_from_slice(&[func, ((adr0 >> 8) & 0xFF) as u8, (adr0 & 0xFF) as u8]);

    if !is_write {
        outb.extend_from_slice(&[((size_words >> 8) & 0xFF) as u8, (size_words & 0xFF) as u8]);
    } else if func == 5 || func == 6 {
        let d = dat.ok_or_else(|| format!("missing write payload for func={}", func))?;
        let b0 = d.first().cloned().unwrap_or(0);
        let b1 = d.get(1).cloned().unwrap_or(0);
        outb.extend_from_slice(&[b0, b1]);
    } else if func == 16 {
        let d = dat.ok_or_else(|| "missing write payload for func=16".to_string())?;
        outb.extend_from_slice(&[
            ((size_words >> 8) & 0xFF) as u8,
            (size_words & 0xFF) as u8,
            (d.len() & 0xFF) as u8,
        ]);
        outb.extend_from_slice(d);
    }

    let crc = crc16(&outb);
    outb.extend_from_slice(&[(crc & 0xFF) as u8, ((crc >> 8) & 0xFF) as u8]);
    Ok(outb.to_vec())
}

#[allow(dead_code)]
/// Собирает Modbus RTU-команду и при необходимости упаковывает её в UDP-пакет.
///
/// # Parameters
/// - `rtu`, `tip`, `adr`, `size`, `dat`: параметры Modbus-команды.
/// - `par`: `Some` для инкапсуляции в UDP; `None` чтобы вернуть только RTU.
///
/// # Returns
/// - `Ok(Vec<u8>)`: RTU-кадр или полный UDP-пакет.
/// - `Err(String)`: ошибка формирования Modbus-части.
pub fn sout(
    rtu: u16,
    tip: u8,
    adr: i32,
    size: u16,
    dat: Option<&[u8]>,
    par: Option<&UdpParams>,
) -> Result<Vec<u8>, String> {
    let mb = sout_mb_only(rtu, tip, adr, size, dat)?;
    let Some(par) = par else { return Ok(mb) };
    let mmax = 22 + mb.len();
    let header = shab(par, mmax);
    let mut out = Vec::with_capacity(mmax);
    out.extend_from_slice(&header);
    out.extend_from_slice(&mb);
    Ok(out)
}

/// Извлекает Modbus-часть из UDP-ответа, пропуская служебный заголовок.
///
/// # Parameters
/// - `resp`: UDP-пакет ответа.
///
/// # Returns
/// - `Some(&[u8])`: срез Modbus RTU.
/// - `None`: если пакет слишком короткий.
pub fn extract_modbus_frame(resp: &[u8]) -> Option<&[u8]> {
    const HDR: usize = 10;
    if resp.len() <= HDR + 3 {
        return None;
    }
    Some(&resp[HDR..])
}

fn crc_ok(frame: &[u8]) -> bool {
    if frame.len() < 4 {
        return false;
    }
    let body = &frame[..frame.len() - 2];
    let crc_lo = frame[frame.len() - 2] as u16;
    let crc_hi = frame[frame.len() - 1] as u16;
    let crc = (crc_hi << 8) | crc_lo;
    crc16(body) == crc
}

/// Разделяет агрегированный UDP-ответ на виртуальные пакеты (header + один RTU кадр).
///
/// # Parameters
/// - `rx`: входной UDP-буфер с одним или несколькими RTU-кадрами.
///
/// # Returns
/// - Вектор виртуальных UDP-пакетов, каждый содержит ровно один валидный RTU-кадр.
pub fn split_rx_to_virtual(rx: &[u8]) -> Vec<Vec<u8>> {
    const HDR10: usize = 10;
    let mut out = Vec::new();
    if rx.len() <= HDR10 {
        return out;
    }
    let hdr = &rx[..HDR10];

    let mut off = HDR10;
    let mut guard = 0;

    while off < rx.len() && guard < 20000 {
        guard += 1;
        if let Some((frame, next_offset)) = slice_next_rtu_frame(rx, off) {
            let mut vp = Vec::with_capacity(HDR10 + frame.len());
            vp.extend_from_slice(hdr);
            vp.extend_from_slice(&frame);
            out.push(vp);
            off = next_offset;
        } else {
            off += 1;
        }
    }
    out
}

fn slice_next_rtu_frame(buf: &[u8], offset: usize) -> Option<(Vec<u8>, usize)> {
    if offset >= buf.len() {
        return None;
    }
    let b0 = buf[offset];
    let ulen = if b0 >= 0xF8 { 2 } else { 1 };
    let func_index = offset + ulen;
    if func_index >= buf.len() {
        return None;
    }
    let func = buf[func_index];

    if (func & 0x80) != 0 {
        let need = ulen + 1 + 1 + 2;
        let end = offset + need;
        if end > buf.len() {
            return None;
        }
        let frame = &buf[offset..end];
        if crc_ok(frame) {
            return Some((frame.to_vec(), end));
        }
        return None;
    }

    if func == 16 {
        let need = ulen + 1 + 4 + 2;
        let end = offset + need;
        if end > buf.len() {
            return None;
        }
        let frame = &buf[offset..end];
        if crc_ok(frame) {
            return Some((frame.to_vec(), end));
        }
        return None;
    }

    if func == 3 || func == 4 {
        let bc1_index = func_index + 1;
        if bc1_index >= buf.len() {
            return None;
        }
        let mut candidates = Vec::new();
        let bc1 = buf[bc1_index] as usize;
        candidates.push(ulen + 1 + 1 + bc1 + 2);

        let bc2_lo_index = bc1_index + 1;
        if bc2_lo_index < buf.len() {
            let bc2 = ((buf[bc1_index] as usize) << 8) | (buf[bc2_lo_index] as usize);
            candidates.push(ulen + 1 + 2 + bc2 + 2);
        }
        candidates.sort();
        for need in candidates {
            if need == 0 {
                continue;
            }
            let end = offset + need;
            if end > buf.len() {
                continue;
            }
            let frame = &buf[offset..end];
            if crc_ok(frame) {
                return Some((frame.to_vec(), end));
            }
        }
        return None;
    }

    None
}

/// Группирует Modbus-кадры в чанки, чтобы каждый итоговый UDP-пакет не превышал лимит.
///
/// # Parameters
/// - `mb_frames`: подготовленные Modbus-кадры.
/// - `limit`: верхний предел размера UDP-пакета.
///
/// # Returns
/// - `Ok(Vec<Vec<Vec<u8>>>)`: список чанков, каждый чанк содержит набор Modbus-кадров.
/// - `Err(String)`: если лимит слишком мал или один кадр не помещается.
pub fn build_mb_chunks(mb_frames: &[Vec<u8>], limit: usize) -> Result<Vec<Vec<Vec<u8>>>, String> {
    const HDR22: usize = 22;
    if limit <= HDR22 + 1 {
        return Err(format!("limit too small: {} (need > {})", limit, HDR22));
    }
    let mut out: Vec<Vec<Vec<u8>>> = Vec::new();
    let mut chunk: Vec<Vec<u8>> = Vec::new();
    let mut payload = 0usize;

    let flush = |out: &mut Vec<Vec<Vec<u8>>>, chunk: &mut Vec<Vec<u8>>, payload: &mut usize| {
        if !chunk.is_empty() {
            out.push(std::mem::take(chunk));
            *payload = 0;
        }
    };

    for mb in mb_frames {
        if HDR22 + mb.len() > limit {
            return Err(format!(
                "single cmd too large for limit={} size={}",
                limit,
                HDR22 + mb.len()
            ));
        }
        if !chunk.is_empty() && HDR22 + payload + mb.len() > limit {
            flush(&mut out, &mut chunk, &mut payload);
        }
        chunk.push(mb.clone());
        payload += mb.len();
    }
    flush(&mut out, &mut chunk, &mut payload);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_read_resp(unit: u8, func: u8, data: &[u8]) -> Vec<u8> {
        let mut f = Vec::with_capacity(3 + data.len() + 2);
        f.push(unit);
        f.push(func);
        f.push(data.len() as u8);
        f.extend_from_slice(data);
        let crc = crc16(&f);
        f.push((crc & 0xFF) as u8);
        f.push((crc >> 8) as u8);
        f
    }

    #[test]
    fn build_mb_chunks_splits_by_limit() {
        let f1 = vec![1u8; 50];
        let f2 = vec![2u8; 50];
        let f3 = vec![3u8; 50];
        let out = build_mb_chunks(&[f1, f2, f3], 22 + 80).expect("chunks");
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].len(), 1);
        assert_eq!(out[1].len(), 1);
        assert_eq!(out[2].len(), 1);
    }

    #[test]
    fn build_mb_chunks_error_when_single_too_large() {
        let f1 = vec![1u8; 200];
        let err = build_mb_chunks(&[f1], 22 + 100).err();
        assert!(err.is_some());
    }

    #[test]
    fn sout_mb_only_fc5_includes_addr_and_value_bytes() {
        let frame = sout_mb_only(301, 5, 0x01FF, 1, Some(&[0xFF, 0x00])).expect("fc5 frame");
        // rtu(301) is 2-byte address prefix: F8 35
        assert_eq!(frame[0], 0xF8);
        assert_eq!(frame[1], 0x35);
        assert_eq!(frame[2], 0x05);
        assert_eq!(frame[3], 0x01);
        assert_eq!(frame[4], 0xFF);
        assert_eq!(frame[5], 0xFF);
        assert_eq!(frame[6], 0x00);
        let crc_wire = ((frame[frame.len() - 1] as u16) << 8) | frame[frame.len() - 2] as u16;
        assert_eq!(crc_wire, crc16(&frame[..frame.len() - 2]));
    }

    #[test]
    fn sout_mb_only_fc6_includes_single_word_payload() {
        let frame = sout_mb_only(301, 6, 0x0202, 1, Some(&[0x12, 0x34])).expect("fc6 frame");
        assert_eq!(frame[0], 0xF8);
        assert_eq!(frame[1], 0x35);
        assert_eq!(frame[2], 0x06);
        assert_eq!(frame[3], 0x02);
        assert_eq!(frame[4], 0x02);
        assert_eq!(frame[5], 0x12);
        assert_eq!(frame[6], 0x34);
        let crc_wire = ((frame[frame.len() - 1] as u16) << 8) | frame[frame.len() - 2] as u16;
        assert_eq!(crc_wire, crc16(&frame[..frame.len() - 2]));
    }

    #[test]
    fn encode_rtu_address_protocol_boundaries() {
        assert_eq!(encode_rtu_address(1).unwrap_or_default(), vec![0x01]);
        assert_eq!(encode_rtu_address(247).unwrap_or_default(), vec![0xF7]);
        assert_eq!(
            encode_rtu_address(248).unwrap_or_default(),
            vec![0xF8, 0x00]
        );
        assert_eq!(
            encode_rtu_address(301).unwrap_or_default(),
            vec![0xF8, 0x35]
        );
        assert_eq!(
            encode_rtu_address(497).unwrap_or_default(),
            vec![0xF8, 0xF9]
        );
        assert_eq!(
            encode_rtu_address(498).unwrap_or_default(),
            vec![0xF9, 0x00]
        );
        assert_eq!(
            encode_rtu_address(1997).unwrap_or_default(),
            vec![0xFE, 0xF9]
        );
    }

    #[test]
    fn encode_rtu_address_rejects_out_of_range_values() {
        assert!(encode_rtu_address(0).is_err());
        assert!(encode_rtu_address(1998).is_err());
        assert!(encode_rtu_address(2000).is_err());
    }

    #[test]
    fn sout_mb_only_extended_rtu_keeps_prefix_in_f8_to_fe_range() {
        // RTU in valid extended range 248..=1997; encoding uses F8..FE prefix + 0..249 second byte.
        let cases = [
            (248u16, [0xF8, 0x00]),
            (301u16, [0xF8, 0x35]),
            (497u16, [0xF8, 0xF9]),
            (1997u16, [0xFE, 0xF9]),
        ];

        for (rtu, expected_prefix) in cases {
            let frame = sout_mb_only(rtu, 4, 0x0000, 1, None).expect("extended read frame");
            assert!(
                (0xF8..=0xFE).contains(&frame[0]),
                "rtu={} must start with F8..FE, got {:02X}",
                rtu,
                frame[0]
            );
            assert_eq!(
                [frame[0], frame[1]],
                expected_prefix,
                "rtu={} must use correct extended encoding",
                rtu
            );
        }
    }

    #[test]
    fn sout_mb_only_extended_rtu_real_cases_encode_expected_bytes() {
        // Extended encoding: ext = rtu - 248, block 250, prefix = F8 + ext/250, second = ext % 250.
        let cases = [
            (248u16, [0xF8, 0x00]),
            (301u16, [0xF8, 0x35]),
            (497u16, [0xF8, 0xF9]),
            (1997u16, [0xFE, 0xF9]),
        ];

        for (rtu, expected) in cases {
            let frame = sout_mb_only(rtu, 4, 0x0000, 1, None).expect("extended read frame");
            let ext = (rtu - 248) as usize;
            let expected_calc = [0xF8u8 + (ext / 250) as u8, (ext % 250) as u8];
            assert_eq!([frame[0], frame[1]], expected, "rtu={}", rtu);
            assert_eq!(
                [frame[0], frame[1]],
                expected_calc,
                "rtu={} encoding formula",
                rtu
            );
        }
    }

    #[test]
    fn split_rx_to_virtual_splits_two_valid_frames() {
        let hdr10 = vec![0x11u8; 10];
        let f1 = mk_read_resp(1, 4, &[0x00, 0x2A]);
        let f2 = mk_read_resp(1, 4, &[0x12, 0x34]);

        let mut rx = Vec::new();
        rx.extend_from_slice(&hdr10);
        rx.extend_from_slice(&f1);
        rx.extend_from_slice(&f2);

        let out = split_rx_to_virtual(&rx);
        assert_eq!(out.len(), 2);
        assert_eq!(extract_modbus_frame(&out[0]).unwrap_or(&[]), &f1[..]);
        assert_eq!(extract_modbus_frame(&out[1]).unwrap_or(&[]), &f2[..]);
    }

    #[test]
    fn split_rx_to_virtual_skips_bad_crc_frame() {
        let hdr10 = vec![0x22u8; 10];
        let good = mk_read_resp(1, 3, &[0x00, 0x01]);
        let mut bad = mk_read_resp(1, 3, &[0x00, 0x02]);
        let n = bad.len();
        bad[n - 1] ^= 0xFF;

        let mut rx = Vec::new();
        rx.extend_from_slice(&hdr10);
        rx.extend_from_slice(&good);
        rx.extend_from_slice(&bad);

        let out = split_rx_to_virtual(&rx);
        assert_eq!(out.len(), 1);
        assert_eq!(extract_modbus_frame(&out[0]).unwrap_or(&[]), &good[..]);
    }
}
