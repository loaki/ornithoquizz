//! Species list per region, from iNaturalist `species_counts`, with EN/FR names and taxonomy.

use crate::{http::Client, regions::{FRANCE_PLACE, REGIONS}};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionCount {
    pub region: String,
    /// Research-grade observations in the region: a proxy for how common the bird is there.
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeciesRow {
    pub taxon_id: i64,
    pub scientific: String,
    pub name_en: String,
    pub name_fr: String,
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub order: String,
    #[serde(default)]
    pub regions: Vec<RegionCount>,
}

impl SpeciesRow {
    pub fn in_region(&self, code: &str) -> bool {
        self.regions.iter().any(|r| r.region == code)
    }
}

const AVES: i64 = 3;
const PER_PAGE: usize = 200;

struct Seen {
    taxon_id: i64,
    scientific: String,
    name_en: String,
    count: i64,
}

/// Every bird seen in the region at least `min_count` times (most observed first).
fn region_list(http: &Client, places: &[i64], min_count: i64) -> Result<Vec<Seen>> {
    let ids: Vec<String> = places.iter().map(|p| p.to_string()).collect();
    let mut out = Vec::new();
    for page in 1.. {
        let url = format!(
            "https://api.inaturalist.org/v1/observations/species_counts?place_id={}\
             &taxon_id={AVES}&quality_grade=research&per_page={PER_PAGE}&page={page}&locale=en",
            ids.join(",")
        );
        let json = http.get_json(&url)?;
        let results = json["results"].as_array().cloned().unwrap_or_default();
        for r in &results {
            let t = &r["taxon"];
            let count = r["count"].as_i64().unwrap_or_default();
            if t["rank"] != "species" || count < min_count {
                continue;
            }
            out.push(Seen {
                taxon_id: t["id"].as_i64().unwrap_or_default(),
                scientific: t["name"].as_str().unwrap_or_default().to_string(),
                name_en: t["preferred_common_name"].as_str().unwrap_or_default().to_string(),
                count,
            });
        }
        let last = results.last().map(|r| r["count"].as_i64().unwrap_or_default()).unwrap_or(0);
        if results.len() < PER_PAGE || last < min_count {
            break;
        }
    }
    Ok(out)
}

/// Birds of every region: each region contributes its `top` most observed species (seen at least
/// `min_count` times); a species keeps the count of every region where it is common enough.
pub fn fetch(http: &Client, min_count: i64, top: usize) -> Result<Vec<SpeciesRow>> {
    let mut lists = Vec::new();
    for region in REGIONS {
        let list = region_list(http, region.places, min_count)?;
        println!("{}: {} species", region.code, list.len());
        lists.push((region.code, list));
    }
    let included: HashSet<i64> = lists.iter().flat_map(|(_, l)| l.iter().take(top).map(|s| s.taxon_id)).collect();

    let mut rows: BTreeMap<i64, SpeciesRow> = BTreeMap::new();
    for (code, list) in &lists {
        for s in list.iter().filter(|s| included.contains(&s.taxon_id)) {
            let row = rows.entry(s.taxon_id).or_insert_with(|| SpeciesRow {
                taxon_id: s.taxon_id,
                scientific: s.scientific.clone(),
                name_en: s.name_en.clone(),
                name_fr: String::new(),
                family: String::new(),
                order: String::new(),
                regions: vec![],
            });
            row.regions.push(RegionCount { region: code.to_string(), count: s.count });
        }
    }
    let mut rows: Vec<SpeciesRow> = rows.into_values().collect();
    enrich(http, &mut rows)?;
    rows.sort_by_key(|r| std::cmp::Reverse(r.regions.iter().map(|c| c.count).sum::<i64>()));
    Ok(rows)
}

/// French names (as used in France), family and order from the taxa endpoint, 30 ids per request.
fn enrich(http: &Client, rows: &mut [SpeciesRow]) -> Result<()> {
    for chunk in rows.chunks_mut(30) {
        let ids: Vec<String> = chunk.iter().map(|r| r.taxon_id.to_string()).collect();
        let url = format!(
            "https://api.inaturalist.org/v1/taxa/{}?locale=fr&preferred_place_id={FRANCE_PLACE}",
            ids.join(",")
        );
        let json = http.get_json(&url)?;
        for t in json["results"].as_array().into_iter().flatten() {
            let id = t["id"].as_i64().unwrap_or_default();
            let Some(row) = chunk.iter_mut().find(|r| r.taxon_id == id) else { continue };
            row.name_fr = t["preferred_common_name"].as_str().unwrap_or_default().to_string();
            for a in t["ancestors"].as_array().into_iter().flatten() {
                let name = a["name"].as_str().unwrap_or_default().to_string();
                match a["rank"].as_str() {
                    Some("family") => row.family = name,
                    Some("order") => row.order = name,
                    _ => {}
                }
            }
        }
    }
    for row in rows.iter_mut() {
        if row.name_en.is_empty() {
            row.name_en = row.scientific.clone();
        }
        if row.name_fr.is_empty() {
            row.name_fr = row.name_en.clone();
        }
    }
    Ok(())
}
