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
