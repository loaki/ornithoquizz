//! The regions players can choose, as sets of iNaturalist places (countries).

pub struct RegionDef {
    pub code: &'static str,
    pub name_en: &'static str,
    pub name_fr: &'static str,
    pub places: &'static [i64],
}

pub const REGIONS: &[RegionDef] = &[
    RegionDef { code: "FR", name_en: "France", name_fr: "France", places: &[6753] },
    RegionDef { code: "IB", name_en: "Iberia", name_fr: "Péninsule ibérique", places: &[6774, 7122] },
    RegionDef { code: "GB", name_en: "British Isles", name_fr: "Îles Britanniques", places: &[6857, 6718] },
    RegionDef { code: "BNL", name_en: "Benelux", name_fr: "Benelux", places: &[7506, 7008, 8147] },
    RegionDef { code: "CEN", name_en: "Central Europe", name_fr: "Europe centrale", places: &[7207, 8057, 7236, 7800, 8264, 8267, 7399] },
    RegionDef { code: "IT", name_en: "Italy", name_fr: "Italie", places: &[6973] },
    RegionDef { code: "SCA", name_en: "Scandinavia", name_fr: "Scandinavie", places: &[7016, 7599, 8051, 7020, 7278] },
    RegionDef { code: "BAL", name_en: "Balkans & SE Europe", name_fr: "Balkans et Europe du Sud-Est", places: &[8196, 7094, 8858, 8241, 8228, 7147] },
    RegionDef { code: "EEU", name_en: "Eastern Europe & Baltics", name_fr: "Europe de l'Est et pays baltes", places: &[8265, 8050, 8266, 8860] },
];

/// iNaturalist's "Europe" place: used for photos, feathers and fallbacks.
pub const EUROPE_PLACE: i64 = 97391;
/// France: French common names follow this place's preferences.
pub const FRANCE_PLACE: i64 = 6753;
