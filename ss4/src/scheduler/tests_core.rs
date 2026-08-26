use super::*;

fn new_state(pool_size: usize) -> SchedulerState {
    SchedulerState::new_with_limits(pool_size, 20_000, pool_size.max(1), 3, 600)
}

fn reg(addr: i32, tip: i32) -> Reg {
    Reg {
        id: addr,
        name: format!("r{}", addr),
        addr,
        n_mb: Some(1),
        tip,
        bits: None,
        grup: Some(1),
        a_en: true,
        a_no_write: 0,
    }
}

#[test]
fn decode_groups_decodes_bits() {
    let mut g = vec![0u8; 64];
    g[0] |= 1 << 0; // group 1
    g[1] |= 1 << 1; // bit 9 => group 10
    let out = decode_groups(&g);
    assert!(out.contains(&1));
    assert!(out.contains(&10));
    assert_eq!(out.len(), 2);
}

#[test]
fn build_blocks_splits_by_gap() {
    let regs = vec![reg(0, 3), reg(1, 3), reg(10, 3)];
    let blocks = build_blocks_with_func(&regs, 120, 4);
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].adr, 0);
    assert_eq!(blocks[0].cnt_words, 2);
    assert_eq!(blocks[0].func, 4);
    assert_eq!(blocks[1].adr, 10);
    assert_eq!(blocks[1].cnt_words, 1);
    assert_eq!(blocks[1].func, 4);
}

#[test]
fn plan_group_reads_builds_sorted_blocks_and_write_ids_in_one_pass() {
    let mut r1 = reg(10, 3);
    r1.id = 101;
    r1.n_mb = Some(1); // FC4

    let mut r2 = reg(0, 3);
    r2.id = 102;
    r2.n_mb = Some(2); // FC3

    let mut r3 = reg(1, 3);
    r3.id = 103;
    r3.n_mb = Some(2); // FC3
    r3.a_no_write = 1;

    let regs = vec![r1.clone(), r2.clone(), r3.clone()];
    let (regs_poll_sorted, write_ids, blocks) =
        plan_group_reads(&regs, Some(1), Some(2), 120).expect("group plan");

    assert_eq!(regs_poll_sorted.len(), 3);
    assert_eq!(regs_poll_sorted[0].addr, 0);
    assert_eq!(regs_poll_sorted[1].addr, 1);
    assert_eq!(regs_poll_sorted[2].addr, 10);

    assert!(write_ids.contains(&101));
    assert!(write_ids.contains(&102));
    assert!(!write_ids.contains(&103));

    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].adr, 0);
    assert_eq!(blocks[0].cnt_words, 2);
    assert_eq!(blocks[0].func, 3);
    assert_eq!(blocks[1].adr, 10);
    assert_eq!(blocks[1].cnt_words, 1);
    assert_eq!(blocks[1].func, 4);
}

#[test]
fn reload_protocol_topology_builds_fallback_script_bindings_by_group() {
    let mut state = new_state(1);
    state.reload_protocol_topology_from_rows(
        vec![(1, "TIT".to_string()), (2, "REG".to_string())],
        vec![
            Reg {
                id: 7001,
                name: "r7001".to_string(),
                addr: 400,
                n_mb: Some(1),
                tip: 3,
                bits: None,
                grup: Some(7),
                a_en: true,
                a_no_write: 0,
            },
            Reg {
                id: 7002,
                name: "r7002".to_string(),
                addr: 401,
                n_mb: Some(2),
                tip: 3,
                bits: None,
                grup: Some(7),
                a_en: true,
                a_no_write: 0,
            },
        ],
        Vec::new(),
        Vec::<ScriptBindingRow>::new(),
    );

    let bindings = state
        .script_fallback_bindings_by_group
        .get(&7)
        .expect("fallback bindings for group 7");
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].logical, 7001);
    assert_eq!(bindings[0].reg_id, 7001);
    assert_eq!(bindings[0].addr, 400);
    assert_eq!(bindings[1].logical, 7002);
    assert_eq!(bindings[1].reg_id, 7002);
    assert_eq!(bindings[1].addr, 401);
}

