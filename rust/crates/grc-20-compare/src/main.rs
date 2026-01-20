//! Benchmark comparison between GRC-20 and Proto formats.
//!
//! Runs both serialization formats on the same data and outputs a comparison report.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use grc_20::{EditBuilder, EntityBuilder, Id, derived_uuid};

/// Creates a deterministic relation ID from from+to+type (to maintain same behavior as removed unique mode).
fn make_relation_id(from: Id, to: Id, rel_type: Id) -> Id {
    let mut input = [0u8; 48];
    input[0..16].copy_from_slice(&from);
    input[16..32].copy_from_slice(&to);
    input[32..48].copy_from_slice(&rel_type);
    derived_uuid(&input)
}
use prost::Message;
use serde::Deserialize;

// Include generated protobuf code
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/grc20.rs"));
}

// =============================================================================
// SHARED CONSTANTS
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

mod props {
    use super::hex;
    pub const NAME: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3d4");
    pub const CODE: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3d5");
    pub const NATIVE_NAME: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3d6");
    pub const POPULATION: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3d7");
    pub const LOCATION: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3d8");
    pub const TIMEZONE: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3d9");
    pub const WIKIDATA_ID: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3da");
    pub const CITY_TYPE: [u8; 16] = hex("a1b2c3d4e5f6071829304050a1b2c3db");
}

mod types {
    use super::hex;
    pub const CITY: [u8; 16] = hex("b1b2c3d4e5f6071829304050a1b2c3d4");
    pub const STATE: [u8; 16] = hex("b1b2c3d4e5f6071829304050a1b2c3d5");
    pub const COUNTRY: [u8; 16] = hex("b1b2c3d4e5f6071829304050a1b2c3d6");
}

mod rel_types {
    use super::hex;
    pub const TYPES: [u8; 16] = hex("c1b2c3d4e5f6071829304050a1b2c3d4");
    pub const IN_STATE: [u8; 16] = hex("c1b2c3d4e5f6071829304050a1b2c3d5");
    pub const IN_COUNTRY: [u8; 16] = hex("c1b2c3d4e5f6071829304050a1b2c3d6");
}

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
            "zh-CN" => Some(CHINESE),
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

const PREFIX_CITY: u8 = 0x01;
const PREFIX_STATE: u8 = 0x02;
const PREFIX_COUNTRY: u8 = 0x03;

// =============================================================================
// DATA STRUCTURES
// =============================================================================

#[derive(Debug, Deserialize)]
struct City {
    id: u32,
    name: String,
    state_id: u32,
    state_code: String,
    state_name: String,
    country_id: u32,
    country_code: String,
    country_name: String,
    latitude: String,
    longitude: String,
    native: Option<String>,
    #[serde(rename = "type")]
    city_type: Option<String>,
    population: Option<i64>,
    timezone: Option<String>,
    translations: Option<std::collections::HashMap<String, String>>,
    #[serde(rename = "wikiDataId")]
    wikidata_id: Option<String>,
}

#[derive(Default)]
struct BenchResult {
    size_uncompressed: usize,
    size_compressed: usize,
    encode_time: Duration,
    compress_time: Duration,
    decode_time: Duration,
    decode_compressed_time: Duration,
}

// =============================================================================
// ID GENERATION
// =============================================================================

fn make_entity_id(prefix: u8, id: u32) -> [u8; 16] {
    let mut uuid = [0u8; 16];
    uuid[0] = prefix;
    uuid[12..16].copy_from_slice(&id.to_be_bytes());
    uuid[6] = (uuid[6] & 0x0F) | 0x80;
    uuid[8] = (uuid[8] & 0x3F) | 0x80;
    uuid
}

fn make_rel_entity_id(prefix: u8, entity_id: u32, rel_type: u8, index: u16) -> Vec<u8> {
    let mut result = [0u8; 16];
    result[0] = prefix;
    result[1] = rel_type;
    result[2..4].copy_from_slice(&index.to_be_bytes());
    result[12..16].copy_from_slice(&entity_id.to_be_bytes());
    result.to_vec()
}

