//! Benchmark for old GRC-20 proto serialization using country data.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::Instant;

use prost::Message;
use serde::Deserialize;

// Include generated protobuf code
pub mod grc20 {
    include!(concat!(env!("OUT_DIR"), "/grc20.rs"));
}

// =============================================================================
// HARDCODED UUIDs FOR SCHEMA (same as grc-20-bench for comparison)
// =============================================================================

const fn hex(s: &str) -> [u8; 16] {
    let bytes = s.as_bytes();
    let mut result = [0u8; 16];
    let mut i = 0;
    while i < 16 {
        let hi = hex_digit(bytes[i * 2]);
        let lo = hex_digit(bytes[i * 2 + 1]);
        result[i] = (hi << 4) | lo;
        i += 1;
    }
    result
}

const fn hex_digit(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("invalid hex digit"),
    }
}

/// Create a deterministic entity ID from a prefix and numeric ID.
fn make_entity_id(prefix: u8, id: u32) -> Vec<u8> {
    let mut result = [0u8; 16];
    result[0] = prefix;
    result[12..16].copy_from_slice(&id.to_be_bytes());
    result.to_vec()
}

/// Create a deterministic relation entity ID.
fn make_rel_entity_id(prefix: u8, entity_id: u32, rel_type: u8, index: u16) -> Vec<u8> {
    let mut result = [0u8; 16];
    result[0] = prefix;
    result[1] = rel_type;
    result[2..4].copy_from_slice(&index.to_be_bytes());
    result[12..16].copy_from_slice(&entity_id.to_be_bytes());
    result.to_vec()
}

// Property IDs (same as grc-20-bench)
mod props {
    use super::hex;

    pub const NAME: [u8; 16] = hex("A0000000000000000000000000000001");
    pub const ISO2: [u8; 16] = hex("A0000000000000000000000000000002");
    pub const ISO3: [u8; 16] = hex("A0000000000000000000000000000003");
    pub const NUMERIC_CODE: [u8; 16] = hex("A0000000000000000000000000000004");
    pub const PHONE_CODE: [u8; 16] = hex("A0000000000000000000000000000005");
    pub const CAPITAL: [u8; 16] = hex("A0000000000000000000000000000006");
    pub const CURRENCY: [u8; 16] = hex("A0000000000000000000000000000007");
    pub const CURRENCY_NAME: [u8; 16] = hex("A0000000000000000000000000000008");
    pub const CURRENCY_SYMBOL: [u8; 16] = hex("A0000000000000000000000000000009");
    pub const TLD: [u8; 16] = hex("A000000000000000000000000000000A");
    pub const NATIVE_NAME: [u8; 16] = hex("A000000000000000000000000000000B");
    pub const LATITUDE: [u8; 16] = hex("A000000000000000000000000000000C");
    pub const LONGITUDE: [u8; 16] = hex("A000000000000000000000000000000D");
    pub const EMOJI: [u8; 16] = hex("A000000000000000000000000000000E");
    pub const EMOJI_UNICODE: [u8; 16] = hex("A000000000000000000000000000000F");
    pub const TRANSLATION: [u8; 16] = hex("A0000000000000000000000000000010");
    pub const ZONE_NAME: [u8; 16] = hex("A0000000000000000000000000000011");
    pub const GMT_OFFSET: [u8; 16] = hex("A0000000000000000000000000000012");
    pub const GMT_OFFSET_NAME: [u8; 16] = hex("A0000000000000000000000000000013");
    pub const ABBREVIATION: [u8; 16] = hex("A0000000000000000000000000000014");
    pub const TZ_NAME: [u8; 16] = hex("A0000000000000000000000000000015");
    pub const POPULATION: [u8; 16] = hex("A0000000000000000000000000000016");
    pub const GDP: [u8; 16] = hex("A0000000000000000000000000000017");
    pub const NATIONALITY: [u8; 16] = hex("A0000000000000000000000000000018");
    pub const AREA_SQ_KM: [u8; 16] = hex("A0000000000000000000000000000019");
    pub const POSTAL_CODE_FORMAT: [u8; 16] = hex("A000000000000000000000000000001A");
    pub const POSTAL_CODE_REGEX: [u8; 16] = hex("A000000000000000000000000000001B");
    pub const WIKIDATA_ID: [u8; 16] = hex("A000000000000000000000000000001C");
    pub const LOCATION: [u8; 16] = hex("A000000000000000000000000000001D");
}

