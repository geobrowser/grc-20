//! Benchmark for GRC-20 serialization using country data.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::Instant;

use grc_20::{
    CreateEntity, CreateProperty, CreateRelation, DataType, Edit, EncodeOptions, Op, PropertyValue,
    RelationIdMode, Value,
};
use serde::Deserialize;

// =============================================================================
// HARDCODED UUIDs FOR SCHEMA
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
        _ => 0,
    }
}

/// Property IDs - using deterministic UUIDs for reproducibility
mod props {
    use super::hex;

    // Country properties
    pub const NAME: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3d4");
    pub const ISO3: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3d5");
    pub const ISO2: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3d6");
    pub const NUMERIC_CODE: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3d7");
    pub const PHONE_CODE: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3d8");
    pub const CAPITAL: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3d9");
    pub const CURRENCY_CODE: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3da");
    pub const CURRENCY_NAME: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3db");
    pub const CURRENCY_SYMBOL: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3dc");
    pub const TLD: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3dd");
    pub const NATIVE_NAME: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3de");
    pub const POPULATION: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3df");
    pub const GDP: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3e0");
    pub const NATIONALITY: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3e1");
    pub const AREA_SQ_KM: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3e2");
    pub const POSTAL_CODE_FORMAT: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3e3");
    pub const POSTAL_CODE_REGEX: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3e4");
    pub const LOCATION: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3e5");
    pub const EMOJI: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3e6");
    pub const WIKIDATA_ID: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3e7");
    pub const EMOJI_UNICODE: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3e8");

    // Timezone properties
    pub const ZONE_NAME: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c4d1");
    pub const GMT_OFFSET: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c4d2");
    pub const GMT_OFFSET_NAME: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c4d3");
    pub const ABBREVIATION: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c4d4");
    pub const TZ_NAME: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c4d5");
}

/// Type IDs
mod types {
    use super::hex;

    pub const COUNTRY: [u8; 16] = hex("b1b2c3d4e5f6071829304050a1b2c3d4");
    pub const REGION: [u8; 16] = hex("b1b2c3d4e5f6071829304050a1b2c3d5");
    pub const SUBREGION: [u8; 16] = hex("b1b2c3d4e5f6071829304050a1b2c3d6");
    pub const TIMEZONE: [u8; 16] = hex("b1b2c3d4e5f6071829304050a1b2c3d7");
}

/// Relation type IDs
mod rel_types {
    use super::hex;

    pub const TYPES: [u8; 16] = hex("c1b2c3d4e5f6071829304050a1b2c3d4");
    pub const IN_REGION: [u8; 16] = hex("c1b2c3d4e5f6071829304050a1b2c3d5");
    pub const IN_SUBREGION: [u8; 16] = hex("c1b2c3d4e5f6071829304050a1b2c3d6");
    pub const HAS_TIMEZONE: [u8; 16] = hex("c1b2c3d4e5f6071829304050a1b2c3d7");
}

/// Language IDs
mod langs {
    use super::hex;

    pub const BRETON: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3d0");
    pub const KOREAN: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3d1");
    pub const PORTUGUESE_BR: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3d2");
    pub const PORTUGUESE: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3d3");
    pub const DUTCH: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3d4");
    pub const CROATIAN: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3d5");
    pub const PERSIAN: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3d6");
    pub const GERMAN: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3d7");
    pub const SPANISH: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3d8");
    pub const FRENCH: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3d9");
    pub const JAPANESE: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3da");
    pub const ITALIAN: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3db");
    pub const CHINESE: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3dc");
    pub const TURKISH: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3dd");
    pub const RUSSIAN: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3de");
    pub const UKRAINIAN: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3df");
    pub const POLISH: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3e0");
    pub const ARABIC: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3e1");
    pub const HINDI: [u8; 16] = hex("d1b2c3d4e5f6071829304050a1b2c3e2");
}

// =============================================================================
// JSON DATA STRUCTURES
// =============================================================================

#[derive(Debug, Deserialize)]
struct Timezone {
    #[serde(rename = "zoneName")]
    zone_name: String,
    #[serde(rename = "gmtOffset")]
    gmt_offset: i64,
    #[serde(rename = "gmtOffsetName")]
    gmt_offset_name: String,
    abbreviation: String,
    #[serde(rename = "tzName")]
    tz_name: String,
}