#[test]
fn words_from_modbus_frame_reads_words() {
    let mb = vec![1u8, 3, 4, 0x12, 0x34, 0x56, 0x78, 0, 0];
    let words = words_from_modbus_frame(&mb, 2);
    assert_eq!(words, vec![0x1234, 0x5678]);
}

#[test]
fn normalize_emit_ts_invalid_returns_fallback() {
    let fallback = 1_760_000_000i64;
    assert_eq!(normalize_emit_ts_unix(0.0, fallback), fallback);
    assert_eq!(normalize_emit_ts_unix(-100.0, fallback), fallback);
}

#[test]
fn idx_quality_resets_after_stale_timeout_and_requires_two_samples_again() {
    let kpz_id = 1;
    let addr = 400;
    let ready_key = svc_key(kpz_id, 60000 + addr * 2);
    let quality_key = svc_key(kpz_id, 80000 + addr);

    let mut s = new_state(1);
    s.update_idx_quality(kpz_id, addr, 1_000, 10.0);
    s.update_idx_quality(kpz_id, addr, 1_001, 11.0);

    assert_eq!(s.get_rv(kpz_id, ready_key), 1.0);
    assert_eq!(s.get_rv(kpz_id, quality_key), 100.0);

    s.refresh_idx_quality_staleness(kpz_id, 1_001 + IDX_QUALITY_STALE_SEC + 1);
    assert_eq!(s.get_rv(kpz_id, ready_key), 0.0);
    assert_eq!(s.get_rv(kpz_id, quality_key), 0.0);

    s.update_idx_quality(kpz_id, addr, 1_002 + IDX_QUALITY_STALE_SEC, 12.0);
    assert_eq!(s.get_rv(kpz_id, ready_key), 0.0);
    assert_eq!(s.get_rv(kpz_id, quality_key), 0.0);

    s.update_idx_quality(kpz_id, addr, 1_003 + IDX_QUALITY_STALE_SEC, 13.0);
    assert_eq!(s.get_rv(kpz_id, ready_key), 1.0);
    assert_eq!(s.get_rv(kpz_id, quality_key), 100.0);
}

fn alarm_rule_gt(hi: f64, hysteresis: f64) -> AlarmRule {
    AlarmRule {
        id: 1,
        kpz_id: 1,
        reg_id: 6002,
        cmp: "gt".to_string(),
        set_lo: None,
        set_hi: Some(hi),
        set_lo_1: None,
        set_hi_1: None,
        hysteresis,
        on_delay_sec: 0,
        off_delay_sec: 0,
        severity: 1,
        code: None,
        message: None,
    }
}

fn alarm_rule_lt(lo: f64, hysteresis: f64) -> AlarmRule {
    AlarmRule {
        id: 2,
        kpz_id: 1,
        reg_id: 6002,
        cmp: "lt".to_string(),
        set_lo: Some(lo),
        set_hi: None,
        set_lo_1: None,
        set_hi_1: None,
        hysteresis,
        on_delay_sec: 0,
        off_delay_sec: 0,
        severity: 1,
        code: None,
        message: None,
    }
}

#[test]
fn alarm_gt_hysteresis_keeps_active_until_hi_minus_h() {
    let rule = alarm_rule_gt(45.0, 1.0);
    assert!(alarm_should_be_active(&rule, 46.0, false));
    assert!(alarm_should_be_active(&rule, 44.5, true));
    assert!(!alarm_should_be_active(&rule, 44.0, true));
}

#[test]
fn alarm_lt_hysteresis_keeps_active_until_lo_plus_h() {
    let rule = alarm_rule_lt(10.0, 2.0);
    assert!(alarm_should_be_active(&rule, 9.0, false));
    assert!(alarm_should_be_active(&rule, 11.0, true));
    assert!(!alarm_should_be_active(&rule, 12.0, true));
}