// Relation type IDs
mod rel_types {
    use super::hex;

    pub const TYPES: [u8; 16] = hex("B0000000000000000000000000000001");
    pub const HAS_TIMEZONE: [u8; 16] = hex("B0000000000000000000000000000002");
    pub const IN_REGION: [u8; 16] = hex("B0000000000000000000000000000003");
    pub const IN_SUBREGION: [u8; 16] = hex("B0000000000000000000000000000004");
}

// Type IDs
mod types {
    use super::hex;

    pub const COUNTRY: [u8; 16] = hex("C0000000000000000000000000000001");
    pub const REGION: [u8; 16] = hex("C0000000000000000000000000000002");
    pub const SUBREGION: [u8; 16] = hex("C0000000000000000000000000000003");
    pub const TIMEZONE: [u8; 16] = hex("C0000000000000000000000000000004");
}

// Language IDs
mod languages {
    use super::hex;

    pub const ENGLISH: [u8; 16] = hex("D0000000000000000000000000000001");
    pub const KOREAN: [u8; 16] = hex("D0000000000000000000000000000002");
    pub const JAPANESE: [u8; 16] = hex("D0000000000000000000000000000003");
    pub const CHINESE_SIMPLIFIED: [u8; 16] = hex("D0000000000000000000000000000004");
    pub const CHINESE_TRADITIONAL: [u8; 16] = hex("D0000000000000000000000000000005");
    pub const FRENCH: [u8; 16] = hex("D0000000000000000000000000000006");
    pub const GERMAN: [u8; 16] = hex("D0000000000000000000000000000007");
    pub const SPANISH: [u8; 16] = hex("D0000000000000000000000000000008");
    pub const ITALIAN: [u8; 16] = hex("D0000000000000000000000000000009");
    pub const PORTUGUESE: [u8; 16] = hex("D000000000000000000000000000000A");
    pub const DUTCH: [u8; 16] = hex("D000000000000000000000000000000B");
    pub const PERSIAN: [u8; 16] = hex("D000000000000000000000000000000C");
    pub const HINDI: [u8; 16] = hex("D000000000000000000000000000000D");
    pub const BRETON: [u8; 16] = hex("D000000000000000000000000000000E");
    pub const CROATIAN: [u8; 16] = hex("D000000000000000000000000000000F");
    pub const TURKISH: [u8; 16] = hex("D0000000000000000000000000000010");
    pub const RUSSIAN: [u8; 16] = hex("D0000000000000000000000000000011");
    pub const UKRAINIAN: [u8; 16] = hex("D0000000000000000000000000000012");
    pub const POLISH: [u8; 16] = hex("D0000000000000000000000000000013");
    pub const ARABIC: [u8; 16] = hex("D0000000000000000000000000000014");
    pub const PORTUGUESE_BR: [u8; 16] = hex("D0000000000000000000000000000015");

    pub fn from_code(code: &str) -> Option<[u8; 16]> {
        match code {
            "br" => Some(BRETON),
            "ko" => Some(KOREAN),
            "pt-BR" => Some(PORTUGUESE_BR),
            "pt" => Some(PORTUGUESE),
            "nl" => Some(DUTCH),
            "hr" => Some(CROATIAN),
            "fa" => Some(PERSIAN),
            "de" => Some(GERMAN),
            "es" => Some(SPANISH),
            "fr" => Some(FRENCH),
            "ja" => Some(JAPANESE),
            "it" => Some(ITALIAN),
            "zh-CN" => Some(CHINESE_SIMPLIFIED),
            "tr" => Some(TURKISH),
            "ru" => Some(RUSSIAN),
            "uk" => Some(UKRAINIAN),
            "pl" => Some(POLISH),
            "ar" => Some(ARABIC),
            "hi" => Some(HINDI),
            _ => None,
        }
    }
}

// Entity ID prefixes
const PREFIX_COUNTRY: u8 = 0x01;
const PREFIX_REGION: u8 = 0x02;
const PREFIX_SUBREGION: u8 = 0x03;
const PREFIX_TIMEZONE: u8 = 0x04;

// =============================================================================
// JSON DATA STRUCTURES (same as grc-20-bench)
// =============================================================================

