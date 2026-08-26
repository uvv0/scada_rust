//! Scheduler-wide constants (retention, metrics, alarms, post-cmd keys). Re-exported by scheduler for submodules.

pub(super) const ELAM_RETENTION_DAYS: i32 = 3;
pub(super) const ELAM_CLEANUP_EVERY_SEC: u64 = 180;
pub(super) const ELAM_CLEANUP_BATCH_LIMIT: i64 = 5000;
pub(super) const ELAM_CLEANUP_MAX_BATCHES: usize = 20;
pub(super) const POLL_LOG_RETENTION_DAYS: i32 = 14;
pub(super) const POLL_LOG_CLEANUP_EVERY_SEC: u64 = 180;
pub(super) const POLL_LOG_CLEANUP_BATCH_LIMIT: i64 = 5000;
pub(super) const POLL_LOG_CLEANUP_MAX_BATCHES: usize = 20;
pub(super) const HEALTH_POLL_LOG_MIN_INTERVAL_SEC: u64 = 60;
pub(super) const METRICS_EVERY_SEC: u64 = 10;
pub(super) const METRICS_ERR_RATE_WARN: f64 = 0.20;
pub(super) const METRICS_ERR_RATE_CRIT: f64 = 0.50;
pub(super) const METRICS_ERR_STREAK_CRIT_WINDOWS: u32 = 3;
pub(super) const DEFAULT_METRICS_P95_WARN_MS: u64 = 1000;
pub(super) const DEFAULT_METRICS_P95_CRIT_MS: u64 = 3000;
pub(super) const DEFAULT_MODBUS_A_TIMEOUT_MS: u64 = 6000;
pub(super) const DEFAULT_MODBUS_SCRIPT_TIMEOUT_MS: u64 = 6000;
pub(super) const DIAG_WARN_MIN_INTERVAL_SEC: u64 = 60;
pub(super) const IDX_QUALITY_STALE_SEC: i64 = 300;
pub(super) const RV_ALARM_EVENT_ON: i32 = 90000;
pub(super) const RV_ALARM_RULE_ID: i32 = 90001;
pub(super) const RV_ALARM_REG_ID: i32 = 90002;
pub(super) const RV_ALARM_VALUE: i32 = 90003;
pub(super) const RV_ALARM_SET_LO: i32 = 90004;
pub(super) const RV_ALARM_SET_HI: i32 = 90005;
pub(super) const RV_ALARM_HYST: i32 = 90006;
pub(super) const RV_ALARM_SEVERITY: i32 = 90007;
pub(super) const RV_ALARM_TS_UNIX: i32 = 90008;

pub(super) const POST_CMD_EN: i32 = 920;
pub(super) const POST_CMD_FUNC: i32 = 921;
pub(super) const POST_CMD_ADDR: i32 = 922;
pub(super) const POST_CMD_VAL: i32 = 923;