#[test]
fn alarm_gt_1_uses_hi_1_threshold() {
    let mut rule = alarm_rule_gt(50.0, 1.0);
    rule.cmp = "gt_1".to_string();
    rule.set_hi_1 = Some(45.0);
    assert!(alarm_should_be_active(&rule, 46.0, false));
    assert!(alarm_should_be_active(&rule, 44.5, true));
    assert!(!alarm_should_be_active(&rule, 44.0, true));
}

#[test]
fn alarm_lt_1_uses_lo_1_threshold() {
    let mut rule = alarm_rule_lt(10.0, 1.0);
    rule.cmp = "lt_1".to_string();
    rule.set_lo_1 = Some(15.0);
    assert!(alarm_should_be_active(&rule, 14.0, false));
    assert!(alarm_should_be_active(&rule, 15.5, true));
    assert!(!alarm_should_be_active(&rule, 16.0, true));
}

#[test]
fn post_command_keys_are_reserved() {
    assert!(is_post_command_key(920));
    assert!(is_post_command_key(921));
    assert!(is_post_command_key(922));
    assert!(is_post_command_key(923));
    assert!(!is_post_command_key(919));
    assert!(!is_post_command_key(924));
}

#[test]
fn post_addr_converts_from_1_based_to_wire() {
    assert_eq!(post_cmd::post_addr_to_wire(1), 0);
    assert_eq!(post_cmd::post_addr_to_wire(512), 511);
    assert_eq!(post_cmd::post_addr_to_wire(515), 514);
    assert_eq!(post_cmd::post_addr_to_wire(0), 0);
}

#[test]
fn extract_post_device_command_reads_920_923_keys() {
    let mut regs = HashMap::new();
    regs.insert(POST_CMD_EN, 1.0);
    regs.insert(POST_CMD_FUNC, 5.0);
    regs.insert(POST_CMD_ADDR, 512.0);
    regs.insert(POST_CMD_VAL, 1.0);

    let cmd = extract_post_device_command(&regs).expect("command");
    assert_eq!(
        cmd,
        PostDeviceCmd {
            func: 5,
            addr: 512,
            value: 1.0
        }
    );
}

#[test]
fn extract_post_device_command_disabled_returns_none() {
    let mut regs = HashMap::new();
    regs.insert(POST_CMD_EN, 0.0);
    regs.insert(POST_CMD_FUNC, 5.0);
    regs.insert(POST_CMD_ADDR, 512.0);
    regs.insert(POST_CMD_VAL, 1.0);
    assert!(extract_post_device_command(&regs).is_none());
}

#[test]
fn build_post_device_mb_fc5_uses_wire_addr_and_ff00() {
    let cmd = PostDeviceCmd {
        func: 5,
        addr: 512,
        value: 1.0,
    };
    let (addr_wire, mb) = build_post_device_mb(301, cmd).expect("mb");
    assert_eq!(addr_wire, 511);
    assert_eq!(mb[0], 0xF8);
    assert_eq!(mb[1], 0x35);
    assert_eq!(mb[2], 0x05);
    assert_eq!(mb[3], 0x01);
    assert_eq!(mb[4], 0xFF);
    assert_eq!(mb[5], 0xFF);
    assert_eq!(mb[6], 0x00);
}

#[test]
fn build_post_device_mb_fc6_uses_wire_addr_and_single_word() {
    let cmd = PostDeviceCmd {
        func: 6,
        addr: 515,
        value: 0x1234 as f64,
    };
    let (addr_wire, mb) = build_post_device_mb(301, cmd).expect("mb");
    assert_eq!(addr_wire, 514);
    assert_eq!(mb[0], 0xF8);
    assert_eq!(mb[1], 0x35);
    assert_eq!(mb[2], 0x06);
    assert_eq!(mb[3], 0x02);
    assert_eq!(mb[4], 0x02);
    assert_eq!(mb[5], 0x12);
    assert_eq!(mb[6], 0x34);
}

#[test]
fn build_post_device_mb_rejects_unsupported_func() {
    let cmd = PostDeviceCmd {
        func: 16,
        addr: 512,
        value: 1.0,
    };
    assert!(build_post_device_mb(301, cmd).is_none());
}

