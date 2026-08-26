use super::*;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio_postgres::NoTls;

fn new_state(pool_size: usize) -> SchedulerState {
    SchedulerState::new_with_limits(pool_size, 20_000, pool_size.max(1), 3, 600)
}

#[tokio::test]
async fn wait_for_tick_or_shutdown_stops_when_shutdown_requested() {
    let mut tick = tokio::time::interval(Duration::from_secs(60));
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    shutdown_tx.send(true).expect("send shutdown");

    let should_continue = wait_for_tick_or_shutdown(&mut tick, &mut shutdown_rx).await;
    assert!(!should_continue, "loop must stop on shutdown signal");
}

#[tokio::test]
async fn run_with_shutdown_exits_immediately_when_flag_is_set() {
    let Some(client) = connect_test_db().await else {
        return;
    };
    let scheduler = Scheduler {
        pool_size: 1,
        tick_ms: 1000,
        sync_period_sec: 10,
        max_queue: 10,
        max_inflight: 1,
        no_response_failures: 3,
        no_response_backoff_sec: 600,
        telegram: None,
        mqtt: None,
    };
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    shutdown_tx.send(true).expect("send shutdown");

    let res = tokio::time::timeout(
        Duration::from_secs(2),
        scheduler.run_with_shutdown(client, shutdown_rx),
    )
    .await;
    assert!(res.is_ok(), "run_with_shutdown must return promptly");
    assert!(
        res.expect("timeout result").is_ok(),
        "run_with_shutdown must exit without error"
    );
}

async fn connect_test_db() -> Option<tokio_postgres::Client> {
    let url = match std::env::var("TEST_DB_URL") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => return None,
    };
    let (client, connection) = match tokio_postgres::connect(&url, NoTls).await {
        Ok(v) => v,
        Err(_) => return None,
    };
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Some(client)
}

#[tokio::test]
async fn run_script_job_success_clears_no_response_streak() {
    let Some(client) = connect_test_db().await else {
        return;
    };
    let client = Arc::new(client);
    let transport = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
        .await
        .expect("bind transport");

    let mut state = new_state(1);
    let kpz_id = 3101;
    let obj_id = 31001;
    state.tasks.insert(
        kpz_id,
        KpzTask {
            kpz: KpzRow {
                id: kpz_id,
                name: Some("kpz-script-streak".to_string()),
                rtu: 1,
                obj: obj_id,
                modem: 1,
                grups: vec![0u8; 64],
                max_pkt_len: 256,
                start: 1,
                t_a: 1,
                t_script: 1,
                en_post: true,
            },
            group_id: 1,
            generation: 1,
            next_a: Instant::now(),
            next_script: Instant::now(),
            busy_a: false,
            busy_s: true,
        },
    );
    Arc::make_mut(&mut state.obj_by_id).insert(
        obj_id,
        ObjRow {
            id: obj_id,
            name: Some("obj-script-streak".to_string()),
            ip: Some("127.0.0.1".to_string()),
            port: Some("65000".to_string()),
            kanal: Some(3),
            speed: Some(8),
            stop: Some(0),
            parit: Some(2),
            bit: Some(8),
        },
    );
    state.no_resp_streak_by_kpz.insert(kpz_id, 2);
    state.queue.push_back(Job {
        kpz_id,
        kind: JobKind::S,
    });

    let res = state.drain_worker_results(&client, &transport).await;
    assert!(
        res.is_ok(),
        "script job without script groups should finish successfully"
    );
    assert!(
        !state.no_resp_streak_by_kpz.contains_key(&kpz_id),
        "successful script job must clear accumulated no-response streak"
    );
    assert!(
        !state.tasks.get(&kpz_id).expect("task").busy_s,
        "busy_s must be released after script job"
    );
}