#[derive(Debug, Deserialize)]
struct Country {
    id: u32,
    name: String,
    iso3: String,
    iso2: String,
    numeric_code: Option<String>,
    #[serde(alias = "phone_code")]
    phonecode: Option<String>,
    capital: Option<String>,
    currency: Option<String>,
    currency_name: Option<String>,
    currency_symbol: Option<String>,
    tld: Option<String>,
    native: Option<String>,
    population: Option<i64>,
    gdp: Option<i64>,
    region: Option<String>,
    region_id: Option<u32>,
    subregion: Option<String>,
    subregion_id: Option<u32>,
    nationality: Option<String>,
    area_sq_km: Option<i64>,
    postal_code_format: Option<String>,
    postal_code_regex: Option<String>,
    timezones: Option<Vec<Timezone>>,
    translations: Option<HashMap<String, String>>,
    latitude: Option<String>,
    longitude: Option<String>,
    emoji: Option<String>,
    #[serde(rename = "emojiU")]
    emoji_u: Option<String>,
    #[serde(rename = "wikiDataId")]
    wikidata_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Timezone {
    #[serde(rename = "zoneName")]
    zone_name: String,
    #[serde(rename = "gmtOffset")]
    gmt_offset: i32,
    #[serde(rename = "gmtOffsetName")]
    gmt_offset_name: String,
    abbreviation: String,
    #[serde(rename = "tzName")]
    tz_name: String,
}

// =============================================================================
// CONVERSION CONTEXT
// =============================================================================

struct ConversionContext {
    ops: Vec<grc20::Op>,
    created_regions: HashSet<u32>,
    created_subregions: HashSet<u32>,
    created_timezones: HashSet<String>,
}

impl ConversionContext {
    fn new() -> Self {
        Self {
            ops: Vec::new(),
            created_regions: HashSet::new(),
            created_subregions: HashSet::new(),
            created_timezones: HashSet::new(),
        }
    }

    fn make_value(&self, property: &[u8], value: String) -> grc20::Value {
        grc20::Value {
            property: property.to_vec(),
            value,
            options: None,
        }
    }

    fn make_text_value(&self, property: &[u8], value: String, language: Option<[u8; 16]>) -> grc20::Value {
        grc20::Value {
            property: property.to_vec(),
            value,
            options: language.map(|lang| grc20::Options {
                value: Some(grc20::options::Value::Text(grc20::TextOptions {
                    language: Some(lang.to_vec()),
                })),
            }),
        }
    }

    fn ensure_region(&mut self, region_id: u32, name: &str) {
        if self.created_regions.contains(&region_id) {
            return;
        }
        self.created_regions.insert(region_id);

        let entity_id = make_entity_id(PREFIX_REGION, region_id);

        // Create region entity
        self.ops.push(grc20::Op {
            payload: Some(grc20::op::Payload::UpdateEntity(grc20::Entity {
                id: entity_id.clone(),
                values: vec![self.make_value(&props::NAME, name.to_string())],
            })),
        });

        // Types relation
        let rel_entity_id = make_rel_entity_id(PREFIX_REGION, region_id, 0, 0);
        self.ops.push(grc20::Op {
            payload: Some(grc20::op::Payload::CreateRelation(grc20::Relation {
                id: rel_entity_id.clone(),
                r#type: rel_types::TYPES.to_vec(),
                from_entity: entity_id,
                from_space: None,
                from_version: None,
                to_entity: types::REGION.to_vec(),
                to_space: None,
                to_version: None,
                entity: rel_entity_id,
                position: None,
                verified: None,
            })),
        });
    }