#[test]
fn phase_offset_is_within_period() {
    let p = Duration::from_secs(20);
    for id in [5, 1001, 1050, 1099] {
        let a = phase_offset(p, id, 0);
        let s = phase_offset(p, id, 1);
        assert!(a < p);
        assert!(s < p);
    }
}

#[test]
fn run_job_conn_error_does_not_abort_scheduler_cycle() {
    let now = Instant::now();
    let mut state = new_state(2);
    let kpz_id = 42;
    let kpz = KpzRow {
        id: kpz_id,
        name: Some("kpz-42".to_string()),
        rtu: 1,
        obj: 1,
        modem: 1,
        grups: vec![0u8; 64],
        max_pkt_len: 256,
        start: 1,
        t_a: 1,
        t_script: 1,
        en_post: true,
    };
    state.tasks.insert(
        kpz_id,
        KpzTask {
            kpz: kpz.clone(),
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(10),
            next_script: now + Duration::from_secs(10),
            busy_a: true,
            busy_s: true,
        },
    );

    let mut worker = new_state(1);
    worker.tasks.insert(
        kpz_id,
        KpzTask {
            kpz,
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(1),
            next_script: now + Duration::from_secs(1),
            busy_a: false,
            busy_s: false,
        },
    );
    worker.set_rv(kpz_id, 12345, 77.0);

    let err: Result<()> = Err(anyhow::anyhow!("conn build failed"));
    state.complete_worker_merge(kpz_id, &err, worker.into_worker_merge(kpz_id));

    let t = state.tasks.get(&kpz_id).expect("task must remain");
    assert!(!t.busy_a);
    assert!(!t.busy_s);
    assert_eq!(state.get_rv(kpz_id, 12345), 77.0);
}

#[test]
fn stale_worker_merge_after_stop_is_dropped() {
    let now = Instant::now();
    let kpz_id = 420;
    let kpz = KpzRow {
        id: kpz_id,
        name: Some("kpz-stop".to_string()),
        rtu: 1,
        obj: 1,
        modem: 1,
        grups: vec![0u8; 64],
        max_pkt_len: 256,
        start: 1,
        t_a: 1,
        t_script: 1,
        en_post: true,
    };

    let mut state = new_state(1);
    state.tasks.insert(
        kpz_id,
        KpzTask {
            kpz: kpz.clone(),
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(30),
            next_script: now + Duration::from_secs(30),
            busy_a: true,
            busy_s: true,
        },
    );

    let mut worker = new_state(1);
    worker.tasks.insert(
        kpz_id,
        KpzTask {
            kpz,
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(1),
            next_script: now + Duration::from_secs(1),
            busy_a: false,
            busy_s: false,
        },
    );
    worker.set_rv(kpz_id, 12345, 77.0);

    state.on_kpz_stop(kpz_id);
    let stopped_generation = state.tasks.get(&kpz_id).expect("task").generation;
    let stopped_task = state.tasks.get(&kpz_id).expect("task").clone();

    let ok: Result<()> = Ok(());
    state.complete_worker_merge(kpz_id, &ok, worker.into_worker_merge(kpz_id));

    let t = state.tasks.get(&kpz_id).expect("task must remain");
    assert_eq!(t.generation, stopped_generation);
    assert_eq!(t.next_a, stopped_task.next_a);
    assert_eq!(t.next_script, stopped_task.next_script);
    assert!(!t.busy_a);
    assert!(!t.busy_s);
    assert_eq!(state.get_rv(kpz_id, 12345), 0.0);
}

