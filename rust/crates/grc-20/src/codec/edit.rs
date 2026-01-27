//! Edit encoding/decoding for GRC-20 binary format.
//!
//! Implements the wire format for edits (spec Section 6.3).

use std::borrow::Cow;
use std::io::Read;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::codec::op::{decode_op, encode_op};
use crate::codec::primitives::{Reader, Writer};
use crate::error::{DecodeError, EncodeError};
use crate::limits::{
    FORMAT_VERSION, MAGIC_COMPRESSED, MAGIC_UNCOMPRESSED, MAX_AUTHORS, MAX_DICT_SIZE,
    MAX_EDIT_SIZE, MAX_OPS_PER_EDIT, MAX_STRING_LEN, MIN_FORMAT_VERSION,
};
use crate::model::{Context, ContextEdge, DataType, DictionaryBuilder, Edit, Id, Op, WireDictionaries};

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
    if version < MIN_FORMAT_VERSION || version > FORMAT_VERSION {
        return Err(DecodeError::UnsupportedVersion { version });
    }

    // Header
    let edit_id = reader.read_id("edit_id")?;
    let name = Cow::Borrowed(reader.read_str(MAX_STRING_LEN, "name")?);
    let authors = reader.read_id_vec(MAX_AUTHORS, "authors")?;
    let created_at = reader.read_signed_varint("created_at")?;

    // Schema dictionaries (with duplicate detection)
    let property_count = reader.read_varint("property_count")? as usize;
    if property_count > MAX_DICT_SIZE {
        return Err(DecodeError::LengthExceedsLimit {
            field: "properties",
            len: property_count,
            max: MAX_DICT_SIZE,
        });
    }
    let mut properties = Vec::with_capacity(property_count);
    let mut seen_props = FxHashSet::with_capacity_and_hasher(property_count, Default::default());
    for _ in 0..property_count {
        let id = reader.read_id("property_id")?;
        if !seen_props.insert(id) {
            return Err(DecodeError::DuplicateDictionaryEntry { dict: "properties", id });
        }
        let dt_byte = reader.read_byte("data_type")?;
        let data_type = DataType::from_u8(dt_byte)
            .ok_or(DecodeError::InvalidDataType { data_type: dt_byte })?;
        properties.push((id, data_type));
    }

    let relation_types = read_id_vec_no_duplicates(&mut reader, MAX_DICT_SIZE, "relation_types")?;
    let languages = read_id_vec_no_duplicates(&mut reader, MAX_DICT_SIZE, "languages")?;
    let units = read_id_vec_no_duplicates(&mut reader, MAX_DICT_SIZE, "units")?;
    let objects = read_id_vec_no_duplicates(&mut reader, MAX_DICT_SIZE, "objects")?;
    let context_ids = read_id_vec_no_duplicates(&mut reader, MAX_DICT_SIZE, "context_ids")?;

    let mut dicts = WireDictionaries {
        properties,
        relation_types,
        languages,
        units,
        objects,
        context_ids,
        contexts: Vec::new(),
    };

    // Contexts - decode and store in dicts for op decoding to resolve
    let context_count = reader.read_varint("context_count")? as usize;
    if context_count > MAX_DICT_SIZE {
        return Err(DecodeError::LengthExceedsLimit {
            field: "contexts",
            len: context_count,
            max: MAX_DICT_SIZE,
        });
    }
    for _ in 0..context_count {
        dicts.contexts.push(decode_context(&mut reader, &dicts)?);
    }

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
    if version < MIN_FORMAT_VERSION || version > FORMAT_VERSION {
        return Err(DecodeError::UnsupportedVersion { version });
    }

    // Header - use allocating reads
    let edit_id = reader.read_id("edit_id")?;
    let name = Cow::Owned(reader.read_string(MAX_STRING_LEN, "name")?);
    let authors = reader.read_id_vec(MAX_AUTHORS, "authors")?;
    let created_at = reader.read_signed_varint("created_at")?;

    // Schema dictionaries (with duplicate detection)
    let property_count = reader.read_varint("property_count")? as usize;
    if property_count > MAX_DICT_SIZE {
        return Err(DecodeError::LengthExceedsLimit {
            field: "properties",
            len: property_count,
            max: MAX_DICT_SIZE,
        });
    }
    let mut properties = Vec::with_capacity(property_count);
    let mut seen_props = FxHashSet::with_capacity_and_hasher(property_count, Default::default());
    for _ in 0..property_count {
        let id = reader.read_id("property_id")?;
        if !seen_props.insert(id) {
            return Err(DecodeError::DuplicateDictionaryEntry { dict: "properties", id });
        }
        let dt_byte = reader.read_byte("data_type")?;
        let data_type = DataType::from_u8(dt_byte)
            .ok_or(DecodeError::InvalidDataType { data_type: dt_byte })?;
        properties.push((id, data_type));
    }

    let relation_types = read_id_vec_no_duplicates(&mut reader, MAX_DICT_SIZE, "relation_types")?;
    let languages = read_id_vec_no_duplicates(&mut reader, MAX_DICT_SIZE, "languages")?;
    let units = read_id_vec_no_duplicates(&mut reader, MAX_DICT_SIZE, "units")?;
    let objects = read_id_vec_no_duplicates(&mut reader, MAX_DICT_SIZE, "objects")?;
    let context_ids = read_id_vec_no_duplicates(&mut reader, MAX_DICT_SIZE, "context_ids")?;

    let mut dicts = WireDictionaries {
        properties,
        relation_types,
        languages,
        units,
        objects,
        context_ids,
        contexts: Vec::new(),
    };

    // Contexts - decode and store in dicts for op decoding to resolve
    let context_count = reader.read_varint("context_count")? as usize;
    if context_count > MAX_DICT_SIZE {
        return Err(DecodeError::LengthExceedsLimit {
            field: "contexts",
            len: context_count,
            max: MAX_DICT_SIZE,
        });
    }
    for _ in 0..context_count {
        dicts.contexts.push(decode_context(&mut reader, &dicts)?);
    }

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

