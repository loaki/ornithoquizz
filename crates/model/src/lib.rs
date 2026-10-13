//! Shared types used by the data pipeline and the app.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lang {
    En,
    Fr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuizMode {
    Photo,
    Feather,
    Sound,
    Description,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaKind {
    Photo,
    Feather,
    Sound,
}

/// How a species occurs in a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Resident,
    Breeding,
    Migrant,
    Winter,
    Rare,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Species {
    pub id: i64,
    pub scientific: String,
    pub name_en: String,
    pub name_fr: String,
    pub family: String,
    pub order: String,
}

impl Species {
    pub fn name(&self, lang: Lang) -> &str {
        match lang {
            Lang::En => &self.name_en,
            Lang::Fr => &self.name_fr,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub code: String,
    pub name_en: String,
    pub name_fr: String,
}

/// Attribution kept for every media file (shown on the credits screen).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credit {
    pub author: String,
    pub license: String,
    pub source: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Media {
    pub id: i64,
    pub species_id: i64,
    pub kind: MediaKind,
    /// Path relative to the media folder.
    pub file: String,
    pub credit: Credit,
    /// Sounds only: bar heights (0-100) for the player's waveform.
    #[serde(default)]
    pub waveform: Vec<u8>,
}

/// A riddle-style text about a bird, with the species name hidden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    pub species_id: i64,
    pub lang: Lang,
    pub text: String,
    /// Wikipedia page the text was adapted from (CC BY-SA 4.0).
    pub source_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeciesRegion {
    pub species_id: i64,
    pub region: String,
    /// Observation count: how common the bird is in the region.
    pub count: i64,
}

/// Everything the app needs, shipped as one `catalog.json` next to the media folder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub species: Vec<Species>,
    pub regions: Vec<Region>,
    pub species_regions: Vec<SpeciesRegion>,
    pub media: Vec<Media>,
    #[serde(default)]
    pub descriptions: Vec<Description>,
}