#[test]
fn stale_worker_merge_after_restart_does_not_restore_backoff() {
    let now = Instant::now();
    let kpz_id = 421;
    let kpz = KpzRow {
        id: kpz_id,
        name: Some("kpz-restart".to_string()),
        rtu: 1,
        obj: 1,
        modem: 1,
        grups: vec![0u8; 64],
        max_pkt_len: 256,
        start: 1,
        t_a: 5,
        t_script: 5,
        en_post: true,
    };

    let mut state = new_state(1);
    state.tasks.insert(
        kpz_id,
        KpzTask {
            kpz: kpz.clone(),
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(30),
            next_script: now + Duration::from_secs(30),
            busy_a: true,
            busy_s: false,
        },
    );

    let mut worker = new_state(1);
    worker.tasks.insert(
        kpz_id,
        KpzTask {
            kpz,
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(600),
            next_script: now + Duration::from_secs(600),
            busy_a: false,
            busy_s: false,
        },
    );
    worker.no_resp_streak_by_kpz.insert(kpz_id, 3);

    state.on_kpz_start(kpz_id, now);
    let restarted_generation = state.tasks.get(&kpz_id).expect("task").generation;
    let restarted_task = state.tasks.get(&kpz_id).expect("task").clone();

    let err: Result<()> = Err(anyhow::anyhow!("timeout"));
    state.complete_worker_merge(kpz_id, &err, worker.into_worker_merge(kpz_id));

    let t = state.tasks.get(&kpz_id).expect("task must remain");
    assert_eq!(t.generation, restarted_generation);
    assert_eq!(t.next_a, restarted_task.next_a);
    assert_eq!(t.next_script, restarted_task.next_script);
    assert!(!state.no_resp_streak_by_kpz.contains_key(&kpz_id));
}

#[test]
fn stale_worker_merge_after_generation_bump_is_dropped() {
    let now = Instant::now();
    let kpz_id = 422;
    let kpz = KpzRow {
        id: kpz_id,
        name: Some("kpz-topology".to_string()),
        rtu: 1,
        obj: 1,
        modem: 1,
        grups: vec![0u8; 64],
        max_pkt_len: 256,
        start: 1,
        t_a: 1,
        t_script: 1,
        en_post: true,
    };

    let mut state = new_state(1);
    state.tasks.insert(
        kpz_id,
        KpzTask {
            kpz: kpz.clone(),
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(30),
            next_script: now + Duration::from_secs(30),
            busy_a: true,
            busy_s: false,
        },
    );

    let mut worker = new_state(1);
    worker.tasks.insert(
        kpz_id,
        KpzTask {
            kpz,
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(2),
            next_script: now + Duration::from_secs(2),
            busy_a: false,
            busy_s: false,
        },
    );
    worker.set_rv(kpz_id, 50001, 11.0);

    {
        let task = state.tasks.get_mut(&kpz_id).expect("task");
        task.generation = task.generation.wrapping_add(1);
        task.group_id = 2;
    }
    let current_generation = state.tasks.get(&kpz_id).expect("task").generation;

    let ok: Result<()> = Ok(());
    state.complete_worker_merge(kpz_id, &ok, worker.into_worker_merge(kpz_id));

    let t = state.tasks.get(&kpz_id).expect("task must remain");
    assert_eq!(t.generation, current_generation);
    assert_eq!(t.group_id, 2);
    assert_eq!(state.get_rv(kpz_id, 50001), 0.0);
}

#[test]
fn stale_worker_merge_after_protocol_generation_bump_is_dropped() {
    let now = Instant::now();
    let kpz_id = 423;
    let kpz = KpzRow {
        id: kpz_id,
        name: Some("kpz-protocol".to_string()),
        rtu: 1,
        obj: 1,
        modem: 1,
        grups: vec![0u8; 64],
        max_pkt_len: 256,
        start: 1,
        t_a: 1,
        t_script: 1,
        en_post: true,
    };

    let mut state = new_state(1);
    state.tasks.insert(
        kpz_id,
        KpzTask {
            kpz: kpz.clone(),
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(30),
            next_script: now + Duration::from_secs(30),
            busy_a: true,
            busy_s: false,
        },
    );

    let mut worker = new_state(1);
    worker.protocol_generation = state.protocol_generation;
    worker.tasks.insert(
        kpz_id,
        KpzTask {
            kpz,
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(2),
            next_script: now + Duration::from_secs(2),
            busy_a: false,
            busy_s: false,
        },
    );
    worker.set_rv(kpz_id, 50002, 22.0);

    state.protocol_generation = state.protocol_generation.wrapping_add(1);
    let current_protocol_generation = state.protocol_generation;

    let ok: Result<()> = Ok(());
    state.complete_worker_merge(kpz_id, &ok, worker.into_worker_merge(kpz_id));

    assert_eq!(state.protocol_generation, current_protocol_generation);
    assert_eq!(state.get_rv(kpz_id, 50002), 0.0);
    let t = state.tasks.get(&kpz_id).expect("task must remain");
    assert!(t.busy_a);
}

