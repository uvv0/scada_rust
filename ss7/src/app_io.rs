use std::net::UdpSocket;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use crate::modbus;

#[derive(Clone, Debug)]
pub(crate) struct IoConn {
    pub(crate) ip: String,
    pub(crate) port: u16,
    pub(crate) rtu: u16,
    pub(crate) modem: u16,
    pub(crate) timeout_ms: u64,
    pub(crate) kan: u8,
    pub(crate) speed: u8,
    pub(crate) stop: u8,
    pub(crate) par: u8,
    pub(crate) data: u8,
    pub(crate) max_pkt_len: usize,
}

fn next_packet_id() -> u8 {
    static PID: AtomicU8 = AtomicU8::new(0);
    PID.fetch_add(1, Ordering::Relaxed)
}

pub(crate) fn send_mb_over_udp(conn: &IoConn, mb: &[u8], timeout: Duration) -> (Vec<u8>, Result<Vec<u8>, String>) {
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

pub(crate) fn validate_modbus_response(resp: &[u8], expected_func: u8) -> Result<(), String> {
    let mb = modbus::extract_modbus_frame(resp).ok_or_else(|| "short response".to_string())?;
    if mb.len() <= 4 {
        return Err("short modbus response".to_string());
    }
    let ulen = if mb[0] >= 0xF8 { 2 } else { 1 };
    if mb.len() <= ulen {
        return Err("bad response frame".to_string());
    }
    let func = mb[ulen];
    if (func & 0x80) != 0 {
        return Err(format!("modbus exception func={func}"));
    }
    if func != expected_func {
        return Err(format!("unexpected func={}, expected={}", func, expected_func));
    }
    Ok(())
}

pub(crate) fn parse_read_words_from_resp(resp: &[u8], expected_func: u8) -> Result<Vec<u16>, String> {
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

pub(crate) fn hex_join(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}