#[tokio::test]
async fn drain_queue_with_conn_errors_returns_ok_and_releases_busy_flags() {
    let Some(client) = connect_test_db().await else {
        return;
    };
    let client = Arc::new(client);
    let transport = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
        .await
        .expect("bind transport");

    let now = Instant::now();
    let mut state = new_state(4);
    let mk_kpz = |id: i32, obj: i32| KpzRow {
        id,
        name: Some(format!("kpz-{}", id)),
        rtu: id,
        obj,
        modem: 1,
        grups: vec![0u8; 64],
        max_pkt_len: 256,
        start: 1,
        t_a: 1,
        t_script: 1,
        en_post: true,
    };

    let kpz_a = mk_kpz(1001, 900001);
    let kpz_b = mk_kpz(1002, 900002);

    state.tasks.insert(
        kpz_a.id,
        KpzTask {
            kpz: kpz_a,
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(10),
            next_script: now + Duration::from_secs(10),
            busy_a: true,
            busy_s: false,
        },
    );
    state.tasks.insert(
        kpz_b.id,
        KpzTask {
            kpz: kpz_b,
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(10),
            next_script: now + Duration::from_secs(10),
            busy_a: true,
            busy_s: false,
        },
    );

    state.queue.push_back(Job {
        kpz_id: 1001,
        kind: JobKind::A,
    });
    state.queue.push_back(Job {
        kpz_id: 1002,
        kind: JobKind::A,
    });

    let res = state.drain_worker_results(&client, &transport).await;
    assert!(res.is_ok(), "drain_queue should not fail on worker errors");
    assert!(state.queue.is_empty(), "queue should be drained");

    let ta = state.tasks.get(&1001).expect("task 1001");
    let tb = state.tasks.get(&1002).expect("task 1002");
    assert!(!ta.busy_a, "busy_a for failed kpz must be released");
    assert!(!tb.busy_a, "busy_a for failed kpz must be released");
    assert_eq!(state.metrics_jobs_started, 2);
    assert_eq!(state.metrics_jobs_err, 2);
}