#[test]
fn drain_queue_like_flow_continues_after_failed_worker_and_merges_next_kpz() {
    let now = Instant::now();
    let mut state = new_state(2);

    let kpz_a = KpzRow {
        id: 100,
        name: Some("kpz-a".to_string()),
        rtu: 1,
        obj: 1,
        modem: 1,
        grups: vec![0u8; 64],
        max_pkt_len: 256,
        start: 1,
        t_a: 1,
        t_script: 1,
        en_post: true,
    };
    let kpz_b = KpzRow {
        id: 200,
        name: Some("kpz-b".to_string()),
        rtu: 2,
        obj: 2,
        modem: 2,
        grups: vec![0u8; 64],
        max_pkt_len: 256,
        start: 1,
        t_a: 1,
        t_script: 1,
        en_post: true,
    };

    state.tasks.insert(
        kpz_a.id,
        KpzTask {
            kpz: kpz_a.clone(),
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(10),
            next_script: now + Duration::from_secs(10),
            busy_a: true,
            busy_s: true,
        },
    );
    state.tasks.insert(
        kpz_b.id,
        KpzTask {
            kpz: kpz_b.clone(),
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(10),
            next_script: now + Duration::from_secs(10),
            busy_a: true,
            busy_s: true,
        },
    );

    let mut worker_a = new_state(1);
    worker_a.tasks.insert(
        kpz_a.id,
        KpzTask {
            kpz: kpz_a,
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(1),
            next_script: now + Duration::from_secs(1),
            busy_a: false,
            busy_s: false,
        },
    );
    worker_a.set_rv(100, 50001, 11.0);

    let mut worker_b = new_state(1);
    worker_b.tasks.insert(
        kpz_b.id,
        KpzTask {
            kpz: kpz_b,
            group_id: 1,
            generation: 1,
            next_a: now + Duration::from_secs(2),
            next_script: now + Duration::from_secs(2),
            busy_a: false,
            busy_s: false,
        },
    );
    worker_b.set_rv(200, 60001, 22.0);

    let err: Result<()> = Err(anyhow::anyhow!("kpz-a failed"));
    state.complete_worker_merge(100, &err, worker_a.into_worker_merge(100));

    let ok: Result<()> = Ok(());
    state.complete_worker_merge(200, &ok, worker_b.into_worker_merge(200));

    assert_eq!(state.get_rv(100, 50001), 11.0);
    assert_eq!(state.get_rv(200, 60001), 22.0);

    let ta = state.tasks.get(&100).expect("task a");
    let tb = state.tasks.get(&200).expect("task b");
    assert!(!ta.busy_a && !ta.busy_s);
    assert!(!tb.busy_a && !tb.busy_s);
}

#[test]
fn pop_next_spawnable_job_skips_running_kpz_without_rotating_queue() {
    let mut state = new_state(2);
    state.queue.push_back(Job {
        kpz_id: 10,
        kind: JobKind::A,
    });
    state.queue.push_back(Job {
        kpz_id: 10,
        kind: JobKind::S,
    });
    state.queue.push_back(Job {
        kpz_id: 20,
        kind: JobKind::A,
    });

    let running_kpz = HashSet::from([10]);
    let next = state
        .pop_next_spawnable_job(&running_kpz)
        .expect("spawnable job");

    assert_eq!(next.kpz_id, 20);
    assert_eq!(state.queue.len(), 2);
    assert_eq!(state.queue[0].kpz_id, 10);
    assert_eq!(state.queue[1].kpz_id, 10);
    assert!(matches!(state.queue[0].kind, JobKind::A));
    assert!(matches!(state.queue[1].kind, JobKind::S));
}