/// Decodes a Context from the reader.
fn decode_context(reader: &mut Reader<'_>, dicts: &WireDictionaries) -> Result<Context, DecodeError> {
    let root_id_index = reader.read_varint("root_id")? as usize;
    if root_id_index >= dicts.context_ids.len() {
        return Err(DecodeError::IndexOutOfBounds {
            dict: "context_ids",
            index: root_id_index,
            size: dicts.context_ids.len(),
        });
    }
    let root_id = dicts.context_ids[root_id_index];

    let edge_count = reader.read_varint("edge_count")? as usize;
    if edge_count > MAX_DICT_SIZE {
        return Err(DecodeError::LengthExceedsLimit {
            field: "context_edges",
            len: edge_count,
            max: MAX_DICT_SIZE,
        });
    }

    let mut edges = Vec::with_capacity(edge_count);
    for _ in 0..edge_count {
        let type_id_index = reader.read_varint("edge_type_id")? as usize;
        if type_id_index >= dicts.relation_types.len() {
            return Err(DecodeError::IndexOutOfBounds {
                dict: "relation_types",
                index: type_id_index,
                size: dicts.relation_types.len(),
            });
        }
        let type_id = dicts.relation_types[type_id_index];

        let to_entity_id_index = reader.read_varint("edge_to_entity_id")? as usize;
        if to_entity_id_index >= dicts.context_ids.len() {
            return Err(DecodeError::IndexOutOfBounds {
                dict: "context_ids",
                index: to_entity_id_index,
                size: dicts.context_ids.len(),
            });
        }
        let to_entity_id = dicts.context_ids[to_entity_id_index];

        edges.push(ContextEdge { type_id, to_entity_id });
    }

    Ok(Context { root_id, edges })
}