#[tokio::test]
async fn drain_queue_mixed_conn_error_and_success_keeps_processing() {
    let Some(client) = connect_test_db().await else {
        return;
    };
    let client = Arc::new(client);
    let transport = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
        .await
        .expect("bind transport");

    let now = Instant::now();
    let mut state = new_state(4);
    let mk_kpz = |id: i32, obj: i32| KpzRow {
        id,
        name: Some(format!("kpz-{}", id)),
        rtu: id,
        obj,
        modem: 1,
        grups: vec![0u8; 64], // no groups => fast A-mode success
        max_pkt_len: 256,
        start: 1,
        t_a: 1,
        t_script: 1,
        en_post: true,
    };

    let kpz_bad = mk_kpz(2001, 990001); // missing obj -> build_conn error
    let kpz_ok = mk_kpz(2002, 990002); // has obj -> run_a_mode returns Ok (no groups)

    state.tasks.insert(
        kpz_bad.id,
        KpzTask {
            kpz: kpz_bad,
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(10),
            next_script: now + Duration::from_secs(10),
            busy_a: true,
            busy_s: false,
        },
    );
    state.tasks.insert(
        kpz_ok.id,
        KpzTask {
            kpz: kpz_ok,
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(10),
            next_script: now + Duration::from_secs(10),
            busy_a: true,
            busy_s: false,
        },
    );

    Arc::make_mut(&mut state.obj_by_id).insert(
        990002,
        ObjRow {
            id: 990002,
            name: Some("obj-ok".to_string()),
            ip: Some("127.0.0.1".to_string()),
            port: Some("65000".to_string()),
            kanal: Some(3),
            speed: Some(8),
            stop: Some(0),
            parit: Some(2),
            bit: Some(8),
        },
    );

    state.queue.push_back(Job {
        kpz_id: 2001,
        kind: JobKind::A,
    });
    state.queue.push_back(Job {
        kpz_id: 2002,
        kind: JobKind::A,
    });

    let res = state.drain_worker_results(&client, &transport).await;
    assert!(
        res.is_ok(),
        "drain_queue should keep processing mixed outcomes"
    );
    assert!(state.queue.is_empty(), "queue should be drained");

    let t_bad = state.tasks.get(&2001).expect("task 2001");
    let t_ok = state.tasks.get(&2002).expect("task 2002");
    assert!(!t_bad.busy_a, "failed kpz busy_a must be released");
    assert!(!t_ok.busy_a, "successful kpz busy_a must be released");
    assert_eq!(state.metrics_jobs_started, 2);
    assert_eq!(state.metrics_jobs_err, 1);
    assert_eq!(state.metrics_jobs_ok, 1);
}

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
async fn run_a_mode_partial_response_sets_warn_and_returns_ok() {
    let Some(client) = connect_test_db().await else {
        return;
    };
    let transport = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
        .await
        .expect("bind transport");
    let server = UdpSocket::bind("127.0.0.1:0").await.expect("bind server");
    let server_port = server.local_addr().expect("server addr").port();

    let j = tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        let (n, peer) = server.recv_from(&mut buf).await.expect("recv request");
        assert!(n >= 22, "A-mode request must contain hdr22");
        let pid = buf[3];
        let dsr_lo = buf[5];
        let dsr_hi = buf[6];

        // Return only one Modbus frame although two were requested.
        let mut resp = vec![0u8; 10];
        resp[3] = pid;
        resp[4] = 1;
        resp[5] = dsr_lo;
        resp[6] = dsr_hi;
        let mb = mk_read_resp_frame(1, 4, &[0x00, 0x2A]);
        resp.extend_from_slice(&mb);
        server.send_to(&resp, peer).await.expect("send response");
    });

    let mut state = new_state(1);
    state.n_mb_tit_id = Some(1);
    state.n_mb_reg_id = Some(2);

    let mut grups = vec![0u8; 64];
    grups[0] |= 1 << 0; // group 1 enabled

    let kpz_id = 3001;
    let obj_id = 30001;
    state.tasks.insert(
        kpz_id,
        KpzTask {
            kpz: KpzRow {
                id: kpz_id,
                name: Some("kpz-a-mode-partial".to_string()),
                rtu: 1,
                obj: obj_id,
                modem: 1,
                grups,
                max_pkt_len: 512,
                start: 1,
                t_a: 1,
                t_script: 1,
                en_post: true,
            },
            group_id: 1,
            generation: 1,
            next_a: Instant::now(),
            next_script: Instant::now(),
            busy_a: true,
            busy_s: false,
        },
    );
    Arc::make_mut(&mut state.obj_by_id).insert(
        obj_id,
        ObjRow {
            id: obj_id,
            name: Some("obj-partial".to_string()),
            ip: Some("127.0.0.1".to_string()),
            port: Some(server_port.to_string()),
            kanal: Some(3),
            speed: Some(8),
            stop: Some(0),
            parit: Some(2),
            bit: Some(8),
        },
    );
    Arc::make_mut(&mut state.regs_by_group).insert(
        1,
        Arc::new(vec![
            Reg {
                id: 7001,
                name: "r7001".to_string(),
                addr: 0,
                n_mb: Some(1),
                tip: 3,
                bits: None,
                grup: Some(1),
                a_en: true,
                a_no_write: 0,
            },
            Reg {
                id: 7002,
                name: "r7002".to_string(),
                addr: 10, // force second block
                n_mb: Some(1),
                tip: 3,
                bits: None,
                grup: Some(1),
                a_en: true,
                a_no_write: 0,
            },
        ]),
    );

    let conn = ConnInfo {
        kpz_id,
        obj_id,
        ip: "127.0.0.1".to_string(),
        port: server_port,
        rtu: 1,
        modem: 1,
        max_pkt_len: 512,
    };

    let res = state.run_a_mode(&client, &transport, &conn, 1).await;
    j.await.expect("join server");
    assert!(
        res.is_ok(),
        "run_a_mode must stay non-fatal on partial response"
    );

    let status = state
        .last_a_glued_status
        .get(&kpz_id)
        .cloned()
        .unwrap_or_default();
    assert!(
        status.starts_with("WARN: responses < commands"),
        "unexpected glued status: {}",
        status
    );
}