#[test]
fn job_queue_retain_keeps_only_matching_kpz_and_preserves_order() {
    let mut q = JobQueue::new();
    q.push_back(Job {
        kpz_id: 10,
        kind: JobKind::A,
    });
    q.push_back(Job {
        kpz_id: 20,
        kind: JobKind::A,
    });
    q.push_back(Job {
        kpz_id: 10,
        kind: JobKind::S,
    });
    q.push_back(Job {
        kpz_id: 30,
        kind: JobKind::A,
    });
    assert_eq!(q.len(), 4);
    q.retain(|j| j.kpz_id == 10);
    assert_eq!(q.len(), 2);
    assert_eq!(q[0].kpz_id, 10);
    assert!(matches!(q[0].kind, JobKind::A));
    assert_eq!(q[1].kpz_id, 10);
    assert!(matches!(q[1].kind, JobKind::S));
}

#[test]
fn job_queue_one_kpz_not_parallel_pop_returns_one_at_a_time_for_same_kpz() {
    let mut q = JobQueue::new();
    q.push_back(Job {
        kpz_id: 5,
        kind: JobKind::A,
    });
    q.push_back(Job {
        kpz_id: 5,
        kind: JobKind::S,
    });
    let running = HashSet::new();
    let first = q.pop_next_spawnable(&running).expect("first");
    assert_eq!(first.kpz_id, 5);
    assert!(matches!(first.kind, JobKind::A));
    let running_5 = HashSet::from([5]);
    assert!(
        q.pop_next_spawnable(&running_5).is_none(),
        "same kpz running, no second"
    );
    let running_empty: HashSet<i32> = HashSet::new();
    let second = q
        .pop_next_spawnable(&running_empty)
        .expect("second after first done");
    assert_eq!(second.kpz_id, 5);
    assert!(matches!(second.kind, JobKind::S));
}

#[test]
fn metrics_health_alert_and_streak_reset_work() {
    let mut s = new_state(1);
    s.next_metrics_log = Instant::now() - Duration::from_millis(1);
    s.metrics_jobs_started = 10;
    s.metrics_jobs_ok = 5;
    s.metrics_jobs_err = 5;

    let alert1 = s.log_metrics_if_due();
    assert!(alert1.is_some(), "first bad window should emit alert");
    assert_eq!(s.metrics_err_windows_streak, 1);
    assert_eq!(s.metrics_jobs_started, 0);
    assert_eq!(s.metrics_jobs_ok, 0);
    assert_eq!(s.metrics_jobs_err, 0);

    s.next_metrics_log = Instant::now() - Duration::from_millis(1);
    s.metrics_jobs_started = 8;
    s.metrics_jobs_ok = 8;
    s.metrics_jobs_err = 0;
    s.metrics_jobs_timeout = 0;
    s.metrics_lat_le_100_ms = 8;
    let alert2 = s.log_metrics_if_due();
    assert!(alert2.is_some(), "clean window should emit health_ok");
    let (kind2, _msg2) = alert2.unwrap();
    assert_eq!(kind2, "health_ok");
    assert_eq!(
        s.metrics_err_windows_streak, 0,
        "clean window must reset streak"
    );
}

#[test]
fn db_delta_is_empty_and_total_rows() {
    let empty = DbDelta::default();
    assert!(empty.is_empty());
    assert_eq!(empty.total_rows(), 0);

    let mut delta = DbDelta::default();
    delta.poll_logs.push(PollLogRow {
        kpz_id: Some(1),
        kind: "test".to_string(),
        msg: "m".to_string(),
    });
    assert!(!delta.is_empty());
    assert_eq!(delta.total_rows(), 1);
    delta.alarm_state_updates.push(AlarmStateUpdate {
        rule_id: 2,
        active: true,
        value: 1.0,
    });
    assert_eq!(delta.total_rows(), 2);
}

