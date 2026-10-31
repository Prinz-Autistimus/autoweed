//! Login mit einem gemeinsamen Passwort, Sessions per Cookie und Schutz
//! gegen Durchprobieren (die WebUI ist öffentlich erreichbar).

use std::collections::HashMap;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::http::{HeaderMap, HeaderValue, header};

pub const COOKIE_NAME: &str = "aw_session";
const SESSION_LIFETIME: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);
const MAX_FAILURES: usize = 10;

pub struct Auth {
    password: String,
    sessions: Mutex<HashMap<String, Instant>>,
    failures: Mutex<HashMap<String, Vec<Instant>>>,
}

pub enum LoginError {
    WrongPassword,
    TooManyAttempts,
}

/// Vergleich in konstanter Zeit, damit die Antwortzeit nichts verrät.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .expect("/dev/urandom nicht lesbar");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// IP des Clients. Hinter dem Nginx Proxy Manager steht sie in `X-Real-IP`.
pub fn client_ip(headers: &HeaderMap, peer: SocketAddr) -> String {
    headers
        .get("x-real-ip")
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|v| v.trim().to_owned())
        .unwrap_or_else(|| peer.ip().to_string())
}

pub fn session_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|pair| {
            pair.trim()
                .strip_prefix(&format!("{COOKIE_NAME}="))
                .map(str::to_owned)
        })
}

/// Über HTTPS (Proxy) bekommt das Cookie `Secure`, im LAN über HTTP nicht.
pub fn set_cookie(headers: &HeaderMap, token: &str, max_age: Duration) -> HeaderValue {
    let secure = headers
        .get("x-forwarded-proto")
        .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"https"));
    let value = format!(
        "{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{}",
        max_age.as_secs(),
        if secure { "; Secure" } else { "" }
    );
    HeaderValue::from_str(&value).expect("Cookie ist gültiges ASCII")
}

impl Auth {
    pub fn new(password: String) -> Self {
        Self {
            password,
            sessions: Mutex::new(HashMap::new()),
            failures: Mutex::new(HashMap::new()),
        }
    }

    /// Gibt bei Erfolg einen neuen Session-Token zurück.
    pub fn login(&self, password: &str, ip: &str) -> Result<String, LoginError> {
        let now = Instant::now();
        {
            let mut failures = self.failures.lock().unwrap();
            failures.retain(|_, times| {
                times.retain(|t| now.duration_since(*t) < FAILURE_WINDOW);
                !times.is_empty()
            });
            if failures.get(ip).is_some_and(|t| t.len() >= MAX_FAILURES) {
                return Err(LoginError::TooManyAttempts);
            }
            if !constant_time_eq(password.as_bytes(), self.password.as_bytes()) {
                failures.entry(ip.to_owned()).or_default().push(now);
                tracing::warn!("Fehlgeschlagener Login von {ip}");
                return Err(LoginError::WrongPassword);
            }
            failures.remove(ip);
        }
        let token = random_token();
        let mut sessions = self.sessions.lock().unwrap();
        sessions.retain(|_, expires| *expires > now);
        sessions.insert(token.clone(), now + SESSION_LIFETIME);
        tracing::info!("Login von {ip}");
        Ok(token)
    }

    pub fn logout(&self, token: &str) {
        self.sessions.lock().unwrap().remove(token);
    }

    pub fn is_valid(&self, token: &str) -> bool {
        self.sessions
            .lock()
            .unwrap()
            .get(token)
            .is_some_and(|expires| *expires > Instant::now())
    }
}

pub const SESSION_MAX_AGE: Duration = SESSION_LIFETIME;