#[derive(Debug, Deserialize)]
struct Country {
    id: u32,
    name: String,
    iso3: String,
    iso2: String,
    numeric_code: Option<String>,
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
    emoji_unicode: Option<String>,
    #[serde(rename = "wikiDataId")]
    wikidata_id: Option<String>,
}

// =============================================================================
// CONVERSION TO GRC-20
// =============================================================================

// Entity ID prefixes
const PREFIX_COUNTRY: u8 = 0x01;
const PREFIX_REGION: u8 = 0x02;
const PREFIX_SUBREGION: u8 = 0x03;
const PREFIX_TIMEZONE: u8 = 0x04;
const PREFIX_REL_ENTITY: u8 = 0x10;

fn make_entity_id(prefix: u8, id: u32) -> [u8; 16] {
    let mut uuid = [0u8; 16];
    uuid[0] = prefix;
    uuid[12..16].copy_from_slice(&id.to_be_bytes());
    // Set version 8 and variant
    uuid[6] = (uuid[6] & 0x0F) | 0x80;
    uuid[8] = (uuid[8] & 0x3F) | 0x80;
    uuid
}

fn make_timezone_id(zone_name: &str) -> [u8; 16] {
    // Hash the zone name to create a deterministic ID
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    zone_name.hash(&mut hasher);
    let hash = hasher.finish();

    let mut uuid = [0u8; 16];
    uuid[0] = PREFIX_TIMEZONE;
    uuid[8..16].copy_from_slice(&hash.to_be_bytes());
    // Set version 8 and variant
    uuid[6] = (uuid[6] & 0x0F) | 0x80;
    uuid[8] = (uuid[8] & 0x3F) | 0x80;
    uuid
}

fn make_rel_entity_id(from_prefix: u8, from_id: u32, rel_type: u8, seq: u32) -> [u8; 16] {
    let mut uuid = [0u8; 16];
    uuid[0] = PREFIX_REL_ENTITY;
    uuid[1] = from_prefix;
    uuid[2] = rel_type;
    uuid[4..8].copy_from_slice(&from_id.to_be_bytes());
    uuid[12..16].copy_from_slice(&seq.to_be_bytes());
    // Set version 8 and variant
    uuid[6] = (uuid[6] & 0x0F) | 0x80;
    uuid[8] = (uuid[8] & 0x3F) | 0x80;
    uuid
}

fn get_language_id(lang_code: &str) -> Option<[u8; 16]> {
    match lang_code {
        "br" => Some(langs::BRETON),
        "ko" => Some(langs::KOREAN),
        "pt-BR" => Some(langs::PORTUGUESE_BR),
        "pt" => Some(langs::PORTUGUESE),
        "nl" => Some(langs::DUTCH),
        "hr" => Some(langs::CROATIAN),
        "fa" => Some(langs::PERSIAN),
        "de" => Some(langs::GERMAN),
        "es" => Some(langs::SPANISH),
        "fr" => Some(langs::FRENCH),
        "ja" => Some(langs::JAPANESE),
        "it" => Some(langs::ITALIAN),
        "zh-CN" => Some(langs::CHINESE),
        "tr" => Some(langs::TURKISH),
        "ru" => Some(langs::RUSSIAN),
        "uk" => Some(langs::UKRAINIAN),
        "pl" => Some(langs::POLISH),
        "ar" => Some(langs::ARABIC),
        "hi" => Some(langs::HINDI),
        _ => None,
    }
}

struct ConversionContext {
    ops: Vec<Op<'static>>,
    created_regions: HashSet<u32>,
    created_subregions: HashSet<u32>,
    created_timezones: HashSet<String>,
}

impl ConversionContext {
    fn new() -> Self {
        Self {
            ops: create_schema_ops(),
            created_regions: HashSet::new(),
            created_subregions: HashSet::new(),
            created_timezones: HashSet::new(),
        }
    }

