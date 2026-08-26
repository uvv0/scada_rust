use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::net::UdpSocket;
use tokio::sync::Semaphore;

use crate::imm;
use crate::{hex_dump, push_ui_log, unix_now_secs, ListenerConfig, UiState};

type SharedSlot = Arc<Mutex<imm::SlotState>>;
type SharedSlots = Arc<Mutex<std::collections::HashMap<u16, SharedSlot>>>;

const MAX_IN_FLIGHT_PACKETS: usize = 256;
const LOG_PACKET_FLOW: bool = false;
const LOG_PACKET_HEX: bool = false;

fn modem_allowed(config: &ListenerConfig, modem: u16) -> bool {
    (config.modem_start..=config.modem_end).contains(&modem)
}

fn get_or_create_slot(slots: &SharedSlots, modem: u16) -> SharedSlot {
    let mut slots = slots.lock().unwrap();
    slots
        .entry(modem)
        .or_insert_with(|| {
            let mut slot = imm::SlotState::new();
            slot.init_ring_archive();
            Arc::new(Mutex::new(slot))
        })
        .clone()
}

fn build_response(
    req: &[u8],
    src: SocketAddr,
    slot_state: &mut imm::SlotState,
    ui: &Arc<Mutex<UiState>>,
) -> Result<(Vec<u8>, u16, u16), String> {
    if req.len() < 10 {
        return Err(format!("short packet: {}", req.len()));
    }
    let modem = (req[7] as u16) | ((req[8] as u16) << 8);
    let dsr = (req[5] as u16) | ((req[6] as u16) << 8);
    if LOG_PACKET_FLOW {
        push_ui_log(ui, format!("UDP RX src={} len={} modem={}", src, req.len(), modem));
    }
    if LOG_PACKET_HEX {
        push_ui_log(ui, format!("RX BUF [{}]: {}", req.len(), hex_dump(req)));
    }

    let mut out_head = req[..10].to_vec();
    if out_head.len() < 9 {
        return Err("short udp header".to_string());
    }
    let (sd, sd1) = (out_head[5], out_head[6]);
    out_head[5] = out_head[7];
    out_head[6] = out_head[8];
    out_head[7] = sd;
    out_head[8] = sd1;
    out_head[4] = 1;

    let hdr_len = if req.len() >= 22 { 22 } else { 10 };
    let payload = &req[hdr_len..];
    let parse_size = payload.len() as u16;

    let mut modbus_out = Vec::new();
    let mut parse_error: Option<String> = None;
    match imm::raz2_with_slot(slot_state, &payload[..parse_size as usize], parse_size) {
        Ok((num, out)) => {
            modbus_out = out;
            let used = num as usize;
            if used > 0 && used <= modbus_out.len() {
                modbus_out.truncate(used);
            }
            if modbus_out.is_empty() {
                return Err("raz2: empty modbus response".to_string());
            }
        }
        Err(e) => {
            parse_error = Some(format!("raz2: {e}"));
            if modbus_out.is_empty() {
                return Err(parse_error.unwrap());
            }
        }
    }

    let mut out = out_head;
    out.extend_from_slice(&modbus_out);

    let total = out.len();
    if total < 10 {
        return Err(format!("short response: {}", total));
    }
    out[1] = (total & 0x00FF) as u8;
    out[2] = ((total & 0xFF00) >> 8) as u8;

    if let Some(err) = parse_error {
        push_ui_log(ui, format!("UDP WARN src={} modem={} err={}", src, modem, err));
    }

    Ok((out, modem, dsr))
}

fn update_rx_state(ui: &Arc<Mutex<UiState>>, src: SocketAddr, n: usize) {
    let mut st = ui.lock().unwrap();
    st.packets = st.packets.saturating_add(1);
    st.last_peer = src.to_string();
    st.last_rx_unix = unix_now_secs();
    st.last_rx_bytes = n;
}

fn update_result_state(ui: &Arc<Mutex<UiState>>, result: &Result<(Vec<u8>, u16, u16), String>) {
    let mut st = ui.lock().unwrap();
    match result {
        Ok(_) => {
            st.last_rx_status = "OK".to_string();
            st.last_error.clear();
        }
        Err(err) if err.starts_with("drop modem:") => {
            st.last_rx_status = "DROP".to_string();
            st.dropped_packets = st.dropped_packets.saturating_add(1);
            st.last_error.clear();
        }
        Err(err) => {
            st.last_rx_status = "ERR".to_string();
            st.err_packets = st.err_packets.saturating_add(1);
            st.last_error = err.clone();
        }
    }
}

