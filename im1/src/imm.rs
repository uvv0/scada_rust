use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub static mut DAT: [u16; 70000] = [0; 70000];
pub static mut DAT1: [u16; 70000] = [0; 70000];
const DAT_LEN: usize = 70000;

pub const RING_START_ADDR: usize = 8192;
pub const RING_INDEX_ADDR: usize = 400;
pub const RING_RECORDS: usize = 546;
pub const RING_FLOATS: usize = 36;
pub const RING_RECORD_WORDS: usize = 2 + RING_FLOATS * 2; // u32 ts + 36*f32
pub const RING_TOTAL_WORDS: usize = RING_RECORDS * RING_RECORD_WORDS;

static LAST_MINUTE: AtomicU64 = AtomicU64::new(0);
static CUR_INDEX: AtomicU16 = AtomicU16::new(0);
static CUR_VALUE_BITS: AtomicU32 = AtomicU32::new(0);
static START_VALUE_BITS: AtomicU32 = AtomicU32::new(0x42C8_0000); // 100.0
static DELTA_BITS: AtomicU32 = AtomicU32::new(0x3DCC_CCCD); // 0.1
static LAST_FC05_ADDR: AtomicU16 = AtomicU16::new(0);
static LAST_FC05_ON: AtomicU16 = AtomicU16::new(0);
static LAST_FC05_SEEN: AtomicU16 = AtomicU16::new(0);

pub struct SlotState {
    pub dat: [u16; DAT_LEN],
    pub dat1: [u16; DAT_LEN],
    pub last_minute: u64,
    pub cur_index: u16,
    pub cur_value_bits: u32,
    pub start_value_bits: u32,
    pub delta_bits: u32,
    pub last_fc05_addr: u16,
    pub last_fc05_on: u16,
    pub last_fc05_seen: u16,
}

#[derive(Clone, Debug)]
pub struct RingRecordPreview {
    pub index: u16,
    pub ts_unix: u32,
    pub value0: f32,
}

fn is_extended_unit_prefix(b: u8) -> bool {
    (0xF8..=0xFE).contains(&b)
}

fn frame_len_for_layout(inb: &[u8], size_lim: usize, off: usize, ulen: usize) -> Option<usize> {
    if off + ulen + 1 > size_lim {
        return None;
    }
    match ulen {
        2 if !is_extended_unit_prefix(inb[off]) => return None,
        1 if is_extended_unit_prefix(inb[off]) => return None,
        _ => {}
    }
    let func = inb[off + ulen];
    match func {
        3..=6 => Some(ulen + 1 + 2 + 2 + 2),
        16 => {
            if off + ulen + 1 + 2 + 2 + 1 > size_lim {
                return None;
            }
            let n = inb[off + ulen + 1 + 2 + 2] as usize;
            Some(ulen + 1 + 2 + 2 + 1 + n + 2)
        }
        _ => None,
    }
}

fn detect_frame_layout(inb: &[u8], size_lim: usize, off: usize) -> Option<(usize, usize)> {
    let layouts: &[usize] = if off < size_lim && is_extended_unit_prefix(inb[off]) {
        &[2usize]
    } else {
        &[1usize]
    };

    for &ulen in layouts {
        let Some(frame_len) = frame_len_for_layout(inb, size_lim, off, ulen) else {
            continue;
        };
        if off + frame_len > size_lim {
            continue;
        }
        if v_crc16(&inb[off..off + frame_len]) {
            return Some((ulen, frame_len));
        }
    }
    None
}

impl SlotState {
    pub fn new() -> Self {
        Self {
            dat: [0; DAT_LEN],
            dat1: [0; DAT_LEN],
            last_minute: 0,
            cur_index: 0,
            cur_value_bits: 0x42C8_0000,
            start_value_bits: 0x42C8_0000,
            delta_bits: 0x3DCC_CCCD,
            last_fc05_addr: 0,
            last_fc05_on: 0,
            last_fc05_seen: 0,
        }
    }

    fn write_u32_hi_lo(&mut self, base: usize, v: u32) {
        self.dat[base] = ((v >> 16) & 0xFFFF) as u16;
        self.dat[base + 1] = (v & 0xFFFF) as u16;
    }

    fn write_f32_lo_hi(&mut self, base: usize, v: f32) {
        let bits = v.to_bits();
        self.dat[base] = (bits & 0xFFFF) as u16;
        self.dat[base + 1] = ((bits >> 16) & 0xFFFF) as u16;
    }

    fn read_u32_hi_lo(&self, base: usize) -> u32 {
        ((self.dat[base] as u32) << 16) | (self.dat[base + 1] as u32)
    }

    fn read_f32_lo_hi(&self, base: usize) -> f32 {
        let bits = (self.dat[base] as u32) | ((self.dat[base + 1] as u32) << 16);
        f32::from_bits(bits)
    }