    fn ensure_region(&mut self, region_id: u32, region_name: &str) {
        if self.created_regions.insert(region_id) {
            let entity_id = make_entity_id(PREFIX_REGION, region_id);

            // Create region entity
            self.ops.push(Op::CreateEntity(CreateEntity {
                id: entity_id,
                values: vec![PropertyValue {
                    property: props::NAME,
                    value: Value::Text {
                        value: Cow::Owned(region_name.to_string()),
                        language: None,
                    },
                }],
            }));

            // Create Types relation (unique mode uses auto-derived entity)
            self.ops.push(Op::CreateRelation(CreateRelation {
                id_mode: RelationIdMode::Unique,
                relation_type: rel_types::TYPES,
                from: entity_id,
                to: types::REGION,
                entity: None,
                position: None,
                from_space: None,
                from_version: None,
                to_space: None,
                to_version: None,
            }));
        }
    }

    fn ensure_subregion(&mut self, subregion_id: u32, subregion_name: &str, region_id: Option<u32>) {
        if self.created_subregions.insert(subregion_id) {
            let entity_id = make_entity_id(PREFIX_SUBREGION, subregion_id);

            // Create subregion entity
            self.ops.push(Op::CreateEntity(CreateEntity {
                id: entity_id,
                values: vec![PropertyValue {
                    property: props::NAME,
                    value: Value::Text {
                        value: Cow::Owned(subregion_name.to_string()),
                        language: None,
                    },
                }],
            }));

            // Create Types relation (unique mode uses auto-derived entity)
            self.ops.push(Op::CreateRelation(CreateRelation {
                id_mode: RelationIdMode::Unique,
                relation_type: rel_types::TYPES,
                from: entity_id,
                to: types::SUBREGION,
                entity: None,
                position: None,
                from_space: None,
                from_version: None,
                to_space: None,
                to_version: None,
            }));

            // Create IN_REGION relation if region is known (unique mode uses auto-derived entity)
            if let Some(rid) = region_id {
                let region_entity_id = make_entity_id(PREFIX_REGION, rid);
                self.ops.push(Op::CreateRelation(CreateRelation {
                    id_mode: RelationIdMode::Unique,
                    relation_type: rel_types::IN_REGION,
                    from: entity_id,
                    to: region_entity_id,
                    entity: None,
                    position: None,
                    from_space: None,
                    from_version: None,
                    to_space: None,
                    to_version: None,
                }));
            }
        }
    }

    fn ensure_timezone(&mut self, tz: &Timezone) {
        if self.created_timezones.insert(tz.zone_name.clone()) {
            let entity_id = make_timezone_id(&tz.zone_name);

            // Create timezone entity
            self.ops.push(Op::CreateEntity(CreateEntity {
                id: entity_id,
                values: vec![
                    PropertyValue {
                        property: props::ZONE_NAME,
                        value: Value::Text {
                            value: Cow::Owned(tz.zone_name.clone()),
                            language: None,
                        },
                    },
                    PropertyValue {
                        property: props::GMT_OFFSET,
                        value: Value::Int64 { value: tz.gmt_offset, unit: None },
                    },
                    PropertyValue {
                        property: props::GMT_OFFSET_NAME,
                        value: Value::Text {
                            value: Cow::Owned(tz.gmt_offset_name.clone()),
                            language: None,
                        },
                    },
                    PropertyValue {
                        property: props::ABBREVIATION,
                        value: Value::Text {
                            value: Cow::Owned(tz.abbreviation.clone()),
                            language: None,
                        },
                    },
                    PropertyValue {
                        property: props::TZ_NAME,
                        value: Value::Text {
                            value: Cow::Owned(tz.tz_name.clone()),
                            language: None,
                        },
                    },
                ],
            }));

            // Create Types relation (unique mode uses auto-derived entity)
            self.ops.push(Op::CreateRelation(CreateRelation {
                id_mode: RelationIdMode::Unique,
                relation_type: rel_types::TYPES,
                from: entity_id,
                to: types::TIMEZONE,
                entity: None,
                position: None,
                from_space: None,
                from_version: None,
                to_space: None,
                to_version: None,
            }));
        }
    }

