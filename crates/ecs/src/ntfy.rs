//! Push-Benachrichtigungen über die bestehende ntfy-Instanz.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde_json::json;

#[derive(Clone)]
pub struct Ntfy {
    http: Client,
    url: String,
    topic: String,
    /// Link, der beim Antippen der Benachrichtigung geöffnet wird.
    click: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum Level {
    Problem,
    Resolved,
}

impl Ntfy {
    pub fn new(url: &str, topic: &str, click: Option<String>) -> Result<Self> {
        Ok(Self {
            http: Client::builder().timeout(Duration::from_secs(10)).build()?,
            url: url.trim_end_matches('/').to_owned(),
            topic: topic.to_owned(),
            click,
        })
    }

    /// Fehler beim Senden werden nur geloggt, damit die Regelung davon unberührt bleibt.
    pub async fn send(&self, level: Level, title: &str, message: &str) {
        if let Err(err) = self.try_send(level, title, message).await {
            tracing::error!("ntfy-Benachrichtigung fehlgeschlagen: {err:#}");
        }
    }

    async fn try_send(&self, level: Level, title: &str, message: &str) -> Result<()> {
        let (priority, tag) = match level {
            Level::Problem => (4, "warning"),
            Level::Resolved => (3, "white_check_mark"),
        };
        // JSON-Variante, damit Umlaute im Titel kein Problem sind.
        let mut body = json!({
            "topic": self.topic,
            "title": format!("AutoWeed: {title}"),
            "message": message,
            "priority": priority,
            "tags": [tag],
        });
        if let Some(click) = &self.click {
            body["click"] = json!(click);
        }
        let response = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .context("ntfy nicht erreichbar")?;
        if !response.status().is_success() {
            bail!("ntfy antwortet mit {}", response.status());
        }
        tracing::info!("ntfy gesendet: {title}");
        Ok(())
    }
}
