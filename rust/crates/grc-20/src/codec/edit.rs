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

/// Decodes an Edit from binary data.
///
/// Automatically detects and handles zstd compression (GRC2Z magic).
pub fn decode_edit(input: &[u8]) -> Result<Edit, DecodeError> {
    if input.len() < 4 {
        return Err(DecodeError::UnexpectedEof { context: "magic" });
    }

    // Detect compression
    let data: Cow<[u8]> = if input.len() >= 5 && &input[0..5] == MAGIC_COMPRESSED {
        let decompressed = decompress_zstd(&input[5..])?;
        if decompressed.len() > MAX_EDIT_SIZE {
            return Err(DecodeError::LengthExceedsLimit {
                field: "edit",
                len: decompressed.len(),
                max: MAX_EDIT_SIZE,
            });
        }
        Cow::Owned(decompressed)
    } else if &input[0..4] == MAGIC_UNCOMPRESSED {
        if input.len() > MAX_EDIT_SIZE {
            return Err(DecodeError::LengthExceedsLimit {
                field: "edit",
                len: input.len(),
                max: MAX_EDIT_SIZE,
            });
        }
        Cow::Borrowed(input)
    } else {
        let mut found = [0u8; 4];
        found.copy_from_slice(&input[0..4]);
        return Err(DecodeError::InvalidMagic { found });
    };

    let mut reader = Reader::new(&data);

    // Skip magic (already validated)
    reader.read_bytes(4, "magic")?;

    // Version
    let version = reader.read_byte("version")?;
    if version != FORMAT_VERSION {
        return Err(DecodeError::UnsupportedVersion { version });
    }

    // Header
    let edit_id = reader.read_id("edit_id")?;
    let name = reader.read_string(MAX_STRING_LEN, "name")?;
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
    let objects = reader.read_id_vec(MAX_DICT_SIZE, "objects")?;

    let dicts = WireDictionaries {
        properties,
        relation_types,
        languages,
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

/// Encodes an Edit to binary format (uncompressed).
///
/// Uses single-pass encoding: ops are encoded to a buffer while building
/// dictionaries, then the final output is assembled.
pub fn encode_edit(edit: &Edit) -> Result<Vec<u8>, EncodeError> {
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
    let uncompressed = encode_edit(edit)?;

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

    fn make_test_edit() -> Edit {
        Edit {
            id: [1u8; 16],
            name: "Test Edit".to_string(),
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
                            value: "Hello".to_string(),
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
        let edit = Edit {
            id: [0u8; 16],
            name: String::new(),
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
}
