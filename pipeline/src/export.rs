//! Writes the static files the app serves: `catalog.json` plus only the media it references.

use crate::{regions::REGIONS, species::SpeciesRow};
use anyhow::Result;
use model::{Catalog, Description, Media, Region, Species, SpeciesRegion};
use std::{fs, path::Path};

pub fn run(out: &Path, dest: &Path) -> Result<()> {
    let species: Vec<SpeciesRow> = serde_json::from_slice(&fs::read(out.join("species.json"))?)?;
    let media = crate::photos::load_media(out)?;
    let descriptions: Vec<Description> = fs::read(out.join("descriptions.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let known: std::collections::HashSet<i64> = species.iter().map(|s| s.taxon_id).collect();
    // Hand-reviewed feather photos that show a dead bird, a body part or no feather.
    let blocked: std::collections::HashSet<&str> =
        include_str!("../feather_blocklist.txt").lines().filter(|l| !l.is_empty() && !l.starts_with('#')).collect();

    let catalog = Catalog {
        descriptions: descriptions.into_iter().filter(|d| known.contains(&d.species_id)).collect(),
        species: species
            .iter()
            .map(|s| Species {
                id: s.taxon_id,
                scientific: s.scientific.clone(),
                name_en: s.name_en.clone(),
                name_fr: s.name_fr.clone(),
                family: s.family.clone(),
                order: s.order.clone(),
            })
            .collect(),
        regions: REGIONS
            .iter()
            .map(|r| Region { code: r.code.into(), name_en: r.name_en.into(), name_fr: r.name_fr.into() })
            .collect(),
        species_regions: species
            .iter()
            .flat_map(|s| {
                s.regions.iter().map(|r| SpeciesRegion { species_id: s.taxon_id, region: r.region.clone(), count: r.count })
            })
            .collect(),
        media: media
            .into_iter()
            .filter(|m| known.contains(&m.taxon_id) && !blocked.contains(m.file.as_str()))
            .enumerate()
            .map(|(i, m)| Media { id: i as i64 + 1, species_id: m.taxon_id, kind: m.kind, file: m.file, credit: m.credit, waveform: m.waveform })
            .collect(),
    };

    let target = dest.join("media");
    let _ = fs::remove_dir_all(&target);
    fs::create_dir_all(&target)?;
    let mut bytes = 0;
    for m in &catalog.media {
        let to = target.join(&m.file);
        fs::create_dir_all(to.parent().unwrap())?;
        bytes += fs::copy(out.join("media").join(&m.file), to)?;
    }
    fs::write(dest.join("catalog.json"), serde_json::to_string(&catalog)?)?;
    println!(
        "{} species, {} media ({:.0} MB), {} descriptions -> {}",
        catalog.species.len(),
        catalog.media.len(),
        bytes as f64 / 1e6,
        catalog.descriptions.len(),
        dest.display()
    );
    Ok(())
}
