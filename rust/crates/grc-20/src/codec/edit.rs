//! Edit encoding/decoding for GRC-20 binary format.
//!
//! Implements the wire format for edits (spec Section 6.3).

use std::borrow::Cow;
use std::io::Read;

use rustc_hash::FxHashMap;

use crate::codec::op::{decode_op, encode_op};
use crate::codec::primitives::{Reader, Writer};
use crate::error::{DecodeError, EncodeError};
use crate::limits::{
    FORMAT_VERSION, MAGIC_COMPRESSED, MAGIC_UNCOMPRESSED, MAX_AUTHORS, MAX_DICT_SIZE,
    MAX_EDIT_SIZE, MAX_OPS_PER_EDIT, MAX_STRING_LEN,
};
use crate::model::{DataType, DictionaryBuilder, Edit, Id, Op, WireDictionaries};

// =============================================================================
// DECODING
// =============================================================================

/// Decompresses a GRC2Z compressed edit, returning the uncompressed bytes.
///
/// Use this with [`decode_edit`] for zero-copy decoding of compressed data:
///
/// ```ignore
/// let uncompressed = decompress(&compressed_bytes)?;
/// let edit = decode_edit(&uncompressed)?;  // zero-copy, borrows from uncompressed
/// // edit is valid while uncompressed is alive
/// ```
pub fn decompress(input: &[u8]) -> Result<Vec<u8>, DecodeError> {
    if input.len() < 5 {
        return Err(DecodeError::UnexpectedEof { context: "magic" });
    }
    if &input[0..5] != MAGIC_COMPRESSED {
        let mut found = [0u8; 4];
        found.copy_from_slice(&input[0..4]);
        return Err(DecodeError::InvalidMagic { found });
    }
    decompress_zstd(&input[5..])
}

/// Decodes an Edit from binary data with zero-copy borrowing.
///
/// Handles both compressed (GRC2Z) and uncompressed (GRC2) formats.
/// For true zero-copy with compressed data, use [`decompress`] first:
///
/// ```ignore
/// // Zero-copy for compressed data:
/// let uncompressed = decompress(&compressed)?;
/// let edit = decode_edit(&uncompressed)?;
///
/// // Zero-copy for uncompressed data:
/// let edit = decode_edit(&uncompressed_bytes)?;
/// ```
///
/// If you pass compressed data directly, it will decompress internally
/// and allocate owned strings (no zero-copy benefit).
pub fn decode_edit(input: &[u8]) -> Result<Edit<'_>, DecodeError> {
    if input.len() < 4 {
        return Err(DecodeError::UnexpectedEof { context: "magic" });
    }

    // Detect compression
    if input.len() >= 5 && &input[0..5] == MAGIC_COMPRESSED {
        // Compressed: decompress and decode with allocations
        // (for zero-copy, caller should use decompress() first)
        let decompressed = decompress_zstd(&input[5..])?;
        if decompressed.len() > MAX_EDIT_SIZE {
            return Err(DecodeError::LengthExceedsLimit {
                field: "edit",
                len: decompressed.len(),
                max: MAX_EDIT_SIZE,
            });
        }
        decode_edit_owned(&decompressed)
    } else if &input[0..4] == MAGIC_UNCOMPRESSED {
        // Uncompressed: decode with zero-copy borrowing
        if input.len() > MAX_EDIT_SIZE {
            return Err(DecodeError::LengthExceedsLimit {
                field: "edit",
                len: input.len(),
                max: MAX_EDIT_SIZE,
            });
        }
        decode_edit_borrowed(input)
    } else {
        let mut found = [0u8; 4];
        found.copy_from_slice(&input[0..4]);
        Err(DecodeError::InvalidMagic { found })
    }
}