// =============================================================================
// GRC-20 BENCHMARK
// =============================================================================

fn build_city_entity_grc20<'a>(city: &'a City) -> EntityBuilder<'a> {
    let mut builder = EntityBuilder::new().text(props::NAME, city.name.as_str(), None);

    if let Some(ref native) = city.native {
        if !native.is_empty() {
            builder = builder.text(props::NATIVE_NAME, native.as_str(), None);
        }
    }

    if let Some(ref city_type) = city.city_type {
        builder = builder.text(props::CITY_TYPE, city_type.as_str(), None);
    }

    if let Some(pop) = city.population {
        builder = builder.int64(props::POPULATION, pop, None);
    }

    if let (Ok(lat), Ok(lon)) = (city.latitude.parse::<f64>(), city.longitude.parse::<f64>()) {
        builder = builder.point(props::LOCATION, lon, lat, None);
    }

    if let Some(ref tz) = city.timezone {
        builder = builder.text(props::TIMEZONE, tz.as_str(), None);
    }

    if let Some(ref wiki_id) = city.wikidata_id {
        builder = builder.text(props::WIKIDATA_ID, wiki_id.as_str(), None);
    }

    if let Some(ref translations) = city.translations {
        for (lang_code, translation) in translations {
            if let Some(lang_id) = langs::from_code(lang_code) {
                builder = builder.text(props::NAME, translation.as_str(), Some(lang_id));
            }
        }
    }

    builder
}

fn benchmark_grc20(cities: &[City], iterations: u32) -> BenchResult {
    let mut result = BenchResult::default();

    // Convert to GRC-20
    let edit_id = make_entity_id(0xFF, 1);
    let author_id = make_entity_id(0xAA, 1);

    let mut builder = EditBuilder::new(edit_id)
        .name("Cities Import")
        .author(author_id)
        .created_at(1704067200_000_000);

    builder = builder
        .create_entity(types::CITY, |e| e.text(props::NAME, "City", None))
        .create_entity(types::STATE, |e| e.text(props::NAME, "State", None))
        .create_entity(types::COUNTRY, |e| e.text(props::NAME, "Country", None));

    let mut created_states: HashSet<u32> = HashSet::new();
    let mut created_countries: HashSet<u32> = HashSet::new();

    for city in cities {
        let city_id = make_entity_id(PREFIX_CITY, city.id);
        let state_id = make_entity_id(PREFIX_STATE, city.state_id);
        let country_id = make_entity_id(PREFIX_COUNTRY, city.country_id);

        if created_countries.insert(city.country_id) {
            builder = builder
                .create_entity(country_id, |e| {
                    e.text(props::NAME, city.country_name.as_str(), None)
                        .text(props::CODE, city.country_code.as_str(), None)
                })
                .create_relation_simple(
                    make_relation_id(country_id, types::COUNTRY, rel_types::TYPES),
                    country_id, types::COUNTRY, rel_types::TYPES
                );
        }

        if created_states.insert(city.state_id) {
            builder = builder
                .create_entity(state_id, |e| {
                    e.text(props::NAME, city.state_name.as_str(), None)
                        .text(props::CODE, city.state_code.as_str(), None)
                })
                .create_relation_simple(
                    make_relation_id(state_id, types::STATE, rel_types::TYPES),
                    state_id, types::STATE, rel_types::TYPES
                )
                .create_relation_simple(
                    make_relation_id(state_id, country_id, rel_types::IN_COUNTRY),
                    state_id, country_id, rel_types::IN_COUNTRY
                );
        }

        builder = builder
            .create_entity(city_id, |_| build_city_entity_grc20(city))
            .create_relation_simple(
                make_relation_id(city_id, types::CITY, rel_types::TYPES),
                city_id, types::CITY, rel_types::TYPES
            )
            .create_relation_simple(
                make_relation_id(city_id, state_id, rel_types::IN_STATE),
                city_id, state_id, rel_types::IN_STATE
            )
            .create_relation_simple(
                make_relation_id(city_id, country_id, rel_types::IN_COUNTRY),
                city_id, country_id, rel_types::IN_COUNTRY
            );
    }

    let edit = builder.build();

    // Encode uncompressed
    let start = Instant::now();
    let encoded = grc_20::encode_edit(&edit).expect("Failed to encode");
    result.encode_time = start.elapsed();
    result.size_uncompressed = encoded.len();

    // Encode compressed
    let start = Instant::now();
    let compressed = grc_20::encode_edit_compressed(&edit, 3).expect("Failed to compress");
    result.compress_time = start.elapsed();
    result.size_compressed = compressed.len();

    // Decode uncompressed (warmup)
    for _ in 0..3 {
        let _ = grc_20::decode_edit(&encoded).expect("Failed to decode");
    }

    // Decode uncompressed (timed)
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = grc_20::decode_edit(&encoded).expect("Failed to decode");
    }
    result.decode_time = start.elapsed() / iterations;

    // Decode compressed (warmup)
    for _ in 0..3 {
        let _ = grc_20::decode_edit(&compressed).expect("Failed to decode");
    }

    // Decode compressed (timed)
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = grc_20::decode_edit(&compressed).expect("Failed to decode");
    }
    result.decode_compressed_time = start.elapsed() / iterations;

    result
}