    fn append_record(&mut self, ts_unix: u32) {
        let idx = ((self.cur_index as usize + 1) % RING_RECORDS) as u16;
        let cur = f32::from_bits(self.cur_value_bits);
        let delta = f32::from_bits(self.delta_bits);
        let base = RING_START_ADDR + idx as usize * RING_RECORD_WORDS;

        self.write_u32_hi_lo(base, ts_unix);
        for k in 0..RING_FLOATS {
            self.write_f32_lo_hi(base + 2 + k * 2, cur + delta * k as f32);
        }

        self.dat[RING_INDEX_ADDR] = idx;
        self.dat[RING_INDEX_ADDR + 1] = 0;
        self.dat[RING_INDEX_ADDR + 2] = RING_RECORD_WORDS as u16;
        self.dat[RING_INDEX_ADDR + 3] = RING_RECORDS as u16;

        self.cur_index = idx;
        self.cur_value_bits = (cur + delta).to_bits();
    }

    pub fn ring_status(&self) -> (u16, f32, u64) {
        (
            self.cur_index,
            f32::from_bits(self.cur_value_bits),
            self.last_minute,
        )
    }

    pub fn reg_u16(&self, addr: usize) -> u16 {
        if addr < DAT_LEN {
            self.dat[addr]
        } else {
            0
        }
    }

    pub fn set_reg_u16(&mut self, addr: usize, value: u16) {
        if addr < DAT_LEN {
            self.dat[addr] = value;
        }
    }

    pub fn last_fc05_status(&self) -> Option<(u16, bool)> {
        if self.last_fc05_seen == 0 {
            return None;
        }
        Some((self.last_fc05_addr, self.last_fc05_on != 0))
    }

    pub fn gen_params(&self) -> (f32, f32) {
        (
            f32::from_bits(self.start_value_bits),
            f32::from_bits(self.delta_bits),
        )
    }

    pub fn set_gen_params(&mut self, start_value: f32, delta: f32, reset_archive: bool) {
        self.start_value_bits = start_value.to_bits();
        self.delta_bits = delta.to_bits();
        if reset_archive {
            self.init_ring_archive();
        }
    }

    pub fn init_ring_archive(&mut self) {
        let start = f32::from_bits(self.start_value_bits);
        self.cur_value_bits = start.to_bits();
        self.cur_index = (RING_RECORDS - 1) as u16;
        self.last_minute = 0;
        self.dat[RING_START_ADDR..RING_START_ADDR + RING_TOTAL_WORDS].fill(0);
        self.dat[RING_INDEX_ADDR] = (RING_RECORDS - 1) as u16;
        self.dat[RING_INDEX_ADDR + 1] = 0;
        self.dat[RING_INDEX_ADDR + 2] = RING_RECORD_WORDS as u16;
        self.dat[RING_INDEX_ADDR + 3] = RING_RECORDS as u16;
        self.tick_ring_archive();
    }

    pub fn tick_ring_archive(&mut self) {
        let now = unix_now_secs();
        let now_min = now / 60;
        let last = self.last_minute;
        if last == 0 {
            self.last_minute = now_min;
            self.append_record(now as u32);
            return;
        }
        if now_min <= last {
            return;
        }
        for m in (last + 1)..=now_min {
            self.append_record((m * 60) as u32);
        }
        self.last_minute = now_min;
    }

    pub fn tick_ring_archive_now(&mut self) {
        let now_min = unix_now_secs() / 60;
        let last = self.last_minute;
        let next_min = if last == 0 {
            now_min
        } else {
            last.saturating_add(1)
        };
        self.append_record((next_min * 60) as u32);
        self.last_minute = next_min;
    }

    pub fn last_records(&self, limit: usize) -> Vec<RingRecordPreview> {
        let mut out = Vec::with_capacity(limit);
        let cur = self.cur_index as usize;
        for off in 0..limit {
            let idx = (cur + RING_RECORDS - off) % RING_RECORDS;
            let base = RING_START_ADDR + idx * RING_RECORD_WORDS;
            let ts = self.read_u32_hi_lo(base);
            if ts == 0 {
                continue;
            }
            out.push(RingRecordPreview {
                index: idx as u16,
                ts_unix: ts,
                value0: self.read_f32_lo_hi(base + 2),
            });
        }
        out
    }

    pub fn wr_float(&mut self, ff: f32, sm: u16) {
        let bytes = ff.to_be_bytes();
        let first_u16 = u16::from_be_bytes([bytes[0], bytes[1]]);
        let second_u16 = u16::from_be_bytes([bytes[2], bytes[3]]);

        if sm as usize + 1 < DAT_LEN {
            self.dat[sm as usize] = first_u16;
            self.dat[sm as usize + 1] = second_u16;
        } else {
            panic!("Index {} out of bounds for dat (len = {})", sm, DAT_LEN);
        }
    }

    pub fn rd_float(&self, sm: u16) -> f32 {
        if sm as usize + 1 >= DAT_LEN {
            return 0.0;
        }
        let first_u16 = self.dat[sm as usize];
        let second_u16 = self.dat[sm as usize + 1];
        let b0 = ((first_u16 >> 8) & 0xFF) as u8;
        let b1 = (first_u16 & 0xFF) as u8;
        let b2 = ((second_u16 >> 8) & 0xFF) as u8;
        let b3 = (second_u16 & 0xFF) as u8;
        f32::from_be_bytes([b0, b1, b2, b3])
    }