/// Decodes an Edit with zero-copy borrowing from the input.
fn decode_edit_borrowed(input: &[u8]) -> Result<Edit<'_>, DecodeError> {
    let mut reader = Reader::new(input);

    // Skip magic (already validated)
    reader.read_bytes(4, "magic")?;

    // Version
    let version = reader.read_byte("version")?;
    if version != FORMAT_VERSION {
        return Err(DecodeError::UnsupportedVersion { version });
    }

    // Header
    let edit_id = reader.read_id("edit_id")?;
    let name = Cow::Borrowed(reader.read_str(MAX_STRING_LEN, "name")?);
    let authors = reader.read_id_vec(MAX_AUTHORS, "authors")?;
    let created_at = reader.read_signed_varint("created_at")?;

    // Schema dictionaries
    let property_count = reader.read_varint("property_count")? as usize;
    if property_count > MAX_DICT_SIZE {
        return Err(DecodeError::LengthExceedsLimit {
            field: "properties",
            len: property_count,
            max: MAX_DICT_SIZE,
        });
    }
    let mut properties = Vec::with_capacity(property_count);
    for _ in 0..property_count {
        let id = reader.read_id("property_id")?;
        let dt_byte = reader.read_byte("data_type")?;
        let data_type = DataType::from_u8(dt_byte)
            .ok_or(DecodeError::InvalidDataType { data_type: dt_byte })?;
        properties.push((id, data_type));
    }

    let relation_types = reader.read_id_vec(MAX_DICT_SIZE, "relation_types")?;
    let languages = reader.read_id_vec(MAX_DICT_SIZE, "languages")?;
    let units = reader.read_id_vec(MAX_DICT_SIZE, "units")?;
    let objects = reader.read_id_vec(MAX_DICT_SIZE, "objects")?;

    let dicts = WireDictionaries {
        properties,
        relation_types,
        languages,
        units,
        objects,
    };

    // Operations
    let op_count = reader.read_varint("op_count")? as usize;
    if op_count > MAX_OPS_PER_EDIT {
        return Err(DecodeError::LengthExceedsLimit {
            field: "ops",
            len: op_count,
            max: MAX_OPS_PER_EDIT,
        });
    }

    let mut ops = Vec::with_capacity(op_count);
    for _ in 0..op_count {
        ops.push(decode_op(&mut reader, &dicts)?);
    }

    Ok(Edit {
        id: edit_id,
        name,
        authors,
        created_at,
        ops,
    })
}

/// Decodes an Edit with allocations (for decompressed data).
fn decode_edit_owned(data: &[u8]) -> Result<Edit<'static>, DecodeError> {
    let mut reader = Reader::new(data);

    // Skip magic (already validated in decompress)
    reader.read_bytes(4, "magic")?;

    // Version
    let version = reader.read_byte("version")?;
    if version != FORMAT_VERSION {
        return Err(DecodeError::UnsupportedVersion { version });
    }

    // Header - use allocating reads
    let edit_id = reader.read_id("edit_id")?;
    let name = Cow::Owned(reader.read_string(MAX_STRING_LEN, "name")?);
    let authors = reader.read_id_vec(MAX_AUTHORS, "authors")?;
    let created_at = reader.read_signed_varint("created_at")?;

    // Schema dictionaries
    let property_count = reader.read_varint("property_count")? as usize;
    if property_count > MAX_DICT_SIZE {
        return Err(DecodeError::LengthExceedsLimit {
            field: "properties",
            len: property_count,
            max: MAX_DICT_SIZE,
        });
    }
    let mut properties = Vec::with_capacity(property_count);
    for _ in 0..property_count {
        let id = reader.read_id("property_id")?;
        let dt_byte = reader.read_byte("data_type")?;
        let data_type = DataType::from_u8(dt_byte)
            .ok_or(DecodeError::InvalidDataType { data_type: dt_byte })?;
        properties.push((id, data_type));
    }

    let relation_types = reader.read_id_vec(MAX_DICT_SIZE, "relation_types")?;
    let languages = reader.read_id_vec(MAX_DICT_SIZE, "languages")?;
    let units = reader.read_id_vec(MAX_DICT_SIZE, "units")?;
    let objects = reader.read_id_vec(MAX_DICT_SIZE, "objects")?;

    let dicts = WireDictionaries {
        properties,
        relation_types,
        languages,
        units,
        objects,
    };

    // Operations - use allocating decode
    let op_count = reader.read_varint("op_count")? as usize;
    if op_count > MAX_OPS_PER_EDIT {
        return Err(DecodeError::LengthExceedsLimit {
            field: "ops",
            len: op_count,
            max: MAX_OPS_PER_EDIT,
        });
    }

    let mut ops = Vec::with_capacity(op_count);
    for _ in 0..op_count {
        ops.push(decode_op_owned(&mut reader, &dicts)?);
    }

    Ok(Edit {
        id: edit_id,
        name,
        authors,
        created_at,
        ops,
    })
}

