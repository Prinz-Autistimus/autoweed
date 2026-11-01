//! Schreiben und Lesen der Messwerte in InfluxDB 2 (Line Protocol bzw. Flux).

use std::fmt::Write as _;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use autoweed_shared::{BufferEntry, Event, History, HistoryPoint};
use chrono::{DateTime, SecondsFormat, Utc};
use reqwest::Client;
use serde_json::json;

/// Ungefähre Anzahl Punkte pro Diagramm.
const TARGET_POINTS: i64 = 500;
const MIN_WINDOW_SECS: i64 = 5;
const MAX_EVENTS: u32 = 500;

#[derive(Clone)]
pub struct Influx {
    http: Client,
    url: String,
    org: String,
    bucket: String,
    token: String,
}

fn bool_field(on: bool) -> u8 {
    u8::from(on)
}

/// Escaping für String-Felder im Line Protocol.
fn escape_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn nanos(at: DateTime<Utc>) -> i64 {
    at.timestamp_nanos_opt().unwrap_or_default()
}

fn rfc3339(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Secs, true)
}

impl Influx {
    pub fn new(url: &str, org: &str, bucket: &str, token: &str) -> Result<Self> {
        Ok(Self {
            http: Client::builder().timeout(Duration::from_secs(15)).build()?,
            url: url.trim_end_matches('/').to_owned(),
            org: org.to_owned(),
            bucket: bucket.to_owned(),
            token: token.to_owned(),
        })
    }

    /// Schreibt Messwerte und Ereignisse vom Board. Gleiche Zeitstempel
    /// überschreiben sich, ein doppelter Abruf ist also unschädlich.
    pub async fn write(&self, entries: &[BufferEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut body = String::new();
        for entry in entries {
            match entry {
                BufferEntry::Measurement(m) => {
                    let mut fields = vec![
                        format!("heater={}i", bool_field(m.actuators.heater)),
                        format!("light={}i", bool_field(m.actuators.light)),
                        format!("pump={}i", bool_field(m.actuators.pump)),
                    ];
                    let s = &m.sensors;
                    if let Some(v) = s.temperature_celsius {
                        fields.push(format!("temperature={v}"));
                    }
                    if let Some(v) = s.humidity_percent {
                        fields.push(format!("humidity={v}"));
                    }
                    if let Some(v) = s.soil_percent {
                        fields.push(format!("soil={v}"));
                    }
                    if let Some(v) = s.soil_raw {
                        fields.push(format!("soil_raw={v}i"));
                    }
                    let _ = writeln!(body, "climate {} {}", fields.join(","), nanos(m.at));
                }
                BufferEntry::Event(e) => {
                    let json = serde_json::to_string(e)?;
                    let kind = serde_json::to_value(&e.kind)?
                        .get("type")
                        .and_then(|t| t.as_str().map(str::to_owned))
                        .unwrap_or_default();
                    let _ = writeln!(
                        body,
                        "event,kind={kind} json=\"{}\" {}",
                        escape_string(&json),
                        nanos(e.at)
                    );
                }
            }
        }
        let response = self
            .http
            .post(format!("{}/api/v2/write", self.url))
            .query(&[
                ("org", &self.org),
                ("bucket", &self.bucket),
                ("precision", &"ns".to_owned()),
            ])
            .header("Authorization", format!("Token {}", self.token))
            .body(body)
            .send()
            .await
            .context("InfluxDB nicht erreichbar")?;
        if !response.status().is_success() {
            bail!(
                "InfluxDB-Schreibfehler {}: {}",
                response.status(),
                response.text().await?
            );
        }
        Ok(())
    }

    /// Führt eine Flux-Abfrage aus und liefert die Zeilen als Spaltenname → Wert.
    async fn query(&self, flux: &str) -> Result<Vec<Vec<(String, String)>>> {
        let response = self
            .http
            .post(format!("{}/api/v2/query", self.url))
            .query(&[("org", &self.org)])
            .header("Authorization", format!("Token {}", self.token))
            .json(&json!({
                "query": flux,
                "type": "flux",
                "dialect": { "header": true, "annotations": [] }
            }))
            .send()
            .await
            .context("InfluxDB nicht erreichbar")?;
        if !response.status().is_success() {
            bail!(
                "InfluxDB-Abfragefehler {}: {}",
                response.status(),
                response.text().await?
            );
        }
        let text = response.text().await?;

        // Jede Tabelle der Antwort beginnt mit einer eigenen Kopfzeile.
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(text.as_bytes());
        let mut header: Vec<String> = Vec::new();
        let mut rows = Vec::new();
        for record in reader.records() {
            let record = record?;
            if record.iter().all(str::is_empty) {
                continue;
            }
            if record.get(1) == Some("result") {
                header = record.iter().map(str::to_owned).collect();
                continue;
            }
            rows.push(
                header
                    .iter()
                    .cloned()
                    .zip(record.iter().map(str::to_owned))
                    .collect(),
            );
        }
        Ok(rows)
    }

    pub async fn history(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<History> {
        let span = (end - start).num_seconds().max(1);
        let window = (span / TARGET_POINTS).max(MIN_WINDOW_SECS);
        let flux = format!(
            r#"from(bucket: "{bucket}")
  |> range(start: time(v: "{start}"), stop: time(v: "{end}"))
  |> filter(fn: (r) => r._measurement == "climate" and r._field != "soil_raw")
  |> toFloat()
  |> aggregateWindow(every: {window}s, fn: mean, createEmpty: false)
  |> pivot(rowKey: ["_time"], columnKey: ["_field"], valueColumn: "_value")
  |> keep(columns: ["_time", "temperature", "humidity", "soil", "heater", "light", "pump"])
  |> sort(columns: ["_time"])"#,
            bucket = self.bucket,
            start = rfc3339(start),
            end = rfc3339(end),
        );
        let points = self
            .query(&flux)
            .await?
            .into_iter()
            .filter_map(|row| {
                let get = |name: &str| {
                    row.iter()
                        .find(|(k, _)| k == name)
                        .and_then(|(_, v)| v.parse::<f32>().ok())
                };
                let at = row.iter().find(|(k, _)| k == "_time")?.1.parse().ok()?;
                Some(HistoryPoint {
                    at,
                    temperature_celsius: get("temperature"),
                    humidity_percent: get("humidity"),
                    soil_percent: get("soil"),
                    heater: get("heater"),
                    light: get("light"),
                    pump: get("pump"),
                })
            })
            .collect();
        Ok(History {
            window_secs: window as u32,
            points,
        })
    }

    /// Neueste Ereignisse zuerst.
    pub async fn events(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<Event>> {
        let flux = format!(
            r#"from(bucket: "{bucket}")
  |> range(start: time(v: "{start}"), stop: time(v: "{end}"))
  |> filter(fn: (r) => r._measurement == "event" and r._field == "json")
  |> group()
  |> sort(columns: ["_time"], desc: true)
  |> limit(n: {MAX_EVENTS})
  |> keep(columns: ["_value"])"#,
            bucket = self.bucket,
            start = rfc3339(start),
            end = rfc3339(end),
        );
        Ok(self
            .query(&flux)
            .await?
            .into_iter()
            .filter_map(|row| {
                let json = &row.iter().find(|(k, _)| k == "_value")?.1;
                serde_json::from_str(json).ok()
            })
            .collect())
    }
}
