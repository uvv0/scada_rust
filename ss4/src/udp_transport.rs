//! Коррелированный UDP-транспорт для request/response обмена с таймаутами и сбором мульти-пакетных ответов.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Mutex};
use tokio::time::timeout;

const OFF_PACKET_ID: usize = 3;
const OFF_PKT_TYPE: usize = 4;
const OFF_DSR_LO: usize = 5;
const OFF_DSR_HI: usize = 6;
const OFF_DSR_ALT_LO: usize = 7;
const OFF_DSR_ALT_HI: usize = 8;
const OFF_MODEM_LO: usize = 7;
const OFF_MODEM_HI: usize = 8;
const MIN_FULL_HEADER_LEN: usize = 22;

fn packet_id_of(p: &[u8]) -> Option<u8> {
    p.get(OFF_PACKET_ID).copied()
}

fn pkt_type_of(p: &[u8]) -> Option<u8> {
    p.get(OFF_PKT_TYPE).copied()
}

fn dsr_at(p: &[u8], lo_off: usize, hi_off: usize) -> Option<u16> {
    let lo = *p.get(lo_off)? as u16;
    let hi = *p.get(hi_off)? as u16;
    Some((hi << 8) | lo)
}

fn dsr_of(p: &[u8]) -> Option<u16> {
    dsr_at(p, OFF_DSR_LO, OFF_DSR_HI)
}

