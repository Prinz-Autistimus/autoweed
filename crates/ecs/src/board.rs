//! HTTP-Client für die Board-API.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use autoweed_shared::{
    BoardConfig, BoardStatus, MeasurementBatch, MeasurementsQuery, OverrideRequest, PumpRequest,
};
use reqwest::{Client, RequestBuilder, Response};
use serde::Serialize;

#[derive(Clone)]
pub struct BoardClient {
    http: Client,
    base: String,
    token: String,
}

impl BoardClient {
    pub fn new(base: &str, token: &str) -> Result<Self> {
        Ok(Self {
            http: Client::builder().timeout(Duration::from_secs(5)).build()?,
            base: base.trim_end_matches('/').to_owned(),
            token: token.to_owned(),
        })
    }

    fn request(&self, method: reqwest::Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}/api/v1/{path}", self.base))
            .bearer_auth(&self.token)
    }

    async fn send(request: RequestBuilder) -> Result<Response> {
        let response = request.send().await.context("Board nicht erreichbar")?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let body = response.text().await.unwrap_or_default();
        bail!("Board antwortet mit {status}: {body}")
    }

    pub async fn status(&self) -> Result<BoardStatus> {
        Ok(Self::send(self.request(reqwest::Method::GET, "status"))
            .await?
            .json()
            .await?)
    }

    pub async fn measurements(&self, since: u64, boot_id: u64) -> Result<MeasurementBatch> {
        let query = MeasurementsQuery { since, boot_id };
        Ok(Self::send(
            self.request(reqwest::Method::GET, "measurements")
                .query(&query),
        )
        .await?
        .json()
        .await?)
    }

    pub async fn put_config(&self, config: &BoardConfig) -> Result<()> {
        Self::send(self.request(reqwest::Method::PUT, "config").json(config)).await?;
        Ok(())
    }

    async fn post(&self, path: &str, body: &impl Serialize) -> Result<()> {
        Self::send(self.request(reqwest::Method::POST, path).json(body)).await?;
        Ok(())
    }

    pub async fn override_actuator(&self, actuator: &str, req: &OverrideRequest) -> Result<()> {
        self.post(&format!("override/{actuator}"), req).await
    }

    pub async fn pump(&self, req: &PumpRequest) -> Result<()> {
        self.post("override/pump", req).await
    }

    pub async fn unlock_pump(&self) -> Result<()> {
        self.post("pump/unlock", &()).await
    }
}