// =============================================================================
// PROTO BENCHMARK
// =============================================================================

struct ProtoContext {
    ops: Vec<proto::Op>,
    created_states: HashSet<u32>,
    created_countries: HashSet<u32>,
}

impl ProtoContext {
    fn new() -> Self {
        Self {
            ops: Vec::new(),
            created_states: HashSet::new(),
            created_countries: HashSet::new(),
        }
    }

    fn make_value(&self, property: &[u8], value: String) -> proto::Value {
        proto::Value {
            property: property.to_vec(),
            value,
            options: None,
        }
    }

    fn make_text_value(&self, property: &[u8], value: String, language: Option<[u8; 16]>) -> proto::Value {
        proto::Value {
            property: property.to_vec(),
            value,
            options: language.map(|lang| proto::Options {
                value: Some(proto::options::Value::Text(proto::TextOptions {
                    language: Some(lang.to_vec()),
                })),
            }),
        }
    }

    fn create_relation(&mut self, from_entity: Vec<u8>, to_entity: Vec<u8>, rel_type: [u8; 16], rel_entity_id: Vec<u8>) {
        self.ops.push(proto::Op {
            payload: Some(proto::op::Payload::CreateRelation(proto::Relation {
                id: rel_entity_id.clone(),
                r#type: rel_type.to_vec(),
                from_entity,
                from_space: None,
                from_version: None,
                to_entity,
                to_space: None,
                to_version: None,
                entity: rel_entity_id,
                position: None,
                verified: None,
            })),
        });
    }

    fn ensure_country(&mut self, country_id: u32, name: &str, code: &str) {
        if self.created_countries.contains(&country_id) {
            return;
        }
        self.created_countries.insert(country_id);

        let entity_id = make_entity_id(PREFIX_COUNTRY, country_id).to_vec();

        self.ops.push(proto::Op {
            payload: Some(proto::op::Payload::UpdateEntity(proto::Entity {
                id: entity_id.clone(),
                values: vec![
                    self.make_value(&props::NAME, name.to_string()),
                    self.make_value(&props::CODE, code.to_string()),
                ],
            })),
        });

        let rel_entity_id = make_rel_entity_id(PREFIX_COUNTRY, country_id, 0, 0);
        self.create_relation(entity_id, types::COUNTRY.to_vec(), rel_types::TYPES, rel_entity_id);
    }

