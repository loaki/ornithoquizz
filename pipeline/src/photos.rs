//! Photos and feather photos from iNaturalist (research grade, CC0 / CC BY / CC BY-NC).

use crate::{
    http::Client,
    jobs::{self, Job, Kind},
    regions::{EUROPE_PLACE, FRANCE_PLACE},
    species::SpeciesRow,
};
use anyhow::Result;
use model::{Credit, MediaKind};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaRow {
    pub taxon_id: i64,
    pub kind: MediaKind,
    /// Relative to the media root.
    pub file: String,
    pub credit: Credit,
    #[serde(default)]
    pub waveform: Vec<u8>,
}

/// Candidates requested per species. Constant so cached answers keep being reused.
const CANDIDATES: usize = 15;
/// iNaturalist annotation "Evidence of presence" = "Feather".
const FEATHER_FILTER: &str = "&term_id=22&term_value_id=23";

pub fn save(out: &Path, name: &str, rows: &[MediaRow]) -> Result<()> {
    fs::write(out.join(name), serde_json::to_string_pretty(rows)?)?;
    Ok(())
}

/// iNaturalist photos of each bird. French birds use France's best-voted photos, others Europe's.
pub fn photos(http: &Client, out: &Path, species: &[SpeciesRow], per_species: usize) -> Result<Vec<MediaRow>> {
    collect(http, out, species, per_species, MediaKind::Photo, "photos.json", |sp| {
        let place = if sp.in_region("FR") { FRANCE_PLACE } else { EUROPE_PLACE };
        (place, String::new())
    })
}

/// Photos of feathers found in the field, from iNaturalist observations tagged "feather".
pub fn feathers(http: &Client, out: &Path, species: &[SpeciesRow], per_species: usize) -> Result<Vec<MediaRow>> {
    collect(http, out, species, per_species, MediaKind::Feather, "feathers.json", |_| {
        (EUROPE_PLACE, FEATHER_FILTER.to_string())
    })
}

fn collect(
    http: &Client,
    out: &Path,
    species: &[SpeciesRow],
    per_species: usize,
    kind: MediaKind,
    manifest: &str,
    query: impl Fn(&SpeciesRow) -> (i64, String),
) -> Result<Vec<MediaRow>> {
    let folder = if kind == MediaKind::Feather { "feathers" } else { "photos" };
    let (mut rows, mut jobs) = (Vec::new(), Vec::new());
    for (i, sp) in species.iter().enumerate() {
        let (place, extra) = query(sp);
        let url = format!(
            "https://api.inaturalist.org/v1/observations?taxon_id={}&place_id={place}\
             &quality_grade=research&photo_license=cc0,cc-by,cc-by-nc{extra}&order_by=votes\
             &per_page={CANDIDATES}&locale=en",
            sp.taxon_id
        );
        let json = http.get_json(&url)?;
        let mut n = 0;
        for obs in json["results"].as_array().into_iter().flatten() {
            if n == per_species {
                break;
            }
            let Some(photo) = obs["photos"].get(0) else { continue };
            let (Some(src), Some(license)) = (photo["url"].as_str(), photo["license_code"].as_str()) else { continue };
            n += 1;
            let rel = format!("{folder}/{}/{n}.webp", sp.taxon_id);
            jobs.push(Job { url: src.replace("/square.", "/large."), dest: out.join("media").join(&rel), kind: if kind == MediaKind::Feather { Kind::Feather } else { Kind::Photo } });
            rows.push(MediaRow {
                taxon_id: sp.taxon_id,
                kind,
                file: rel,
                credit: Credit {
                    author: photo["attribution"].as_str().unwrap_or("unknown").to_string(),
                    license: license.to_uppercase(),
                    source: "iNaturalist".into(),
                    url: obs["uri"].as_str().unwrap_or_default().to_string(),
                },
                waveform: vec![],
            });
        }
        if (i + 1) % 50 == 0 {
            println!("[{}/{}] planned {} {folder}", i + 1, species.len(), rows.len());
        }
    }
    // Checkpoint: exporters only keep rows whose file exists, so this is safe to use mid-run.
    save(out, manifest, &rows)?;
    println!("{} {folder} planned, downloading…", rows.len());
    jobs::run(jobs, 6, &http.downloader());
    Ok(rows)
}

/// All media manifests, keeping only rows whose file really exists.
pub fn load_media(out: &Path) -> Result<Vec<MediaRow>> {
    let mut all = Vec::new();
    for name in ["photos.json", "feathers.json", "sounds.json"] {
        if let Ok(bytes) = fs::read(out.join(name)) {
            all.extend(serde_json::from_slice::<Vec<MediaRow>>(&bytes)?);
        }
    }
    all.retain(|m| out.join("media").join(&m.file).exists());
    Ok(all)
}