    fn add_country(&mut self, country: &Country) {
        let entity_id = make_entity_id(PREFIX_COUNTRY, country.id);
        let mut values = Vec::new();

        // Required fields
        values.push(PropertyValue {
            property: props::NAME,
            value: Value::Text {
                value: Cow::Owned(country.name.clone()),
                language: None,
            },
        });

        values.push(PropertyValue {
            property: props::ISO3,
            value: Value::Text {
                value: Cow::Owned(country.iso3.clone()),
                language: None,
            },
        });

        values.push(PropertyValue {
            property: props::ISO2,
            value: Value::Text {
                value: Cow::Owned(country.iso2.clone()),
                language: None,
            },
        });

        // Optional text fields
        if let Some(ref v) = country.numeric_code {
            values.push(PropertyValue {
                property: props::NUMERIC_CODE,
                value: Value::Text { value: Cow::Owned(v.clone()), language: None },
            });
        }

        if let Some(ref v) = country.phonecode {
            values.push(PropertyValue {
                property: props::PHONE_CODE,
                value: Value::Text { value: Cow::Owned(v.clone()), language: None },
            });
        }

        if let Some(ref v) = country.capital {
            values.push(PropertyValue {
                property: props::CAPITAL,
                value: Value::Text { value: Cow::Owned(v.clone()), language: None },
            });
        }

        if let Some(ref v) = country.currency {
            values.push(PropertyValue {
                property: props::CURRENCY_CODE,
                value: Value::Text { value: Cow::Owned(v.clone()), language: None },
            });
        }

        if let Some(ref v) = country.currency_name {
            values.push(PropertyValue {
                property: props::CURRENCY_NAME,
                value: Value::Text { value: Cow::Owned(v.clone()), language: None },
            });
        }

        if let Some(ref v) = country.currency_symbol {
            values.push(PropertyValue {
                property: props::CURRENCY_SYMBOL,
                value: Value::Text { value: Cow::Owned(v.clone()), language: None },
            });
        }

        if let Some(ref v) = country.tld {
            values.push(PropertyValue {
                property: props::TLD,
                value: Value::Text { value: Cow::Owned(v.clone()), language: None },
            });
        }

        if let Some(ref v) = country.native {
            values.push(PropertyValue {
                property: props::NATIVE_NAME,
                value: Value::Text { value: Cow::Owned(v.clone()), language: None },
            });
        }

        if let Some(ref v) = country.nationality {
            values.push(PropertyValue {
                property: props::NATIONALITY,
                value: Value::Text { value: Cow::Owned(v.clone()), language: None },
            });
        }

        if let Some(ref v) = country.postal_code_format {
            values.push(PropertyValue {
                property: props::POSTAL_CODE_FORMAT,
                value: Value::Text { value: Cow::Owned(v.clone()), language: None },
            });
        }

        if let Some(ref v) = country.postal_code_regex {
            values.push(PropertyValue {
                property: props::POSTAL_CODE_REGEX,
                value: Value::Text { value: Cow::Owned(v.clone()), language: None },
            });
        }

        if let Some(ref v) = country.emoji {
            values.push(PropertyValue {
                property: props::EMOJI,
                value: Value::Text { value: Cow::Owned(v.clone()), language: None },
            });
        }

        if let Some(ref v) = country.emoji_unicode {
            values.push(PropertyValue {
                property: props::EMOJI_UNICODE,
                value: Value::Text { value: Cow::Owned(v.clone()), language: None },
            });
        }

        if let Some(ref v) = country.wikidata_id {
            values.push(PropertyValue {
                property: props::WIKIDATA_ID,
                value: Value::Text { value: Cow::Owned(v.clone()), language: None },
            });
        }

        // Numeric fields
        if let Some(v) = country.population {
            values.push(PropertyValue {
                property: props::POPULATION,
                value: Value::Int64 { value: v, unit: None },
            });
        }

        if let Some(v) = country.gdp {
            values.push(PropertyValue {
                property: props::GDP,
                value: Value::Int64 { value: v, unit: None },
            });
        }

        if let Some(v) = country.area_sq_km {
            values.push(PropertyValue {
                property: props::AREA_SQ_KM,
                value: Value::Int64 { value: v, unit: None },
            });
        }

        // Location as POINT
        if let (Some(lat_str), Some(lon_str)) = (&country.latitude, &country.longitude) {
            if let (Ok(lat), Ok(lon)) = (lat_str.parse::<f64>(), lon_str.parse::<f64>()) {
                values.push(PropertyValue {
                    property: props::LOCATION,
                    value: Value::Point { lat, lon },
                });
            }
        }

        // Translations as multi-value TEXT with language
        if let Some(ref translations) = country.translations {
            for (lang_code, translation) in translations {
                if let Some(lang_id) = get_language_id(lang_code) {
                    values.push(PropertyValue {
                        property: props::NAME,
                        value: Value::Text {
                            value: Cow::Owned(translation.clone()),
                            language: Some(lang_id),
                        },
                    });
                }
            }
        }

        // Create entity
        self.ops.push(Op::CreateEntity(CreateEntity {
            id: entity_id,
            values,
        }));

        // Create Types relation (unique mode uses auto-derived entity)
        self.ops.push(Op::CreateRelation(CreateRelation {
            id_mode: RelationIdMode::Unique,
            relation_type: rel_types::TYPES,
            from: entity_id,
            to: types::COUNTRY,
            entity: None,
            position: None,
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
        }));

        // Create region/subregion entities and relations
        if let (Some(region_id), Some(region_name)) = (country.region_id, &country.region) {
            self.ensure_region(region_id, region_name);

            // IN_REGION relation (unique mode uses auto-derived entity)
            let region_entity_id = make_entity_id(PREFIX_REGION, region_id);
            self.ops.push(Op::CreateRelation(CreateRelation {
                id_mode: RelationIdMode::Unique,
                relation_type: rel_types::IN_REGION,
                from: entity_id,
                to: region_entity_id,
                entity: None,
                position: None,
                from_space: None,
                from_version: None,
                to_space: None,
                to_version: None,
            }));
        }

        if let (Some(subregion_id), Some(subregion_name)) = (country.subregion_id, &country.subregion) {
            self.ensure_subregion(subregion_id, subregion_name, country.region_id);

            // IN_SUBREGION relation (unique mode uses auto-derived entity)
            let subregion_entity_id = make_entity_id(PREFIX_SUBREGION, subregion_id);
            self.ops.push(Op::CreateRelation(CreateRelation {
                id_mode: RelationIdMode::Unique,
                relation_type: rel_types::IN_SUBREGION,
                from: entity_id,
                to: subregion_entity_id,
                entity: None,
                position: None,
                from_space: None,
                from_version: None,
                to_space: None,
                to_version: None,
            }));
        }

        // Create timezone relations (instance mode with auto-derived entity)
        if let Some(ref timezones) = country.timezones {
            for (i, tz) in timezones.iter().enumerate() {
                self.ensure_timezone(tz);

                let tz_entity_id = make_timezone_id(&tz.zone_name);
                let rel_id = make_rel_entity_id(PREFIX_COUNTRY, country.id, 3, i as u32);
                self.ops.push(Op::CreateRelation(CreateRelation {
                    id_mode: RelationIdMode::Many(rel_id),
                    relation_type: rel_types::HAS_TIMEZONE,
                    from: entity_id,
                    to: tz_entity_id,
                    entity: None, // Auto-derive entity from relation ID
                    position: None,
                    from_space: None,
                    from_version: None,
                    to_space: None,
                    to_version: None,
                }));
            }
        }
    }
}

