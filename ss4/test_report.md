# Отчет о тестах (ss4)

Дата: 2026-03-13
Проект: `C:\andr\my2\ss4`

## Команда
`cargo test`

## Результат
- Всего: 99
- Успешно: **95**
- Ошибок: 0
- Игнорировано: 4
- Статус: PASS

Игнорируемые тесты:
- `db_integration_alarm_and_arx_val_roundtrip`
- `db_integration_specific_rule_kpz5_reg6002_rule1`
- `db_integration_obj_fingerprint_query_accepts_integer_port_column`
- `db_integration_load_topology_fingerprint_succeeds`

Причина: DB-интеграционные тесты требуют `TEST_DB_URL` и запускаются отдельно.

### Обычный набор — заметные группы

**UDP transport**
- `send_late_response_after_timeout_is_dropped_and_pending_cleaned`
- `send_reordered_responses_both_correlate_correctly`
- `send_duplicate_response_accepts_and_does_not_crash`
- `send_accepts_response_with_swapped_dsr_modem_fields`, modem mismatch

**Scheduler (tests_core)**
- `job_queue_retain_keeps_only_matching_kpz_and_preserves_order`
- `job_queue_one_kpz_not_parallel_pop_returns_one_at_a_time_for_same_kpz`
- `db_delta_is_empty_and_total_rows`
- `db_delta_drop_poll_logs_clears_only_poll_logs`

**smode**
- `decode_pre_cmds_skips_when_enable_zero`
- `decode_pre_cmds_valid_addr_cnt`
- `decode_pre_cmds_rejects_cnt_over_max_words`
- `decode_pre_cmds_multiple_commands`

**Modbus**
- Тесты расширенного RTU — валидный диапазон 248..1997.

**Прочие регрессии**
- `scheduler::tests_async::run_script_job_success_clears_no_response_streak`
- `scheduler::tests_async::run_script_mode_partial_response_persists_elam_summary_before_error`
- `db_queries::tests::build_conn_*` — lookup, direct ip, reject empty/missing.

**MQTT**
- `mqtt_publisher::tests::normalizes_empty_topic_prefix_to_default`
- `mqtt_publisher::tests::trims_topic_prefix_slashes`
- `mqtt_publisher::tests::value_payload_serializes_expected_shape`
- `tests::mqtt_config_defaults_are_safe_for_local_broker`
- `tests::mqtt_config_prefers_env_secrets_and_normalizes_fields`
- `tests::mqtt_config_unknown_qos_falls_back_to_qos1`

## Команда DB-интеграции
```powershell
$env:TEST_DB_URL = "postgresql://ss4_user:change-me@localhost:5432/ss4_db"
cargo test db_integration -- --ignored --nocapture
```

## Результат DB-интеграции
- Всего: 4
- Успешно: 4
- Ошибок: 0
- Статус: PASS

Покрытые DB-specific проверки:
- `db_queries::tests::db_integration_alarm_and_arx_val_roundtrip`
- `db_queries::tests::db_integration_specific_rule_kpz5_reg6002_rule1`
- `db_queries::tests::db_integration_obj_fingerprint_query_accepts_integer_port_column`
- `db_queries::tests::db_integration_load_topology_fingerprint_succeeds`
