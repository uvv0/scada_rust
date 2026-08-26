use tokio::sync::mpsc;

struct OutMsg {
    chat_id: Option<String>,
    text: String,
}

#[derive(Clone)]
pub struct TelegramNotifier {
    tx: mpsc::Sender<OutMsg>,
}

#[derive(serde::Serialize)]
struct SendMessageReq<'a> {
    chat_id: &'a str,
    text: &'a str,
}

impl TelegramNotifier {
    pub fn new(bot_token: String, default_chat_id: Option<String>, queue_cap: usize) -> Self {
        let (tx, mut rx) = mpsc::channel::<OutMsg>(queue_cap.max(1));
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
            while let Some(msg) = rx.recv().await {
                let target_chat = match msg.chat_id.or_else(|| default_chat_id.clone()) {
                    Some(v) if !v.trim().is_empty() => v,
                    _ => {
                        tracing::warn!("telegram message dropped: no chat_id route and no default chat configured");
                        continue;
                    }
                };
                let body = SendMessageReq {
                    chat_id: &target_chat,
                    text: &msg.text,
                };
                let res = client.post(&url).json(&body).send().await;
                if let Err(e) = res {
                    tracing::warn!(err = %e, "telegram send failed");
                }
            }
        });
        Self { tx }
    }

    pub fn try_send_to(&self, chat_id: String, text: String) {
        if let Err(e) = self.tx.try_send(OutMsg {
            chat_id: Some(chat_id),
            text,
        }) {
            tracing::warn!(err = %e, "telegram queue is full or closed");
        }
    }
}