fn create_schema_ops() -> Vec<Op<'static>> {
    vec![
        // Country properties
        Op::CreateProperty(CreateProperty { id: props::NAME, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::ISO3, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::ISO2, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::NUMERIC_CODE, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::PHONE_CODE, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::CAPITAL, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::CURRENCY_CODE, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::CURRENCY_NAME, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::CURRENCY_SYMBOL, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::TLD, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::NATIVE_NAME, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::POPULATION, data_type: DataType::Int64 }),
        Op::CreateProperty(CreateProperty { id: props::GDP, data_type: DataType::Int64 }),
        Op::CreateProperty(CreateProperty { id: props::NATIONALITY, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::AREA_SQ_KM, data_type: DataType::Int64 }),
        Op::CreateProperty(CreateProperty { id: props::POSTAL_CODE_FORMAT, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::POSTAL_CODE_REGEX, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::LOCATION, data_type: DataType::Point }),
        Op::CreateProperty(CreateProperty { id: props::EMOJI, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::EMOJI_UNICODE, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::WIKIDATA_ID, data_type: DataType::Text }),
        // Timezone properties
        Op::CreateProperty(CreateProperty { id: props::ZONE_NAME, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::GMT_OFFSET, data_type: DataType::Int64 }),
        Op::CreateProperty(CreateProperty { id: props::GMT_OFFSET_NAME, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::ABBREVIATION, data_type: DataType::Text }),
        Op::CreateProperty(CreateProperty { id: props::TZ_NAME, data_type: DataType::Text }),
        // Type entities
        Op::CreateEntity(CreateEntity {
            id: types::COUNTRY,
            values: vec![PropertyValue {
                property: props::NAME,
                value: Value::Text { value: Cow::Borrowed("Country"), language: None },
            }],
        }),
        Op::CreateEntity(CreateEntity {
            id: types::REGION,
            values: vec![PropertyValue {
                property: props::NAME,
                value: Value::Text { value: Cow::Borrowed("Region"), language: None },
            }],
        }),
        Op::CreateEntity(CreateEntity {
            id: types::SUBREGION,
            values: vec![PropertyValue {
                property: props::NAME,
                value: Value::Text { value: Cow::Borrowed("Subregion"), language: None },
            }],
        }),
        Op::CreateEntity(CreateEntity {
            id: types::TIMEZONE,
            values: vec![PropertyValue {
                property: props::NAME,
                value: Value::Text { value: Cow::Borrowed("Timezone"), language: None },
            }],
        }),
    ]
}

