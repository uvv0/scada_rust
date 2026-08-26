use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use rumqttc::{AsyncClient, EventLoop, LastWill, MqttOptions, QoS};
use serde::Serialize;
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
pub struct MqttPublisherConfig {
    pub host: String,
    pub port: u16,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub topic_prefix: String,
    pub queue_cap: usize,
    pub qos: MqttQos,
    pub retain_health: bool,
    pub publish_values: bool,
    pub publish_alarms: bool,
    pub value_kpz_ids: HashSet<i32>,
    pub value_group_ids: HashSet<i32>,
    pub value_reg_ids: HashSet<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MqttQos {
    Qos0,
    Qos1,
    Qos2,
}

impl MqttQos {
    fn as_rumqttc(self) -> QoS {
        match self {
            Self::Qos0 => QoS::AtMostOnce,
            Self::Qos1 => QoS::AtLeastOnce,
            Self::Qos2 => QoS::ExactlyOnce,
        }
    }
}

#[derive(Clone)]
pub struct MqttPublisher {
    tx: mpsc::Sender<MqttEvent>,
    cfg: Arc<MqttPublisherConfig>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MqttValueItem {
    pub reg_id: i32,
    pub addr: Option<i32>,
    pub name: Option<String>,
    pub group_id: Option<i32>,
    pub tip: i32,
    pub value: f64,
    pub quality: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct MqttAlarmPayload {
    pub kpz_id: i32,
    pub reg_id: i32,
    pub rule_id: i64,
    pub event: &'static str,
    pub value: f64,
    pub set_lo: Option<f64>,
    pub set_hi: Option<f64>,
    pub severity: i16,
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MqttHealthPayload {
    pub kind: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub enum MqttEvent {
    Values {
        kpz_id: i32,
        kpz_name: Option<String>,
        ts: i64,
        values: Vec<MqttValueItem>,
    },
    Alarm(MqttAlarmPayload),
    Health(MqttHealthPayload),
}

#[derive(Serialize)]
struct ValuesPayload {
    ts: i64,
    kpz_id: i32,
    kpz_name: Option<String>,
    values: Vec<MqttValueItem>,
}

impl MqttPublisher {
    pub fn start(cfg: MqttPublisherConfig) -> Self {
        let (tx, rx) = mpsc::channel(cfg.queue_cap.max(1));
        let cfg = Arc::new(cfg);

        let mut options = MqttOptions::new(cfg.client_id.clone(), cfg.host.clone(), cfg.port);
        options.set_keep_alive(Duration::from_secs(30));
        if let (Some(username), Some(password)) = (&cfg.username, &cfg.password) {
            options.set_credentials(username.clone(), password.clone());
        }
        options.set_last_will(LastWill::new(
            topic(&cfg.topic_prefix, "status"),
            "offline",
            cfg.qos.as_rumqttc(),
            cfg.retain_health,
        ));

        let (client, eventloop) = AsyncClient::new(options, cfg.queue_cap.max(1));
        tokio::spawn(run_eventloop(eventloop));
        tokio::spawn(run_publisher(client, Arc::clone(&cfg), rx));

        Self { tx, cfg }
    }

    pub fn try_publish(&self, event: MqttEvent) {
        if let Err(e) = self.tx.try_send(event) {
            tracing::warn!(err = %e, "mqtt publish queue full or closed; dropping event");
        }
    }

    pub fn should_publish_value(&self, kpz_id: i32, group_id: Option<i32>, reg_id: i32) -> bool {
        let cfg = &self.cfg;
        if !cfg.publish_values {
            return false;
        }
        if !cfg.value_kpz_ids.is_empty() && !cfg.value_kpz_ids.contains(&kpz_id) {
            return false;
        }
        if !cfg.value_reg_ids.is_empty() && !cfg.value_reg_ids.contains(&reg_id) {
            return false;
        }
        if !cfg.value_group_ids.is_empty() {
            return group_id
                .map(|id| cfg.value_group_ids.contains(&id))
                .unwrap_or(false);
        }
        true
    }
}

async fn run_eventloop(mut eventloop: EventLoop) {
    loop {
        if let Err(e) = eventloop.poll().await {
            tracing::warn!(err = %e, "mqtt eventloop error; retrying");
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}

async fn run_publisher(
    client: AsyncClient,
    cfg: Arc<MqttPublisherConfig>,
    mut rx: mpsc::Receiver<MqttEvent>,
) {
    publish_raw(
        &client,
        &topic(&cfg.topic_prefix, "status"),
        cfg.qos,
        cfg.retain_health,
        "online",
    )
    .await;

    while let Some(event) = rx.recv().await {
        match event {
            MqttEvent::Values {
                kpz_id,
                kpz_name,
                ts,
                values,
            } => {
                if !cfg.publish_values || values.is_empty() {
                    continue;
                }
                let payload = ValuesPayload {
                    ts,
                    kpz_id,
                    kpz_name,
                    values,
                };
                publish_json(
                    &client,
                    &topic(&cfg.topic_prefix, &format!("values/{kpz_id}")),
                    cfg.qos,
                    false,
                    &payload,
                )
                .await;
            }
            MqttEvent::Alarm(payload) => {
                if !cfg.publish_alarms {
                    continue;
                }
                publish_json(
                    &client,
                    &topic(
                        &cfg.topic_prefix,
                        &format!("alarms/{}/{}", payload.kpz_id, payload.rule_id),
                    ),
                    cfg.qos,
                    false,
                    &payload,
                )
                .await;
            }
            MqttEvent::Health(payload) => {
                publish_json(
                    &client,
                    &topic(&cfg.topic_prefix, "health"),
                    cfg.qos,
                    cfg.retain_health,
                    &payload,
                )
                .await;
            }
        }
    }
}

async fn publish_json<T: Serialize>(
    client: &AsyncClient,
    topic: &str,
    qos: MqttQos,
    retain: bool,
    payload: &T,
) {
    match serde_json::to_vec(payload) {
        Ok(bytes) => {
            if let Err(e) = client.publish(topic, qos.as_rumqttc(), retain, bytes).await {
                tracing::warn!(topic = topic, err = %e, "mqtt publish failed");
            }
        }
        Err(e) => tracing::warn!(topic = topic, err = %e, "mqtt payload serialization failed"),
    }
}

async fn publish_raw(
    client: &AsyncClient,
    topic: &str,
    qos: MqttQos,
    retain: bool,
    payload: &'static str,
) {
    if let Err(e) = client
        .publish(topic, qos.as_rumqttc(), retain, payload.as_bytes().to_vec())
        .await
    {
        tracing::warn!(topic = topic, err = %e, "mqtt publish failed");
    }
}

pub fn normalize_topic_prefix(prefix: &str) -> String {
    let prefix = prefix.trim().trim_matches('/');
    if prefix.is_empty() {
        "ss4/v1".to_string()
    } else {
        prefix.to_string()
    }
}

fn topic(prefix: &str, suffix: &str) -> String {
    format!(
        "{}/{}",
        normalize_topic_prefix(prefix),
        suffix.trim().trim_matches('/')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_empty_topic_prefix_to_default() {
        assert_eq!(normalize_topic_prefix("  /  "), "ss4/v1");
    }

    #[test]
    fn trims_topic_prefix_slashes() {
        assert_eq!(topic("/plant/ss4/", "/health/"), "plant/ss4/health");
    }

    #[test]
    fn value_payload_serializes_expected_shape() {
        let payload = ValuesPayload {
            ts: 123,
            kpz_id: 7,
            kpz_name: Some("kpz-7".to_string()),
            values: vec![MqttValueItem {
                reg_id: 42,
                addr: Some(30401),
                name: Some("pressure".to_string()),
                group_id: Some(3),
                tip: 1,
                value: 12.5,
                quality: "ok",
            }],
        };

        let json = serde_json::to_string(&payload).expect("json");
        assert!(json.contains("\"kpz_id\":7"));
        assert!(json.contains("\"kpz_name\":\"kpz-7\""));
        assert!(json.contains("\"reg_id\":42"));
        assert!(json.contains("\"addr\":30401"));
        assert!(json.contains("\"name\":\"pressure\""));
        assert!(json.contains("\"group_id\":3"));
        assert!(json.contains("\"quality\":\"ok\""));
    }

    fn test_publisher_with(mut cfg: MqttPublisherConfig) -> MqttPublisher {
        cfg.queue_cap = 1;
        let (tx, _rx) = mpsc::channel(1);
        MqttPublisher {
            tx,
            cfg: Arc::new(cfg),
        }
    }

    fn test_cfg() -> MqttPublisherConfig {
        MqttPublisherConfig {
            host: "127.0.0.1".to_string(),
            port: 1883,
            client_id: "test".to_string(),
            username: None,
            password: None,
            topic_prefix: "ss4/v1".to_string(),
            queue_cap: 1,
            qos: MqttQos::Qos1,
            retain_health: true,
            publish_values: true,
            publish_alarms: true,
            value_kpz_ids: HashSet::new(),
            value_group_ids: HashSet::new(),
            value_reg_ids: HashSet::new(),
        }
    }

    #[test]
    fn value_filters_allow_all_when_empty() {
        let publisher = test_publisher_with(test_cfg());

        assert!(publisher.should_publish_value(3, Some(21), 6001));
        assert!(publisher.should_publish_value(4, None, 7001));
    }

    #[test]
    fn value_filters_use_all_configured_dimensions() {
        let mut cfg = test_cfg();
        cfg.value_kpz_ids = HashSet::from([3]);
        cfg.value_group_ids = HashSet::from([21]);
        cfg.value_reg_ids = HashSet::from([6001, 6002]);
        let publisher = test_publisher_with(cfg);

        assert!(publisher.should_publish_value(3, Some(21), 6001));
        assert!(!publisher.should_publish_value(4, Some(21), 6001));
        assert!(!publisher.should_publish_value(3, Some(22), 6001));
        assert!(!publisher.should_publish_value(3, Some(21), 7001));
        assert!(!publisher.should_publish_value(3, None, 6001));
    }

    #[test]
    fn value_filters_respect_publish_values_switch() {
        let mut cfg = test_cfg();
        cfg.publish_values = false;
        let publisher = test_publisher_with(cfg);

        assert!(!publisher.should_publish_value(3, Some(21), 6001));
    }
}