    pub fn wr_float_holding(&mut self, ff: f32, sm: u16) {
        let bytes = ff.to_be_bytes();
        let first_u16 = u16::from_be_bytes([bytes[0], bytes[1]]);
        let second_u16 = u16::from_be_bytes([bytes[2], bytes[3]]);
        if sm as usize + 1 < DAT_LEN {
            self.dat1[sm as usize] = first_u16;
            self.dat1[sm as usize + 1] = second_u16;
        } else {
            panic!("Index {} out of bounds for dat1 (len = {})", sm, DAT_LEN);
        }
    }

    pub fn rd_float_holding(&self, sm: u16) -> f32 {
        if sm as usize + 1 >= DAT_LEN {
            return 0.0;
        }
        let first_u16 = self.dat1[sm as usize];
        let second_u16 = self.dat1[sm as usize + 1];
        let b0 = ((first_u16 >> 8) & 0xFF) as u8;
        let b1 = (first_u16 & 0xFF) as u8;
        let b2 = ((second_u16 >> 8) & 0xFF) as u8;
        let b3 = (second_u16 & 0xFF) as u8;
        f32::from_be_bytes([b0, b1, b2, b3])
    }

    #[allow(dead_code)]
    pub fn wr_u16(&mut self, ff: u16, sm: u16) {
        if sm as usize + 1 < DAT_LEN {
            self.dat[sm as usize] = ff;
        } else {
            panic!("Index {} out of bounds for dat (len = {})", sm, DAT_LEN);
        }
    }

    pub fn process_request(
        &mut self,
        inb: &[u8],
        size: u16,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let size_lim = (size as usize).min(inb.len());
        let mut outb = Vec::new();
        if size_lim < 4 {
            return Ok(outb);
        }

        let mut off: usize = 0;
        while off + 4 <= size_lim {
            let Some((ulen, frame_len)) = detect_frame_layout(inb, size_lim, off) else {
                break;
            };

            let frame = &inb[off..off + frame_len];
            let extended = ulen == 2;
            let func = frame[ulen];

            let adr_hi = frame[ulen + 1];
            let adr_lo = frame[ulen + 2];
            let val_hi = frame[ulen + 3];
            let val_lo = frame[ulen + 4];
            let addr = u16::from_be_bytes([adr_hi, adr_lo]);
            let req_val = u16::from_be_bytes([val_hi, val_lo]);

            let mut resp: Vec<u8> = Vec::new();
            resp.extend_from_slice(&frame[..ulen]);
            resp.push(func);

            match func {
                3 | 4 => {
                    let cnt_words = req_val as usize;
                    let byte_count = cnt_words * 2;
                    if extended {
                        resp.push(((byte_count >> 8) & 0xFF) as u8);
                        resp.push((byte_count & 0xFF) as u8);
                    } else {
                        resp.push((byte_count & 0xFF) as u8);
                    }
                    for i in 0..cnt_words {
                        let idx = addr as usize + i;
                        let w = if idx < DAT_LEN {
                            if func == 3 {
                                self.dat1[idx]
                            } else {
                                self.dat[idx]
                            }
                        } else {
                            0
                        };
                        resp.push((w >> 8) as u8);
                        resp.push((w & 0xFF) as u8);
                    }
                }
                5 => {
                    resp.push(adr_hi);
                    resp.push(adr_lo);
                    resp.push(val_hi);
                    resp.push(val_lo);
                    if addr <= 10_000 {
                        let on = req_val == 0xFF00 || req_val == 0x00FF || req_val == 0x0001;
                        self.last_fc05_addr = addr;
                        self.last_fc05_on = if on { 1 } else { 0 };
                        self.last_fc05_seen = 1;
                    }
                }
                6 => {
                    if (addr as usize) < DAT_LEN {
                        self.dat1[addr as usize] = req_val;
                    }
                    resp.push(adr_hi);
                    resp.push(adr_lo);
                    resp.push(val_hi);
                    resp.push(val_lo);
                }
                16 => {
                    let cnt_words = req_val as usize;
                    let data_start = off + ulen + 1 + 2 + 2 + 1;
                    for i in 0..cnt_words {
                        let p = data_start + i * 2;
                        if p + 1 >= off + frame_len - 2 {
                            break;
                        }
                        let w = u16::from_be_bytes([inb[p], inb[p + 1]]);
                        let idx = addr as usize + i;
                        if idx < DAT_LEN {
                            self.dat1[idx] = w;
                        }
                    }
                    resp.push(adr_hi);
                    resp.push(adr_lo);
                    resp.push(val_hi);
                    resp.push(val_lo);
                }
                _ => {}
            }

            add_crc(&mut resp);
            outb.extend_from_slice(&resp);
            off += frame_len;
        }

        Ok(outb)
    }
}

#[allow(dead_code, static_mut_refs)]
fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn write_u32_hi_lo(base: usize, v: u32) {
    unsafe {
        DAT[base] = ((v >> 16) & 0xFFFF) as u16;
        DAT[base + 1] = (v & 0xFFFF) as u16;
    }
}

fn write_f32_lo_hi(base: usize, v: f32) {
    let bits = v.to_bits();
    unsafe {
        DAT[base] = (bits & 0xFFFF) as u16;
        DAT[base + 1] = ((bits >> 16) & 0xFFFF) as u16;
    }
}

fn read_u32_hi_lo(base: usize) -> u32 {
    unsafe { ((DAT[base] as u32) << 16) | (DAT[base + 1] as u32) }
}

