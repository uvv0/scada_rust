use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicU16, AtomicU8, Ordering};
use std::time::Duration;

use crate::modbus;

#[derive(Clone, Debug)]
pub struct ServiceConn {
    pub ip: String,
    pub port: u16,
    pub rtu: u16,
    pub modem: u16,
    pub kan: u8,
    pub speed: u8,
    pub stop: u8,
    pub par: u8,
    pub data: u8,
    pub max_pkt_len: usize,
}

#[derive(Clone, Debug)]
pub struct ReadItem {
    pub id: i32,
    pub mb: i32,
    pub tip: i32,
    pub bits: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct ReadGroupResult {
    pub values_by_id: HashMap<i32, f64>,
    pub tx: Vec<u8>,
    pub rx: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ReadReq {
    pub func: u8,
    pub addr_human: i32,
    pub cnt_words: i32,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ModbusResultReq {
    pub func: u8,
    pub addr_human: i32,
    pub cnt_words: i32,
    pub response: Option<Vec<u8>>,
    pub status: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ModbusResultMultiReq {
    pub results: Vec<ModbusResultReq>,
    pub request: Vec<u8>,
    pub response: Option<Vec<u8>>,
    pub status: String,
    pub trace_lines: Vec<String>,
}

/// Function: $name.
pub fn read_group_glued(
    conn: &ServiceConn,
    func: u8,
    items: &[ReadItem],
    timeout: Duration,
) -> Result<ReadGroupResult, String> {
    let mut regs: Vec<ReadItem> = items.to_vec();
    if regs.is_empty() {
        return Ok(ReadGroupResult {
            values_by_id: HashMap::new(),
            tx: Vec::new(),
            rx: Vec::new(),
        });
    }
    regs.sort_by_key(|r| r.mb);
    let max_words_by_pkt = ((conn.max_pkt_len.saturating_sub(64)) / 2).clamp(16, 240) as i32;
    let mut blocks: Vec<(i32, i32)> = Vec::new();
    let mut start = regs[0].mb;
    let mut end = start + if matches!(regs[0].tip, 2 | 4 | 5) { 2 } else { 1 } - 1;
    for r in regs.iter().skip(1) {
        let r_end = r.mb + if matches!(r.tip, 2 | 4 | 5) { 2 } else { 1 } - 1;
        let gap = r.mb > end + 1;
        let would = r_end - start + 1;
        if gap || would > max_words_by_pkt {
            blocks.push((start, end));
            start = r.mb;
            end = r_end;
        } else if r_end > end {
            end = r_end;
        }
    }
    blocks.push((start, end));

    let mut word_by_addr: HashMap<i32, u16> = HashMap::new();
    let mut tx_last: Vec<u8> = Vec::new();
    let mut rx_last: Vec<u8> = Vec::new();
    for (first_addr, last_addr) in blocks {
        let cnt_words = (last_addr - first_addr + 1).max(1) as u16;
        let mb = modbus::sout_mb_only(conn.rtu, func, first_addr, cnt_words, None)?;
        let (tx, resp_res) = send_mb_over_udp(conn, &mb, timeout);
        let resp = match resp_res {
            Ok(v) => v,
            Err(e) => return Err(format!("{e}; tx={}", hex_join(&tx))),
        };
        let words = parse_read_words_from_resp(&resp, func)?;
        for (i, w) in words.iter().enumerate() {
            word_by_addr.insert(first_addr + i as i32, *w);
        }
        tx_last = tx;
        rx_last = resp;
    }

    let mut values_by_id: HashMap<i32, f64> = HashMap::new();
    for r in &regs {
        if matches!(r.tip, 2 | 4 | 5) {
            let hi = *word_by_addr.get(&r.mb).unwrap_or(&0);
            let lo = *word_by_addr.get(&(r.mb + 1)).unwrap_or(&0);
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
            values_by_id.insert(r.id, v);
            continue;
        }
        if r.tip == 0 {
            if let Some(bit) = r.bits {
                if (0..=15).contains(&bit) {
                    let w = *word_by_addr.get(&r.mb).unwrap_or(&0);
                    values_by_id.insert(r.id, ((w >> bit) & 1) as f64);
                    continue;
                }
            }
        }
        let w = *word_by_addr.get(&r.mb).unwrap_or(&0);
        let v = match r.tip {
            1 => (w as i16) as f64,
            _ => w as f64,
        };
        values_by_id.insert(r.id, v);
    }

    Ok(ReadGroupResult {
        values_by_id,
        tx: tx_last,
        rx: rx_last,
    })
}

fn next_dsr() -> u16 {
    static DSR: AtomicU16 = AtomicU16::new(1);
    let v = DSR.fetch_add(1, Ordering::Relaxed);
    if v == 0 { 1 } else { v }
}

fn next_pid() -> u8 {
    static PID: AtomicU8 = AtomicU8::new(0);
    PID.fetch_add(1, Ordering::Relaxed)
}

fn send_chunk(
    conn: &ServiceConn,
    tx: &[u8],
    pid: u8,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("udp bind failed: {e}"))?;
    sock.set_read_timeout(Some(timeout))
        .map_err(|e| format!("udp timeout set failed: {e}"))?;
    sock.send_to(tx, format!("{}:{}", conn.ip, conn.port))
        .map_err(|e| format!("udp send failed: {e}"))?;

    let mut buf = vec![0u8; conn.max_pkt_len.max(65535)];
    loop {
        let (n, _) = sock
            .recv_from(&mut buf)
            .map_err(|e| format!("udp recv failed: {e}"))?;
        if n < 12 {
            continue;
        }
        let pkt = &buf[..n];
        if pkt[3] != pid || pkt[4] != 1 {
            continue;
        }
        return Ok(pkt.to_vec());
    }
}

#[allow(dead_code)]
pub fn request_reqs_glued(
    conn: &ServiceConn,
    reqs: &[ReadReq],
    timeout_per_chunk: Duration,
    idle_timeout: Duration,
) -> Result<ModbusResultMultiReq, String> {
    let mut results: Vec<ModbusResultReq> = Vec::new();
    let mut plans: Vec<(u8, i32, i32)> = Vec::new();
    let mut mb_cmds: Vec<Vec<u8>> = Vec::new();
    let mut tx_all: Vec<u8> = Vec::new();
    let mut rx_all: Vec<u8> = Vec::new();

    for r in reqs {
        if r.cnt_words <= 0 || !(r.func == 3 || r.func == 4) {
            continue;
        }
        let mb = modbus::sout_mb_only(conn.rtu, r.func, r.addr_human, r.cnt_words as u16, None)?;
        mb_cmds.push(mb);
        plans.push((r.func, r.addr_human, r.cnt_words));
    }

    if mb_cmds.is_empty() {
        return Ok(ModbusResultMultiReq {
            results,
            request: vec![],
            response: None,
            status: "ERROR: no commands".to_string(),
            trace_lines: Vec::new(),
        });
    }

    let mb_chunks = modbus::build_mb_chunks(&mb_cmds, conn.max_pkt_len)
        .map_err(|e| format!("chunk build failed: {e}"))?;
    let mut rx_virtual_all: Vec<Vec<u8>> = Vec::new();
    let mut trace_lines: Vec<String> = Vec::new();

    for (chunk_idx, chunk_frames) in mb_chunks.into_iter().enumerate() {
        let pid = next_pid();
        let par = modbus::UdpParams {
            kan: conn.kan,
            speed: conn.speed,
            stop: conn.stop,
            par: conn.par,
            data: conn.data,
            rtu: conn.rtu,
            modem: conn.modem,
            port: conn.port,
            ip: conn.ip.clone(),
            packet_id: pid,
            pkt_type: 0,
            dsr: next_dsr(),
            ..Default::default()
        };
        let payload: usize = chunk_frames.iter().map(|m| m.len()).sum();
        let total_len = 22 + payload;
        let mut tx = modbus::shab(&par, total_len);
        for mb in &chunk_frames {
            tx.extend_from_slice(mb);
        }
        tx_all.extend_from_slice(&tx);
        trace_lines.push(format!(
            "CHUNK {} cmds={} header+payload tx=[{}]",
            chunk_idx + 1,
            chunk_frames.len(),
            tx.iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ")
        ));

        let rx = send_chunk(conn, &tx, pid, timeout_per_chunk + idle_timeout)?;
        rx_all.extend_from_slice(&rx);
        trace_lines.push(format!(
            "CHUNK {} rx=[{}]",
            chunk_idx + 1,
            rx.iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ")
        ));
        rx_virtual_all.extend(modbus::split_rx_to_virtual(&rx));
    }

    let n = std::cmp::min(plans.len(), rx_virtual_all.len());
    for i in 0..n {
        let (func, addr, cnt) = plans[i];
        results.push(ModbusResultReq {
            func,
            addr_human: addr,
            cnt_words: cnt,
            response: Some(rx_virtual_all[i].clone()),
            status: "OK".to_string(),
        });
    }

    Ok(ModbusResultMultiReq {
        results,
        request: tx_all,
        response: if rx_all.is_empty() { None } else { Some(rx_all) },
        status: if n < plans.len() {
            format!("WARN: responses < commands ({}/{})", n, plans.len())
        } else {
            "OK".to_string()
        },
        trace_lines,
    })
}

/// Function: $name.
fn send_mb_over_udp(
    conn: &ServiceConn,
    mb: &[u8],
    timeout: Duration,
) -> (Vec<u8>, Result<Vec<u8>, String>) {
    let pid = next_packet_id();
    let par = modbus::UdpParams {
        kan: conn.kan,
        speed: conn.speed,
        stop: conn.stop,
        par: conn.par,
        data: conn.data,
        rtu: conn.rtu,
        modem: conn.modem,
        port: conn.port,
        ip: conn.ip.clone(),
        packet_id: pid,
        pkt_type: 0,
        ..Default::default()
    };
    let header = modbus::shab(&par, 22 + mb.len());
    let mut tx: Vec<u8> = Vec::with_capacity(22 + mb.len());
    tx.extend_from_slice(&header);
    tx.extend_from_slice(mb);

    let sock = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => return (tx, Err(format!("udp bind failed: {e}"))),
    };
    if let Err(e) = sock.set_read_timeout(Some(timeout)) {
        return (tx, Err(format!("udp timeout set failed: {e}")));
    }
    if let Err(e) = sock.send_to(&tx, format!("{}:{}", conn.ip, conn.port)) {
        return (tx, Err(format!("udp send failed: {e}")));
    }

    let mut buf = vec![0u8; conn.max_pkt_len.max(65535)];
    loop {
        let (n, _) = match sock.recv_from(&mut buf) {
            Ok(v) => v,
            Err(e) => return (tx, Err(format!("udp recv failed: {e}"))),
        };
        if n < 12 {
            continue;
        }
        let pkt = &buf[..n];
        if pkt[3] != pid || pkt[4] != 1 {
            continue;
        }
        return (tx, Ok(pkt.to_vec()));
    }
}

/// Function: $name.
fn parse_read_words_from_resp(resp: &[u8], expected_func: u8) -> Result<Vec<u16>, String> {
    let mb = modbus::extract_modbus_frame(resp).ok_or_else(|| "short response".to_string())?;
    if mb.len() <= 4 {
        return Err("short response".to_string());
    }
    let ulen = if mb[0] >= 0xF8 { 2 } else { 1 };
    let fi = ulen;
    if mb.len() <= fi + 1 {
        return Err("bad response frame".to_string());
    }
    let func = mb[fi];
    if (func & 0x80) != 0 {
        return Err(format!("modbus exception func={func}"));
    }
    if func != expected_func {
        return Err(format!("unexpected func={}, expected={}", func, expected_func));
    }
    let (byte_count, data_start) = if ulen == 2 {
        if mb.len() < fi + 3 {
            return Err("short 2-byte-len frame".to_string());
        }
        ((((mb[fi + 1] as usize) << 8) | (mb[fi + 2] as usize)), fi + 3)
    } else {
        (mb[fi + 1] as usize, fi + 2)
    };
    if byte_count == 0 || (byte_count % 2) != 0 {
        return Err(format!("bad byte_count={}", byte_count));
    }
    if mb.len() < data_start + byte_count {
        return Err("short data".to_string());
    }
    let mut out = Vec::with_capacity(byte_count / 2);
    let data = &mb[data_start..data_start + byte_count];
    for i in 0..(byte_count / 2) {
        let hi = data[i * 2] as u16;
        let lo = data[i * 2 + 1] as u16;
        out.push((hi << 8) | lo);
    }
    Ok(out)
}

/// Function: $name.
fn next_packet_id() -> u8 {
    static PID: AtomicU8 = AtomicU8::new(0);
    PID.fetch_add(1, Ordering::Relaxed)
}

/// Function: $name.
fn hex_join(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}