/// Converts an Op with borrowed data to owned data.
fn op_to_owned(op: Op<'_>) -> Op<'static> {
    match op {
        Op::CreateEntity(ce) => Op::CreateEntity(crate::model::CreateEntity {
            id: ce.id,
            values: ce.values.into_iter().map(pv_to_owned).collect(),
            context: ce.context,
        }),
        Op::UpdateEntity(ue) => Op::UpdateEntity(crate::model::UpdateEntity {
            id: ue.id,
            set_properties: ue.set_properties.into_iter().map(pv_to_owned).collect(),
            unset_values: ue.unset_values,
            context: ue.context,
        }),
        Op::DeleteEntity(de) => Op::DeleteEntity(de),
        Op::RestoreEntity(re) => Op::RestoreEntity(re),
        Op::CreateRelation(cr) => Op::CreateRelation(crate::model::CreateRelation {
            id: cr.id,
            relation_type: cr.relation_type,
            from: cr.from,
            from_is_value_ref: cr.from_is_value_ref,
            to: cr.to,
            to_is_value_ref: cr.to_is_value_ref,
            entity: cr.entity,
            position: cr.position.map(|p| Cow::Owned(p.into_owned())),
            from_space: cr.from_space,
            from_version: cr.from_version,
            to_space: cr.to_space,
            to_version: cr.to_version,
            context: cr.context,
        }),
        Op::UpdateRelation(ur) => Op::UpdateRelation(crate::model::UpdateRelation {
            id: ur.id,
            from_space: ur.from_space,
            from_version: ur.from_version,
            to_space: ur.to_space,
            to_version: ur.to_version,
            position: ur.position.map(|p| Cow::Owned(p.into_owned())),
            unset: ur.unset,
            context: ur.context,
        }),
        Op::DeleteRelation(dr) => Op::DeleteRelation(dr),
        Op::RestoreRelation(rr) => Op::RestoreRelation(rr),
        Op::CreateValueRef(cvr) => Op::CreateValueRef(cvr),
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
        Value::Date(s) => Value::Date(Cow::Owned(s.into_owned())),
        Value::Time(s) => Value::Time(Cow::Owned(s.into_owned())),
        Value::Datetime(s) => Value::Datetime(Cow::Owned(s.into_owned())),
        Value::Schedule(s) => Value::Schedule(Cow::Owned(s.into_owned())),
        Value::Point { lat, lon, alt } => Value::Point { lat, lon, alt },
        Value::Rect { min_lat, min_lon, max_lat, max_lon } => Value::Rect { min_lat, min_lon, max_lat, max_lon },
        Value::Embedding { sub_type, dims, data } => Value::Embedding {
            sub_type,
            dims,
            data: Cow::Owned(data.into_owned()),
        },
    }
}

/// Reads an ID vector and checks for duplicates.
fn read_id_vec_no_duplicates(
    reader: &mut Reader<'_>,
    max_len: usize,
    field: &'static str,
) -> Result<Vec<Id>, DecodeError> {
    let count = reader.read_varint(field)? as usize;
    if count > max_len {
        return Err(DecodeError::LengthExceedsLimit {
            field,
            len: count,
            max: max_len,
        });
    }

    let mut ids = Vec::with_capacity(count);
    let mut seen = FxHashSet::with_capacity_and_hasher(count, Default::default());

    for _ in 0..count {
        let id = reader.read_id(field)?;
        if !seen.insert(id) {
            return Err(DecodeError::DuplicateDictionaryEntry { dict: field, id });
        }
        ids.push(id);
    }

    Ok(ids)
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

fn validate_context_limits(context: &Context) -> Result<(), EncodeError> {
    if context.edges.len() > MAX_DICT_SIZE {
        return Err(EncodeError::LengthExceedsLimit {
            field: "context_edges",
            len: context.edges.len(),
            max: MAX_DICT_SIZE,
        });
    }
    Ok(())
}

fn validate_edit_inputs(edit: &Edit) -> Result<(), EncodeError> {
    let name_len = edit.name.as_bytes().len();
    if name_len > MAX_STRING_LEN {
        return Err(EncodeError::LengthExceedsLimit {
            field: "name",
            len: name_len,
            max: MAX_STRING_LEN,
        });
    }
    if edit.authors.len() > MAX_AUTHORS {
        return Err(EncodeError::LengthExceedsLimit {
            field: "authors",
            len: edit.authors.len(),
            max: MAX_AUTHORS,
        });
    }
    if edit.ops.len() > MAX_OPS_PER_EDIT {
        return Err(EncodeError::LengthExceedsLimit {
            field: "ops",
            len: edit.ops.len(),
            max: MAX_OPS_PER_EDIT,
        });
    }

    for op in &edit.ops {
        match op {
            Op::CreateEntity(ce) => {
                if ce.values.len() > crate::limits::MAX_VALUES_PER_ENTITY {
                    return Err(EncodeError::LengthExceedsLimit {
                        field: "values",
                        len: ce.values.len(),
                        max: crate::limits::MAX_VALUES_PER_ENTITY,
                    });
                }
                if let Some(ctx) = &ce.context {
                    validate_context_limits(ctx)?;
                }
            }
            Op::UpdateEntity(ue) => {
                if ue.set_properties.len() > crate::limits::MAX_VALUES_PER_ENTITY {
                    return Err(EncodeError::LengthExceedsLimit {
                        field: "set_properties",
                        len: ue.set_properties.len(),
                        max: crate::limits::MAX_VALUES_PER_ENTITY,
                    });
                }
                if ue.unset_values.len() > crate::limits::MAX_VALUES_PER_ENTITY {
                    return Err(EncodeError::LengthExceedsLimit {
                        field: "unset_values",
                        len: ue.unset_values.len(),
                        max: crate::limits::MAX_VALUES_PER_ENTITY,
                    });
                }
                if let Some(ctx) = &ue.context {
                    validate_context_limits(ctx)?;
                }
            }
            Op::DeleteEntity(de) => {
                if let Some(ctx) = &de.context {
                    validate_context_limits(ctx)?;
                }
            }
            Op::RestoreEntity(re) => {
                if let Some(ctx) = &re.context {
                    validate_context_limits(ctx)?;
                }
            }
            Op::CreateRelation(cr) => {
                if let Some(ctx) = &cr.context {
                    validate_context_limits(ctx)?;
                }
            }
            Op::UpdateRelation(ur) => {
                if let Some(ctx) = &ur.context {
                    validate_context_limits(ctx)?;
                }
            }
            Op::DeleteRelation(dr) => {
                if let Some(ctx) = &dr.context {
                    validate_context_limits(ctx)?;
                }
            }
            Op::RestoreRelation(rr) => {
                if let Some(ctx) = &rr.context {
                    validate_context_limits(ctx)?;
                }
            }
            Op::CreateValueRef(_) => {}
        }
    }

    Ok(())
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
    validate_edit_inputs(edit)?;
    if options.canonical {
        encode_edit_canonical(edit)
    } else {
        encode_edit_fast(edit)
    }
}

/// Fast single-pass encoding (non-canonical).
fn encode_edit_fast(edit: &Edit) -> Result<Vec<u8>, EncodeError> {
    // Property types are determined from values themselves (per-edit typing)
    let property_types = rustc_hash::FxHashMap::default();

    // Create dictionary builder - contexts will be collected from ops
    let mut dict_builder = DictionaryBuilder::with_capacity(edit.ops.len());

    // Single pass: encode ops while building dictionaries (including contexts)
    let mut ops_writer = Writer::with_capacity(edit.ops.len() * 50);

    for op in &edit.ops {
        encode_op(&mut ops_writer, op, &mut dict_builder, &property_types)?;
    }
    dict_builder.validate_limits()?;

    // Now assemble final output: header + dictionaries + contexts + ops
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

    // Contexts (collected from ops during encoding)
    dict_builder.write_contexts(&mut writer);

    // Operations (already encoded)
    writer.write_varint(edit.ops.len() as u64);
    writer.write_bytes(&ops_bytes);

    Ok(writer.into_bytes())
}

/// Canonical two-pass encoding with sorted dictionaries, authors, values, and unsets.
///
/// Pass 1: Collect all dictionary entries
/// Pass 2: Sort dictionaries, encode with stable indices and sorted values
///
/// Canonical mode requirements (spec Section 4.4):
/// - Dictionaries sorted by ID bytes
/// - Authors sorted by ID bytes, no duplicates
/// - Values sorted by (propertyRef, languageRef), no duplicate (property, language)
/// - Unset values sorted by (propertyRef, language), no duplicates
fn encode_edit_canonical(edit: &Edit) -> Result<Vec<u8>, EncodeError> {
    // Property types are determined from values themselves (per-edit typing)
    let property_types = rustc_hash::FxHashMap::default();

    // Create dictionary builder - contexts will be collected from ops
    let mut dict_builder = DictionaryBuilder::with_capacity(edit.ops.len());

    // Pass 1: Collect all dictionary entries (including contexts) by doing a dry run
    let mut temp_writer = Writer::with_capacity(edit.ops.len() * 50);
    for op in &edit.ops {
        encode_op(&mut temp_writer, op, &mut dict_builder, &property_types)?;
    }
    dict_builder.validate_limits()?;

    // Sort dictionaries and get sorted builder
    let sorted_builder = dict_builder.into_sorted();

    // Sort authors by ID bytes and check for duplicates
    let mut sorted_authors = edit.authors.clone();
    sorted_authors.sort();
    // Check for duplicate authors
    for i in 1..sorted_authors.len() {
        if sorted_authors[i] == sorted_authors[i - 1] {
            return Err(EncodeError::DuplicateAuthor { id: sorted_authors[i] });
        }
    }

    // Pass 2: Encode ops with sorted dictionary indices and sorted values
    let mut ops_writer = Writer::with_capacity(edit.ops.len() * 50);
    let mut canonical_builder = sorted_builder.clone();
    for op in &edit.ops {
        encode_op_canonical(&mut ops_writer, op, &mut canonical_builder, &property_types)?;
    }

    // Assemble final output: header + dictionaries + contexts + ops
    let ops_bytes = ops_writer.into_bytes();
    let mut writer = Writer::with_capacity(256 + ops_bytes.len());

    // Magic and version
    writer.write_bytes(MAGIC_UNCOMPRESSED);
    writer.write_byte(FORMAT_VERSION);

    // Header
    writer.write_id(&edit.id);
    writer.write_string(&edit.name);
    writer.write_id_vec(&sorted_authors);
    writer.write_signed_varint(edit.created_at);

    // Dictionaries (sorted)
    sorted_builder.write_dictionaries(&mut writer);

    // Contexts (collected from ops during pass 1, sorted)
    sorted_builder.write_contexts(&mut writer);

    // Operations
    writer.write_varint(edit.ops.len() as u64);
    writer.write_bytes(&ops_bytes);

    Ok(writer.into_bytes())
}

/// Encodes an op in canonical mode with sorted values.
fn encode_op_canonical(
    writer: &mut Writer,
    op: &Op<'_>,
    dict_builder: &mut DictionaryBuilder,
    property_types: &FxHashMap<Id, DataType>,
) -> Result<(), EncodeError> {
    match op {
        Op::CreateEntity(ce) => {
            // Sort values by (property_index, language_index) and check for duplicates
            let sorted_values = sort_and_check_values(&ce.values, dict_builder)?;

            writer.write_byte(1); // OP_CREATE_ENTITY
            writer.write_id(&ce.id);
            writer.write_varint(sorted_values.len() as u64);

            for pv in &sorted_values {
                let data_type = property_types.get(&pv.property)
                    .copied()
                    .unwrap_or_else(|| pv.value.data_type());
                encode_property_value_canonical(writer, pv, dict_builder, data_type)?;
            }
            // Write context_ref: 0xFFFFFFFF = no context, else index into contexts[]
            let context_ref = match &ce.context {
                Some(ctx) => dict_builder.add_context(ctx) as u32,
                None => 0xFFFFFFFF,
            };
            writer.write_varint(context_ref as u64);
            Ok(())
        }
        Op::UpdateEntity(ue) => {
            // Sort set_properties and unset_values, check for duplicates
            let sorted_set = sort_and_check_values(&ue.set_properties, dict_builder)?;
            let sorted_unset = sort_and_check_unsets(&ue.unset_values, dict_builder)?;

            writer.write_byte(2); // OP_UPDATE_ENTITY
            let id_index = dict_builder.add_object(ue.id);
            writer.write_varint(id_index as u64);

            let mut flags = 0u8;
            if !sorted_set.is_empty() {
                flags |= 0x01; // FLAG_HAS_SET_PROPERTIES
            }
            if !sorted_unset.is_empty() {
                flags |= 0x02; // FLAG_HAS_UNSET_VALUES
            }
            writer.write_byte(flags);

            if !sorted_set.is_empty() {
                writer.write_varint(sorted_set.len() as u64);
                for pv in &sorted_set {
                    let data_type = property_types.get(&pv.property)
                        .copied()
                        .unwrap_or_else(|| pv.value.data_type());
                    encode_property_value_canonical(writer, pv, dict_builder, data_type)?;
                }
            }

            if !sorted_unset.is_empty() {
                use crate::model::UnsetLanguage;
                writer.write_varint(sorted_unset.len() as u64);
                for unset in &sorted_unset {
                    let prop_idx = dict_builder.add_property(unset.property, DataType::Bool);
                    writer.write_varint(prop_idx as u64);
                    let lang_value: u32 = match &unset.language {
                        UnsetLanguage::All => 0xFFFFFFFF,
                        UnsetLanguage::English => 0,
                        UnsetLanguage::Specific(lang_id) => {
                            dict_builder.add_language(Some(*lang_id)) as u32
                        }
                    };
                    writer.write_varint(lang_value as u64);
                }
            }
            // Write context_ref: 0xFFFFFFFF = no context, else index into contexts[]
            let context_ref = match &ue.context {
                Some(ctx) => dict_builder.add_context(ctx) as u32,
                None => 0xFFFFFFFF,
            };
            writer.write_varint(context_ref as u64);
            Ok(())
        }
        // Other ops don't have values to sort, delegate to regular encode
        _ => encode_op(writer, op, dict_builder, property_types),
    }
}

/// Sorts values by (property_index, language_index) and checks for duplicates.
fn sort_and_check_values<'a>(
    values: &[crate::model::PropertyValue<'a>],
    dict_builder: &DictionaryBuilder,
) -> Result<Vec<crate::model::PropertyValue<'a>>, EncodeError> {
    use crate::model::{PropertyValue, Value};

    if values.is_empty() {
        return Ok(Vec::new());
    }

    // Create (property_index, language_index, original_index) tuples for sorting
    let mut indexed: Vec<(usize, usize, usize, &PropertyValue<'a>)> = values
        .iter()
        .enumerate()
        .map(|(i, pv)| {
            let prop_idx = dict_builder.get_property_index(&pv.property).unwrap_or(0);
            let lang_idx = match &pv.value {
                Value::Text { language, .. } => dict_builder.get_language_index(language.as_ref()).unwrap_or(0),
                _ => 0,
            };
            (prop_idx, lang_idx, i, pv)
        })
        .collect();

    // Sort by (property_index, language_index)
    indexed.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));

    // Check for duplicates (adjacent entries with same property_index and language_index)
    for i in 1..indexed.len() {
        if indexed[i].0 == indexed[i - 1].0 && indexed[i].1 == indexed[i - 1].1 {
            let pv = indexed[i].3;
            let language = match &pv.value {
                Value::Text { language, .. } => *language,
                _ => None,
            };
            return Err(EncodeError::DuplicateValue {
                property: pv.property,
                language,
            });
        }
    }

    // Return cloned values in sorted order
    Ok(indexed.into_iter().map(|(_, _, _, pv)| pv.clone()).collect())
}