fn read_f32_lo_hi(base: usize) -> f32 {
    let bits = unsafe { (DAT[base] as u32) | ((DAT[base + 1] as u32) << 16) };
    f32::from_bits(bits)
}

fn append_record(ts_unix: u32) {
    let idx = ((CUR_INDEX.load(Ordering::Relaxed) as usize + 1) % RING_RECORDS) as u16;
    let cur = f32::from_bits(CUR_VALUE_BITS.load(Ordering::Relaxed));
    let delta = f32::from_bits(DELTA_BITS.load(Ordering::Relaxed));
    let base = RING_START_ADDR + idx as usize * RING_RECORD_WORDS;

    write_u32_hi_lo(base, ts_unix);
    for k in 0..RING_FLOATS {
        write_f32_lo_hi(base + 2 + k * 2, cur + delta * k as f32);
    }

    unsafe {
        DAT[RING_INDEX_ADDR] = idx;
        DAT[RING_INDEX_ADDR + 1] = 0;
        DAT[RING_INDEX_ADDR + 2] = RING_RECORD_WORDS as u16;
        DAT[RING_INDEX_ADDR + 3] = RING_RECORDS as u16;
    }
    CUR_INDEX.store(idx, Ordering::Relaxed);
    CUR_VALUE_BITS.store((cur + delta).to_bits(), Ordering::Relaxed);
}

pub fn ring_config() -> (usize, usize, usize, usize, usize) {
    (RING_START_ADDR, RING_INDEX_ADDR, RING_RECORDS, RING_FLOATS, RING_RECORD_WORDS)
}

#[allow(dead_code)]
pub fn ring_status() -> (u16, f32, u64) {
    (
        CUR_INDEX.load(Ordering::Relaxed),
        f32::from_bits(CUR_VALUE_BITS.load(Ordering::Relaxed)),
        LAST_MINUTE.load(Ordering::Relaxed),
    )
}

#[allow(dead_code)]
pub fn reg_u16(addr: usize) -> u16 {
    unsafe {
        if addr < DAT_LEN {
            DAT[addr]
        } else {
            0
        }
    }
}

#[allow(dead_code)]
pub fn set_reg_u16(addr: usize, value: u16) {
    unsafe {
        if addr < DAT_LEN {
            DAT[addr] = value;
        }
    }
}

#[allow(dead_code)]
pub fn last_fc05_status() -> Option<(u16, bool)> {
    if LAST_FC05_SEEN.load(Ordering::Relaxed) == 0 {
        return None;
    }
    let addr = LAST_FC05_ADDR.load(Ordering::Relaxed);
    let on = LAST_FC05_ON.load(Ordering::Relaxed) != 0;
    Some((addr, on))
}

#[allow(dead_code)]
pub fn gen_params() -> (f32, f32) {
    (
        f32::from_bits(START_VALUE_BITS.load(Ordering::Relaxed)),
        f32::from_bits(DELTA_BITS.load(Ordering::Relaxed)),
    )
}

#[allow(dead_code)]
pub fn set_gen_params(start_value: f32, delta: f32, reset_archive: bool) {
    START_VALUE_BITS.store(start_value.to_bits(), Ordering::Relaxed);
    DELTA_BITS.store(delta.to_bits(), Ordering::Relaxed);
    if reset_archive {
        init_ring_archive();
    }
}

#[allow(dead_code)]
pub fn init_ring_archive() {
    let start = f32::from_bits(START_VALUE_BITS.load(Ordering::Relaxed));
    CUR_VALUE_BITS.store(start.to_bits(), Ordering::Relaxed);
    CUR_INDEX.store((RING_RECORDS - 1) as u16, Ordering::Relaxed);
    LAST_MINUTE.store(0, Ordering::Relaxed);
    unsafe {
        DAT[RING_START_ADDR..RING_START_ADDR + RING_TOTAL_WORDS].fill(0);
        DAT[RING_INDEX_ADDR] = (RING_RECORDS - 1) as u16;
        DAT[RING_INDEX_ADDR + 1] = 0;
        DAT[RING_INDEX_ADDR + 2] = RING_RECORD_WORDS as u16;
        DAT[RING_INDEX_ADDR + 3] = RING_RECORDS as u16;
    }
    tick_ring_archive();
}

#[allow(dead_code)]
pub fn tick_ring_archive() {
    let now = unix_now_secs();
    let now_min = now / 60;
    let last = LAST_MINUTE.load(Ordering::Relaxed);
    if last == 0 {
        LAST_MINUTE.store(now_min, Ordering::Relaxed);
        append_record(now as u32);
        return;
    }
    if now_min <= last {
        return;
    }
    for m in (last + 1)..=now_min {
        append_record((m * 60) as u32);
    }
    LAST_MINUTE.store(now_min, Ordering::Relaxed);
}

#[allow(dead_code)]
pub fn tick_ring_archive_now() {
    let now_min = unix_now_secs() / 60;
    let last = LAST_MINUTE.load(Ordering::Relaxed);
    let next_min = if last == 0 { now_min } else { last.saturating_add(1) };
    append_record((next_min * 60) as u32);
    LAST_MINUTE.store(next_min, Ordering::Relaxed);
}

