//! Parallel downloads: API queries stay serial (rate limits), the file downloads and ffmpeg
//! conversions run on several threads.

use anyhow::{bail, Result};
use std::{
    collections::VecDeque,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    thread,
    time::Duration,
};

pub enum Kind {
    /// Bird photo: 720px wide, lossy WebP.
    Photo,
    /// Feather photo: 640px wide, lossy WebP (feather textures compress badly, so smaller).
    Feather,
    /// 12 s mono Opus clip, loudness-normalised.
    Audio,
}

pub struct Job {
    pub url: String,
    pub dest: PathBuf,
    pub kind: Kind,
}

pub fn run(jobs: Vec<Job>, threads: usize, client: &reqwest::blocking::Client) {
    let total = jobs.len();
    let queue = Mutex::new(jobs.into_iter().collect::<VecDeque<_>>());
    let done = Mutex::new(0usize);
    thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let Some(job) = queue.lock().unwrap().pop_front() else { break };
                if !job.dest.exists() {
                    if let Err(e) = process(&job, client) {
                        eprintln!("  skip {}: {e:#}", job.url);
                        let _ = fs::remove_file(&job.dest);
                    }
                }
                let mut d = done.lock().unwrap();
                *d += 1;
                if *d % 100 == 0 {
                    println!("  downloads {}/{total}", *d);
                }
            });
        }
    });
}

fn process(job: &Job, client: &reqwest::blocking::Client) -> Result<()> {
    let bytes = download(client, &job.url)?;
    if let Some(dir) = job.dest.parent() {
        fs::create_dir_all(dir)?;
    }
    match job.kind {
        Kind::Photo => to_webp(&bytes, &job.dest, 720, 68),
        Kind::Feather => to_webp(&bytes, &job.dest, 640, 60),
        Kind::Audio => to_opus(&bytes, &job.dest),
    }
}

/// GET with a few retries on rate limits and server errors.
fn download(client: &reqwest::blocking::Client, url: &str) -> Result<Vec<u8>> {
    let mut wait = 2;
    for attempt in 0..4 {
        match client.get(url).send() {
            Ok(r) if r.status().is_success() => return Ok(r.bytes()?.to_vec()),
            Ok(r) if r.status().as_u16() == 429 || r.status().is_server_error() => {}
            Ok(r) => bail!("HTTP {}", r.status()),
            Err(_) if attempt < 3 => {}
            Err(e) => return Err(e.into()),
        }
        thread::sleep(Duration::from_secs(wait));
        wait *= 3;
    }
    bail!("gave up after retries")
}

fn ffmpeg(args: &[&str], dest: &Path, input: &[u8]) -> Result<()> {
    let mut child = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-threads", "1", "-i", "pipe:0"])
        .args(args)
        .arg(dest)
        .stdin(Stdio::piped())
        .spawn()?;
    // ffmpeg may stop reading early; a broken pipe is not an error by itself.
    let _ = child.stdin.take().unwrap().write_all(input);
    if !child.wait()?.success() {
        bail!("ffmpeg failed");
    }
    Ok(())
}

fn to_webp(input: &[u8], dest: &Path, width: u32, quality: u32) -> Result<()> {
    let scale = format!("scale='min({width},iw)':-2");
    ffmpeg(&["-vf", &scale, "-c:v", "libwebp", "-quality", &quality.to_string()], dest, input)
}

fn to_opus(input: &[u8], dest: &Path) -> Result<()> {
    let filter = "silenceremove=start_periods=1:start_threshold=-40dB,atrim=duration=12,loudnorm=I=-18:TP=-2,aresample=24000";
    ffmpeg(&["-vn", "-ac", "1", "-af", filter, "-c:a", "libopus", "-b:a", "28k", "-f", "ogg"], dest, input)?;
    if fs::metadata(dest)?.len() < 2_000 {
        bail!("converted clip is almost empty");
    }
    Ok(())
}