#[tokio::test]
async fn run_a_mode_timeout_only_returns_err() {
    let Some(client) = connect_test_db().await else {
        return;
    };
    let transport = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
        .await
        .expect("bind transport");
    let server = UdpSocket::bind("127.0.0.1:0").await.expect("bind server");
    let server_port = server.local_addr().expect("server addr").port();

    let j = tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        let (_n, _peer) = server.recv_from(&mut buf).await.expect("recv request");
        // Intentionally no response: emulate pure timeout.
        tokio::time::sleep(Duration::from_millis(2200)).await;
    });

    let mut state = new_state(1);
    state.n_mb_tit_id = Some(1);
    state.n_mb_reg_id = Some(2);

    let mut grups = vec![0u8; 64];
    grups[0] |= 1 << 0;
    let kpz_id = 3002;
    let obj_id = 30002;
    state.tasks.insert(
        kpz_id,
        KpzTask {
            kpz: KpzRow {
                id: kpz_id,
                name: Some("kpz-a-mode-timeout".to_string()),
                rtu: 1,
                obj: obj_id,
                modem: 1,
                grups,
                max_pkt_len: 512,
                start: 1,
                t_a: 1,
                t_script: 1,
                en_post: true,
            },
            group_id: 1,
            generation: 1,
            next_a: Instant::now(),
            next_script: Instant::now(),
            busy_a: true,
            busy_s: false,
        },
    );
    Arc::make_mut(&mut state.obj_by_id).insert(
        obj_id,
        ObjRow {
            id: obj_id,
            name: Some("obj-timeout".to_string()),
            ip: Some("127.0.0.1".to_string()),
            port: Some(server_port.to_string()),
            kanal: Some(3),
            speed: Some(8),
            stop: Some(0),
            parit: Some(2),
            bit: Some(8),
        },
    );
    Arc::make_mut(&mut state.regs_by_group).insert(
        1,
        Arc::new(vec![Reg {
            id: 7101,
            name: "r7101".to_string(),
            addr: 0,
            n_mb: Some(1),
            tip: 3,
            bits: None,
            grup: Some(1),
            a_en: true,
            a_no_write: 0,
        }]),
    );
    let conn = ConnInfo {
        kpz_id,
        obj_id,
        ip: "127.0.0.1".to_string(),
        port: server_port,
        rtu: 1,
        modem: 1,
        max_pkt_len: 512,
    };

    let res = state.run_a_mode(&client, &transport, &conn, 1).await;
    j.await.expect("join server");
    assert!(
        res.is_err(),
        "timeout-only path must return error for caller handling"
    );
}

#[tokio::test]
async fn run_a_mode_reordered_responses_returns_ok_and_updates_values() {
    let Some(client) = connect_test_db().await else {
        return;
    };
    let transport = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
        .await
        .expect("bind transport");
    let server = UdpSocket::bind("127.0.0.1:0").await.expect("bind server");
    let server_port = server.local_addr().expect("server addr").port();

    let j = tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        let (n, peer) = server.recv_from(&mut buf).await.expect("recv request");
        assert!(n >= 22, "A-mode request must contain hdr22");
        let pid = buf[3];
        let dsr_lo = buf[5];
        let dsr_hi = buf[6];

        let mut resp = vec![0u8; 10];
        resp[3] = pid;
        resp[4] = 1;
        resp[5] = dsr_lo;
        resp[6] = dsr_hi;

        // Return two frames in reverse order относительно запросов.
        let mb_second = mk_read_resp_frame(1, 4, &[0x00, 0xDE]); // 222
        let mb_first = mk_read_resp_frame(1, 4, &[0x00, 0x6F]); // 111
        resp.extend_from_slice(&mb_second);
        resp.extend_from_slice(&mb_first);
        server.send_to(&resp, peer).await.expect("send response");
    });

    let mut state = new_state(1);
    state.n_mb_tit_id = Some(1);
    state.n_mb_reg_id = Some(2);

    let mut grups = vec![0u8; 64];
    grups[0] |= 1 << 0;
    let kpz_id = 3003;
    let obj_id = 30003;
    state.tasks.insert(
        kpz_id,
        KpzTask {
            kpz: KpzRow {
                id: kpz_id,
                name: Some("kpz-a-mode-reordered".to_string()),
                rtu: 1,
                obj: obj_id,
                modem: 1,
                grups,
                max_pkt_len: 512,
                start: 1,
                t_a: 1,
                t_script: 1,
                en_post: true,
            },
            group_id: 1,
            generation: 1,
            next_a: Instant::now(),
            next_script: Instant::now(),
            busy_a: true,
            busy_s: false,
        },
    );
    Arc::make_mut(&mut state.obj_by_id).insert(
        obj_id,
        ObjRow {
            id: obj_id,
            name: Some("obj-reordered".to_string()),
            ip: Some("127.0.0.1".to_string()),
            port: Some(server_port.to_string()),
            kanal: Some(3),
            speed: Some(8),
            stop: Some(0),
            parit: Some(2),
            bit: Some(8),
        },
    );
    Arc::make_mut(&mut state.regs_by_group).insert(
        1,
        Arc::new(vec![
            Reg {
                id: 7201,
                name: "r7201".to_string(),
                addr: 0,
                n_mb: Some(1),
                tip: 3,
                bits: None,
                grup: Some(1),
                a_en: true,
                a_no_write: 0,
            },
            Reg {
                id: 7202,
                name: "r7202".to_string(),
                addr: 10,
                n_mb: Some(1),
                tip: 3,
                bits: None,
                grup: Some(1),
                a_en: true,
                a_no_write: 0,
            },
        ]),
    );
    let conn = ConnInfo {
        kpz_id,
        obj_id,
        ip: "127.0.0.1".to_string(),
        port: server_port,
        rtu: 1,
        modem: 1,
        max_pkt_len: 512,
    };

    let res = state.run_a_mode(&client, &transport, &conn, 1).await;
    j.await.expect("join server");
    assert!(res.is_ok(), "reordered responses must not crash A-mode");
    assert_eq!(
        state.last_a_glued_status.get(&kpz_id).map(String::as_str),
        Some("OK")
    );

    let v1 = state.rv_reg_id(kpz_id, 7201).unwrap_or(0.0) as i32;
    let v2 = state.rv_reg_id(kpz_id, 7202).unwrap_or(0.0) as i32;
    assert!(
        (v1 == 111 || v1 == 222) && (v2 == 111 || v2 == 222) && v1 != v2,
        "expected both decoded values present, got v1={}, v2={}",
        v1,
        v2
    );
}