    fn ensure_subregion(&mut self, subregion_id: u32, name: &str, region_id: Option<u32>) {
        if self.created_subregions.contains(&subregion_id) {
            return;
        }
        self.created_subregions.insert(subregion_id);

        let entity_id = make_entity_id(PREFIX_SUBREGION, subregion_id);

        // Create subregion entity
        self.ops.push(grc20::Op {
            payload: Some(grc20::op::Payload::UpdateEntity(grc20::Entity {
                id: entity_id.clone(),
                values: vec![self.make_value(&props::NAME, name.to_string())],
            })),
        });

        // Types relation
        let rel_entity_id = make_rel_entity_id(PREFIX_SUBREGION, subregion_id, 0, 0);
        self.ops.push(grc20::Op {
            payload: Some(grc20::op::Payload::CreateRelation(grc20::Relation {
                id: rel_entity_id.clone(),
                r#type: rel_types::TYPES.to_vec(),
                from_entity: entity_id.clone(),
                from_space: None,
                from_version: None,
                to_entity: types::SUBREGION.to_vec(),
                to_space: None,
                to_version: None,
                entity: rel_entity_id,
                position: None,
                verified: None,
            })),
        });

        // IN_REGION relation if we have a region
        if let Some(region_id) = region_id {
            let region_entity_id = make_entity_id(PREFIX_REGION, region_id);
            let rel_entity_id = make_rel_entity_id(PREFIX_SUBREGION, subregion_id, 1, 0);
            self.ops.push(grc20::Op {
                payload: Some(grc20::op::Payload::CreateRelation(grc20::Relation {
                    id: rel_entity_id.clone(),
                    r#type: rel_types::IN_REGION.to_vec(),
                    from_entity: entity_id,
                    from_space: None,
                    from_version: None,
                    to_entity: region_entity_id,
                    to_space: None,
                    to_version: None,
                    entity: rel_entity_id,
                    position: None,
                    verified: None,
                })),
            });
        }
    }

    fn ensure_timezone(&mut self, tz: &Timezone) -> Vec<u8> {
        let tz_key = format!("{}|{}", tz.zone_name, tz.gmt_offset);

        if self.created_timezones.contains(&tz_key) {
            // Return existing ID
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(&tz_key, &mut hasher);
            let hash = std::hash::Hasher::finish(&hasher);
            let mut id = [0u8; 16];
            id[0] = PREFIX_TIMEZONE;
            id[8..16].copy_from_slice(&hash.to_be_bytes());
            return id.to_vec();
        }
        self.created_timezones.insert(tz_key.clone());

        // Create deterministic ID from timezone key
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&tz_key, &mut hasher);
        let hash = std::hash::Hasher::finish(&hasher);
        let mut entity_id = [0u8; 16];
        entity_id[0] = PREFIX_TIMEZONE;
        entity_id[8..16].copy_from_slice(&hash.to_be_bytes());
        let entity_id = entity_id.to_vec();

        // Create timezone entity with all properties
        self.ops.push(grc20::Op {
            payload: Some(grc20::op::Payload::UpdateEntity(grc20::Entity {
                id: entity_id.clone(),
                values: vec![
                    self.make_value(&props::ZONE_NAME, tz.zone_name.clone()),
                    self.make_value(&props::GMT_OFFSET, tz.gmt_offset.to_string()),
                    self.make_value(&props::GMT_OFFSET_NAME, tz.gmt_offset_name.clone()),
                    self.make_value(&props::ABBREVIATION, tz.abbreviation.clone()),
                    self.make_value(&props::TZ_NAME, tz.tz_name.clone()),
                ],
            })),
        });

        // Types relation
        let rel_entity_id = {
            let mut id = entity_id.clone();
            id[1] = 0xFF; // Mark as relation entity
            id
        };
        self.ops.push(grc20::Op {
            payload: Some(grc20::op::Payload::CreateRelation(grc20::Relation {
                id: rel_entity_id.clone(),
                r#type: rel_types::TYPES.to_vec(),
                from_entity: entity_id.clone(),
                from_space: None,
                from_version: None,
                to_entity: types::TIMEZONE.to_vec(),
                to_space: None,
                to_version: None,
                entity: rel_entity_id,
                position: None,
                verified: None,
            })),
        });

        entity_id
    }