fn main() {
    // Find the data file
    let data_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../data/countries.json".to_string());

    println!("Loading countries from: {}", data_path);

    let json_data = fs::read_to_string(&data_path).expect("Failed to read countries.json");

    let parse_start = Instant::now();
    let countries: Vec<Country> = serde_json::from_str(&json_data).expect("Failed to parse JSON");
    let parse_time = parse_start.elapsed();

    println!("Loaded {} countries in {:?}", countries.len(), parse_time);

    // Convert to GRC-20 operations
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
    let mut property_count = 0;
    let mut total_values = 0;
    for op in &ctx.ops {
        match op {
            Op::CreateEntity(e) => {
                entity_count += 1;
                total_values += e.values.len();
            }
            Op::CreateRelation(_) => relation_count += 1,
            Op::CreateProperty(_) => property_count += 1,
            _ => {}
        }
    }
    println!("  - {} entities, {} relations, {} properties, {} total values",
             entity_count, relation_count, property_count, total_values);

    // Create edit
    let edit = Edit {
        id: make_entity_id(0xFF, 1),
        name: Cow::Borrowed("Countries Import"),
        authors: vec![make_entity_id(0xAA, 1)],
        created_at: 1704067200_000_000,
        ops: ctx.ops,
    };

    // Benchmark encoding (uncompressed, fast mode)
    let encode_start = Instant::now();
    let encoded = grc_20::encode_edit(&edit).expect("Failed to encode");
    let encode_time = encode_start.elapsed();

    println!(
        "\nUncompressed (fast): {} bytes in {:?}",
        encoded.len(),
        encode_time
    );
    println!(
        "  Throughput: {:.2} MB/s",
        (encoded.len() as f64 / 1_000_000.0) / encode_time.as_secs_f64()
    );

    // Benchmark encoding (uncompressed, canonical mode)
    let canonical_start = Instant::now();
    let canonical_encoded = grc_20::encode_edit_with_options(&edit, EncodeOptions::canonical())
        .expect("Failed to encode canonical");
    let canonical_time = canonical_start.elapsed();

    println!(
        "\nUncompressed (canonical): {} bytes in {:?}",
        canonical_encoded.len(),
        canonical_time
    );
    println!(
        "  Throughput: {:.2} MB/s",
        (canonical_encoded.len() as f64 / 1_000_000.0) / canonical_time.as_secs_f64()
    );
    println!(
        "  Overhead vs fast: {:.1}x slower",
        canonical_time.as_secs_f64() / encode_time.as_secs_f64()
    );

    // Verify canonical encoding is deterministic (encode twice)
    let canonical_encoded2 = grc_20::encode_edit_with_options(&edit, EncodeOptions::canonical())
        .expect("Failed to encode canonical");
    assert_eq!(canonical_encoded, canonical_encoded2, "Canonical encoding should be deterministic");

    // Benchmark encoding (compressed)
    let compress_start = Instant::now();
    let compressed = grc_20::encode_edit_compressed(&edit, 3).expect("Failed to compress");
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
        let _ = grc_20::decode_edit(&encoded).expect("Failed to decode");
    }
    let decode_start = Instant::now();
    let mut decoded = None;
    for _ in 0..DECODE_ITERS {
        decoded = Some(grc_20::decode_edit(&encoded).expect("Failed to decode"));
    }
    let decode_time = decode_start.elapsed() / DECODE_ITERS;
    let decoded = decoded.unwrap();

    println!("\nDecode (uncompressed): {:?} (avg of {} iterations)", decode_time, DECODE_ITERS);
    println!(
        "  Throughput: {:.2} MB/s",
        (encoded.len() as f64 / 1_000_000.0) / decode_time.as_secs_f64()
    );
    assert_eq!(decoded.ops.len(), edit.ops.len());

    // Benchmark decoding (compressed, allocating) - multiple iterations
    // Warmup
    for _ in 0..10 {
        let _ = grc_20::decode_edit(&compressed).expect("Failed to decode compressed");
    }
    let decode_compressed_start = Instant::now();
    let mut decoded_compressed = None;
    for _ in 0..DECODE_ITERS {
        decoded_compressed = Some(grc_20::decode_edit(&compressed).expect("Failed to decode compressed"));
    }
    let decode_compressed_time = decode_compressed_start.elapsed() / DECODE_ITERS;
    let decoded_compressed = decoded_compressed.unwrap();

    println!("\nDecode (compressed, allocating): {:?} (avg of {} iterations)", decode_compressed_time, DECODE_ITERS);
    println!(
        "  Throughput: {:.2} MB/s (uncompressed equivalent)",
        (encoded.len() as f64 / 1_000_000.0) / decode_compressed_time.as_secs_f64()
    );
    assert_eq!(decoded_compressed.ops.len(), edit.ops.len());

    // Benchmark decoding (compressed, zero-copy) - two-step API
    // Warmup
    for _ in 0..10 {
        let decompressed = grc_20::decompress(&compressed).expect("Failed to decompress");
        let _ = grc_20::decode_edit(&decompressed).expect("Failed to decode");
    }
    let decode_zc_start = Instant::now();
    for _ in 0..DECODE_ITERS {
        let decompressed = grc_20::decompress(&compressed).expect("Failed to decompress");
        let decoded = grc_20::decode_edit(&decompressed).expect("Failed to decode");
        assert_eq!(decoded.ops.len(), edit.ops.len());
    }
    let decode_zc_time = decode_zc_start.elapsed() / DECODE_ITERS;

    println!("\nDecode (compressed, zero-copy): {:?} (avg of {} iterations)", decode_zc_time, DECODE_ITERS);
    println!(
        "  Throughput: {:.2} MB/s (uncompressed equivalent)",
        (encoded.len() as f64 / 1_000_000.0) / decode_zc_time.as_secs_f64()
    );
    println!(
        "  Speedup vs allocating: {:.1}%",
        100.0 * (decode_compressed_time.as_secs_f64() - decode_zc_time.as_secs_f64()) / decode_compressed_time.as_secs_f64()
    );

    // Write output files
    let input_path = Path::new(&data_path);
    let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
    let parent = input_path.parent().unwrap_or(Path::new("."));

    let output_uncompressed = parent.join(format!("{}.g20", stem));
    let output_compressed = parent.join(format!("{}.g20z", stem));

    fs::write(&output_uncompressed, &encoded).expect("Failed to write .g20 file");
    fs::write(&output_compressed, &compressed).expect("Failed to write .g20z file");

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
    println!("GRC-20 uncompressed: {} bytes ({:.1} KB)", encoded.len(), encoded.len() as f64 / 1024.0);
    println!("GRC-20 compressed: {} bytes ({:.1} KB)", compressed.len(), compressed.len() as f64 / 1024.0);
    println!(
        "Size vs JSON: {:.1}% (uncompressed), {:.1}% (compressed)",
        100.0 * encoded.len() as f64 / json_data.len() as f64,
        100.0 * compressed.len() as f64 / json_data.len() as f64
    );
}