#[tokio::test]
async fn run_script_mode_partial_response_persists_elam_summary_before_error() {
    let Some(client) = connect_test_db().await else {
        return;
    };
    client.execute("begin", &[]).await.expect("begin");

    let transport = UdpCorrelatedTransport::bind("127.0.0.1:0".parse().unwrap())
        .await
        .expect("bind transport");
    let server = UdpSocket::bind("127.0.0.1:0").await.expect("bind server");
    let server_port = server.local_addr().expect("server addr").port();

    let j = tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        let (n, peer) = server.recv_from(&mut buf).await.expect("recv request");
        assert!(n >= 22, "script-mode request must contain hdr22");
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
        server.send_to(&resp, peer).await.expect("send response");
    });

    let mut state = new_state(1);
    let mut grups = vec![0u8; 64];
    grups[0] |= 1 << 0;
    let kpz_id = 3301;
    let obj_id = 33001;
    state.tasks.insert(
        kpz_id,
        KpzTask {
            kpz: KpzRow {
                id: kpz_id,
                name: Some("kpz-script-partial".to_string()),
                rtu: 1,
                obj: obj_id,
                modem: 1,
                grups,
                max_pkt_len: 512,
                start: 1,
                t_a: 1,
                t_script: 1,
                en_post: true,
            },
            group_id: 1,
            generation: 1,
            next_a: Instant::now(),
            next_script: Instant::now(),
            busy_a: false,
            busy_s: true,
        },
    );
    Arc::make_mut(&mut state.obj_by_id).insert(
        obj_id,
        ObjRow {
            id: obj_id,
            name: Some("obj-script-partial".to_string()),
            ip: Some("127.0.0.1".to_string()),
            port: Some(server_port.to_string()),
            kanal: Some(3),
            speed: Some(8),
            stop: Some(0),
            parit: Some(2),
            bit: Some(8),
        },
    );
    Arc::make_mut(&mut state.g_script_by_group).insert(
        1,
        Arc::new(GScriptRow {
            grup: 1,
            pre_src: Some(
                "reg(1000)=1; reg(1001)=1; reg(1002)=1; reg(1003)=11; reg(1004)=1;".to_string(),
            ),
            post_src: Some("reg(1)=1;".to_string()),
            max_k: Some(2),
            max_words: Some(10),
            en: Some(true),
            ver: Some(1),
        }),
    );

    let conn = ConnInfo {
        kpz_id,
        obj_id,
        ip: "127.0.0.1".to_string(),
        port: server_port,
        rtu: 1,
        modem: 1,
        max_pkt_len: 512,
    };

    let res = state.run_script_mode(&client, &transport, &conn).await;
    j.await.expect("join server");
    assert!(
        res.is_err(),
        "partial script response must still surface mismatch error"
    );

    let count: i64 = client
        .query_one(
            "select count(*) from elam where kpz_id = $1 and status = $2",
            &[&kpz_id, &"SUMMARY: responses < commands (1/2), missing=1"],
        )
        .await
        .expect("query elam")
        .get(0);
    assert_eq!(
        count, 1,
        "ELAM summary row must be persisted before returning mismatch error"
    );

    client.execute("rollback", &[]).await.expect("rollback");
}