#[allow(dead_code)]
pub fn last_records(limit: usize) -> Vec<RingRecordPreview> {
    let mut out = Vec::with_capacity(limit);
    let cur = CUR_INDEX.load(Ordering::Relaxed) as usize;
    for off in 0..limit {
        let idx = (cur + RING_RECORDS - off) % RING_RECORDS;
        let base = RING_START_ADDR + idx * RING_RECORD_WORDS;
        let ts = read_u32_hi_lo(base);
        if ts == 0 {
            continue;
        }
        out.push(RingRecordPreview {
            index: idx as u16,
            ts_unix: ts,
            value0: read_f32_lo_hi(base + 2),
        });
    }
    out
}
pub fn crc16(data: &[u8]) -> u16 {
    const CRC16_TABLE: [u16; 256] = [
        0x0000, 0xC0C1, 0xC181, 0x0140, 0xC301, 0x03C0, 0x0280, 0xC241,
        0xC601, 0x06C0, 0x0780, 0xC741, 0x0500, 0xC5C1, 0xC481, 0x0440,
        0xCC01, 0x0CC0, 0x0D80, 0xCD41, 0x0F00, 0xCFC1, 0xCE81, 0x0E40,
        0x0A00, 0xCAC1, 0xCB81, 0x0B40, 0xC901, 0x09C0, 0x0880, 0xC841,
        0xD801, 0x18C0, 0x1980, 0xD941, 0x1B00, 0xDBC1, 0xDA81, 0x1A40,
        0x1E00, 0xDEC1, 0xDF81, 0x1F40, 0xDD01, 0x1DC0, 0x1C80, 0xDC41,
        0x1400, 0xD4C1, 0xD581, 0x1540, 0xD701, 0x17C0, 0x1680, 0xD641,
        0xD201, 0x12C0, 0x1380, 0xD341, 0x1100, 0xD1C1, 0xD081, 0x1040,
        0xF001, 0x30C0, 0x3180, 0xF141, 0x3300, 0xF3C1, 0xF281, 0x3240,
        0x3600, 0xF6C1, 0xF781, 0x3740, 0xF501, 0x35C0, 0x3480, 0xF441,
        0x3C00, 0xFCC1, 0xFD81, 0x3D40, 0xFF01, 0x3FC0, 0x3E80, 0xFE41,
        0xFA01, 0x3AC0, 0x3B80, 0xFB41, 0x3900, 0xF9C1, 0xF881, 0x3840,
        0x2800, 0xE8C1, 0xE981, 0x2940, 0xEB01, 0x2BC0, 0x2A80, 0xEA41,
        0xEE01, 0x2EC0, 0x2F80, 0xEF41, 0x2D00, 0xEDC1, 0xEC81, 0x2C40,
        0xE401, 0x24C0, 0x2580, 0xE541, 0x2700, 0xE7C1, 0xE681, 0x2640,
        0x2200, 0xE2C1, 0xE381, 0x2340, 0xE101, 0x21C0, 0x2080, 0xE041,
        0xA001, 0x60C0, 0x6180, 0xA141, 0x6300, 0xA3C1, 0xA281, 0x6240,
        0x6600, 0xA6C1, 0xA781, 0x6740, 0xA501, 0x65C0, 0x6480, 0xA441,
        0x6C00, 0xACC1, 0xAD81, 0x6D40, 0xAF01, 0x6FC0, 0x6E80, 0xAE41,
        0xAA01, 0x6AC0, 0x6B80, 0xAB41, 0x6900, 0xA9C1, 0xA881, 0x6840,
        0x7800, 0xB8C1, 0xB981, 0x7940, 0xBB01, 0x7BC0, 0x7A80, 0xBA41,
        0xBE01, 0x7EC0, 0x7F80, 0xBF41, 0x7D00, 0xBDC1, 0xBC81, 0x7C40,
        0xB401, 0x74C0, 0x7580, 0xB541, 0x7700, 0xB7C1, 0xB681, 0x7640,
        0x7200, 0xB2C1, 0xB381, 0x7340, 0xB101, 0x71C0, 0x7080, 0xB041,
        0x5000, 0x90C1, 0x9181, 0x5140, 0x9301, 0x53C0, 0x5280, 0x9241,
        0x9601, 0x56C0, 0x5780, 0x9741, 0x5500, 0x95C1, 0x9481, 0x5440,
        0x9C01, 0x5CC0, 0x5D80, 0x9D41, 0x5F00, 0x9FC1, 0x9E81, 0x5E40,
        0x5A00, 0x9AC1, 0x9B81, 0x5B40, 0x9901, 0x59C0, 0x5880, 0x9841,
        0x8801, 0x48C0, 0x4980, 0x8941, 0x4B00, 0x8BC1, 0x8A81, 0x4A40,
        0x4E00, 0x8EC1, 0x8F81, 0x4F40, 0x8D01, 0x4DC0, 0x4C80, 0x8C41,
        0x4400, 0x84C1, 0x8581, 0x4540, 0x8701, 0x47C0, 0x4680, 0x8641,
        0x8201, 0x42C0, 0x4380, 0x8341, 0x4100, 0x81C1, 0x8081, 0x4040
    ];

    let mut crc: u16 = 0xFFFF; // РќР°С‡Р°Р»СЊРЅРѕРµ Р·РЅР°С‡РµРЅРёРµ CRC
    for &byte in data.iter() {
        let xor = byte ^ (crc as u8); // XOR СЃ РјР»Р°РґС€РёРј Р±Р°Р№С‚РѕРј crc
        crc >>= 8; // РЎРґРІРёРіР°РµРј crc РІРїСЂР°РІРѕ РЅР° 8 Р±РёС‚
        crc ^= CRC16_TABLE[xor as usize]; // РџСЂРёРјРµРЅСЏРµРј С‚Р°Р±Р»РёС†Сѓ CRC
    }

    crc
}
/// Р”РѕР±Р°РІР»СЏРµС‚ CRC-16 РІ РєРѕРЅРµС† Р±СѓС„РµСЂР°
fn add_crc(buffer: &mut Vec<u8>) {
    let crc = crc16(buffer);
    buffer.push((crc & 0xFF) as u8); // РњР»Р°РґС€РёР№ Р±Р°Р№С‚
    buffer.push((crc >> 8) as u8);   // РЎС‚Р°СЂС€РёР№ Р±Р°Р№С‚
}
// РџСЂРѕРІРµСЂСЏРµС‚ CRC-16 (СЃСЂР°РІРЅРёРІР°РµС‚ РІС‹С‡РёСЃР»РµРЅРЅС‹Р№ CRC СЃ С‚РµРј, С‡С‚Рѕ РІ РєРѕРЅС†Рµ Р±СѓС„РµСЂР°)
// pub fn vCRC16(buffer: &Vec<u8>) -> bool {
//     if buffer.len() < 2 {     return false;} // РќРµРґРѕСЃС‚Р°С‚РѕС‡РЅРѕ РґР°РЅРЅС‹С… РґР»СЏ РїСЂРѕРІРµСЂРєРё
//     let len = buffer.len();
//     let expected_crc = u16::from_le_bytes([buffer[len - 2], buffer[len - 1]]);
//     let calculated_crc = crc16(&buffer[..len - 2]);
//     expected_crc == calculated_crc
// }
pub fn v_crc16(buffer: &[u8]) -> bool {
    if buffer.len() < 3 {
        return false; // РќРµРґРѕСЃС‚Р°С‚РѕС‡РЅРѕ РґР°РЅРЅС‹С… РґР»СЏ РїСЂРѕРІРµСЂРєРё
    }
    let len = buffer.len();
    let expected_crc = u16::from_le_bytes([buffer[len - 2], buffer[len - 1]]);
    let calculated_crc = crc16(&buffer[..len - 2]);
    expected_crc == calculated_crc
}
#[allow(dead_code)]
fn addw(vec: &mut Vec<u8>, num: u16) {
    vec.extend_from_slice(&num.to_le_bytes()); // Р”РѕР±Р°РІР»СЏРµРј РІ С„РѕСЂРјР°С‚Рµ little-endian
}

