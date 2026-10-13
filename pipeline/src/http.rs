//! Polite HTTP client: on-disk cache keyed by URL, plus a minimum delay between live requests.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

/// Hides API keys (`key=...`) so URLs can appear in error messages and logs.
pub fn redact(url: &str) -> String {
    match url.find("key=") {
        Some(i) => {
            let rest = &url[i..];
            let end = rest.find('&').unwrap_or(rest.len());
            format!("{}key=***{}", &url[..i], &rest[end..])
        }
        None => url.to_string(),
    }
}

pub struct Client {
    inner: reqwest::blocking::Client,
    cache_dir: PathBuf,
    min_delay: Duration,
    last: Cell<Option<Instant>>,
}

impl Client {
    pub fn new(cache_dir: impl Into<PathBuf>, min_delay: Duration) -> Result<Self> {
        let cache_dir = cache_dir.into();
        fs::create_dir_all(&cache_dir)?;
        let inner = reqwest::blocking::Client::builder()
            .user_agent("quizzozoizo-pipeline/0.1 (personal offline bird quiz)")
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self { inner, cache_dir, min_delay, last: Cell::new(None) })
    }

    fn cache_path(&self, url: &str) -> PathBuf {
        let hash = Sha256::digest(url.as_bytes());
        let name: String = hash.iter().take(16).map(|b| format!("{b:02x}")).collect();
        self.cache_dir.join(name)
    }

    /// A clone of the underlying client for parallel downloads (no cache, no delay).
    pub fn downloader(&self) -> reqwest::blocking::Client {
        self.inner.clone()
    }

    /// GET `url` as bytes, served from cache when already fetched.
    pub fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let path = self.cache_path(url);
        if let Ok(bytes) = fs::read(&path) {
            return Ok(bytes);
        }
        let bytes = self.fetch(url)?;
        fs::write(&path, &bytes)?;
        Ok(bytes)
    }

    /// GET without touching the cache: for big files we only keep the processed result of.
    pub fn fetch(&self, url: &str) -> Result<Vec<u8>> {
        let mut backoff = Duration::from_secs(10);
        let mut attempts = 0;
        let bytes = loop {
            if let Some(last) = self.last.get() {
                let elapsed = last.elapsed();
                if elapsed < self.min_delay {
                    std::thread::sleep(self.min_delay - elapsed);
                }
            }
            let resp = match self.inner.get(url).send() {
                Ok(r) => r,
                // Dropped connection: wait a bit and try again a few times before giving up.
                Err(_) if attempts < 4 => {
                    attempts += 1;
                    std::thread::sleep(Duration::from_secs(3 * attempts));
                    continue;
                }
                Err(e) => return Err(anyhow::anyhow!("GET {}: {}", redact(url), e.without_url())),
            };
            self.last.set(Some(Instant::now()));
            // Too many requests: wait as asked (or back off exponentially) and try again.
            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS && backoff < Duration::from_secs(300) {
                let wait = resp
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
                    .map(Duration::from_secs)
                    .unwrap_or(backoff);
                eprintln!("  429, waiting {}s", wait.as_secs());
                std::thread::sleep(wait);
                backoff *= 2;
                continue;
            }
            break resp
                .error_for_status()
                .with_context(|| format!("GET {}", redact(url)))?
                .bytes()?
                .to_vec();
        };
        Ok(bytes)
    }

    pub fn get_json(&self, url: &str) -> Result<serde_json::Value> {
        Ok(serde_json::from_slice(&self.get_bytes(url)?)?)
    }
}
