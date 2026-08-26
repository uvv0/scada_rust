use bytes::BytesMut;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct UdpParams {
    pub kan: u8,
    pub speed: u8,
    pub stop: u8,
    pub par: u8,
    pub data: u8,
    pub rtu: u16,
    pub out_max: u16,
    pub packet_id: u8,
    pub pkt_type: u8, // 0=req, 1=resp
    pub dsr: u16,
    pub modem: u16,
    pub port: u16,
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

pub fn crc16(data: &[u8]) -> u16 {
    const TABLE: [u16; 256] = [
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
        0x8201, 0x42C0, 0x4380, 0x8341, 0x4100, 0x81C1, 0x8081, 0x4040,
    ];

    let mut crc: u16 = 0xFFFF;
    for &b in data {
        let xor = (b as u16) ^ (crc & 0x00FF);
        crc = (crc >> 8) ^ TABLE[xor as usize];
    }
    crc & 0xFFFF
}

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

pub fn sout_mb_only(rtu: u16, tip: u8, adr: i32, size: u16, dat: Option<&[u8]>) -> Result<Vec<u8>, String> {
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

    if adr0 < 0 || adr0 > 0xFFFF {
        return Err(format!("adr0 out of range: adrHuman={} adr0={}", adr_human, adr0));
    }

    let mut outb = BytesMut::with_capacity(16 + dat.map(|d| d.len()).unwrap_or(0));

    if rtu > 247 {
        let mut r1: i32 = rtu as i32 - 248;
        let r2: i32 = r1 / 256;
        let r3: i32 = 248 + r2;
        r1 -= 248 * r2;
        outb.extend_from_slice(&[(r3 & 0xFF) as u8, (r1 & 0xFF) as u8]);
    } else {
        outb.extend_from_slice(&[(rtu & 0xFF) as u8]);
    }

    outb.extend_from_slice(&[
        func,
        ((adr0 >> 8) & 0xFF) as u8,
        (adr0 & 0xFF) as u8,
    ]);

    if !is_write {
        outb.extend_from_slice(&[
            ((size_words >> 8) & 0xFF) as u8,
            (size_words & 0xFF) as u8,
        ]);
    } else if func == 5 || func == 6 {
        let d = dat.unwrap();
        let b0 = d.get(0).cloned().unwrap_or(0);
        let b1 = d.get(1).cloned().unwrap_or(0);
        outb.extend_from_slice(&[b0, b1]);
    } else if func == 16 {
        let d = dat.unwrap();
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
pub fn sout(rtu: u16, tip: u8, adr: i32, size: u16, dat: Option<&[u8]>, par: Option<&UdpParams>) -> Result<Vec<u8>, String> {
    let mb = sout_mb_only(rtu, tip, adr, size, dat)?;
    let Some(par) = par else { return Ok(mb) };
    let mmax = 22 + mb.len();
    let header = shab(par, mmax);
    let mut out = Vec::with_capacity(mmax);
    out.extend_from_slice(&header);
    out.extend_from_slice(&mb);
    Ok(out)
}

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
    let crc = ((crc_hi << 8) | crc_lo) & 0xFFFF;
    crc16(body) == crc
}

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
        if end > buf.len() { return None; }
        let frame = &buf[offset..end];
        if crc_ok(frame) {
            return Some((frame.to_vec(), end));
        }
        return None;
    }

    if func == 16 {
        let need = ulen + 1 + 4 + 2;
        let end = offset + need;
        if end > buf.len() { return None; }
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
            if need == 0 { continue; }
            let end = offset + need;
            if end > buf.len() { continue; }
            let frame = &buf[offset..end];
            if crc_ok(frame) {
                return Some((frame.to_vec(), end));
            }
        }
        return None;
    }

    None
}

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
            return Err(format!("single cmd too large for limit={} size={}", limit, HDR22 + mb.len()));
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