// РњРµРЅСЏРµРј Р±Р°Р№С‚Р°РјРё РІ size С‡РµС‚РЅРѕРµ РєРѕР»РёС‡РµСЃС‚РІРѕ
// fn set_mem(ff: &mut Vec<u8>, size: u16) {
//     let size = size as usize;
//     let mut k = 0;
//     while k < size {
//         ff.swap(k, k + 1);
//         k += 2;
//     }
// }
#[allow(dead_code)]
fn set_mem(ff: &mut [u8], size: u16) {
    let mut k: u16 = 0;
    while k + 1 < size {
        ff.swap(k as usize, (k + 1) as usize);
        k += 2;
    }
}
// Р¤СѓРЅРєС†РёСЏ РґР»СЏ РґРѕР±Р°РІР»РµРЅРёСЏ CRC РІ РєРѕРЅРµС† РјР°СЃСЃРёРІР° РґР°РЅРЅС‹С…
#[allow(dead_code)]
fn set_crc1_p(ff: &mut Vec<u8>){
    let buf = &ff[0..ff.len() - 2];
    let k = crc16(buf);
    // РњР»Р°РґС€РёР№ Рё СЃС‚Р°СЂС€РёР№ Р±Р°Р№С‚С‹ CRC
    let low_byte = (k & 0xFF) as u8;
    let high_byte = ((k >> 8) & 0xFF) as u8;

    // Р”РѕР±Р°РІР»СЏРµРј CRC Рє Р·Р°РїСЂРѕСЃСѓ
    ff.push(low_byte);
    ff.push(high_byte);
  }
use std::error::Error;