fn dsr_candidates_of(p: &[u8]) -> Vec<u16> {
    let mut out = Vec::with_capacity(2);
    if let Some(v) = dsr_at(p, OFF_DSR_LO, OFF_DSR_HI) {
        out.push(v);
    }
    if let Some(v) = dsr_at(p, OFF_DSR_ALT_LO, OFF_DSR_ALT_HI) {
        if !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

fn modem_of_full_header(p: &[u8]) -> Option<u16> {
    if p.len() < MIN_FULL_HEADER_LEN {
        return None;
    }
    if p.first().copied() != Some(1) {
        return None;
    }
    dsr_at(p, OFF_MODEM_LO, OFF_MODEM_HI)
}

fn modem_candidates_of(p: &[u8]) -> Vec<u16> {
    if p.len() < MIN_FULL_HEADER_LEN {
        return Vec::new();
    }
    if p.first().copied() != Some(1) {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(2);
    if let Some(v) = dsr_at(p, OFF_MODEM_LO, OFF_MODEM_HI) {
        out.push(v);
    }
    // Some devices/gateways return swapped DSR/MODEM positions in RX header.
    if let Some(v) = dsr_at(p, OFF_DSR_LO, OFF_DSR_HI) {
        if !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

fn key(ip: &str, port: u16, pid: u8, dsr: u16) -> String {
    format!("{}:{}:{}:{}", ip, port, pid, dsr)
}

fn hex_preview(data: &[u8], max_bytes: usize) -> String {
    let take = data.len().min(max_bytes);
    let mut out = String::with_capacity(take.saturating_mul(3));
    for (i, b) in data.iter().take(take).enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{:02X}", b));
    }
    if data.len() > take {
        out.push_str(" ...");
    }
    out
}

struct Pending {
    tx: mpsc::UnboundedSender<Vec<u8>>,
    deadline: Instant,
    expected_modem: Option<u16>,
}

#[derive(Clone)]
/// UDP-транспорт с корреляцией ответов по `(ip, port, packet_id, dsr)`.
pub struct UdpCorrelatedTransport {
    socket: Arc<UdpSocket>,
    pending: Arc<Mutex<HashMap<String, Pending>>>,
}

impl UdpCorrelatedTransport {
    /// Создаёт UDP-сокет транспорта и запускает задачи приёма/очистки pending-запросов.
    ///
    /// # Parameters
    /// - `bind_addr`: локальный адрес для bind сокета.
    ///
    /// # Returns
    /// - `Ok(UdpCorrelatedTransport)`: готовый транспорт.
    /// - `Err(...)`: ошибка bind/инициализации.
    pub async fn bind(bind_addr: SocketAddr) -> anyhow::Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        let socket = Arc::new(socket);
        let pending: Arc<Mutex<HashMap<String, Pending>>> = Arc::new(Mutex::new(HashMap::new()));

        let recv_socket = socket.clone();
        let recv_pending = pending.clone();

        tokio::spawn(async move {
            let mut buf = vec![0u8; 65535];
            loop {
                let (len, addr) = match recv_socket.recv_from(&mut buf).await {
                    Ok(v) => v,
                    Err(_) => break,
                };
                if len == 0 {
                    continue;
                }
                let data = &buf[..len];
                let Some(pkt_type) = pkt_type_of(data) else {
                    continue;
                };
                let Some(pid) = packet_id_of(data) else {
                    continue;
                };
                if dsr_of(data).is_none() {
                    continue;
                }
                if pkt_type != 1 {
                    continue;
                }
                let got_modem = modem_of_full_header(data);
                let got_modem_candidates = modem_candidates_of(data);

                tracing::debug!(
                    peer = %addr,
                    len = len,
                    pid = pid,
                    pkt_type = pkt_type,
                    modem = ?got_modem,
                    modem_candidates = ?got_modem_candidates,
                    data = %hex_preview(data, 96),
                    "udp rx"
                );
                let endpoint_ip = addr.ip().to_string();
                let candidates = dsr_candidates_of(data);
                let map = recv_pending.lock().await;
                for cand in candidates {
                    let k = key(&endpoint_ip, addr.port(), pid, cand);
                    if let Some(p) = map.get(&k) {
                        if let Some(exp_modem) = p.expected_modem {
                            if !got_modem_candidates.is_empty()
                                && !got_modem_candidates.contains(&exp_modem)
                            {
                                tracing::warn!(
                                    peer = %addr,
                                    pid = pid,
                                    dsr = cand,
                                    expected_modem = exp_modem,
                                    got_modem = ?got_modem,
                                    got_modem_candidates = ?got_modem_candidates,
                                    "udp rx dropped: modem mismatch"
                                );
                                continue;
                            }
                        }
                        let _ = p.tx.send(data.to_vec());
                        break;
                    }
                }
            }
        });

        // GC task
        let gc_pending = pending.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let now = Instant::now();
                let mut map = gc_pending.lock().await;
                let keys: Vec<String> = map
                    .iter()
                    .filter(|(_, p)| now >= p.deadline)
                    .map(|(k, _)| k.clone())
                    .collect();
                for k in keys {
                    map.remove(&k);
                }
            }
        });

        Ok(Self { socket, pending })
    }

    /// Отправляет UDP-запрос и ожидает коррелированный ответ.
    ///
    /// # Parameters
    /// - `request`: полный UDP-пакет с заголовком и payload.
    /// - `ip`, `port`: адрес назначения.
    /// - `timeout_total`: общий таймаут ожидания.
    /// - `collect_all`: если `true`, собирает все пакеты до `idle_timeout`.
    /// - `idle_timeout`: пауза без новых пакетов при режиме `collect_all`.
    ///
    /// # Returns
    /// - `Ok(Some(Vec<u8>))`: получен ответ (или склеенный набор ответов).
    /// - `Ok(None)`: таймаут/дубликат pending/некорректный request header.
    /// - `Err(...)`: ошибка сети/адресации.
    pub async fn send(
        &self,
        request: &[u8],
        ip: &str,
        port: u16,
        timeout_total: Duration,
        collect_all: bool,
        idle_timeout: Duration,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let Some(req_type) = pkt_type_of(request) else {
            return Ok(None);
        };
        if req_type != 0 {
            return Ok(None);
        }
        let Some(pid) = packet_id_of(request) else {
            return Ok(None);
        };
        let Some(dsr) = dsr_of(request) else {
            return Ok(None);
        };
        let expected_modem = modem_of_full_header(request);
        let k = key(ip, port, pid, dsr);

        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        {
            let mut map = self.pending.lock().await;
            if map.contains_key(&k) {
                return Ok(None);
            }
            map.insert(
                k.clone(),
                Pending {
                    tx,
                    deadline: Instant::now() + timeout_total,
                    expected_modem,
                },
            );
        }

        let addr: SocketAddr = format!("{}:{}", ip, port).parse()?;
        tracing::debug!(
            peer = %addr,
            len = request.len(),
            pid = pid,
            dsr = dsr,
            pkt_type = req_type,
            modem = ?expected_modem,
            data = %hex_preview(request, 96),
            "udp tx"
        );
        let _ = self.socket.send_to(request, addr).await?;

        let first = match timeout(timeout_total, rx.recv()).await {
            Ok(Some(v)) => {
                tracing::debug!(
                    ip = ip,
                    port = port,
                    pid = pid,
                    dsr = dsr,
                    expected_modem = ?expected_modem,
                    got_modem = ?modem_of_full_header(&v),
                    len = v.len(),
                    data = %hex_preview(&v, 96),
                    "udp rx first"
                );
                v
            }
            _ => {
                tracing::warn!(
                    ip = ip,
                    port = port,
                    pid = pid,
                    timeout_ms = timeout_total.as_millis() as u64,
                    "udp response timeout"
                );
                let mut map = self.pending.lock().await;
                map.remove(&k);
                return Ok(None);
            }
        };

        if !collect_all {
            let mut map = self.pending.lock().await;
            map.remove(&k);
            return Ok(Some(first));
        }

        let mut out = Vec::new();
        out.extend_from_slice(&first);

        let deadline = Instant::now() + timeout_total;
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let remain = deadline - now;
            let to = if remain < idle_timeout {
                remain
            } else {
                idle_timeout
            };
            match timeout(to, rx.recv()).await {
                Ok(Some(v)) => {
                    tracing::debug!(
                        ip = ip,
                        port = port,
                        pid = pid,
                        dsr = dsr,
                        expected_modem = ?expected_modem,
                        got_modem = ?modem_of_full_header(&v),
                        len = v.len(),
                        data = %hex_preview(&v, 96),
                        "udp rx extra"
                    );
                    out.extend_from_slice(&v);
                    continue;
                }
                _ => break,
            }
        }

        let mut map = self.pending.lock().await;
        map.remove(&k);
        Ok(Some(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UdpSocket;
    use tokio::time::Duration;

    fn make_req(pid: u8, dsr: u16) -> Vec<u8> {
        let mut b = vec![0u8; 12];
        b[3] = pid;
        b[4] = 0; // request
        b[5] = (dsr & 0x00FF) as u8;
        b[6] = ((dsr >> 8) & 0x00FF) as u8;
        b
    }

    fn make_resp(pid: u8, dsr: u16, tail: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; 10];
        b[3] = pid;
        b[4] = 1; // response
        b[5] = (dsr & 0x00FF) as u8;
        b[6] = ((dsr >> 8) & 0x00FF) as u8;
        b.extend_from_slice(tail);
        b
    }

    fn make_req_full(pid: u8, dsr: u16, modem: u16) -> Vec<u8> {
        let mut b = vec![0u8; 22];
        b[0] = 1;
        b[3] = pid;
        b[4] = 0; // request
        b[5] = (dsr & 0x00FF) as u8;
        b[6] = ((dsr >> 8) & 0x00FF) as u8;
        b[7] = (modem & 0x00FF) as u8;
        b[8] = ((modem >> 8) & 0x00FF) as u8;
        b
    }

    fn make_resp_full(pid: u8, dsr: u16, modem: u16, tail: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; 22];
        b[0] = 1;
        b[3] = pid;
        b[4] = 1; // response
        b[5] = (dsr & 0x00FF) as u8;
        b[6] = ((dsr >> 8) & 0x00FF) as u8;
        b[7] = (modem & 0x00FF) as u8;
        b[8] = ((modem >> 8) & 0x00FF) as u8;
        b.extend_from_slice(tail);
        b
    }

    fn make_resp_full_swapped(pid: u8, dsr: u16, modem: u16, tail: &[u8]) -> Vec<u8> {
        // Compatibility frame: DSR and MODEM fields are swapped in response header.
        let mut b = vec![0u8; 22];
        b[0] = 1;
        b[3] = pid;
        b[4] = 1; // response
                  // Put modem into DSR bytes (5..6), and dsr into MODEM bytes (7..8).
        b[5] = (modem & 0x00FF) as u8;
        b[6] = ((modem >> 8) & 0x00FF) as u8;
        b[7] = (dsr & 0x00FF) as u8;
        b[8] = ((dsr >> 8) & 0x00FF) as u8;
        b.extend_from_slice(tail);
        b
    }

    #[tokio::test]
    async fn send_times_out_and_cleans_pending() {
        let tr = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind");
        let req = make_req(7, 0x2233);

        let out = tr
            .send(
                &req,
                "127.0.0.1",
                6553,
                Duration::from_millis(60),
                false,
                Duration::from_millis(20),
            )
            .await
            .expect("send");
        assert!(out.is_none());

        let map = tr.pending.lock().await;
        assert!(map.is_empty(), "pending map must be empty after timeout");
    }

    #[tokio::test]
    async fn send_collect_all_aggregates_multiple_packets() {
        let tr = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind");
        let responder = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("responder bind");
        let server_addr = responder.local_addr().expect("local addr");

        let j = tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            let (n, peer) = responder.recv_from(&mut buf).await.expect("recv");
            assert!(n >= 7, "request must include header fields");
            let pid = buf[3];
            let dsr = ((buf[6] as u16) << 8) | (buf[5] as u16);

            let r1 = make_resp(pid, dsr, &[0xAA, 0x01, 0x02]);
            let r2 = make_resp(pid, dsr, &[0xBB, 0x03]);
            responder.send_to(&r1, peer).await.expect("send r1");
            responder.send_to(&r2, peer).await.expect("send r2");
        });

        let req = make_req(21, 0x7788);
        let out = tr
            .send(
                &req,
                "127.0.0.1",
                server_addr.port(),
                Duration::from_millis(500),
                true,
                Duration::from_millis(70),
            )
            .await
            .expect("send");
        j.await.expect("join");

        let out = out.expect("must have response");
        let first_tail = [0xAA, 0x01, 0x02];
        let second_tail = [0xBB, 0x03];
        assert!(
            out.windows(first_tail.len()).any(|w| w == first_tail),
            "aggregated response must contain first payload"
        );
        assert!(
            out.windows(second_tail.len()).any(|w| w == second_tail),
            "aggregated response must contain second payload"
        );
    }

    #[tokio::test]
    async fn send_drops_response_with_wrong_modem_and_accepts_correct_one() {
        let tr = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind");
        let responder = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("responder bind");
        let server_addr = responder.local_addr().expect("local addr");

        let expected_modem: u16 = 50002;
        let wrong_modem: u16 = 50001;
        let j = tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            let (n, peer) = responder.recv_from(&mut buf).await.expect("recv");
            assert!(n >= 22, "request must include full header");
            let pid = buf[3];
            let dsr = ((buf[6] as u16) << 8) | (buf[5] as u16);

            let bad = make_resp_full(pid, dsr, wrong_modem, &[0xDE, 0xAD]);
            responder.send_to(&bad, peer).await.expect("send bad");
            tokio::time::sleep(Duration::from_millis(20)).await;

            let good = make_resp_full(pid, dsr, expected_modem, &[0xBE, 0xEF]);
            responder.send_to(&good, peer).await.expect("send good");
        });

        let req = make_req_full(31, 0x3344, expected_modem);
        let out = tr
            .send(
                &req,
                "127.0.0.1",
                server_addr.port(),
                Duration::from_millis(500),
                false,
                Duration::from_millis(70),
            )
            .await
            .expect("send");
        j.await.expect("join");

        let out = out.expect("must accept response with expected modem");
        let good_tail = [0xBE, 0xEF];
        assert!(
            out.windows(good_tail.len()).any(|w| w == good_tail),
            "response must contain payload from the packet with matching modem"
        );
        let bad_tail = [0xDE, 0xAD];
        assert!(
            !out.windows(bad_tail.len()).any(|w| w == bad_tail),
            "response must not contain payload from modem-mismatched packet"
        );
    }

    #[tokio::test]
    async fn send_accepts_short_response_without_full_header_for_compatibility() {
        let tr = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind");
        let responder = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("responder bind");
        let server_addr = responder.local_addr().expect("local addr");

        let j = tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            let (n, peer) = responder.recv_from(&mut buf).await.expect("recv");
            assert!(n >= 22, "request must include full header");
            let pid = buf[3];
            let dsr = ((buf[6] as u16) << 8) | (buf[5] as u16);

            // Legacy/short response format: no full 22-byte header.
            let short = make_resp(pid, dsr, &[0xCA, 0xFE]);
            responder.send_to(&short, peer).await.expect("send short");
        });

        let req = make_req_full(41, 0x5566, 50002);
        let out = tr
            .send(
                &req,
                "127.0.0.1",
                server_addr.port(),
                Duration::from_millis(500),
                false,
                Duration::from_millis(70),
            )
            .await
            .expect("send");
        j.await.expect("join");

        let out = out.expect("must accept short response for backward compatibility");
        let tail = [0xCA, 0xFE];
        assert!(
            out.windows(tail.len()).any(|w| w == tail),
            "short response payload must be delivered"
        );
    }

    #[tokio::test]
    async fn send_accepts_response_with_swapped_dsr_modem_fields() {
        let tr = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind");
        let responder = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("responder bind");
        let server_addr = responder.local_addr().expect("local addr");

        let expected_modem: u16 = 50002;
        let j = tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            let (n, peer) = responder.recv_from(&mut buf).await.expect("recv");
            assert!(n >= 22, "request must include full header");
            let pid = buf[3];
            let dsr = ((buf[6] as u16) << 8) | (buf[5] as u16);

            let swapped = make_resp_full_swapped(pid, dsr, expected_modem, &[0xAB, 0xCD]);
            responder
                .send_to(&swapped, peer)
                .await
                .expect("send swapped");
        });

        let req = make_req_full(51, 0x1122, expected_modem);
        let out = tr
            .send(
                &req,
                "127.0.0.1",
                server_addr.port(),
                Duration::from_millis(500),
                false,
                Duration::from_millis(70),
            )
            .await
            .expect("send");
        j.await.expect("join");

        let out = out.expect("must accept response with swapped modem/dsr fields");
        let tail = [0xAB, 0xCD];
        assert!(
            out.windows(tail.len()).any(|w| w == tail),
            "swapped-header response payload must be delivered"
        );
    }

    /// Late response: after timeout the pending entry is removed; a response arriving later is dropped and does not attach to a new request.
    #[tokio::test]
    async fn send_late_response_after_timeout_is_dropped_and_pending_cleaned() {
        let tr = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind");
        let responder = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("responder bind");
        let server_addr = responder.local_addr().expect("local addr");

        let j = tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            let (n1, peer) = responder.recv_from(&mut buf).await.expect("recv 1");
            assert!(n1 >= 7);
            let pid1 = buf[3];
            let dsr1 = ((buf[6] as u16) << 8) | (buf[5] as u16);
            tokio::time::sleep(Duration::from_millis(120)).await;
            let late = make_resp(pid1, dsr1, &[0xDE, 0xAD]);
            responder.send_to(&late, peer).await.expect("send late");
            let (n2, peer2) = responder.recv_from(&mut buf).await.expect("recv 2");
            assert!(n2 >= 7);
            let pid2 = buf[3];
            let dsr2 = ((buf[6] as u16) << 8) | (buf[5] as u16);
            let ok_resp = make_resp(pid2, dsr2, &[0xBE, 0xEF]);
            responder.send_to(&ok_resp, peer2).await.expect("send ok");
        });

        let req1 = make_req(10, 0x1111);
        let out1 = tr
            .send(
                &req1,
                "127.0.0.1",
                server_addr.port(),
                Duration::from_millis(80),
                false,
                Duration::from_millis(30),
            )
            .await
            .expect("send");
        assert!(out1.is_none(), "must time out with no response");

        let map = tr.pending.lock().await;
        assert!(map.is_empty(), "pending must be empty after timeout");
        drop(map);

        let req2 = make_req(11, 0x2222);
        let out2 = tr
            .send(
                &req2,
                "127.0.0.1",
                server_addr.port(),
                Duration::from_millis(500),
                false,
                Duration::from_millis(50),
            )
            .await
            .expect("send");
        j.await.expect("join");
        let out2 = out2.expect("second request must get response");
        assert!(
            out2.windows(2).any(|w| w == [0xBE, 0xEF]),
            "must get payload from second request, not late first"
        );
    }

    /// Reordered responses: two requests A and B; responses arrive in order B, A. Both must correlate correctly.
    #[tokio::test]
    async fn send_reordered_responses_both_correlate_correctly() {
        let tr = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind");
        let responder = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("responder bind");
        let server_addr = responder.local_addr().expect("local addr");

        let j = tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            let (n1, peer) = responder.recv_from(&mut buf).await.expect("recv 1");
            assert!(n1 >= 7);
            let pid1 = buf[3];
            let dsr1 = ((buf[6] as u16) << 8) | (buf[5] as u16);
            let (n2, _) = responder.recv_from(&mut buf).await.expect("recv 2");
            assert!(n2 >= 7);
            let pid2 = buf[3];
            let dsr2 = ((buf[6] as u16) << 8) | (buf[5] as u16);
            let r2 = make_resp(pid2, dsr2, &[0xBB, 0x22]);
            let r1 = make_resp(pid1, dsr1, &[0xAA, 0x11]);
            responder.send_to(&r2, peer).await.expect("send r2");
            responder.send_to(&r1, peer).await.expect("send r1");
        });

        let req_a = make_req(20, 0xAAAA);
        let req_b = make_req(21, 0xBBBB);
        let tr_b = tr.clone();
        let port = server_addr.port();
        let task_b = tokio::spawn(async move {
            tr_b.send(
                &req_b,
                "127.0.0.1",
                port,
                Duration::from_millis(500),
                false,
                Duration::from_millis(70),
            )
            .await
            .expect("send b")
        });
        let task_a = tokio::spawn(async move {
            tr.send(
                &req_a,
                "127.0.0.1",
                port,
                Duration::from_millis(500),
                false,
                Duration::from_millis(70),
            )
            .await
            .expect("send a")
        });
        j.await.expect("server join");
        let out_a = task_a.await.expect("task a").expect("a must have response");
        let out_b = task_b.await.expect("task b").expect("b must have response");
        assert!(
            out_a.windows(2).any(|w| w == [0xAA, 0x11]),
            "A must get AA 11"
        );
        assert!(
            out_b.windows(2).any(|w| w == [0xBB, 0x22]),
            "B must get BB 22"
        );
    }

    /// Duplicate response: one request, server sends same response twice. Transport aggregates or accepts first; no crash.
    #[tokio::test]
    async fn send_duplicate_response_accepts_and_does_not_crash() {
        let tr = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
            .await
            .expect("bind");
        let responder = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("responder bind");
        let server_addr = responder.local_addr().expect("local addr");

        let j = tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            let (n, peer) = responder.recv_from(&mut buf).await.expect("recv");
            assert!(n >= 7);
            let pid = buf[3];
            let dsr = ((buf[6] as u16) << 8) | (buf[5] as u16);
            let r = make_resp(pid, dsr, &[0xCC, 0xDD]);
            responder.send_to(&r, peer).await.expect("send 1");
            responder.send_to(&r, peer).await.expect("send 2");
        });

        let req = make_req(30, 0x3333);
        let out = tr
            .send(
                &req,
                "127.0.0.1",
                server_addr.port(),
                Duration::from_millis(500),
                true,
                Duration::from_millis(100),
            )
            .await
            .expect("send");
        j.await.expect("join");
        let out = out.expect("must get response");
        assert!(
            out.windows(2).any(|w| w == [0xCC, 0xDD]),
            "payload must contain response data"
        );
    }
}
