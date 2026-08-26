//! Модуль высокоуровневого Modbus-обмена (glued запросы по группам/блокам) с агрегацией ответов.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;

use crate::modbus;
use crate::reg::Reg;
use crate::udp_transport::UdpCorrelatedTransport;

#[derive(Clone, Debug)]
#[allow(dead_code)]
/// Группа регистров для одного Modbus-запроса (чтение/запись).
pub struct ReadWriteGroup {
    pub func: u8, // 3/4/16
    pub regs: Vec<Reg>,
    pub write_val_by_id: Option<HashMap<i32, i32>>,
}

#[derive(Clone, Debug)]
/// Нормализованный read-запрос по функции/адресу/количеству слов.
pub struct ReadReq {
    pub func: u8,
    pub addr_human: i32,
    pub cnt_words: i32,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
/// Результат чтения одной группы регистров с декодированными значениями.
pub struct ModbusResultReg {
    pub regs: Vec<Reg>,
    pub values_by_id: HashMap<i32, f64>,
    pub response: Option<Vec<u8>>,
    pub status: String,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
/// Сводный результат выполнения нескольких групповых Modbus-запросов.
pub struct ModbusResultMulti {
    pub results: Vec<ModbusResultReg>,
    pub request: Vec<u8>,
    pub response: Option<Vec<u8>>,
    pub status: String,
}

#[derive(Clone, Debug)]
/// Результат выполнения одного запроса из списка `ReadReq`.
pub struct ModbusResultReq {
    pub func: u8,
    pub addr_human: i32,
    pub cnt_words: i32,
    pub response: Option<Vec<u8>>,
    #[allow(dead_code)]
    pub status: String,
}

#[derive(Clone, Debug)]
/// Сводный результат выполнения пакета запросов `ReadReq`.
pub struct ModbusResultMultiReq {
    pub results: Vec<ModbusResultReq>,
    pub request: Vec<u8>,
    #[allow(dead_code)]
    pub response: Option<Vec<u8>>,
    pub status: String,
}

#[allow(dead_code)]
fn calc_registers_count(regs: &[Reg]) -> i32 {
    let mut n = 0;
    let mut bit_addrs = std::collections::HashSet::new();
    for r in regs {
        if r.is_bit() {
            if bit_addrs.insert(r.addr) {
                n += 1;
            }
            continue;
        }
        n += if r.is_32() { 2 } else { 1 };
    }
    n
}

#[allow(dead_code)]
fn parse_values_from_read_response(mb: &[u8], regs: &[Reg], first_addr: i32) -> HashMap<i32, f64> {
    let mut out = HashMap::new();
    if mb.len() < 4 {
        return out;
    }

    let ulen = if mb[0] >= 0xF8 { 2 } else { 1 };
    let fi = ulen;
    if mb.len() <= fi + 1 {
        return out;
    }

    let func = mb[fi];
    if func != 3 && func != 4 {
        return out;
    }

    let (byte_count, data_start) = if ulen == 2 {
        if mb.len() < fi + 3 {
            return out;
        }
        let bc = ((mb[fi + 1] as usize) << 8) | (mb[fi + 2] as usize);
        (bc, fi + 3)
    } else {
        if mb.len() < fi + 2 {
            return out;
        }
        (mb[fi + 1] as usize, fi + 2)
    };

    let data_end = data_start + byte_count;
    if data_end > mb.len() {
        return out;
    }
    let data = &mb[data_start..data_end];

    let mut word_by_addr: HashMap<i32, i32> = HashMap::new();
    let words = data.len() / 2;
    for i in 0..words {
        let hi = data[i * 2] as i32;
        let lo = data[i * 2 + 1] as i32;
        let w = ((hi << 8) | lo) & 0xFFFF;
        word_by_addr.insert(first_addr + i as i32, w);
    }

    for r in regs {
        if r.is_bit() {
            if let Some(bit) = r.bits {
                let w = *word_by_addr.get(&r.addr).unwrap_or(&0);
                let v = ((w >> bit) & 1) as f64;
                out.insert(r.id, v);
            }
            continue;
        }

        if r.is_32() {
            let hi = *word_by_addr.get(&r.addr).unwrap_or(&0) as u16;
            let lo = *word_by_addr.get(&(r.addr + 1)).unwrap_or(&0) as u16;
            let bytes = [
                ((hi >> 8) & 0xFF) as u8,
                (hi & 0xFF) as u8,
                ((lo >> 8) & 0xFF) as u8,
                (lo & 0xFF) as u8,
            ];
            let val = match r.tip {
                5 => f32::from_be_bytes(bytes) as f64,
                4 => u32::from_be_bytes(bytes) as f64,
                2 => i32::from_be_bytes(bytes) as f64,
                _ => u32::from_be_bytes(bytes) as f64,
            };
            out.insert(r.id, val);
            continue;
        }

        let v = *word_by_addr.get(&r.addr).unwrap_or(&0) as f64;
        out.insert(r.id, v);
    }

    out
}

#[allow(dead_code)]
/// Выполняет набор групповых запросов, склеивая несколько Modbus-кадров в UDP-чанки.
///
/// # Parameters
/// - `transport`: UDP-транспорт с корреляцией ответов.
/// - `obj_row`: параметры канала (kanal/speed/stop/parit/bit).
/// - `ip`, `port`, `rtu`, `modem`: адрес и маршрутизация устройства.
/// - `groups`: список групп регистров для запроса.
/// - `limit`: лимит размера UDP-пакета.
/// - `timeout_per_chunk`: таймаут ожидания ответа на чанк.
/// - `idle_timeout`: таймаут дозагрузки дополнительных пакетов.
///
/// # Returns
/// - `Ok(ModbusResultMulti)`: данные по группам, сырой request/response и статус.
/// - `Err(...)`: ошибка отправки/получения/кодирования запроса.
pub async fn request_groups_glued(
    transport: &UdpCorrelatedTransport,
    obj_row: &HashMap<String, i32>,
    ip: &str,
    port: u16,
    rtu: i32,
    modem: i32,
    groups: &[ReadWriteGroup],
    limit: usize,
    timeout_per_chunk: Duration,
    idle_timeout: Duration,
) -> Result<ModbusResultMulti> {
    let mut results: Vec<ModbusResultReg> = Vec::new();
    let mut plans: Vec<(u8, Vec<Reg>, i32)> = Vec::new();
    let mut mb_cmds: Vec<Vec<u8>> = Vec::new();

    let mut tx_all: Vec<u8> = Vec::new();
    let mut rx_all: Vec<u8> = Vec::new();

    for g0 in groups {
        let mut regs = g0.regs.clone();
        if regs.is_empty() {
            continue;
        }
        regs.sort_by_key(|r| r.addr);
        let Some(first_reg) = regs.first() else {
            continue;
        };
        let first_addr = first_reg.addr;
        let regs_count = calc_registers_count(&regs);

        if g0.func == 16 {
            // write not supported in this path
            continue;
        }

        let mb = modbus::sout_mb_only(rtu as u16, g0.func, first_addr, regs_count as u16, None)
            .map_err(|e| anyhow::anyhow!(e))?;
        mb_cmds.push(mb);
        plans.push((g0.func, regs, first_addr));
    }

    if mb_cmds.is_empty() {
        return Ok(ModbusResultMulti {
            results,
            request: vec![],
            response: None,
            status: "ERROR: no commands".to_string(),
        });
    }

    let mb_chunks = modbus::build_mb_chunks(&mb_cmds, limit).map_err(|e| anyhow::anyhow!(e))?;

    let mut rx_virtual_all: Vec<Vec<u8>> = Vec::new();

    for chunk_frames in mb_chunks {
        let pid = next_pid(ip, port, timeout_per_chunk + idle_timeout);
        let par = modbus::UdpParams {
            kan: obj_row.get("kanal").copied().unwrap_or(3) as u8,
            speed: obj_row.get("speed").copied().unwrap_or(8) as u8,
            stop: obj_row.get("stop").copied().unwrap_or(0) as u8,
            par: obj_row.get("parit").copied().unwrap_or(2) as u8,
            dsr: next_dsr(),
            data: obj_row.get("bit").copied().unwrap_or(8) as u8,
            rtu: rtu as u16,
            modem: modem as u16,
            port,
            ip: ip.to_string(),
            packet_id: pid,
            pkt_type: 0,
            ..Default::default()
        };

        let mut payload = 0usize;
        for mb in &chunk_frames {
            payload += mb.len();
        }
        let total_len = 22 + payload;
        let header = modbus::shab(&par, total_len);

        let mut tx: Vec<u8> = Vec::with_capacity(total_len);
        tx.extend_from_slice(&header);
        for mb in &chunk_frames {
            tx.extend_from_slice(mb);
        }
        tx_all.extend_from_slice(&tx);

        let rx = transport
            .send(&tx, ip, port, timeout_per_chunk, true, idle_timeout)
            .await?
            .ok_or_else(|| anyhow::anyhow!("timeout"))?;

        rx_all.extend_from_slice(&rx);
        let parts = modbus::split_rx_to_virtual(&rx);
        rx_virtual_all.extend(parts);
    }

    let n = std::cmp::min(plans.len(), rx_virtual_all.len());
    for i in 0..n {
        let (func, regs, first_addr) = plans[i].clone();
        let v_pkt = &rx_virtual_all[i];
        let mb = modbus::extract_modbus_frame(v_pkt).unwrap_or(&[]);
        let parsed = if func != 16 {
            parse_values_from_read_response(mb, &regs, first_addr)
        } else {
            HashMap::new()
        };

        results.push(ModbusResultReg {
            regs,
            values_by_id: parsed,
            response: Some(v_pkt.clone()),
            status: "OK".to_string(),
        });
    }

    Ok(ModbusResultMulti {
        results,
        request: tx_all,
        response: if rx_all.is_empty() {
            None
        } else {
            Some(rx_all)
        },
        status: if rx_virtual_all.len() < plans.len() {
            format!(
                "WARN: responses < commands ({}/{})",
                rx_virtual_all.len(),
                plans.len()
            )
        } else {
            "OK".to_string()
        },
    })
}

/// Выполняет набор запросов `ReadReq`, объединяя их в UDP-чанки и собирая ответы.
///
/// # Parameters
/// - `transport`: UDP-транспорт с корреляцией ответов.
/// - `obj_row`: параметры канала (kanal/speed/stop/parit/bit).
/// - `ip`, `port`, `rtu`, `modem`: адрес и маршрутизация устройства.
/// - `reqs`: нормализованные read-запросы.
/// - `limit`: лимит размера UDP-пакета.
/// - `timeout_per_chunk`: таймаут ожидания ответа на чанк.
/// - `idle_timeout`: таймаут дозагрузки дополнительных пакетов.
///
/// # Returns
/// - `Ok(ModbusResultMultiReq)`: ответы по запросам и суммарный статус.
/// - `Err(...)`: ошибка отправки/получения/кодирования запроса.
pub async fn request_reqs_glued(
    transport: &UdpCorrelatedTransport,
    obj_row: &HashMap<String, i32>,
    ip: &str,
    port: u16,
    rtu: i32,
    modem: i32,
    reqs: &[ReadReq],
    limit: usize,
    timeout_per_chunk: Duration,
    idle_timeout: Duration,
) -> Result<ModbusResultMultiReq> {
    let mut results: Vec<ModbusResultReq> = Vec::new();
    let mut plans: Vec<(u8, i32, i32)> = Vec::new();
    let mut mb_cmds: Vec<Vec<u8>> = Vec::new();
    let mut tx_all: Vec<u8> = Vec::new();
    let mut rx_all: Vec<u8> = Vec::new();

    for r in reqs {
        if r.cnt_words <= 0 {
            continue;
        }
        if r.func != 3 && r.func != 4 {
            continue;
        }
        let mb = modbus::sout_mb_only(rtu as u16, r.func, r.addr_human, r.cnt_words as u16, None)
            .map_err(|e| anyhow::anyhow!(e))?;
        mb_cmds.push(mb);
        plans.push((r.func, r.addr_human, r.cnt_words));
    }

    if mb_cmds.is_empty() {
        return Ok(ModbusResultMultiReq {
            results,
            request: vec![],
            response: None,
            status: "ERROR: no commands".to_string(),
        });
    }

    let mb_chunks = modbus::build_mb_chunks(&mb_cmds, limit).map_err(|e| anyhow::anyhow!(e))?;
    let mut rx_virtual_all: Vec<Vec<u8>> = Vec::new();

    for chunk_frames in mb_chunks {
        let pid = next_pid(ip, port, timeout_per_chunk + idle_timeout);
        let par = modbus::UdpParams {
            kan: obj_row.get("kanal").copied().unwrap_or(3) as u8,
            speed: obj_row.get("speed").copied().unwrap_or(8) as u8,
            stop: obj_row.get("stop").copied().unwrap_or(0) as u8,
            par: obj_row.get("parit").copied().unwrap_or(2) as u8,
            dsr: next_dsr(),
            data: obj_row.get("bit").copied().unwrap_or(8) as u8,
            rtu: rtu as u16,
            modem: modem as u16,
            port,
            ip: ip.to_string(),
            packet_id: pid,
            pkt_type: 0,
            ..Default::default()
        };

        let mut payload = 0usize;
        for mb in &chunk_frames {
            payload += mb.len();
        }
        let total_len = 22 + payload;
        let header = modbus::shab(&par, total_len);
        let mut tx: Vec<u8> = Vec::with_capacity(total_len);
        tx.extend_from_slice(&header);
        for mb in &chunk_frames {
            tx.extend_from_slice(mb);
        }
        tx_all.extend_from_slice(&tx);

        let rx = transport
            .send(&tx, ip, port, timeout_per_chunk, true, idle_timeout)
            .await?
            .ok_or_else(|| anyhow::anyhow!("timeout"))?;
        rx_all.extend_from_slice(&rx);
        let parts = modbus::split_rx_to_virtual(&rx);
        rx_virtual_all.extend(parts);
    }

    let n = std::cmp::min(plans.len(), rx_virtual_all.len());
    for i in 0..n {
        let (func, addr, cnt) = plans[i];
        let v_pkt = rx_virtual_all[i].clone();
        results.push(ModbusResultReq {
            func,
            addr_human: addr,
            cnt_words: cnt,
            response: Some(v_pkt),
            status: "OK".to_string(),
        });
    }

    Ok(ModbusResultMultiReq {
        results,
        request: tx_all,
        response: if rx_all.is_empty() {
            None
        } else {
            Some(rx_all)
        },
        status: if rx_virtual_all.len() < plans.len() {
            format!(
                "WARN: responses < commands ({}/{})",
                rx_virtual_all.len(),
                plans.len()
            )
        } else {
            "OK".to_string()
        },
    })
}

#[derive(Debug)]
struct PidAllocState {
    next: u8,
    last_used: Vec<Option<Instant>>,
}

fn next_pid(ip: &str, port: u16, guard_ttl: Duration) -> u8 {
    static PID_STATE: OnceLock<Mutex<HashMap<String, PidAllocState>>> = OnceLock::new();
    let state = PID_STATE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = match state.lock() {
        Ok(g) => g,
        Err(poisoned) => {
            tracing::warn!("pid allocator mutex was poisoned; recovering");
            poisoned.into_inner()
        }
    };

    let key = format!("{}:{}", ip, port);
    let now = Instant::now();
    let ttl = guard_ttl.max(Duration::from_millis(300));
    let st = map.entry(key).or_insert_with(|| PidAllocState {
        next: 0,
        last_used: vec![None; 256],
    });

    for _ in 0..256 {
        let pid = st.next;
        st.next = st.next.wrapping_add(1);
        let idx = pid as usize;
        let reusable = match st.last_used[idx] {
            None => true,
            Some(last) => now.duration_since(last) >= ttl,
        };
        if reusable {
            st.last_used[idx] = Some(now);
            return pid;
        }
    }

    // All pids are inside TTL window. Reuse the next one as a fallback.
    let pid = st.next;
    st.next = st.next.wrapping_add(1);
    st.last_used[pid as usize] = Some(now);
    pid
}

fn next_dsr() -> u16 {
    use std::sync::atomic::{AtomicU16, Ordering};
    static DSR: AtomicU16 = AtomicU16::new(1);
    let v = DSR.fetch_add(1, Ordering::Relaxed);
    if v == 0 {
        1
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UdpSocket;
    use tokio::time::{sleep, Duration};

    fn mk_read_resp_frame(unit: u8, func: u8, data: &[u8]) -> Vec<u8> {
        let mut f = Vec::with_capacity(3 + data.len() + 2);
        f.push(unit);
        f.push(func);
        f.push(data.len() as u8);
        f.extend_from_slice(data);
        let crc = crate::modbus::crc16(&f);
        f.push((crc & 0xFF) as u8);
        f.push((crc >> 8) as u8);
        f
    }

    #[tokio::test]
    async fn request_reqs_glued_warn_when_responses_less_than_commands() {
        let transport = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind transport");
        let server = UdpSocket::bind("127.0.0.1:0").await.expect("bind server");
        let server_port = server.local_addr().expect("local addr").port();

        let j = tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let (n, peer) = server.recv_from(&mut buf).await.expect("recv");
            assert!(n >= 22, "must receive request with UDP hdr22");
            let pid = buf[3];
            let dsr_lo = buf[5];
            let dsr_hi = buf[6];

            let mut resp = vec![0u8; 10];
            resp[3] = pid;
            resp[4] = 1;
            resp[5] = dsr_lo;
            resp[6] = dsr_hi;
            let mb = mk_read_resp_frame(1, 4, &[0x00, 0x2A]);
            resp.extend_from_slice(&mb);

            server.send_to(&resp, peer).await.expect("send");
        });

        let obj_row: HashMap<String, i32> = HashMap::new();
        let reqs = vec![
            ReadReq {
                func: 4,
                addr_human: 300,
                cnt_words: 1,
            },
            ReadReq {
                func: 4,
                addr_human: 301,
                cnt_words: 1,
            },
        ];

        let res = request_reqs_glued(
            &transport,
            &obj_row,
            "127.0.0.1",
            server_port,
            1,
            1,
            &reqs,
            512,
            Duration::from_millis(400),
            Duration::from_millis(80),
        )
        .await
        .expect("request_reqs_glued");
        j.await.expect("join");

        assert!(
            res.status.starts_with("WARN: responses < commands"),
            "status was: {}",
            res.status
        );
        assert_eq!(res.results.len(), 1, "must contain only received response");
    }

    #[tokio::test]
    async fn next_pid_reuses_only_after_ttl() {
        let ip = "127.11.22.33";
        let port = 39999;
        let ttl = Duration::from_millis(10);

        let first = next_pid(ip, port, ttl);

        let mut seen_first_early = false;
        for _ in 0..10 {
            if next_pid(ip, port, ttl) == first {
                seen_first_early = true;
                break;
            }
        }
        assert!(!seen_first_early, "pid reused before ttl window elapsed");

        sleep(Duration::from_millis(320)).await;

        let mut seen_first_after = false;
        for _ in 0..300 {
            if next_pid(ip, port, ttl) == first {
                seen_first_after = true;
                break;
            }
        }
        assert!(
            seen_first_after,
            "pid was not reused after ttl elapsed and wrap-around"
        );
    }
}