    fn add_country(&mut self, country: &Country) {
        let entity_id = make_entity_id(PREFIX_COUNTRY, country.id);

        // Build values list - start with required fields
        let mut values = vec![
            self.make_text_value(&props::NAME, country.name.clone(), Some(languages::ENGLISH)),
            self.make_value(&props::ISO2, country.iso2.clone()),
            self.make_value(&props::ISO3, country.iso3.clone()),
        ];

        // Add optional string fields
        if let Some(ref v) = country.numeric_code {
            if !v.is_empty() {
                values.push(self.make_value(&props::NUMERIC_CODE, v.clone()));
            }
        }
        if let Some(ref v) = country.phonecode {
            if !v.is_empty() {
                values.push(self.make_value(&props::PHONE_CODE, v.clone()));
            }
        }
        if let Some(ref v) = country.capital {
            if !v.is_empty() {
                values.push(self.make_value(&props::CAPITAL, v.clone()));
            }
        }
        if let Some(ref v) = country.currency {
            if !v.is_empty() {
                values.push(self.make_value(&props::CURRENCY, v.clone()));
            }
        }
        if let Some(ref v) = country.currency_name {
            if !v.is_empty() {
                values.push(self.make_value(&props::CURRENCY_NAME, v.clone()));
            }
        }
        if let Some(ref v) = country.currency_symbol {
            if !v.is_empty() {
                values.push(self.make_value(&props::CURRENCY_SYMBOL, v.clone()));
            }
        }
        if let Some(ref v) = country.tld {
            if !v.is_empty() {
                values.push(self.make_value(&props::TLD, v.clone()));
            }
        }
        if let Some(ref v) = country.native {
            if !v.is_empty() {
                values.push(self.make_value(&props::NATIVE_NAME, v.clone()));
            }
        }
        if let Some(ref v) = country.nationality {
            if !v.is_empty() {
                values.push(self.make_value(&props::NATIONALITY, v.clone()));
            }
        }
        if let Some(ref v) = country.postal_code_format {
            if !v.is_empty() {
                values.push(self.make_value(&props::POSTAL_CODE_FORMAT, v.clone()));
            }
        }
        if let Some(ref v) = country.postal_code_regex {
            if !v.is_empty() {
                values.push(self.make_value(&props::POSTAL_CODE_REGEX, v.clone()));
            }
        }
        if let Some(ref v) = country.wikidata_id {
            if !v.is_empty() {
                values.push(self.make_value(&props::WIKIDATA_ID, v.clone()));
            }
        }
        if let Some(ref v) = country.emoji {
            if !v.is_empty() {
                values.push(self.make_value(&props::EMOJI, v.clone()));
            }
        }
        if let Some(ref v) = country.emoji_u {
            if !v.is_empty() {
                values.push(self.make_value(&props::EMOJI_UNICODE, v.clone()));
            }
        }

        // Add numeric fields (as strings for proto)
        if let Some(v) = country.population {
            values.push(self.make_value(&props::POPULATION, v.to_string()));
        }
        if let Some(v) = country.gdp {
            values.push(self.make_value(&props::GDP, v.to_string()));
        }
        if let Some(v) = country.area_sq_km {
            values.push(self.make_value(&props::AREA_SQ_KM, v.to_string()));
        }

        // Add location as a string "lat,lon" (proto doesn't have native Point type)
        if let (Some(lat), Some(lon)) = (&country.latitude, &country.longitude) {
            values.push(self.make_value(&props::LOCATION, format!("{},{}", lat, lon)));
        }

        // Add translations
        if let Some(ref translations) = country.translations {
            for (lang_code, translation) in translations {
                if let Some(lang_id) = languages::from_code(lang_code) {
                    values.push(self.make_text_value(&props::TRANSLATION, translation.clone(), Some(lang_id)));
                }
            }
        }

        // Create country entity
        self.ops.push(grc20::Op {
            payload: Some(grc20::op::Payload::UpdateEntity(grc20::Entity {
                id: entity_id.clone(),
                values,
            })),
        });

        // Types relation
        let rel_entity_id = make_rel_entity_id(PREFIX_COUNTRY, country.id, 0, 0);
        self.ops.push(grc20::Op {
            payload: Some(grc20::op::Payload::CreateRelation(grc20::Relation {
                id: rel_entity_id.clone(),
                r#type: rel_types::TYPES.to_vec(),
                from_entity: entity_id.clone(),
                from_space: None,
                from_version: None,
                to_entity: types::COUNTRY.to_vec(),
                to_space: None,
                to_version: None,
                entity: rel_entity_id,
                position: None,
                verified: None,
            })),
        });

        // Region/subregion relations
        if let (Some(region_id), Some(region_name)) = (country.region_id, &country.region) {
            self.ensure_region(region_id, region_name);

            let region_entity_id = make_entity_id(PREFIX_REGION, region_id);
            let rel_entity_id = make_rel_entity_id(PREFIX_COUNTRY, country.id, 1, 0);
            self.ops.push(grc20::Op {
                payload: Some(grc20::op::Payload::CreateRelation(grc20::Relation {
                    id: rel_entity_id.clone(),
                    r#type: rel_types::IN_REGION.to_vec(),
                    from_entity: entity_id.clone(),
                    from_space: None,
                    from_version: None,
                    to_entity: region_entity_id,
                    to_space: None,
                    to_version: None,
                    entity: rel_entity_id,
                    position: None,
                    verified: None,
                })),
            });
        }

        if let (Some(subregion_id), Some(subregion_name)) = (country.subregion_id, &country.subregion) {
            self.ensure_subregion(subregion_id, subregion_name, country.region_id);

            let subregion_entity_id = make_entity_id(PREFIX_SUBREGION, subregion_id);
            let rel_entity_id = make_rel_entity_id(PREFIX_COUNTRY, country.id, 2, 0);
            self.ops.push(grc20::Op {
                payload: Some(grc20::op::Payload::CreateRelation(grc20::Relation {
                    id: rel_entity_id.clone(),
                    r#type: rel_types::IN_SUBREGION.to_vec(),
                    from_entity: entity_id.clone(),
                    from_space: None,
                    from_version: None,
                    to_entity: subregion_entity_id,
                    to_space: None,
                    to_version: None,
                    entity: rel_entity_id,
                    position: None,
                    verified: None,
                })),
            });
        }

        // Timezone relations
        if let Some(ref timezones) = country.timezones {
            for (idx, tz) in timezones.iter().enumerate() {
                let tz_entity_id = self.ensure_timezone(tz);
                let rel_entity_id = make_rel_entity_id(PREFIX_COUNTRY, country.id, 3, idx as u16);
                self.ops.push(grc20::Op {
                    payload: Some(grc20::op::Payload::CreateRelation(grc20::Relation {
                        id: rel_entity_id.clone(),
                        r#type: rel_types::HAS_TIMEZONE.to_vec(),
                        from_entity: entity_id.clone(),
                        from_space: None,
                        from_version: None,
                        to_entity: tz_entity_id,
                        to_space: None,
                        to_version: None,
                        entity: rel_entity_id,
                        position: None,
                        verified: None,
                    })),
                });
            }
        }
    }
}