#[allow(dead_code)]
pub fn raz2(inb: Vec<u8>, outb: &mut Vec<u8>, size: u16) -> Result<(u16, Vec<u8>), Box<dyn Error>> {
    let size_lim = (size as usize).min(inb.len());
    outb.clear();
    if size_lim < 4 {
        return Ok((0, outb.clone()));
    }

    let mut off: usize = 0;
    while off + 4 <= size_lim {
        let Some((ulen, frame_len)) = detect_frame_layout(&inb, size_lim, off) else {
            break;
        };
        let extended = ulen == 2;
        if off + frame_len > size_lim {
            break;
        }

        let frame = &inb[off..off + frame_len];
        let func = frame[ulen];

        let adr_hi = frame[ulen + 1];
        let adr_lo = frame[ulen + 2];
        let val_hi = frame[ulen + 3];
        let val_lo = frame[ulen + 4];
        let addr = u16::from_be_bytes([adr_hi, adr_lo]);
        let req_val = u16::from_be_bytes([val_hi, val_lo]);

        let mut resp: Vec<u8> = Vec::new();
        resp.extend_from_slice(&frame[..ulen]); // unit id (1 or 2 bytes)
        resp.push(func);

        match func {
            3 | 4 => {
                let cnt_words = req_val as usize;
                let byte_count = cnt_words * 2;
                if extended {
                    resp.push(((byte_count >> 8) & 0xFF) as u8);
                    resp.push((byte_count & 0xFF) as u8);
                } else {
                    resp.push((byte_count & 0xFF) as u8);
                }
                for i in 0..cnt_words {
                    let w = unsafe {
                        if func == 3 {
                            DAT1[addr as usize + i]
                        } else {
                            DAT[addr as usize + i]
                        }
                    };
                    resp.push((w >> 8) as u8);
                    resp.push((w & 0xFF) as u8);
                }
            }
            5 => {
                resp.push(adr_hi);
                resp.push(adr_lo);
                resp.push(val_hi);
                resp.push(val_lo);
                if addr <= 10_000 {
                    let on = req_val == 0xFF00 || req_val == 0x00FF || req_val == 0x0001;
                    LAST_FC05_ADDR.store(addr, Ordering::Relaxed);
                    LAST_FC05_ON.store(if on { 1 } else { 0 }, Ordering::Relaxed);
                    LAST_FC05_SEEN.store(1, Ordering::Relaxed);
                }
            }
            6 => {
                unsafe {
                    if (addr as usize) < DAT_LEN {
                        DAT1[addr as usize] = req_val;
                    }
                }
                // FC6 response echoes addr and written value.
                resp.push(adr_hi);
                resp.push(adr_lo);
                resp.push(val_hi);
                resp.push(val_lo);
            }
            16 => {
                let cnt_words = req_val as usize;
                let data_start = off + ulen + 1 + 2 + 2 + 1;
                for i in 0..cnt_words {
                    let p = data_start + i * 2;
                    if p + 1 >= off + frame_len - 2 {
                        break;
                    }
                    let w = u16::from_be_bytes([inb[p], inb[p + 1]]);
                    unsafe {
                        DAT1[addr as usize + i] = w;
                    }
                }
                // FC16 response echoes start addr and quantity.
                resp.push(adr_hi);
                resp.push(adr_lo);
                resp.push(val_hi);
                resp.push(val_lo);
            }
            _ => {}
        }

        add_crc(&mut resp);
        outb.extend_from_slice(&resp);
        off += frame_len;
    }

    Ok((outb.len() as u16, outb.clone()))
}

pub fn raz2_with_slot(
    slot: &mut SlotState,
    inb: &[u8],
    size: u16,
) -> Result<(u16, Vec<u8>), Box<dyn Error>> {
    let outb = slot.process_request(inb, size)?;
    Ok((outb.len() as u16, outb))
}

#[allow(dead_code, static_mut_refs)]
pub fn load_globals_into_slot(slot: &mut SlotState) {
    unsafe {
        slot.dat.copy_from_slice(&DAT);
        slot.dat1.copy_from_slice(&DAT1);
    }
    slot.last_minute = LAST_MINUTE.load(Ordering::Relaxed);
    slot.cur_index = CUR_INDEX.load(Ordering::Relaxed);
    slot.cur_value_bits = CUR_VALUE_BITS.load(Ordering::Relaxed);
    slot.start_value_bits = START_VALUE_BITS.load(Ordering::Relaxed);
    slot.delta_bits = DELTA_BITS.load(Ordering::Relaxed);
    slot.last_fc05_addr = LAST_FC05_ADDR.load(Ordering::Relaxed);
    slot.last_fc05_on = LAST_FC05_ON.load(Ordering::Relaxed);
    slot.last_fc05_seen = LAST_FC05_SEEN.load(Ordering::Relaxed);
}

#[allow(dead_code, static_mut_refs)]
pub fn store_slot_into_globals(slot: &SlotState) {
    unsafe {
        DAT.copy_from_slice(&slot.dat);
        DAT1.copy_from_slice(&slot.dat1);
    }
    LAST_MINUTE.store(slot.last_minute, Ordering::Relaxed);
    CUR_INDEX.store(slot.cur_index, Ordering::Relaxed);
    CUR_VALUE_BITS.store(slot.cur_value_bits, Ordering::Relaxed);
    START_VALUE_BITS.store(slot.start_value_bits, Ordering::Relaxed);
    DELTA_BITS.store(slot.delta_bits, Ordering::Relaxed);
    LAST_FC05_ADDR.store(slot.last_fc05_addr, Ordering::Relaxed);
    LAST_FC05_ON.store(slot.last_fc05_on, Ordering::Relaxed);
    LAST_FC05_SEEN.store(slot.last_fc05_seen, Ordering::Relaxed);
}