    fn ensure_state(&mut self, state_id: u32, name: &str, code: &str, country_id: u32) {
        if self.created_states.contains(&state_id) {
            return;
        }
        self.created_states.insert(state_id);

        let entity_id = make_entity_id(PREFIX_STATE, state_id).to_vec();
        let country_entity_id = make_entity_id(PREFIX_COUNTRY, country_id).to_vec();

        self.ops.push(proto::Op {
            payload: Some(proto::op::Payload::UpdateEntity(proto::Entity {
                id: entity_id.clone(),
                values: vec![
                    self.make_value(&props::NAME, name.to_string()),
                    self.make_value(&props::CODE, code.to_string()),
                ],
            })),
        });

        let rel_entity_id = make_rel_entity_id(PREFIX_STATE, state_id, 0, 0);
        self.create_relation(entity_id.clone(), types::STATE.to_vec(), rel_types::TYPES, rel_entity_id);

        let rel_entity_id = make_rel_entity_id(PREFIX_STATE, state_id, 1, 0);
        self.create_relation(entity_id, country_entity_id, rel_types::IN_COUNTRY, rel_entity_id);
    }

    fn add_city(&mut self, city: &City) {
        let entity_id = make_entity_id(PREFIX_CITY, city.id).to_vec();
        let state_entity_id = make_entity_id(PREFIX_STATE, city.state_id).to_vec();
        let country_entity_id = make_entity_id(PREFIX_COUNTRY, city.country_id).to_vec();

        self.ensure_country(city.country_id, &city.country_name, &city.country_code);
        self.ensure_state(city.state_id, &city.state_name, &city.state_code, city.country_id);

        let mut values = vec![self.make_value(&props::NAME, city.name.clone())];

        if let Some(ref native) = city.native {
            if !native.is_empty() {
                values.push(self.make_value(&props::NATIVE_NAME, native.clone()));
            }
        }

        if let Some(ref city_type) = city.city_type {
            values.push(self.make_value(&props::CITY_TYPE, city_type.clone()));
        }

        if let Some(pop) = city.population {
            values.push(self.make_value(&props::POPULATION, pop.to_string()));
        }

        if let (Ok(lat), Ok(lon)) = (city.latitude.parse::<f64>(), city.longitude.parse::<f64>()) {
            values.push(self.make_value(&props::LOCATION, format!("{},{}", lat, lon)));
        }

        if let Some(ref tz) = city.timezone {
            values.push(self.make_value(&props::TIMEZONE, tz.clone()));
        }

        if let Some(ref wiki_id) = city.wikidata_id {
            values.push(self.make_value(&props::WIKIDATA_ID, wiki_id.clone()));
        }

        if let Some(ref translations) = city.translations {
            for (lang_code, translation) in translations {
                if let Some(lang_id) = langs::from_code(lang_code) {
                    values.push(self.make_text_value(&props::NAME, translation.clone(), Some(lang_id)));
                }
            }
        }

        self.ops.push(proto::Op {
            payload: Some(proto::op::Payload::UpdateEntity(proto::Entity {
                id: entity_id.clone(),
                values,
            })),
        });

        let rel_entity_id = make_rel_entity_id(PREFIX_CITY, city.id, 0, 0);
        self.create_relation(entity_id.clone(), types::CITY.to_vec(), rel_types::TYPES, rel_entity_id);

        let rel_entity_id = make_rel_entity_id(PREFIX_CITY, city.id, 1, 0);
        self.create_relation(entity_id.clone(), state_entity_id, rel_types::IN_STATE, rel_entity_id);

        let rel_entity_id = make_rel_entity_id(PREFIX_CITY, city.id, 2, 0);
        self.create_relation(entity_id, country_entity_id, rel_types::IN_COUNTRY, rel_entity_id);
    }
}

