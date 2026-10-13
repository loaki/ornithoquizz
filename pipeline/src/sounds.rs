//! Bird sounds from xeno-canto: quality A/B, no-derivatives licences excluded, 12 s Opus clips.

use crate::{
    http::Client,
    jobs::{self, Job, Kind},
    photos::{save, MediaRow},
    species::SpeciesRow,
};
use anyhow::{bail, Result};
use model::{Credit, MediaKind};
use std::{path::Path, process::Command};

const BARS: usize = 44;

struct Candidate {
    id: String,
    recordist: String,
    license: String,
    page: String,
    file: String,
}

pub fn fetch(http: &Client, out: &Path, api_key: &str, species: &[SpeciesRow], per_species: usize) -> Result<Vec<MediaRow>> {
    let (mut rows, mut jobs) = (Vec::new(), Vec::new());
    for (i, sp) in species.iter().enumerate() {
        for (n, c) in candidates(http, api_key, sp)?.into_iter().take(per_species).enumerate() {
            let rel = format!("sounds/{}/{}.ogg", sp.taxon_id, n + 1);
            jobs.push(Job { url: c.file, dest: out.join("media").join(&rel), kind: Kind::Audio });
            rows.push(MediaRow {
                taxon_id: sp.taxon_id,
                kind: MediaKind::Sound,
                file: rel,
                credit: Credit {
                    author: c.recordist,
                    license: c.license,
                    source: format!("xeno-canto XC{}", c.id),
                    url: c.page,
                },
                waveform: vec![],
            });
        }
        if (i + 1) % 50 == 0 {
            println!("[{}/{}] planned {} sounds", i + 1, species.len(), rows.len());
        }
    }
    save(out, "sounds.json", &rows)?;
    println!("{} sounds planned, downloading…", rows.len());
    // xeno-canto is fragile: few parallel downloads.
    jobs::run(jobs, 3, &http.downloader());
    for row in rows.iter_mut() {
        row.waveform = waveform(&out.join("media").join(&row.file)).unwrap_or_default();
    }
    save(out, "sounds.json", &rows)?;
    Ok(rows)
}

/// Best recordings first: songs before calls, one per recordist, quality A then B.
fn candidates(http: &Client, key: &str, sp: &SpeciesRow) -> Result<Vec<Candidate>> {
    let mut parts = sp.scientific.split_whitespace();
    let (Some(g), Some(s)) = (parts.next(), parts.next()) else { return Ok(vec![]) };
    let mut found: Vec<(u8, Candidate)> = Vec::new();
    for (quality, rank) in [("A", 0u8), ("B", 1)] {
        let url = format!(
            "https://xeno-canto.org/api/3/recordings?query=gen:{g}+sp:{s}+grp:birds+q:{quality}+area:europe+len:8-40&per_page=100&key={key}"
        );
        let json = http.get_json(&url)?;
        for r in json["recordings"].as_array().into_iter().flatten() {
            let lic = r["lic"].as_str().unwrap_or_default();
            if lic.contains("-nd") || lic.is_empty() {
                continue;
            }
            let song = r["type"].as_str().unwrap_or_default().to_lowercase().starts_with("song");
            found.push((
                rank * 4 + if song { 0 } else { 2 },
                Candidate {
                    id: r["id"].as_str().unwrap_or_default().into(),
                    recordist: r["rec"].as_str().unwrap_or("unknown").trim().into(),
                    license: license_name(lic),
                    page: r["url"].as_str().unwrap_or_default().into(),
                    file: r["file"].as_str().unwrap_or_default().into(),
                },
            ));
        }
        if found.len() >= 6 {
            break;
        }
    }
    found.sort_by_key(|(rank, _)| *rank);
    let mut recordists = Vec::new();
    let mut out = Vec::new();
    for (_, c) in found {
        if c.file.is_empty() || recordists.contains(&c.recordist) {
            continue;
        }
        recordists.push(c.recordist.clone());
        out.push(c);
    }
    Ok(out)
}

/// "https://creativecommons.org/licenses/by-nc-sa/4.0/" -> "CC BY-NC-SA 4.0".
fn license_name(url: &str) -> String {
    if url.contains("publicdomain/zero") {
        return "CC0".into();
    }
    let parts: Vec<&str> = url.trim_end_matches('/').rsplit('/').take(2).collect();
    match parts.as_slice() {
        [version, kind] => format!("CC {} {version}", kind.to_uppercase()),
        _ => url.into(),
    }
}

/// Loudness per bar (0-100), for the waveform drawn by the player.
fn waveform(clip: &Path) -> Result<Vec<u8>> {
    let pcm = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(clip)
        .args(["-f", "s16le", "-ac", "1", "-ar", "8000", "pipe:1"])
        .output()?
        .stdout;
    let samples: Vec<f32> = pcm.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32).collect();
    if samples.len() < BARS {
        bail!("clip too short");
    }
    let size = samples.len() / BARS;
    let rms: Vec<f32> = (0..BARS)
        .map(|i| {
            let chunk = &samples[i * size..(i + 1) * size];
            (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt()
        })
        .collect();
    let max = rms.iter().cloned().fold(1.0, f32::max);
    Ok(rms.iter().map(|v| ((v / max) * 100.0).round().max(6.0) as u8).collect())
}