async fn handle_packet(
    socket: Arc<UdpSocket>,
    local_addr: SocketAddr,
    ui: Arc<Mutex<UiState>>,
    slots: SharedSlots,
    config: ListenerConfig,
    src: SocketAddr,
    req: Vec<u8>,
) -> std::io::Result<()> {
    let n = req.len();
    let is_response = n > 4 && req[4] == 1;
    if src == local_addr && is_response {
        push_ui_log(&ui, format!("UDP SELF-ECHO drop src={} len={} type=resp", src, n));
        return Ok(());
    }

    if req.len() < 10 {
        update_rx_state(&ui, src, n);
        update_result_state(&ui, &Err(format!("short packet: {}", req.len())));
        return Ok(());
    }

    let modem = (req[7] as u16) | ((req[8] as u16) << 8);
    if !modem_allowed(&config, modem) {
        if LOG_PACKET_FLOW {
            push_ui_log(
                &ui,
                format!(
                    "UDP DROP src={} modem={} allowed=[{}..={}]",
                    src,
                    modem,
                    config.modem_start,
                    config.modem_end
                ),
            );
        }
        let result = Err(format!("drop modem: {}", modem));
        update_rx_state(&ui, src, n);
        update_result_state(&ui, &result);
        return Ok(());
    }

    let result = {
        let slot = get_or_create_slot(&slots, modem);
        let mut slot = slot.lock().unwrap();
        slot.tick_ring_archive();
        if LOG_PACKET_FLOW {
            push_ui_log(&ui, format!("MODEM OK modem={} slot={}", modem, modem));
        }
        build_response(&req, src, &mut slot, &ui)
    };

    update_rx_state(&ui, src, n);
    update_result_state(&ui, &result);

    if let Ok((out, modem, dsr)) = result {
        socket.send_to(&out, src).await?;
        {
            let mut st = ui.lock().unwrap();
            st.sent_packets = st.sent_packets.saturating_add(1);
        }
        let mut mirrored = false;
        if src == local_addr && dsr != 0 && dsr != src.port() {
            let mirror_dst = SocketAddr::new(src.ip(), dsr);
            if socket.send_to(&out, mirror_dst).await.is_ok() {
                mirrored = true;
            }
        }

        if LOG_PACKET_HEX {
            push_ui_log(&ui, format!("TX BUF [{}]: {}", out.len(), hex_dump(&out)));
        }
        if LOG_PACKET_FLOW {
            push_ui_log(
                &ui,
                format!(
                    "UDP FLOW peer={} dst={} modem={} dsr={} mirror={} rx_len={} -> tx_len={}",
                    src,
                    src,
                    modem,
                    dsr,
                    mirrored,
                    n,
                    out.len()
                ),
            );
        }
    }

    Ok(())
}

async fn tick_all_slots(slots: SharedSlots) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        interval.tick().await;
        let slot_list = {
            let slots = slots.lock().unwrap();
            slots.values().cloned().collect::<Vec<_>>()
        };
        for slot in slot_list {
            slot.lock().unwrap().tick_ring_archive();
        }
    }
}

async fn run_udp_server_async(
    ui: Arc<Mutex<UiState>>,
    slots: SharedSlots,
    config: ListenerConfig,
) -> std::io::Result<()> {
    let socket = Arc::new(UdpSocket::bind(config.bind_addr()).await?);
    let local_addr = socket.local_addr()?;
    let permit_pool = Arc::new(Semaphore::new(MAX_IN_FLIGHT_PACKETS));
    let tick_slots = slots.clone();

    tokio::spawn(async move {
        tick_all_slots(tick_slots).await;
    });

    loop {
        let mut buffer = vec![0u8; 65535];
        let (n, src) = socket.recv_from(&mut buffer).await?;
        let req = buffer[..n].to_vec();
        let Ok(permit) = permit_pool.clone().acquire_owned().await else {
            continue;
        };
        let socket = socket.clone();
        let ui = ui.clone();
        let slots = slots.clone();
        let config = config.clone();

        tokio::spawn(async move {
            let _permit = permit;
            if let Err(err) = handle_packet(socket, local_addr, ui.clone(), slots, config, src, req).await {
                let mut st = ui.lock().unwrap();
                st.last_rx_status = "ERR".to_string();
                st.err_packets = st.err_packets.saturating_add(1);
                st.last_error = format!("udp handler failed: {err}");
            }
        });
    }
}

pub fn run_udp_server(
    ui: Arc<Mutex<UiState>>,
    slots: SharedSlots,
    config: ListenerConfig,
) -> std::io::Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?;

    rt.block_on(async move { run_udp_server_async(ui, slots, config).await })
}