fn benchmark_proto(cities: &[City], iterations: u32) -> BenchResult {
    let mut result = BenchResult::default();

    // Convert to proto
    let mut ctx = ProtoContext::new();
    for city in cities {
        ctx.add_city(city);
    }

    let edit = proto::Edit {
        id: make_entity_id(0xFF, 1).to_vec(),
        name: "Cities Import".to_string(),
        ops: ctx.ops,
        authors: vec![make_entity_id(0xAA, 1).to_vec()],
        language: None,
    };

    let file = proto::File {
        version: "1.0.0".to_string(),
        payload: Some(proto::file::Payload::AddEdit(edit)),
    };

    // Encode uncompressed
    let start = Instant::now();
    let encoded = file.encode_to_vec();
    result.encode_time = start.elapsed();
    result.size_uncompressed = encoded.len();

    // Encode compressed
    let start = Instant::now();
    let compressed = zstd::encode_all(encoded.as_slice(), 3).expect("Failed to compress");
    result.compress_time = start.elapsed();
    result.size_compressed = compressed.len();

    // Decode uncompressed (warmup)
    for _ in 0..3 {
        let _ = proto::File::decode(encoded.as_slice()).expect("Failed to decode");
    }

    // Decode uncompressed (timed)
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = proto::File::decode(encoded.as_slice()).expect("Failed to decode");
    }
    result.decode_time = start.elapsed() / iterations;

    // Decode compressed (warmup)
    for _ in 0..3 {
        let decompressed = zstd::decode_all(compressed.as_slice()).expect("Failed to decompress");
        let _ = proto::File::decode(decompressed.as_slice()).expect("Failed to decode");
    }

    // Decode compressed (timed)
    let start = Instant::now();
    for _ in 0..iterations {
        let decompressed = zstd::decode_all(compressed.as_slice()).expect("Failed to decompress");
        let _ = proto::File::decode(decompressed.as_slice()).expect("Failed to decode");
    }
    result.decode_compressed_time = start.elapsed() / iterations;

    result
}

// =============================================================================
// REPORT GENERATION
// =============================================================================

fn format_size(bytes: usize) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{} B", bytes)
    }
}

fn format_duration(duration: Duration) -> String {
    let micros = duration.as_micros();
    if micros >= 1_000_000 {
        format!("{:.2} s", duration.as_secs_f64())
    } else if micros >= 1_000 {
        format!("{:.1} ms", micros as f64 / 1_000.0)
    } else {
        format!("{} µs", micros)
    }
}

fn format_winner(grc20_value: f64, proto_value: f64, higher_is_better: bool) -> String {
    let ratio = if higher_is_better {
        grc20_value / proto_value
    } else {
        proto_value / grc20_value
    };

    if ratio > 1.05 {
        format!("GRC-20 {:.1}x", ratio)
    } else if ratio < 0.95 {
        format!("Proto {:.1}x", 1.0 / ratio)
    } else {
        "~same".to_string()
    }
}