#[allow(dead_code)]
pub fn wr_float(ff: f32,sm: u16){
let bytes = ff.to_be_bytes(); // [b0, b1, b2, b3], например 16.300 -> 41 82 66 67
let first_u16 = u16::from_be_bytes([bytes[0], bytes[1]]);
let second_u16 = u16::from_be_bytes([bytes[2], bytes[3]]);
unsafe {
if sm as usize + 1 < DAT_LEN {
DAT[sm as usize] = first_u16;
DAT[sm as usize + 1] = second_u16;
} else {
panic!("Index {} out of bounds for DAT (len = {})", sm, DAT_LEN);
}
}
}
#[allow(dead_code)]
pub fn rd_float(sm: u16) -> f32 {
unsafe {
if sm as usize + 1 >= DAT_LEN {
return 0.0;
}
let first_u16 = DAT[sm as usize];
let second_u16 = DAT[sm as usize + 1];
let b0 = ((first_u16 >> 8) & 0xFF) as u8;
let b1 = (first_u16 & 0xFF) as u8;
let b2 = ((second_u16 >> 8) & 0xFF) as u8;
let b3 = (second_u16 & 0xFF) as u8;
f32::from_be_bytes([b0, b1, b2, b3])
}
}

#[allow(dead_code)]
pub fn wr_float_holding(ff: f32,sm: u16){
let bytes = ff.to_be_bytes();
let first_u16 = u16::from_be_bytes([bytes[0], bytes[1]]);
let second_u16 = u16::from_be_bytes([bytes[2], bytes[3]]);
unsafe {
if sm as usize + 1 < DAT_LEN {
DAT1[sm as usize] = first_u16;
DAT1[sm as usize + 1] = second_u16;
} else {
panic!("Index {} out of bounds for DAT1 (len = {})", sm, DAT_LEN);
}
}
}

#[allow(dead_code)]
pub fn rd_float_holding(sm: u16) -> f32 {
unsafe {
if sm as usize + 1 >= DAT_LEN {
return 0.0;
}
let first_u16 = DAT1[sm as usize];
let second_u16 = DAT1[sm as usize + 1];
let b0 = ((first_u16 >> 8) & 0xFF) as u8;
let b1 = (first_u16 & 0xFF) as u8;
let b2 = ((second_u16 >> 8) & 0xFF) as u8;
let b3 = (second_u16 & 0xFF) as u8;
f32::from_be_bytes([b0, b1, b2, b3])
}
}

#[allow(dead_code)]
pub fn wr_u16(ff: u16,sm: u16){

  //  let ff1 = ff.swap_bytes(); // 0x3412 (РїРѕРјРµРЅСЏР»РёСЃСЊ РјРµСЃС‚Р°РјРё)
    // Р—Р°РїРёСЃС‹РІР°РµРј РІ DAT СЃ РїСЂРѕРІРµСЂРєРѕР№ РіСЂР°РЅРёС†
    unsafe {
        // РџСЂРѕРІРµСЂСЏРµРј, С‡С‚РѕР±С‹ sm Рё sm+1 РЅРµ РІС‹С…РѕРґРёР»Рё Р·Р° РїСЂРµРґРµР»С‹ DAT
        if sm as usize + 1 < DAT_LEN {
            DAT[sm as usize] = ff;
       } else {
            panic!("Index {} out of bounds for DAT (len = {})", sm, DAT_LEN);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_read_req(unit: &[u8], func: u8, addr: u16, count: u16) -> Vec<u8> {
        let mut frame = Vec::new();
        frame.extend_from_slice(unit);
        frame.push(func);
        frame.extend_from_slice(&addr.to_be_bytes());
        frame.extend_from_slice(&count.to_be_bytes());
        add_crc(&mut frame);
        frame
    }

    #[test]
    fn detect_frame_layout_uses_extended_only_for_f8_to_fe_prefix() {
        let ext = mk_read_req(&[0xF8, 0x35], 4, 0x001E, 4);
        let plain = mk_read_req(&[0x35], 4, 0x001E, 4);

        assert_eq!(detect_frame_layout(&ext, ext.len(), 0), Some((2, 9)));
        assert_eq!(detect_frame_layout(&plain, plain.len(), 0), Some((1, 8)));
    }

    #[test]
    fn process_request_builds_extended_response_for_rtu_301() {
        let mut slot = SlotState::new();
        slot.dat[0x001E] = 0x1122;
        slot.dat[0x001F] = 0x3344;
        slot.dat[0x0020] = 0x5566;
        slot.dat[0x0021] = 0x7788;

        let req = mk_read_req(&[0xF8, 0x35], 4, 0x001E, 4);
        let resp = slot.process_request(&req, req.len() as u16).expect("response");

        assert_eq!(&resp[..2], &[0xF8, 0x35]);
        assert_eq!(resp[2], 4);
        assert_eq!(resp[3], 0);
        assert_eq!(resp[4], 8);
        assert_eq!(
            &resp[5..13],
            &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]
        );
        assert!(v_crc16(&resp));
    }

    #[test]
    fn process_request_keeps_plain_modbus_for_unit_below_f8() {
        let mut slot = SlotState::new();
        slot.dat[0x001E] = 0xA1B2;

        let req = mk_read_req(&[0x35], 4, 0x001E, 1);
        let resp = slot.process_request(&req, req.len() as u16).expect("response");

        assert_eq!(resp[0], 0x35);
        assert_eq!(resp[1], 4);
        assert_eq!(resp[2], 2);
        assert_eq!(&resp[3..5], &[0xA1, 0xB2]);
        assert!(v_crc16(&resp));
    }
}