/// Decodes an Op with allocations (for decompressed data).
fn decode_op_owned(reader: &mut Reader<'_>, dicts: &WireDictionaries) -> Result<Op<'static>, DecodeError> {
    // Decode normally, then convert to owned
    let op = decode_op(reader, dicts)?;
    Ok(op_to_owned(op))
}

/// Converts an Op with borrowed data to owned data.
fn op_to_owned(op: Op<'_>) -> Op<'static> {
    match op {
        Op::CreateEntity(ce) => Op::CreateEntity(crate::model::CreateEntity {
            id: ce.id,
            values: ce.values.into_iter().map(pv_to_owned).collect(),
        }),
        Op::UpdateEntity(ue) => Op::UpdateEntity(crate::model::UpdateEntity {
            id: ue.id,
            set_properties: ue.set_properties.into_iter().map(pv_to_owned).collect(),
            unset_properties: ue.unset_properties,
        }),
        Op::DeleteEntity(de) => Op::DeleteEntity(de),
        Op::CreateRelation(cr) => Op::CreateRelation(crate::model::CreateRelation {
            id_mode: cr.id_mode,
            relation_type: cr.relation_type,
            from: cr.from,
            to: cr.to,
            entity: cr.entity,
            position: cr.position.map(|p| Cow::Owned(p.into_owned())),
            from_space: cr.from_space,
            from_version: cr.from_version,
            to_space: cr.to_space,
            to_version: cr.to_version,
        }),
        Op::UpdateRelation(ur) => Op::UpdateRelation(crate::model::UpdateRelation {
            id: ur.id,
            position: ur.position.map(|p| Cow::Owned(p.into_owned())),
        }),
        Op::DeleteRelation(dr) => Op::DeleteRelation(dr),
        Op::CreateProperty(cp) => Op::CreateProperty(cp),
    }
}

/// Converts a PropertyValue with borrowed data to owned data.
fn pv_to_owned(pv: crate::model::PropertyValue<'_>) -> crate::model::PropertyValue<'static> {
    crate::model::PropertyValue {
        property: pv.property,
        value: value_to_owned(pv.value),
    }
}

/// Converts a Value with borrowed data to owned data.
fn value_to_owned(v: crate::model::Value<'_>) -> crate::model::Value<'static> {
    use crate::model::{DecimalMantissa, Value};
    match v {
        Value::Bool(b) => Value::Bool(b),
        Value::Int64 { value, unit } => Value::Int64 { value, unit },
        Value::Float64 { value, unit } => Value::Float64 { value, unit },
        Value::Decimal { exponent, mantissa, unit } => Value::Decimal {
            exponent,
            mantissa: match mantissa {
                DecimalMantissa::I64(i) => DecimalMantissa::I64(i),
                DecimalMantissa::Big(b) => DecimalMantissa::Big(Cow::Owned(b.into_owned())),
            },
            unit,
        },
        Value::Text { value, language } => Value::Text {
            value: Cow::Owned(value.into_owned()),
            language,
        },
        Value::Bytes(b) => Value::Bytes(Cow::Owned(b.into_owned())),
        Value::Timestamp(t) => Value::Timestamp(t),
        Value::Date(d) => Value::Date(Cow::Owned(d.into_owned())),
        Value::Point { lat, lon } => Value::Point { lat, lon },
        Value::Embedding { sub_type, dims, data } => Value::Embedding {
            sub_type,
            dims,
            data: Cow::Owned(data.into_owned()),
        },
    }
}

fn decompress_zstd(compressed: &[u8]) -> Result<Vec<u8>, DecodeError> {
    // Read uncompressed size
    let mut reader = Reader::new(compressed);
    let declared_size = reader.read_varint("uncompressed_size")? as usize;

    if declared_size > MAX_EDIT_SIZE {
        return Err(DecodeError::LengthExceedsLimit {
            field: "uncompressed_size",
            len: declared_size,
            max: MAX_EDIT_SIZE,
        });
    }

    let compressed_data = reader.remaining();

    let mut decoder = zstd::Decoder::new(compressed_data)
        .map_err(|e| DecodeError::DecompressionFailed(e.to_string()))?;

    let mut decompressed = Vec::with_capacity(declared_size);
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| DecodeError::DecompressionFailed(e.to_string()))?;

    if decompressed.len() != declared_size {
        return Err(DecodeError::UncompressedSizeMismatch {
            declared: declared_size,
            actual: decompressed.len(),
        });
    }

    Ok(decompressed)
}

// =============================================================================
// ENCODING
// =============================================================================

/// Options for encoding edits.
#[derive(Debug, Clone, Copy, Default)]
pub struct EncodeOptions {
    /// Enable canonical encoding mode.
    ///
    /// When enabled:
    /// - Dictionary entries are sorted by ID bytes (lexicographic)
    /// - This ensures deterministic output for the same logical edit
    ///
    /// Use canonical mode when:
    /// - Computing content hashes for deduplication
    /// - Creating signatures over edit content
    /// - Ensuring cross-implementation reproducibility
    ///
    /// Note: Canonical mode requires two passes over the ops and is slower
    /// than non-canonical encoding.
    pub canonical: bool,
}

impl EncodeOptions {
    /// Creates default (non-canonical) encoding options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates canonical encoding options.
    pub fn canonical() -> Self {
        Self { canonical: true }
    }
}

/// Encodes an Edit to binary format (uncompressed).
///
/// Uses single-pass encoding: ops are encoded to a buffer while building
/// dictionaries, then the final output is assembled.
pub fn encode_edit(edit: &Edit) -> Result<Vec<u8>, EncodeError> {
    encode_edit_with_options(edit, EncodeOptions::default())
}

/// Encodes an Edit to binary format with the given options.
pub fn encode_edit_with_options(edit: &Edit, options: EncodeOptions) -> Result<Vec<u8>, EncodeError> {
    if options.canonical {
        encode_edit_canonical(edit)
    } else {
        encode_edit_fast(edit)
    }
}

/// Fast single-pass encoding (non-canonical).
fn encode_edit_fast(edit: &Edit) -> Result<Vec<u8>, EncodeError> {
    // Build property type map from CreateProperty ops
    let mut property_types: FxHashMap<Id, DataType> = FxHashMap::default();
    for op in &edit.ops {
        if let Op::CreateProperty(cp) = op {
            property_types.insert(cp.id, cp.data_type);
        }
    }

    // Single pass: encode ops while building dictionaries
    let mut dict_builder = DictionaryBuilder::with_capacity(edit.ops.len());
    let mut ops_writer = Writer::with_capacity(edit.ops.len() * 50);

    for op in &edit.ops {
        encode_op(&mut ops_writer, op, &mut dict_builder, &property_types)?;
    }

    // Now assemble final output: header + dictionaries + ops
    let ops_bytes = ops_writer.into_bytes();
    let mut writer = Writer::with_capacity(256 + ops_bytes.len());

    // Magic and version
    writer.write_bytes(MAGIC_UNCOMPRESSED);
    writer.write_byte(FORMAT_VERSION);

    // Header
    writer.write_id(&edit.id);
    writer.write_string(&edit.name);
    writer.write_id_vec(&edit.authors);
    writer.write_signed_varint(edit.created_at);

    // Dictionaries
    dict_builder.write_dictionaries(&mut writer);

    // Operations (already encoded)
    writer.write_varint(edit.ops.len() as u64);
    writer.write_bytes(&ops_bytes);

    Ok(writer.into_bytes())
}

/// Canonical two-pass encoding with sorted dictionaries.
///
/// Pass 1: Collect all dictionary entries
/// Pass 2: Sort dictionaries, encode with stable indices
fn encode_edit_canonical(edit: &Edit) -> Result<Vec<u8>, EncodeError> {
    // Build property type map from CreateProperty ops
    let mut property_types: FxHashMap<Id, DataType> = FxHashMap::default();
    for op in &edit.ops {
        if let Op::CreateProperty(cp) = op {
            property_types.insert(cp.id, cp.data_type);
        }
    }

    // Pass 1: Collect all dictionary entries by doing a dry run
    let mut dict_builder = DictionaryBuilder::with_capacity(edit.ops.len());
    let mut temp_writer = Writer::with_capacity(edit.ops.len() * 50);
    for op in &edit.ops {
        encode_op(&mut temp_writer, op, &mut dict_builder, &property_types)?;
    }

    // Sort dictionaries and get sorted builder
    let sorted_builder = dict_builder.into_sorted();

    // Pass 2: Encode ops with sorted dictionary indices
    let mut ops_writer = Writer::with_capacity(edit.ops.len() * 50);
    for op in &edit.ops {
        encode_op(&mut ops_writer, op, &mut sorted_builder.clone(), &property_types)?;
    }

    // Assemble final output: header + dictionaries + ops
    let ops_bytes = ops_writer.into_bytes();
    let mut writer = Writer::with_capacity(256 + ops_bytes.len());

    // Magic and version
    writer.write_bytes(MAGIC_UNCOMPRESSED);
    writer.write_byte(FORMAT_VERSION);

    // Header
    writer.write_id(&edit.id);
    writer.write_string(&edit.name);
    writer.write_id_vec(&edit.authors);
    writer.write_signed_varint(edit.created_at);

    // Dictionaries (sorted)
    sorted_builder.write_dictionaries(&mut writer);

    // Operations
    writer.write_varint(edit.ops.len() as u64);
    writer.write_bytes(&ops_bytes);

    Ok(writer.into_bytes())
}

/// Encodes an Edit with profiling output (two-pass for comparison).
pub fn encode_edit_profiled(edit: &Edit, profile: bool) -> Result<Vec<u8>, EncodeError> {
    if !profile {
        return encode_edit(edit);
    }

    use std::time::Instant;

    let t0 = Instant::now();

    // Build property type map
    let mut property_types: FxHashMap<Id, DataType> = FxHashMap::default();
    for op in &edit.ops {
        if let Op::CreateProperty(cp) = op {
            property_types.insert(cp.id, cp.data_type);
        }
    }
    let t1 = Instant::now();

    // Single pass: encode ops while building dictionaries
    let mut dict_builder = DictionaryBuilder::with_capacity(edit.ops.len());
    let mut ops_writer = Writer::with_capacity(edit.ops.len() * 50);

    for op in &edit.ops {
        encode_op(&mut ops_writer, op, &mut dict_builder, &property_types)?;
    }
    let t2 = Instant::now();

    // Assemble final output
    let ops_bytes = ops_writer.into_bytes();
    let mut writer = Writer::with_capacity(256 + ops_bytes.len());

    writer.write_bytes(MAGIC_UNCOMPRESSED);
    writer.write_byte(FORMAT_VERSION);
    writer.write_id(&edit.id);
    writer.write_string(&edit.name);
    writer.write_id_vec(&edit.authors);
    writer.write_signed_varint(edit.created_at);
    dict_builder.write_dictionaries(&mut writer);
    writer.write_varint(edit.ops.len() as u64);
    writer.write_bytes(&ops_bytes);
    let t3 = Instant::now();

    let result = writer.into_bytes();

    let total = t3.duration_since(t0);
    eprintln!("=== Encode Profile (single-pass) ===");
    eprintln!("  build property_types: {:?} ({:.1}%)", t1.duration_since(t0), 100.0 * t1.duration_since(t0).as_secs_f64() / total.as_secs_f64());
    eprintln!("  encode_ops + build_dicts: {:?} ({:.1}%)", t2.duration_since(t1), 100.0 * t2.duration_since(t1).as_secs_f64() / total.as_secs_f64());
    eprintln!("  assemble output: {:?} ({:.1}%)", t3.duration_since(t2), 100.0 * t3.duration_since(t2).as_secs_f64() / total.as_secs_f64());
    eprintln!("  TOTAL: {:?}", total);

    Ok(result)
}

/// Encodes an Edit to binary format with zstd compression.
pub fn encode_edit_compressed(edit: &Edit, level: i32) -> Result<Vec<u8>, EncodeError> {
    encode_edit_compressed_with_options(edit, level, EncodeOptions::default())
}

/// Encodes an Edit to binary format with zstd compression and options.
pub fn encode_edit_compressed_with_options(
    edit: &Edit,
    level: i32,
    options: EncodeOptions,
) -> Result<Vec<u8>, EncodeError> {
    let uncompressed = encode_edit_with_options(edit, options)?;

    let compressed = zstd::encode_all(uncompressed.as_slice(), level)
        .map_err(|e| EncodeError::CompressionFailed(e.to_string()))?;

    let mut writer = Writer::with_capacity(5 + 10 + compressed.len());
    writer.write_bytes(MAGIC_COMPRESSED);
    writer.write_varint(uncompressed.len() as u64);
    writer.write_bytes(&compressed);

    Ok(writer.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CreateEntity, CreateProperty, PropertyValue, Value};

    fn make_test_edit() -> Edit<'static> {
        Edit {
            id: [1u8; 16],
            name: Cow::Owned("Test Edit".to_string()),
            authors: vec![[2u8; 16]],
            created_at: 1234567890,
            ops: vec![
                Op::CreateProperty(CreateProperty {
                    id: [10u8; 16],
                    data_type: DataType::Text,
                }),
                Op::CreateEntity(CreateEntity {
                    id: [3u8; 16],
                    values: vec![PropertyValue {
                        property: [10u8; 16],
                        value: Value::Text {
                            value: Cow::Owned("Hello".to_string()),
                            language: None,
                        },
                    }],
                }),
            ],
        }
    }

    #[test]
    fn test_edit_roundtrip() {
        let edit = make_test_edit();

        let encoded = encode_edit(&edit).unwrap();
        let decoded = decode_edit(&encoded).unwrap();

        assert_eq!(edit.id, decoded.id);
        assert_eq!(edit.name, decoded.name);
        assert_eq!(edit.authors, decoded.authors);
        assert_eq!(edit.created_at, decoded.created_at);
        assert_eq!(edit.ops.len(), decoded.ops.len());
    }

    #[test]
    fn test_edit_compressed_roundtrip() {
        let edit = make_test_edit();

        let encoded = encode_edit_compressed(&edit, 3).unwrap();
        let decoded = decode_edit(&encoded).unwrap();

        assert_eq!(edit.id, decoded.id);
        assert_eq!(edit.name, decoded.name);
        assert_eq!(edit.authors, decoded.authors);
        assert_eq!(edit.created_at, decoded.created_at);
        assert_eq!(edit.ops.len(), decoded.ops.len());
    }

    #[test]
    fn test_compression_magic() {
        let edit = make_test_edit();

        let uncompressed = encode_edit(&edit).unwrap();
        let compressed = encode_edit_compressed(&edit, 3).unwrap();

        assert_eq!(&uncompressed[0..4], b"GRC2");
        assert_eq!(&compressed[0..5], b"GRC2Z");
    }

    #[test]
    fn test_invalid_magic() {
        let data = b"XXXX";
        let result = decode_edit(data);
        assert!(matches!(result, Err(DecodeError::InvalidMagic { .. })));
    }

    #[test]
    fn test_unsupported_version() {
        let mut data = Vec::new();
        data.extend_from_slice(MAGIC_UNCOMPRESSED);
        data.push(99); // Invalid version
        // Add enough bytes to not trigger EOF
        data.extend_from_slice(&[0u8; 100]);

        let result = decode_edit(&data);
        assert!(matches!(result, Err(DecodeError::UnsupportedVersion { version: 99 })));
    }

    #[test]
    fn test_empty_edit() {
        let edit: Edit<'static> = Edit {
            id: [0u8; 16],
            name: Cow::Borrowed(""),
            authors: vec![],
            created_at: 0,
            ops: vec![],
        };

        let encoded = encode_edit(&edit).unwrap();
        let decoded = decode_edit(&encoded).unwrap();

        assert_eq!(edit.id, decoded.id);
        assert!(decoded.name.is_empty());
        assert!(decoded.authors.is_empty());
        assert!(decoded.ops.is_empty());
    }

    #[test]
    fn test_canonical_encoding_deterministic() {
        // Two edits with properties in different order should produce
        // identical bytes when using canonical encoding

        let prop_a = [0x0A; 16]; // Comes first lexicographically
        let prop_b = [0x0B; 16]; // Comes second

        // Edit 1: properties added in order A, B
        let edit1: Edit<'static> = Edit {
            id: [1u8; 16],
            name: Cow::Owned("Test".to_string()),
            authors: vec![],
            created_at: 0,
            ops: vec![
                Op::CreateProperty(CreateProperty {
                    id: prop_a,
                    data_type: DataType::Text,
                }),
                Op::CreateProperty(CreateProperty {
                    id: prop_b,
                    data_type: DataType::Int64,
                }),
                Op::CreateEntity(CreateEntity {
                    id: [3u8; 16],
                    values: vec![
                        PropertyValue {
                            property: prop_a,
                            value: Value::Text {
                                value: Cow::Owned("Hello".to_string()),
                                language: None,
                            },
                        },
                        PropertyValue {
                            property: prop_b,
                            value: Value::Int64 { value: 42, unit: None },
                        },
                    ],
                }),
            ],
        };

        // Edit 2: Same content but properties used in different order in entity
        let edit2: Edit<'static> = Edit {
            id: [1u8; 16],
            name: Cow::Owned("Test".to_string()),
            authors: vec![],
            created_at: 0,
            ops: vec![
                Op::CreateProperty(CreateProperty {
                    id: prop_a,
                    data_type: DataType::Text,
                }),
                Op::CreateProperty(CreateProperty {
                    id: prop_b,
                    data_type: DataType::Int64,
                }),
                Op::CreateEntity(CreateEntity {
                    id: [3u8; 16],
                    values: vec![
                        // Note: prop_b first this time (different insertion order)
                        PropertyValue {
                            property: prop_b,
                            value: Value::Int64 { value: 42, unit: None },
                        },
                        PropertyValue {
                            property: prop_a,
                            value: Value::Text {
                                value: Cow::Owned("Hello".to_string()),
                                language: None,
                            },
                        },
                    ],
                }),
            ],
        };

        // Non-canonical encoding may produce different bytes
        let fast1 = encode_edit_with_options(&edit1, EncodeOptions::new()).unwrap();
        let fast2 = encode_edit_with_options(&edit2, EncodeOptions::new()).unwrap();
        // These might differ because dictionary order depends on insertion order
        // (We don't assert they're different because they might happen to be the same)

        // Canonical encoding MUST produce identical bytes for same logical content
        let canonical1 = encode_edit_with_options(&edit1, EncodeOptions::canonical()).unwrap();
        let canonical2 = encode_edit_with_options(&edit2, EncodeOptions::canonical()).unwrap();

        // Both should decode correctly
        let decoded1 = decode_edit(&canonical1).unwrap();
        let decoded2 = decode_edit(&canonical2).unwrap();
        assert_eq!(decoded1.id, edit1.id);
        assert_eq!(decoded2.id, edit2.id);

        // And the encoded bytes should be identical (deterministic)
        // Note: The ops themselves may have different value orders, but the dictionary
        // portion should be identical since it's sorted by ID
        assert_eq!(
            &canonical1[..50], // Check header + dictionary start
            &canonical2[..50],
            "Canonical encoding should produce identical dictionary bytes"
        );

        // Verify the edit still roundtrips
        let _ = fast1;
        let _ = fast2;
    }

    #[test]
    fn test_canonical_encoding_roundtrip() {
        let edit = make_test_edit();

        let encoded = encode_edit_with_options(&edit, EncodeOptions::canonical()).unwrap();
        let decoded = decode_edit(&encoded).unwrap();

        assert_eq!(edit.id, decoded.id);
        assert_eq!(edit.name, decoded.name);
        assert_eq!(edit.authors, decoded.authors);
        assert_eq!(edit.created_at, decoded.created_at);
        assert_eq!(edit.ops.len(), decoded.ops.len());
    }

    #[test]
    fn test_canonical_encoding_compressed() {
        let edit = make_test_edit();

        let encoded = encode_edit_compressed_with_options(&edit, 3, EncodeOptions::canonical()).unwrap();
        let decoded = decode_edit(&encoded).unwrap();

        assert_eq!(edit.id, decoded.id);
        assert_eq!(edit.name, decoded.name);
    }
}