fn print_report(grc20: &BenchResult, proto: &BenchResult, json_size: usize, city_count: usize) {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                     GRC-20 vs Proto Benchmark Comparison                     ║");
    println!("╠══════════════════════════════════════════════════════════════════════════════╣");
    println!("║  Dataset: {} cities | JSON size: {:>10}                            ║", city_count, format_size(json_size));
    println!("╠══════════════════════════════════════════════════════════════════════════════╣");
    println!("║  SIZE                                                                        ║");
    println!("║  ┌─────────────────┬─────────────────┬─────────────────┬───────────────────┐ ║");
    println!("║  │                 │     GRC-20      │      Proto      │      Winner       │ ║");
    println!("║  ├─────────────────┼─────────────────┼─────────────────┼───────────────────┤ ║");
    println!("║  │ Uncompressed    │ {:>13}   │ {:>13}   │ {:^17} │ ║",
        format_size(grc20.size_uncompressed),
        format_size(proto.size_uncompressed),
        format_winner(grc20.size_uncompressed as f64, proto.size_uncompressed as f64, false)
    );
    println!("║  │ Compressed      │ {:>13}   │ {:>13}   │ {:^17} │ ║",
        format_size(grc20.size_compressed),
        format_size(proto.size_compressed),
        format_winner(grc20.size_compressed as f64, proto.size_compressed as f64, false)
    );
    println!("║  │ vs JSON         │ {:>12.1}%   │ {:>12.1}%   │                   │ ║",
        100.0 * grc20.size_compressed as f64 / json_size as f64,
        100.0 * proto.size_compressed as f64 / json_size as f64
    );
    println!("║  └─────────────────┴─────────────────┴─────────────────┴───────────────────┘ ║");
    println!("╠──────────────────────────────────────────────────────────────────────────────╣");
    println!("║  ENCODE TIME                                                                 ║");
    println!("║  ┌─────────────────┬─────────────────┬─────────────────┬───────────────────┐ ║");
    println!("║  │                 │     GRC-20      │      Proto      │      Winner       │ ║");
    println!("║  ├─────────────────┼─────────────────┼─────────────────┼───────────────────┤ ║");
    println!("║  │ Uncompressed    │ {:>13}   │ {:>13}   │ {:^17} │ ║",
        format_duration(grc20.encode_time),
        format_duration(proto.encode_time),
        format_winner(grc20.encode_time.as_secs_f64(), proto.encode_time.as_secs_f64(), false)
    );
    println!("║  │ Compressed      │ {:>13}   │ {:>13}   │ {:^17} │ ║",
        format_duration(grc20.compress_time),
        format_duration(proto.compress_time),
        format_winner(grc20.compress_time.as_secs_f64(), proto.compress_time.as_secs_f64(), false)
    );
    println!("║  └─────────────────┴─────────────────┴─────────────────┴───────────────────┘ ║");
    println!("╠──────────────────────────────────────────────────────────────────────────────╣");
    println!("║  DECODE TIME                                                                 ║");
    println!("║  ┌─────────────────┬─────────────────┬─────────────────┬───────────────────┐ ║");
    println!("║  │                 │     GRC-20      │      Proto      │      Winner       │ ║");
    println!("║  ├─────────────────┼─────────────────┼─────────────────┼───────────────────┤ ║");
    println!("║  │ Uncompressed    │ {:>13}   │ {:>13}   │ {:^17} │ ║",
        format_duration(grc20.decode_time),
        format_duration(proto.decode_time),
        format_winner(grc20.decode_time.as_secs_f64(), proto.decode_time.as_secs_f64(), false)
    );
    println!("║  │ Compressed      │ {:>13}   │ {:>13}   │ {:^17} │ ║",
        format_duration(grc20.decode_compressed_time),
        format_duration(proto.decode_compressed_time),
        format_winner(grc20.decode_compressed_time.as_secs_f64(), proto.decode_compressed_time.as_secs_f64(), false)
    );
    println!("║  └─────────────────┴─────────────────┴─────────────────┴───────────────────┘ ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════╝");
    println!();
}

// =============================================================================
// MAIN
// =============================================================================

fn main() {
    let data_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../../../out/cities.json".to_string());

    println!("Loading data from: {}", data_path);

    // Check if file exists, if not try to decompress from data/
    if !Path::new(&data_path).exists() {
        let compressed_path = data_path.replace("/out/", "/data/") + ".gz";
        if Path::new(&compressed_path).exists() {
            println!("Decompressing {} to {}", compressed_path, data_path);
            let compressed = fs::read(&compressed_path).expect("Failed to read compressed file");
            let mut decoder = flate2::read::GzDecoder::new(compressed.as_slice());
            let mut decompressed = String::new();
            std::io::Read::read_to_string(&mut decoder, &mut decompressed)
                .expect("Failed to decompress");
            fs::create_dir_all(Path::new(&data_path).parent().unwrap()).ok();
            fs::write(&data_path, &decompressed).expect("Failed to write decompressed file");
        }
    }

    let json_data = fs::read_to_string(&data_path).expect("Failed to read data file");
    let json_size = json_data.len();

    println!("Parsing JSON...");
    let cities: Vec<City> = serde_json::from_str(&json_data).expect("Failed to parse JSON");
    let city_count = cities.len();
    println!("Loaded {} cities\n", city_count);

    let iterations = 10;

    println!("Running GRC-20 benchmark...");
    let grc20_result = benchmark_grc20(&cities, iterations);

    println!("Running Proto benchmark...");
    let proto_result = benchmark_proto(&cities, iterations);

    print_report(&grc20_result, &proto_result, json_size, city_count);
}
