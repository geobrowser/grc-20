//! Benchmark for GRC-20 serialization using city data.
//!
//! Demonstrates the builder API with a large dataset (153k cities).

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::Instant;

use grc_20::{
    EditBuilder, EncodeOptions, EntityBuilder, Id, Op, derived_uuid,
};

/// Creates a deterministic relation ID from from+to+type (to maintain same behavior as removed unique mode).
fn make_relation_id(from: Id, to: Id, rel_type: Id) -> Id {
    let mut input = [0u8; 48];
    input[0..16].copy_from_slice(&from);
    input[16..32].copy_from_slice(&to);
    input[32..48].copy_from_slice(&rel_type);
    derived_uuid(&input)
}
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

/// Property IDs
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

/// Type IDs
mod types {
    use super::hex;

    pub const CITY: [u8; 16] = hex("b1b2c3d4e5f6071829304050a1b2c3d4");
    pub const STATE: [u8; 16] = hex("b1b2c3d4e5f6071829304050a1b2c3d5");
    pub const COUNTRY: [u8; 16] = hex("b1b2c3d4e5f6071829304050a1b2c3d6");
}

/// Relation type IDs
mod rel_types {
    use super::hex;

    pub const TYPES: [u8; 16] = hex("c1b2c3d4e5f6071829304050a1b2c3d4");
    pub const IN_STATE: [u8; 16] = hex("c1b2c3d4e5f6071829304050a1b2c3d5");
    pub const IN_COUNTRY: [u8; 16] = hex("c1b2c3d4e5f6071829304050a1b2c3d6");
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

// =============================================================================
// ID GENERATION
// =============================================================================

const PREFIX_CITY: u8 = 0x01;
const PREFIX_STATE: u8 = 0x02;
const PREFIX_COUNTRY: u8 = 0x03;

fn make_entity_id(prefix: u8, id: u32) -> [u8; 16] {
    let mut uuid = [0u8; 16];
    uuid[0] = prefix;
    uuid[12..16].copy_from_slice(&id.to_be_bytes());
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

// =============================================================================
// CONVERSION TO GRC-20 USING BUILDER API
// =============================================================================

fn build_city_entity<'a>(city: &'a City) -> EntityBuilder<'a> {
    let mut builder = EntityBuilder::new()
        .text(props::NAME, city.name.as_str(), None);

    // Native name
    if let Some(ref native) = city.native {
        if !native.is_empty() {
            builder = builder.text(props::NATIVE_NAME, native.as_str(), None);
        }
    }

    // City type
    if let Some(ref city_type) = city.city_type {
        builder = builder.text(props::CITY_TYPE, city_type.as_str(), None);
    }

    // Population
    if let Some(pop) = city.population {
        builder = builder.int64(props::POPULATION, pop, None);
    }

    // Location
    if let (Ok(lat), Ok(lon)) = (city.latitude.parse::<f64>(), city.longitude.parse::<f64>()) {
        builder = builder.point(props::LOCATION, lon, lat, None);
    }

    // Timezone
    if let Some(ref tz) = city.timezone {
        builder = builder.text(props::TIMEZONE, tz.as_str(), None);
    }

    // Wikidata ID
    if let Some(ref wiki_id) = city.wikidata_id {
        builder = builder.text(props::WIKIDATA_ID, wiki_id.as_str(), None);
    }

    // Translations (multi-value TEXT with language)
    if let Some(ref translations) = city.translations {
        for (lang_code, translation) in translations {
            if let Some(lang_id) = get_language_id(lang_code) {
                builder = builder.text(props::NAME, translation.as_str(), Some(lang_id));
            }
        }
    }

    builder
}

/// Convert cities to a GRC-20 Edit, borrowing strings from the input (zero-copy).
fn convert_cities_to_edit<'a>(cities: &'a [City]) -> grc_20::Edit<'a> {
    let edit_id = make_entity_id(0xFF, 1);
    let author_id = make_entity_id(0xAA, 1);

    let mut builder = EditBuilder::new(edit_id)
        .name("Cities Import")
        .author(author_id)
        .created_at(1704067200_000_000);

    // Create type entities
    builder = builder
        .create_entity(types::CITY, |e| e.text(props::NAME, "City", None))
        .create_entity(types::STATE, |e| e.text(props::NAME, "State", None))
        .create_entity(types::COUNTRY, |e| e.text(props::NAME, "Country", None));

    // Track created states and countries for deduplication
    let mut created_states: HashSet<u32> = HashSet::new();
    let mut created_countries: HashSet<u32> = HashSet::new();

    for city in cities {
        let city_id = make_entity_id(PREFIX_CITY, city.id);
        let state_id = make_entity_id(PREFIX_STATE, city.state_id);
        let country_id = make_entity_id(PREFIX_COUNTRY, city.country_id);

        // Ensure country exists
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

        // Ensure state exists
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

        // Create city entity using the builder
        builder = builder
            .create_entity(city_id, |_| build_city_entity(city))
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

    builder.build()
}