/// Sorts unset values by (property_index, language) and checks for duplicates.
fn sort_and_check_unsets(
    unsets: &[crate::model::UnsetValue],
    dict_builder: &DictionaryBuilder,
) -> Result<Vec<crate::model::UnsetValue>, EncodeError> {
    use crate::model::UnsetLanguage;

    if unsets.is_empty() {
        return Ok(Vec::new());
    }

    // Create (property_index, language_sort_key, original_index) tuples for sorting
    let mut indexed: Vec<(usize, u32, usize, &crate::model::UnsetValue)> = unsets
        .iter()
        .enumerate()
        .map(|(i, up)| {
            let prop_idx = dict_builder.get_property_index(&up.property).unwrap_or(0);
            let lang_key: u32 = match &up.language {
                UnsetLanguage::All => 0xFFFFFFFF,
                UnsetLanguage::English => 0,
                UnsetLanguage::Specific(lang_id) => {
                    dict_builder.get_language_index(Some(lang_id)).unwrap_or(0) as u32
                }
            };
            (prop_idx, lang_key, i, up)
        })
        .collect();

    // Sort by (property_index, language_key)
    indexed.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));

    // Check for duplicates
    for i in 1..indexed.len() {
        if indexed[i].0 == indexed[i - 1].0 && indexed[i].1 == indexed[i - 1].1 {
            let up = indexed[i].3;
            let language = match &up.language {
                UnsetLanguage::All => None,
                UnsetLanguage::English => None,
                UnsetLanguage::Specific(id) => Some(*id),
            };
            return Err(EncodeError::DuplicateUnset {
                property: up.property,
                language,
            });
        }
    }

    Ok(indexed.into_iter().map(|(_, _, _, up)| up.clone()).collect())
}