fn main() {
    // Find the data file
    let data_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../../data/countries.json".to_string());

    println!("Loading countries from: {}", data_path);

    let json_data = fs::read_to_string(&data_path).expect("Failed to read countries.json");

    let parse_start = Instant::now();
    let countries: Vec<Country> = serde_json::from_str(&json_data).expect("Failed to parse JSON");
    let parse_time = parse_start.elapsed();

    println!("Loaded {} countries in {:?}", countries.len(), parse_time);

    // Convert to proto operations
    let convert_start = Instant::now();
    let mut ctx = ConversionContext::new();
    for country in &countries {
        ctx.add_country(country);
    }
    let convert_time = convert_start.elapsed();

    println!(
        "Converted to {} operations in {:?}",
        ctx.ops.len(),
        convert_time
    );
    println!(
        "  - {} regions, {} subregions, {} timezones",
        ctx.created_regions.len(),
        ctx.created_subregions.len(),
        ctx.created_timezones.len()
    );

    // Count operation types
    let mut entity_count = 0;
    let mut relation_count = 0;
    let mut total_values = 0;
    for op in &ctx.ops {
        match &op.payload {
            Some(grc20::op::Payload::UpdateEntity(e)) => {
                entity_count += 1;
                total_values += e.values.len();
            }
            Some(grc20::op::Payload::CreateRelation(_)) => relation_count += 1,
            _ => {}
        }
    }
    println!("  - {} entities, {} relations, {} total values", entity_count, relation_count, total_values);

    // Create edit
    let edit = grc20::Edit {
        id: make_entity_id(0xFF, 1),
        name: "Countries Import".to_string(),
        ops: ctx.ops,
        authors: vec![make_entity_id(0xAA, 1)],
        language: None,
    };

    // Wrap in File message
    let file = grc20::File {
        version: "1.0.0".to_string(),
        payload: Some(grc20::file::Payload::AddEdit(edit.clone())),
    };

    // Benchmark encoding (uncompressed)
    let encode_start = Instant::now();
    let encoded = file.encode_to_vec();
    let encode_time = encode_start.elapsed();

    println!(
        "\nUncompressed: {} bytes in {:?}",
        encoded.len(),
        encode_time
    );
    println!(
        "  Throughput: {:.2} MB/s",
        (encoded.len() as f64 / 1_000_000.0) / encode_time.as_secs_f64()
    );

    // Benchmark encoding (compressed with zstd)
    let compress_start = Instant::now();
    let compressed = zstd::encode_all(encoded.as_slice(), 3).expect("Failed to compress");
    let compress_time = compress_start.elapsed();

    println!(
        "\nCompressed (level 3): {} bytes in {:?}",
        compressed.len(),
        compress_time
    );
    println!(
        "  Compression ratio: {:.1}x",
        encoded.len() as f64 / compressed.len() as f64
    );
    println!(
        "  Throughput: {:.2} MB/s (uncompressed equivalent)",
        (encoded.len() as f64 / 1_000_000.0) / compress_time.as_secs_f64()
    );

    // Benchmark decoding (uncompressed) - multiple iterations
    const DECODE_ITERS: u32 = 100;
    // Warmup
    for _ in 0..10 {
        let _ = grc20::File::decode(encoded.as_slice()).expect("Failed to decode");
    }
    let decode_start = Instant::now();
    let mut decoded = None;
    for _ in 0..DECODE_ITERS {
        decoded = Some(grc20::File::decode(encoded.as_slice()).expect("Failed to decode"));
    }
    let decode_time = decode_start.elapsed() / DECODE_ITERS;
    let decoded = decoded.unwrap();

    println!("\nDecode (uncompressed): {:?} (avg of {} iterations)", decode_time, DECODE_ITERS);
    println!(
        "  Throughput: {:.2} MB/s",
        (encoded.len() as f64 / 1_000_000.0) / decode_time.as_secs_f64()
    );

    // Verify decode
    if let Some(grc20::file::Payload::AddEdit(decoded_edit)) = decoded.payload {
        assert_eq!(decoded_edit.ops.len(), edit.ops.len());
    }

    // Benchmark decoding (compressed) - multiple iterations
    // Warmup
    for _ in 0..10 {
        let decompressed = zstd::decode_all(compressed.as_slice()).expect("Failed to decompress");
        let _ = grc20::File::decode(decompressed.as_slice()).expect("Failed to decode");
    }
    let decode_compressed_start = Instant::now();
    let mut decoded_compressed = None;
    for _ in 0..DECODE_ITERS {
        let decompressed = zstd::decode_all(compressed.as_slice()).expect("Failed to decompress");
        decoded_compressed = Some(grc20::File::decode(decompressed.as_slice()).expect("Failed to decode"));
    }
    let decode_compressed_time = decode_compressed_start.elapsed() / DECODE_ITERS;
    let decoded_compressed = decoded_compressed.unwrap();

    println!("\nDecode (compressed): {:?} (avg of {} iterations)", decode_compressed_time, DECODE_ITERS);
    println!(
        "  Throughput: {:.2} MB/s (uncompressed equivalent)",
        (encoded.len() as f64 / 1_000_000.0) / decode_compressed_time.as_secs_f64()
    );

    if let Some(grc20::file::Payload::AddEdit(decoded_edit)) = decoded_compressed.payload {
        assert_eq!(decoded_edit.ops.len(), edit.ops.len());
    }

    // Write output files
    let input_path = Path::new(&data_path);
    let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
    let parent = input_path.parent().unwrap_or(Path::new("."));

    let output_uncompressed = parent.join(format!("{}.pb", stem));
    let output_compressed = parent.join(format!("{}.pbz", stem));

    fs::write(&output_uncompressed, &encoded).expect("Failed to write .pb file");
    fs::write(&output_compressed, &compressed).expect("Failed to write .pbz file");

    println!("\n=== Output Files ===");
    println!("Uncompressed: {}", output_uncompressed.display());
    println!("Compressed:   {}", output_compressed.display());

    // Summary
    println!("\n=== Summary ===");
    println!("Countries: {}", countries.len());
    println!("Regions: {}", ctx.created_regions.len());
    println!("Subregions: {}", ctx.created_subregions.len());
    println!("Timezones: {}", ctx.created_timezones.len());
    println!("Total operations: {}", edit.ops.len());
    println!("JSON size: {} bytes ({:.1} KB)", json_data.len(), json_data.len() as f64 / 1024.0);
    println!("Proto uncompressed: {} bytes ({:.1} KB)", encoded.len(), encoded.len() as f64 / 1024.0);
    println!("Proto compressed: {} bytes ({:.1} KB)", compressed.len(), compressed.len() as f64 / 1024.0);
    println!(
        "Size vs JSON: {:.1}% (uncompressed), {:.1}% (compressed)",
        100.0 * encoded.len() as f64 / json_data.len() as f64,
        100.0 * compressed.len() as f64 / json_data.len() as f64
    );
}