#[test]
fn db_delta_drop_poll_logs_clears_only_poll_logs() {
    let mut delta = DbDelta::default();
    delta.poll_logs.push(PollLogRow {
        kpz_id: None,
        kind: "k".to_string(),
        msg: "m".to_string(),
    });
    delta.poll_logs.push(PollLogRow {
        kpz_id: Some(1),
        kind: "k2".to_string(),
        msg: "m2".to_string(),
    });
    delta.alarm_state_updates.push(AlarmStateUpdate {
        rule_id: 1,
        active: false,
        value: 0.0,
    });
    let dropped = delta.drop_poll_logs();
    assert_eq!(dropped, 2);
    assert!(delta.poll_logs.is_empty());
    assert_eq!(delta.alarm_state_updates.len(), 1);
    assert_eq!(delta.drop_poll_logs(), 0);
}

#[test]
fn db_delta_coalesces_alarm_state_updates_by_rule_id() {
    let mut delta = DbDelta::default();
    delta.alarm_state_updates.push(AlarmStateUpdate {
        rule_id: 10,
        active: false,
        value: 1.0,
    });
    delta.alarm_state_updates.push(AlarmStateUpdate {
        rule_id: 10,
        active: true,
        value: 2.5,
    });
    delta.alarm_state_updates.push(AlarmStateUpdate {
        rule_id: 11,
        active: true,
        value: 3.0,
    });

    let (alarm_dropped, arx_dropped) = delta.coalesce_runtime_updates();
    assert_eq!(alarm_dropped, 1);
    assert_eq!(arx_dropped, 0);
    assert_eq!(delta.alarm_state_updates.len(), 2);

    let rule10 = delta
        .alarm_state_updates
        .iter()
        .find(|row| row.rule_id == 10)
        .expect("rule 10");
    assert!(rule10.active);
    assert!((rule10.value - 2.5).abs() < f64::EPSILON);
}

#[test]
fn db_delta_coalesces_arx_state_updates_by_kpz_and_arx_id() {
    let mut delta = DbDelta::default();
    delta.arx_state_updates.push(ArxStateUpdate {
        kpz_id: 1,
        arx_id: 7,
        last_ind: 10,
    });
    delta.arx_state_updates.push(ArxStateUpdate {
        kpz_id: 1,
        arx_id: 7,
        last_ind: 11,
    });
    delta.arx_state_updates.push(ArxStateUpdate {
        kpz_id: 2,
        arx_id: 7,
        last_ind: 12,
    });

    let (alarm_dropped, arx_dropped) = delta.coalesce_runtime_updates();
    assert_eq!(alarm_dropped, 0);
    assert_eq!(arx_dropped, 1);
    assert_eq!(delta.arx_state_updates.len(), 2);

    let row = delta
        .arx_state_updates
        .iter()
        .find(|row| row.kpz_id == 1 && row.arx_id == 7)
        .expect("kpz/arx row");
    assert_eq!(row.last_ind, 11);
}

#[test]
fn db_delta_append_coalesces_runtime_updates_incrementally() {
    let mut batch = DbDelta::default();
    batch.alarm_state_updates.push(AlarmStateUpdate {
        rule_id: 10,
        active: false,
        value: 1.0,
    });
    batch.arx_state_updates.push(ArxStateUpdate {
        kpz_id: 1,
        arx_id: 7,
        last_ind: 10,
    });

    let mut next = DbDelta::default();
    next.alarm_state_updates.push(AlarmStateUpdate {
        rule_id: 10,
        active: true,
        value: 2.0,
    });
    next.arx_state_updates.push(ArxStateUpdate {
        kpz_id: 1,
        arx_id: 7,
        last_ind: 11,
    });

    let (coalesced_alarm, coalesced_arx) = batch.append_coalescing_runtime_updates(next);
    assert_eq!(coalesced_alarm, 1);
    assert_eq!(coalesced_arx, 1);
    assert_eq!(batch.alarm_state_updates.len(), 1);
    assert_eq!(batch.arx_state_updates.len(), 1);
    assert!(batch.alarm_state_updates[0].active);
    assert_eq!(batch.arx_state_updates[0].last_ind, 11);
}