fn main() {
    // Find the data file (look in out/ directory)
    let data_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../../../out/cities.json".to_string());

    println!("Loading cities from: {}", data_path);

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

    let json_data = fs::read_to_string(&data_path).expect("Failed to read cities.json");

    let parse_start = Instant::now();
    let cities: Vec<City> = serde_json::from_str(&json_data).expect("Failed to parse JSON");
    let parse_time = parse_start.elapsed();

    println!("Loaded {} cities in {:?}", cities.len(), parse_time);

    // Convert to GRC-20 using builder API
    let convert_start = Instant::now();
    let edit = convert_cities_to_edit(&cities);
    let convert_time = convert_start.elapsed();

    // Count statistics
    let mut entity_count = 0;
    let mut relation_count = 0;
    let mut total_values = 0;
    for op in &edit.ops {
        match op {
            Op::CreateEntity(e) => {
                entity_count += 1;
                total_values += e.values.len();
            }
            Op::CreateRelation(_) => relation_count += 1,
            _ => {}
        }
    }

    println!(
        "Converted to {} operations in {:?}",
        edit.ops.len(),
        convert_time
    );
    println!(
        "  - {} entities, {} relations, {} total values",
        entity_count, relation_count, total_values
    );

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

    // Verify canonical encoding is deterministic
    let canonical_encoded2 = grc_20::encode_edit_with_options(&edit, EncodeOptions::canonical())
        .expect("Failed to encode canonical");
    assert_eq!(
        canonical_encoded, canonical_encoded2,
        "Canonical encoding should be deterministic"
    );

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

    // Benchmark decoding (uncompressed)
    const DECODE_ITERS: u32 = 10; // Fewer iterations due to larger data

    // Warmup
    for _ in 0..3 {
        let _ = grc_20::decode_edit(&encoded).expect("Failed to decode");
    }

    let decode_start = Instant::now();
    let mut decoded = None;
    for _ in 0..DECODE_ITERS {
        decoded = Some(grc_20::decode_edit(&encoded).expect("Failed to decode"));
    }
    let decode_time = decode_start.elapsed() / DECODE_ITERS;
    let decoded = decoded.unwrap();

    println!(
        "\nDecode (uncompressed): {:?} (avg of {} iterations)",
        decode_time, DECODE_ITERS
    );
    println!(
        "  Throughput: {:.2} MB/s",
        (encoded.len() as f64 / 1_000_000.0) / decode_time.as_secs_f64()
    );
    assert_eq!(decoded.ops.len(), edit.ops.len());

    // Benchmark decoding (compressed, allocating)
    for _ in 0..3 {
        let _ = grc_20::decode_edit(&compressed).expect("Failed to decode compressed");
    }

    let decode_compressed_start = Instant::now();
    let mut decoded_compressed = None;
    for _ in 0..DECODE_ITERS {
        decoded_compressed =
            Some(grc_20::decode_edit(&compressed).expect("Failed to decode compressed"));
    }
    let decode_compressed_time = decode_compressed_start.elapsed() / DECODE_ITERS;
    let decoded_compressed = decoded_compressed.unwrap();

    println!(
        "\nDecode (compressed, allocating): {:?} (avg of {} iterations)",
        decode_compressed_time, DECODE_ITERS
    );
    println!(
        "  Throughput: {:.2} MB/s (uncompressed equivalent)",
        (encoded.len() as f64 / 1_000_000.0) / decode_compressed_time.as_secs_f64()
    );
    assert_eq!(decoded_compressed.ops.len(), edit.ops.len());

    // Benchmark decoding (compressed, zero-copy)
    for _ in 0..3 {
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

    println!(
        "\nDecode (compressed, zero-copy): {:?} (avg of {} iterations)",
        decode_zc_time, DECODE_ITERS
    );
    println!(
        "  Throughput: {:.2} MB/s (uncompressed equivalent)",
        (encoded.len() as f64 / 1_000_000.0) / decode_zc_time.as_secs_f64()
    );
    println!(
        "  Speedup vs allocating: {:.1}%",
        100.0 * (decode_compressed_time.as_secs_f64() - decode_zc_time.as_secs_f64())
            / decode_compressed_time.as_secs_f64()
    );

    // Write output files
    let input_path = Path::new(&data_path);
    let stem = input_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
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
    println!("Cities: {}", cities.len());
    println!("Total operations: {}", edit.ops.len());
    println!(
        "JSON size: {} bytes ({:.1} MB)",
        json_data.len(),
        json_data.len() as f64 / 1_000_000.0
    );
    println!(
        "GRC-20 uncompressed: {} bytes ({:.1} MB)",
        encoded.len(),
        encoded.len() as f64 / 1_000_000.0
    );
    println!(
        "GRC-20 compressed: {} bytes ({:.1} MB)",
        compressed.len(),
        compressed.len() as f64 / 1_000_000.0
    );
    println!(
        "Size vs JSON: {:.1}% (uncompressed), {:.1}% (compressed)",
        100.0 * encoded.len() as f64 / json_data.len() as f64,
        100.0 * compressed.len() as f64 / json_data.len() as f64
    );
}