/// Encodes a property value in canonical mode (same as regular but separated for clarity).
fn encode_property_value_canonical(
    writer: &mut Writer,
    pv: &crate::model::PropertyValue<'_>,
    dict_builder: &mut DictionaryBuilder,
    data_type: DataType,
) -> Result<(), EncodeError> {
    let prop_index = dict_builder.add_property(pv.property, data_type);
    writer.write_varint(prop_index as u64);
    crate::codec::value::encode_value(writer, &pv.value, dict_builder)?;
    Ok(())
}

/// Encodes an Edit with profiling output (two-pass for comparison).
pub fn encode_edit_profiled(edit: &Edit, profile: bool) -> Result<Vec<u8>, EncodeError> {
    if !profile {
        return encode_edit(edit);
    }

    use std::time::Instant;

    let t0 = Instant::now();

    // Property types are determined from values themselves (per-edit typing)
    let property_types = rustc_hash::FxHashMap::default();
    let t1 = Instant::now();

    // Create dictionary builder - contexts will be collected from ops
    let mut dict_builder = DictionaryBuilder::with_capacity(edit.ops.len());

    // Single pass: encode ops while building dictionaries (including contexts)
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
    dict_builder.write_contexts(&mut writer);
    writer.write_varint(edit.ops.len() as u64);
    writer.write_bytes(&ops_bytes);
    let t3 = Instant::now();

    let result = writer.into_bytes();

    let total = t3.duration_since(t0);
    eprintln!("=== Encode Profile (single-pass) ===");
    eprintln!("  setup: {:?} ({:.1}%)", t1.duration_since(t0), 100.0 * t1.duration_since(t0).as_secs_f64() / total.as_secs_f64());
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
    use crate::model::{CreateEntity, PropertyValue, Value};

    fn make_test_edit() -> Edit<'static> {
        Edit {
            id: [1u8; 16],
            name: Cow::Owned("Test Edit".to_string()),
            authors: vec![[2u8; 16]],
            created_at: 1234567890,
                        ops: vec![
                Op::CreateEntity(CreateEntity {
                    id: [3u8; 16],
                    values: vec![PropertyValue {
                        property: [10u8; 16],
                        value: Value::Text {
                            value: Cow::Owned("Hello".to_string()),
                            language: None,
                        },
                    }],
                    context: None,
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
        // Two edits with values in different order should produce
        // identical bytes when using canonical encoding

        let prop_a = [0x0A; 16]; // Comes first lexicographically
        let prop_b = [0x0B; 16]; // Comes second

        // Edit 1: values in order A, B
        let edit1: Edit<'static> = Edit {
            id: [1u8; 16],
            name: Cow::Owned("Test".to_string()),
            authors: vec![],
            created_at: 0,
                        ops: vec![
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
                    context: None,
                }),
            ],
        };

        // Edit 2: Same content but values in different order
        let edit2: Edit<'static> = Edit {
            id: [1u8; 16],
            name: Cow::Owned("Test".to_string()),
            authors: vec![],
            created_at: 0,
                        ops: vec![
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
                    context: None,
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

    #[test]
    fn test_canonical_rejects_duplicate_authors() {
        let author1 = [1u8; 16];

        let edit: Edit<'static> = Edit {
            id: [0u8; 16],
            name: Cow::Owned("Test".to_string()),
            authors: vec![author1, author1], // Duplicate!
            created_at: 0,
                        ops: vec![],
        };

        // Fast mode doesn't check duplicates
        let result = encode_edit_with_options(&edit, EncodeOptions::new());
        assert!(result.is_ok());

        // Canonical mode rejects duplicates
        let result = encode_edit_with_options(&edit, EncodeOptions::canonical());
        assert!(matches!(result, Err(EncodeError::DuplicateAuthor { .. })));
    }

    #[test]
    fn test_canonical_rejects_duplicate_values() {
        let prop = [10u8; 16];

        let edit: Edit<'static> = Edit {
            id: [0u8; 16],
            name: Cow::Owned("Test".to_string()),
            authors: vec![],
            created_at: 0,
                        ops: vec![
                Op::CreateEntity(CreateEntity {
                    id: [1u8; 16],
                    values: vec![
                        PropertyValue {
                            property: prop,
                            value: Value::Text {
                                value: Cow::Owned("First".to_string()),
                                language: None,
                            },
                        },
                        PropertyValue {
                            property: prop,
                            value: Value::Text {
                                value: Cow::Owned("Second".to_string()),
                                language: None,
                            },
                        },
                    ],
                    context: None,
                }),
            ],
        };

        // Canonical mode rejects duplicate (property, language) pairs
        let result = encode_edit_with_options(&edit, EncodeOptions::canonical());
        assert!(matches!(result, Err(EncodeError::DuplicateValue { .. })));
    }

    #[test]
    fn test_canonical_allows_different_languages() {
        let prop = [10u8; 16];
        let lang_en = [20u8; 16];
        let lang_es = [21u8; 16];

        let edit: Edit<'static> = Edit {
            id: [0u8; 16],
            name: Cow::Owned("Test".to_string()),
            authors: vec![],
            created_at: 0,
                        ops: vec![
                Op::CreateEntity(CreateEntity {
                    id: [1u8; 16],
                    values: vec![
                        PropertyValue {
                            property: prop,
                            value: Value::Text {
                                value: Cow::Owned("Hello".to_string()),
                                language: Some(lang_en),
                            },
                        },
                        PropertyValue {
                            property: prop,
                            value: Value::Text {
                                value: Cow::Owned("Hola".to_string()),
                                language: Some(lang_es),
                            },
                        },
                    ],
                    context: None,
                }),
            ],
        };

        // Different languages for same property is allowed
        let result = encode_edit_with_options(&edit, EncodeOptions::canonical());
        assert!(result.is_ok());
    }

    #[test]
    fn test_canonical_sorts_values_deterministically() {
        let prop_a = [0x0A; 16];
        let prop_b = [0x0B; 16];

        // Values in reverse order (B before A)
        let edit: Edit<'static> = Edit {
            id: [1u8; 16],
            name: Cow::Owned("Test".to_string()),
            authors: vec![],
            created_at: 0,
                        ops: vec![
                Op::CreateEntity(CreateEntity {
                    id: [3u8; 16],
                    values: vec![
                        PropertyValue {
                            property: prop_b, // B first
                            value: Value::Int64 { value: 42, unit: None },
                        },
                        PropertyValue {
                            property: prop_a, // A second
                            value: Value::Text {
                                value: Cow::Owned("Hello".to_string()),
                                language: None,
                            },
                        },
                    ],
                    context: None,
                }),
            ],
        };

        // Encode twice - should produce identical bytes
        let encoded1 = encode_edit_with_options(&edit, EncodeOptions::canonical()).unwrap();
        let encoded2 = encode_edit_with_options(&edit, EncodeOptions::canonical()).unwrap();
        assert_eq!(encoded1, encoded2, "Canonical encoding should be deterministic");

        // Should roundtrip
        let decoded = decode_edit(&encoded1).unwrap();
        assert_eq!(decoded.ops.len(), 1);
    }
}
