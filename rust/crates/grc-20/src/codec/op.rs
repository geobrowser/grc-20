//! Operation encoding/decoding for GRC-20 binary format.
//!
//! Implements the wire format for operations (spec Section 6.4).

use crate::codec::primitives::{Reader, Writer};
use crate::codec::value::{decode_position, decode_property_value, validate_position};
use crate::error::{DecodeError, EncodeError};
use crate::limits::MAX_VALUES_PER_ENTITY;
use crate::model::{
    CreateEntity, CreateRelation, CreateValueRef, DataType, DeleteEntity, DeleteRelation,
    DictionaryBuilder, Op, PropertyValue, RestoreEntity, RestoreRelation,
    UnsetLanguage, UnsetValue, UnsetRelationField, UpdateEntity, UpdateRelation, WireDictionaries,
};

// Op type constants (grouped by lifecycle: Create, Update, Delete, Restore)
const OP_CREATE_ENTITY: u8 = 1;
const OP_UPDATE_ENTITY: u8 = 2;
const OP_DELETE_ENTITY: u8 = 3;
const OP_RESTORE_ENTITY: u8 = 4;
const OP_CREATE_RELATION: u8 = 5;
const OP_UPDATE_RELATION: u8 = 6;
const OP_DELETE_RELATION: u8 = 7;
const OP_RESTORE_RELATION: u8 = 8;
const OP_CREATE_VALUE_REF: u8 = 9;

// UpdateEntity flags
const FLAG_HAS_SET_PROPERTIES: u8 = 0x01;
const FLAG_HAS_UNSET_VALUES: u8 = 0x02;
const UPDATE_ENTITY_RESERVED_MASK: u8 = 0xFC;

// CreateRelation flags (bit order matches field order in spec Section 6.4)
const FLAG_HAS_FROM_SPACE: u8 = 0x01;
const FLAG_HAS_FROM_VERSION: u8 = 0x02;
const FLAG_HAS_TO_SPACE: u8 = 0x04;
const FLAG_HAS_TO_VERSION: u8 = 0x08;
const FLAG_HAS_ENTITY: u8 = 0x10;
const FLAG_HAS_POSITION: u8 = 0x20;
const FLAG_FROM_IS_VALUE_REF: u8 = 0x40;
const FLAG_TO_IS_VALUE_REF: u8 = 0x80;

// CreateValueRef flags
const FLAG_HAS_LANGUAGE: u8 = 0x01;
const FLAG_HAS_SPACE: u8 = 0x02;
const CREATE_VALUE_REF_RESERVED_MASK: u8 = 0xFC;

// UpdateRelation set flags (bit order matches field order in spec Section 6.4)
const UPDATE_SET_FROM_SPACE: u8 = 0x01;
const UPDATE_SET_FROM_VERSION: u8 = 0x02;
const UPDATE_SET_TO_SPACE: u8 = 0x04;
const UPDATE_SET_TO_VERSION: u8 = 0x08;
const UPDATE_SET_POSITION: u8 = 0x10;
const UPDATE_SET_RESERVED_MASK: u8 = 0xE0;

// UpdateRelation unset flags
const UPDATE_UNSET_FROM_SPACE: u8 = 0x01;
const UPDATE_UNSET_FROM_VERSION: u8 = 0x02;
const UPDATE_UNSET_TO_SPACE: u8 = 0x04;
const UPDATE_UNSET_TO_VERSION: u8 = 0x08;
const UPDATE_UNSET_POSITION: u8 = 0x10;
const UPDATE_UNSET_RESERVED_MASK: u8 = 0xE0;

// =============================================================================
// DECODING
// =============================================================================

/// Decodes an Op from the reader (zero-copy).
pub fn decode_op<'a>(reader: &mut Reader<'a>, dicts: &WireDictionaries) -> Result<Op<'a>, DecodeError> {
    let op_type = reader.read_byte("op_type")?;

    match op_type {
        OP_CREATE_ENTITY => decode_create_entity(reader, dicts),
        OP_UPDATE_ENTITY => decode_update_entity(reader, dicts),
        OP_DELETE_ENTITY => decode_delete_entity(reader, dicts),
        OP_RESTORE_ENTITY => decode_restore_entity(reader, dicts),
        OP_CREATE_RELATION => decode_create_relation(reader, dicts),
        OP_UPDATE_RELATION => decode_update_relation(reader, dicts),
        OP_DELETE_RELATION => decode_delete_relation(reader, dicts),
        OP_RESTORE_RELATION => decode_restore_relation(reader, dicts),
        OP_CREATE_VALUE_REF => decode_create_value_ref(reader, dicts),
        _ => Err(DecodeError::InvalidOpType { op_type }),
    }
}

fn decode_create_entity<'a>(
    reader: &mut Reader<'a>,
    dicts: &WireDictionaries,
) -> Result<Op<'a>, DecodeError> {
    let id = reader.read_id("entity_id")?;
    let value_count = reader.read_varint("value_count")? as usize;

    if value_count > MAX_VALUES_PER_ENTITY {
        return Err(DecodeError::LengthExceedsLimit {
            field: "values",
            len: value_count,
            max: MAX_VALUES_PER_ENTITY,
        });
    }

    let mut values = Vec::with_capacity(value_count);
    for _ in 0..value_count {
        values.push(decode_property_value(reader, dicts)?);
    }

    Ok(Op::CreateEntity(CreateEntity { id, values }))
}

fn decode_update_entity<'a>(
    reader: &mut Reader<'a>,
    dicts: &WireDictionaries,
) -> Result<Op<'a>, DecodeError> {
    let id_index = reader.read_varint("entity_id")? as usize;
    if id_index >= dicts.objects.len() {
        return Err(DecodeError::IndexOutOfBounds {
            dict: "objects",
            index: id_index,
            size: dicts.objects.len(),
        });
    }
    let id = dicts.objects[id_index];

    let flags = reader.read_byte("update_flags")?;

    // Check reserved bits
    if flags & UPDATE_ENTITY_RESERVED_MASK != 0 {
        return Err(DecodeError::ReservedBitsSet {
            context: "UpdateEntity flags",
        });
    }

    let mut update = UpdateEntity::new(id);

    if flags & FLAG_HAS_SET_PROPERTIES != 0 {
        let count = reader.read_varint("set_properties_count")? as usize;
        if count > MAX_VALUES_PER_ENTITY {
            return Err(DecodeError::LengthExceedsLimit {
                field: "set_properties",
                len: count,
                max: MAX_VALUES_PER_ENTITY,
            });
        }
        for _ in 0..count {
            update.set_properties.push(decode_property_value(reader, dicts)?);
        }
    }

    if flags & FLAG_HAS_UNSET_VALUES != 0 {
        let count = reader.read_varint("unset_values_count")? as usize;
        if count > MAX_VALUES_PER_ENTITY {
            return Err(DecodeError::LengthExceedsLimit {
                field: "unset_values",
                len: count,
                max: MAX_VALUES_PER_ENTITY,
            });
        }
        for _ in 0..count {
            let prop_index = reader.read_varint("property")? as usize;
            if prop_index >= dicts.properties.len() {
                return Err(DecodeError::IndexOutOfBounds {
                    dict: "properties",
                    index: prop_index,
                    size: dicts.properties.len(),
                });
            }
            let property = dicts.properties[prop_index].0;

            // Language encoding: 0xFFFFFFFF = all, 0 = non-linguistic, 1+ = specific language
            let lang_value = reader.read_varint("unset.language")? as u32;
            let language = if lang_value == 0xFFFFFFFF {
                UnsetLanguage::All
            } else if lang_value == 0 {
                UnsetLanguage::NonLinguistic
            } else {
                let idx = (lang_value - 1) as usize;
                if idx >= dicts.languages.len() {
                    return Err(DecodeError::IndexOutOfBounds {
                        dict: "languages",
                        index: lang_value as usize,
                        size: dicts.languages.len() + 1,
                    });
                }
                UnsetLanguage::Specific(dicts.languages[idx])
            };

            update.unset_values.push(UnsetValue { property, language });
        }
    }

    Ok(Op::UpdateEntity(update))
}

fn decode_delete_entity<'a>(
    reader: &mut Reader<'a>,
    dicts: &WireDictionaries,
) -> Result<Op<'a>, DecodeError> {
    let id_index = reader.read_varint("entity_id")? as usize;
    if id_index >= dicts.objects.len() {
        return Err(DecodeError::IndexOutOfBounds {
            dict: "objects",
            index: id_index,
            size: dicts.objects.len(),
        });
    }
    let id = dicts.objects[id_index];
    Ok(Op::DeleteEntity(DeleteEntity { id }))
}

fn decode_restore_entity<'a>(
    reader: &mut Reader<'a>,
    dicts: &WireDictionaries,
) -> Result<Op<'a>, DecodeError> {
    let id_index = reader.read_varint("entity_id")? as usize;
    if id_index >= dicts.objects.len() {
        return Err(DecodeError::IndexOutOfBounds {
            dict: "objects",
            index: id_index,
            size: dicts.objects.len(),
        });
    }
    let id = dicts.objects[id_index];
    Ok(Op::RestoreEntity(RestoreEntity { id }))
}

fn decode_create_relation<'a>(
    reader: &mut Reader<'a>,
    dicts: &WireDictionaries,
) -> Result<Op<'a>, DecodeError> {
    let id = reader.read_id("relation_id")?;

    let type_index = reader.read_varint("relation_type")? as usize;
    if type_index >= dicts.relation_types.len() {
        return Err(DecodeError::IndexOutOfBounds {
            dict: "relation_types",
            index: type_index,
            size: dicts.relation_types.len(),
        });
    }
    let relation_type = dicts.relation_types[type_index];

    let flags = reader.read_byte("relation_flags")?;
    let from_is_value_ref = flags & FLAG_FROM_IS_VALUE_REF != 0;
    let to_is_value_ref = flags & FLAG_TO_IS_VALUE_REF != 0;

    // Read from endpoint: inline ID if value ref, otherwise ObjectRef
    let from = if from_is_value_ref {
        reader.read_id("from")?
    } else {
        let from_index = reader.read_varint("from")? as usize;
        if from_index >= dicts.objects.len() {
            return Err(DecodeError::IndexOutOfBounds {
                dict: "objects",
                index: from_index,
                size: dicts.objects.len(),
            });
        }
        dicts.objects[from_index]
    };

    // Read to endpoint: inline ID if value ref, otherwise ObjectRef
    let to = if to_is_value_ref {
        reader.read_id("to")?
    } else {
        let to_index = reader.read_varint("to")? as usize;
        if to_index >= dicts.objects.len() {
            return Err(DecodeError::IndexOutOfBounds {
                dict: "objects",
                index: to_index,
                size: dicts.objects.len(),
            });
        }
        dicts.objects[to_index]
    };

    // Read optional fields in spec order: from_space, from_version, to_space, to_version, entity, position
    let from_space = if flags & FLAG_HAS_FROM_SPACE != 0 {
        Some(reader.read_id("from_space")?)
    } else {
        None
    };

    let from_version = if flags & FLAG_HAS_FROM_VERSION != 0 {
        Some(reader.read_id("from_version")?)
    } else {
        None
    };

    let to_space = if flags & FLAG_HAS_TO_SPACE != 0 {
        Some(reader.read_id("to_space")?)
    } else {
        None
    };

    let to_version = if flags & FLAG_HAS_TO_VERSION != 0 {
        Some(reader.read_id("to_version")?)
    } else {
        None
    };

    let entity = if flags & FLAG_HAS_ENTITY != 0 {
        Some(reader.read_id("entity_id")?)
    } else {
        None
    };

    let position = if flags & FLAG_HAS_POSITION != 0 {
        Some(decode_position(reader)?)
    } else {
        None
    };

    Ok(Op::CreateRelation(CreateRelation {
        id,
        relation_type,
        from,
        from_is_value_ref,
        to,
        to_is_value_ref,
        entity,
        position,
        from_space,
        from_version,
        to_space,
        to_version,
    }))
}

fn decode_update_relation<'a>(
    reader: &mut Reader<'a>,
    dicts: &WireDictionaries,
) -> Result<Op<'a>, DecodeError> {
    let id_index = reader.read_varint("relation_id")? as usize;
    if id_index >= dicts.objects.len() {
        return Err(DecodeError::IndexOutOfBounds {
            dict: "objects",
            index: id_index,
            size: dicts.objects.len(),
        });
    }
    let id = dicts.objects[id_index];

    let set_flags = reader.read_byte("set_flags")?;
    let unset_flags = reader.read_byte("unset_flags")?;

    // Check reserved bits
    if set_flags & UPDATE_SET_RESERVED_MASK != 0 {
        return Err(DecodeError::ReservedBitsSet {
            context: "UpdateRelation set_flags",
        });
    }
    if unset_flags & UPDATE_UNSET_RESERVED_MASK != 0 {
        return Err(DecodeError::ReservedBitsSet {
            context: "UpdateRelation unset_flags",
        });
    }

    // Read set fields
    let from_space = if set_flags & UPDATE_SET_FROM_SPACE != 0 {
        Some(reader.read_id("from_space")?)
    } else {
        None
    };

    let from_version = if set_flags & UPDATE_SET_FROM_VERSION != 0 {
        Some(reader.read_id("from_version")?)
    } else {
        None
    };

    let to_space = if set_flags & UPDATE_SET_TO_SPACE != 0 {
        Some(reader.read_id("to_space")?)
    } else {
        None
    };

    let to_version = if set_flags & UPDATE_SET_TO_VERSION != 0 {
        Some(reader.read_id("to_version")?)
    } else {
        None
    };

    let position = if set_flags & UPDATE_SET_POSITION != 0 {
        Some(decode_position(reader)?)
    } else {
        None
    };

    // Build unset list
    let mut unset = Vec::new();
    if unset_flags & UPDATE_UNSET_FROM_SPACE != 0 {
        unset.push(UnsetRelationField::FromSpace);
    }
    if unset_flags & UPDATE_UNSET_FROM_VERSION != 0 {
        unset.push(UnsetRelationField::FromVersion);
    }
    if unset_flags & UPDATE_UNSET_TO_SPACE != 0 {
        unset.push(UnsetRelationField::ToSpace);
    }
    if unset_flags & UPDATE_UNSET_TO_VERSION != 0 {
        unset.push(UnsetRelationField::ToVersion);
    }
    if unset_flags & UPDATE_UNSET_POSITION != 0 {
        unset.push(UnsetRelationField::Position);
    }

    Ok(Op::UpdateRelation(UpdateRelation {
        id,
        from_space,
        from_version,
        to_space,
        to_version,
        position,
        unset,
    }))
}

fn decode_delete_relation<'a>(
    reader: &mut Reader<'a>,
    dicts: &WireDictionaries,
) -> Result<Op<'a>, DecodeError> {
    let id_index = reader.read_varint("relation_id")? as usize;
    if id_index >= dicts.objects.len() {
        return Err(DecodeError::IndexOutOfBounds {
            dict: "objects",
            index: id_index,
            size: dicts.objects.len(),
        });
    }
    let id = dicts.objects[id_index];
    Ok(Op::DeleteRelation(DeleteRelation { id }))
}

fn decode_restore_relation<'a>(
    reader: &mut Reader<'a>,
    dicts: &WireDictionaries,
) -> Result<Op<'a>, DecodeError> {
    let id_index = reader.read_varint("relation_id")? as usize;
    if id_index >= dicts.objects.len() {
        return Err(DecodeError::IndexOutOfBounds {
            dict: "objects",
            index: id_index,
            size: dicts.objects.len(),
        });
    }
    let id = dicts.objects[id_index];
    Ok(Op::RestoreRelation(RestoreRelation { id }))
}

fn decode_create_value_ref<'a>(
    reader: &mut Reader<'a>,
    dicts: &WireDictionaries,
) -> Result<Op<'a>, DecodeError> {
    let id = reader.read_id("value_ref_id")?;

    let entity_index = reader.read_varint("entity")? as usize;
    if entity_index >= dicts.objects.len() {
        return Err(DecodeError::IndexOutOfBounds {
            dict: "objects",
            index: entity_index,
            size: dicts.objects.len(),
        });
    }
    let entity = dicts.objects[entity_index];

    let property_index = reader.read_varint("property")? as usize;
    if property_index >= dicts.properties.len() {
        return Err(DecodeError::IndexOutOfBounds {
            dict: "properties",
            index: property_index,
            size: dicts.properties.len(),
        });
    }
    let property = dicts.properties[property_index].0;
    let data_type = dicts.properties[property_index].1;

    let flags = reader.read_byte("value_ref_flags")?;

    // Check reserved bits
    if flags & CREATE_VALUE_REF_RESERVED_MASK != 0 {
        return Err(DecodeError::ReservedBitsSet {
            context: "CreateValueRef flags",
        });
    }

    let language = if flags & FLAG_HAS_LANGUAGE != 0 {
        // Validate: language is only allowed for TEXT properties
        if data_type != DataType::Text {
            return Err(DecodeError::MalformedEncoding {
                context: "CreateValueRef has_language=1 but property DataType is not TEXT",
            });
        }
        let lang_index = reader.read_varint("language")? as usize;
        // Language index 0 = non-linguistic (no language), 1+ = language_ids[index-1]
        if lang_index == 0 {
            None // Non-linguistic
        } else {
            let idx = lang_index - 1;
            if idx >= dicts.languages.len() {
                return Err(DecodeError::IndexOutOfBounds {
                    dict: "languages",
                    index: lang_index,
                    size: dicts.languages.len() + 1,
                });
            }
            Some(dicts.languages[idx])
        }
    } else {
        None
    };

    let space = if flags & FLAG_HAS_SPACE != 0 {
        Some(reader.read_id("space")?)
    } else {
        None
    };

    Ok(Op::CreateValueRef(CreateValueRef {
        id,
        entity,
        property,
        language,
        space,
    }))
}

// =============================================================================
// ENCODING
// =============================================================================

/// Encodes an Op to the writer.
///
/// Note: This function requires that the dictionary builder has already been
/// populated with all IDs that will be referenced. Call `collect_op_ids` first.
pub fn encode_op(
    writer: &mut Writer,
    op: &Op<'_>,
    dict_builder: &mut DictionaryBuilder,
    property_types: &rustc_hash::FxHashMap<crate::model::Id, DataType>,
) -> Result<(), EncodeError> {
    match op {
        Op::CreateEntity(ce) => encode_create_entity(writer, ce, dict_builder, property_types),
        Op::UpdateEntity(ue) => encode_update_entity(writer, ue, dict_builder, property_types),
        Op::DeleteEntity(de) => encode_delete_entity(writer, de, dict_builder),
        Op::RestoreEntity(re) => encode_restore_entity(writer, re, dict_builder),
        Op::CreateRelation(cr) => encode_create_relation(writer, cr, dict_builder),
        Op::UpdateRelation(ur) => encode_update_relation(writer, ur, dict_builder),
        Op::DeleteRelation(dr) => encode_delete_relation(writer, dr, dict_builder),
        Op::RestoreRelation(rr) => encode_restore_relation(writer, rr, dict_builder),
        Op::CreateValueRef(cvr) => encode_create_value_ref(writer, cvr, dict_builder),
    }
}

fn encode_create_entity(
    writer: &mut Writer,
    ce: &CreateEntity<'_>,
    dict_builder: &mut DictionaryBuilder,
    property_types: &rustc_hash::FxHashMap<crate::model::Id, DataType>,
) -> Result<(), EncodeError> {
    writer.write_byte(OP_CREATE_ENTITY);
    writer.write_id(&ce.id);
    writer.write_varint(ce.values.len() as u64);

    for pv in &ce.values {
        let data_type = property_types.get(&pv.property)
            .copied()
            .unwrap_or_else(|| pv.value.data_type());
        encode_property_value(writer, pv, dict_builder, data_type)?;
    }

    Ok(())
}

fn encode_update_entity(
    writer: &mut Writer,
    ue: &UpdateEntity<'_>,
    dict_builder: &mut DictionaryBuilder,
    property_types: &rustc_hash::FxHashMap<crate::model::Id, DataType>,
) -> Result<(), EncodeError> {
    writer.write_byte(OP_UPDATE_ENTITY);

    let id_index = dict_builder.add_object(ue.id);
    writer.write_varint(id_index as u64);

    let mut flags = 0u8;
    if !ue.set_properties.is_empty() {
        flags |= FLAG_HAS_SET_PROPERTIES;
    }
    if !ue.unset_values.is_empty() {
        flags |= FLAG_HAS_UNSET_VALUES;
    }
    writer.write_byte(flags);

    if !ue.set_properties.is_empty() {
        writer.write_varint(ue.set_properties.len() as u64);
        for pv in &ue.set_properties {
            let data_type = property_types.get(&pv.property)
                .copied()
                .unwrap_or_else(|| pv.value.data_type());
            encode_property_value(writer, pv, dict_builder, data_type)?;
        }
    }

    if !ue.unset_values.is_empty() {
        writer.write_varint(ue.unset_values.len() as u64);
        for unset in &ue.unset_values {
            // We need the data type to add to dictionary, use a placeholder
            let idx = dict_builder.add_property(unset.property, DataType::Bool);
            writer.write_varint(idx as u64);
            // Language encoding: 0xFFFFFFFF = all, 0 = non-linguistic, 1+ = specific language
            let lang_value: u32 = match &unset.language {
                UnsetLanguage::All => 0xFFFFFFFF,
                UnsetLanguage::NonLinguistic => 0,
                UnsetLanguage::Specific(lang_id) => {
                    let lang_index = dict_builder.add_language(Some(*lang_id));
                    lang_index as u32
                }
            };
            writer.write_varint(lang_value as u64);
        }
    }

    Ok(())
}

fn encode_delete_entity(
    writer: &mut Writer,
    de: &DeleteEntity,
    dict_builder: &mut DictionaryBuilder,
) -> Result<(), EncodeError> {
    writer.write_byte(OP_DELETE_ENTITY);
    let id_index = dict_builder.add_object(de.id);
    writer.write_varint(id_index as u64);
    Ok(())
}

fn encode_restore_entity(
    writer: &mut Writer,
    re: &RestoreEntity,
    dict_builder: &mut DictionaryBuilder,
) -> Result<(), EncodeError> {
    writer.write_byte(OP_RESTORE_ENTITY);
    let id_index = dict_builder.add_object(re.id);
    writer.write_varint(id_index as u64);
    Ok(())
}

fn encode_create_relation(
    writer: &mut Writer,
    cr: &CreateRelation<'_>,
    dict_builder: &mut DictionaryBuilder,
) -> Result<(), EncodeError> {
    writer.write_byte(OP_CREATE_RELATION);
    writer.write_id(&cr.id);

    let type_index = dict_builder.add_relation_type(cr.relation_type);
    writer.write_varint(type_index as u64);

    // Build flags (bit order matches field order in spec Section 6.4)
    let mut flags = 0u8;
    if cr.from_space.is_some() {
        flags |= FLAG_HAS_FROM_SPACE;
    }
    if cr.from_version.is_some() {
        flags |= FLAG_HAS_FROM_VERSION;
    }
    if cr.to_space.is_some() {
        flags |= FLAG_HAS_TO_SPACE;
    }
    if cr.to_version.is_some() {
        flags |= FLAG_HAS_TO_VERSION;
    }
    if cr.entity.is_some() {
        flags |= FLAG_HAS_ENTITY;
    }
    if cr.position.is_some() {
        flags |= FLAG_HAS_POSITION;
    }
    if cr.from_is_value_ref {
        flags |= FLAG_FROM_IS_VALUE_REF;
    }
    if cr.to_is_value_ref {
        flags |= FLAG_TO_IS_VALUE_REF;
    }
    writer.write_byte(flags);

    // Write from endpoint: inline ID if value ref, otherwise ObjectRef
    if cr.from_is_value_ref {
        writer.write_id(&cr.from);
    } else {
        let from_index = dict_builder.add_object(cr.from);
        writer.write_varint(from_index as u64);
    }

    // Write to endpoint: inline ID if value ref, otherwise ObjectRef
    if cr.to_is_value_ref {
        writer.write_id(&cr.to);
    } else {
        let to_index = dict_builder.add_object(cr.to);
        writer.write_varint(to_index as u64);
    }

    // Write optional fields in spec order: from_space, from_version, to_space, to_version, entity, position
    if let Some(space) = &cr.from_space {
        writer.write_id(space);
    }

    if let Some(version) = &cr.from_version {
        writer.write_id(version);
    }

    if let Some(space) = &cr.to_space {
        writer.write_id(space);
    }

    if let Some(version) = &cr.to_version {
        writer.write_id(version);
    }

    if let Some(entity) = &cr.entity {
        writer.write_id(entity);
    }

    if let Some(pos) = &cr.position {
        validate_position(pos)?;
        writer.write_string(pos);
    }

    Ok(())
}

fn encode_update_relation(
    writer: &mut Writer,
    ur: &UpdateRelation<'_>,
    dict_builder: &mut DictionaryBuilder,
) -> Result<(), EncodeError> {
    writer.write_byte(OP_UPDATE_RELATION);

    let id_index = dict_builder.add_object(ur.id);
    writer.write_varint(id_index as u64);

    // Build set flags
    let mut set_flags = 0u8;
    if ur.from_space.is_some() {
        set_flags |= UPDATE_SET_FROM_SPACE;
    }
    if ur.from_version.is_some() {
        set_flags |= UPDATE_SET_FROM_VERSION;
    }
    if ur.to_space.is_some() {
        set_flags |= UPDATE_SET_TO_SPACE;
    }
    if ur.to_version.is_some() {
        set_flags |= UPDATE_SET_TO_VERSION;
    }
    if ur.position.is_some() {
        set_flags |= UPDATE_SET_POSITION;
    }
    writer.write_byte(set_flags);

    // Build unset flags
    let mut unset_flags = 0u8;
    for field in &ur.unset {
        match field {
            UnsetRelationField::FromSpace => unset_flags |= UPDATE_UNSET_FROM_SPACE,
            UnsetRelationField::FromVersion => unset_flags |= UPDATE_UNSET_FROM_VERSION,
            UnsetRelationField::ToSpace => unset_flags |= UPDATE_UNSET_TO_SPACE,
            UnsetRelationField::ToVersion => unset_flags |= UPDATE_UNSET_TO_VERSION,
            UnsetRelationField::Position => unset_flags |= UPDATE_UNSET_POSITION,
        }
    }
    writer.write_byte(unset_flags);

    // Write set fields in order
    if let Some(space) = &ur.from_space {
        writer.write_id(space);
    }
    if let Some(version) = &ur.from_version {
        writer.write_id(version);
    }
    if let Some(space) = &ur.to_space {
        writer.write_id(space);
    }
    if let Some(version) = &ur.to_version {
        writer.write_id(version);
    }
    if let Some(pos) = &ur.position {
        validate_position(pos)?;
        writer.write_string(pos);
    }

    Ok(())
}

fn encode_delete_relation(
    writer: &mut Writer,
    dr: &DeleteRelation,
    dict_builder: &mut DictionaryBuilder,
) -> Result<(), EncodeError> {
    writer.write_byte(OP_DELETE_RELATION);
    let id_index = dict_builder.add_object(dr.id);
    writer.write_varint(id_index as u64);
    Ok(())
}

fn encode_restore_relation(
    writer: &mut Writer,
    rr: &RestoreRelation,
    dict_builder: &mut DictionaryBuilder,
) -> Result<(), EncodeError> {
    writer.write_byte(OP_RESTORE_RELATION);
    let id_index = dict_builder.add_object(rr.id);
    writer.write_varint(id_index as u64);
    Ok(())
}

fn encode_create_value_ref(
    writer: &mut Writer,
    cvr: &CreateValueRef,
    dict_builder: &mut DictionaryBuilder,
) -> Result<(), EncodeError> {
    writer.write_byte(OP_CREATE_VALUE_REF);
    writer.write_id(&cvr.id);

    let entity_index = dict_builder.add_object(cvr.entity);
    writer.write_varint(entity_index as u64);

    // For CreateValueRef, we need to add the property to the dictionary.
    // Use DataType::Text as a placeholder if language is present, otherwise Bool.
    // The actual data type will be determined by the property's declaration elsewhere.
    let data_type = if cvr.language.is_some() { DataType::Text } else { DataType::Bool };
    let property_index = dict_builder.add_property(cvr.property, data_type);
    writer.write_varint(property_index as u64);

    let mut flags = 0u8;
    if cvr.language.is_some() {
        flags |= FLAG_HAS_LANGUAGE;
    }
    if cvr.space.is_some() {
        flags |= FLAG_HAS_SPACE;
    }
    writer.write_byte(flags);

    if let Some(lang_id) = cvr.language {
        let lang_index = dict_builder.add_language(Some(lang_id));
        writer.write_varint(lang_index as u64);
    }

    if let Some(space) = &cvr.space {
        writer.write_id(space);
    }

    Ok(())
}

fn encode_property_value(
    writer: &mut Writer,
    pv: &PropertyValue<'_>,
    dict_builder: &mut DictionaryBuilder,
    data_type: DataType,
) -> Result<(), EncodeError> {
    let prop_index = dict_builder.add_property(pv.property, data_type);
    writer.write_varint(prop_index as u64);
    crate::codec::value::encode_value(writer, &pv.value, dict_builder)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::model::Value;

    #[test]
    fn test_create_entity_roundtrip() {
        let op = Op::CreateEntity(CreateEntity {
            id: [1u8; 16],
            values: vec![PropertyValue {
                property: [2u8; 16],
                value: Value::Text {
                    value: Cow::Owned("test".to_string()),
                    language: None,
                },
            }],
        });

        let mut dict_builder = DictionaryBuilder::new();
        let mut property_types = rustc_hash::FxHashMap::default();
        property_types.insert([2u8; 16], DataType::Text);

        let mut writer = Writer::new();
        encode_op(&mut writer, &op, &mut dict_builder, &property_types).unwrap();

        let dicts = dict_builder.build();
        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_op(&mut reader, &dicts).unwrap();

        // Compare by extracting values since Cow::Owned vs Cow::Borrowed
        match (&op, &decoded) {
            (Op::CreateEntity(e1), Op::CreateEntity(e2)) => {
                assert_eq!(e1.id, e2.id);
                assert_eq!(e1.values.len(), e2.values.len());
                for (v1, v2) in e1.values.iter().zip(e2.values.iter()) {
                    assert_eq!(v1.property, v2.property);
                    match (&v1.value, &v2.value) {
                        (Value::Text { value: s1, language: l1 }, Value::Text { value: s2, language: l2 }) => {
                            assert_eq!(s1.as_ref(), s2.as_ref());
                            assert_eq!(l1, l2);
                        }
                        _ => panic!("expected Text values"),
                    }
                }
            }
            _ => panic!("expected CreateEntity"),
        }
    }

    #[test]
    fn test_create_relation_roundtrip() {
        // Test with explicit entity
        let op = Op::CreateRelation(CreateRelation {
            id: [10u8; 16],
            relation_type: [1u8; 16],
            from: [2u8; 16],
            from_is_value_ref: false,
            to: [3u8; 16],
            to_is_value_ref: false,
            entity: Some([4u8; 16]),
            position: Some(Cow::Owned("abc".to_string())),
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
        });

        let mut dict_builder = DictionaryBuilder::new();
        let property_types = rustc_hash::FxHashMap::default();

        let mut writer = Writer::new();
        encode_op(&mut writer, &op, &mut dict_builder, &property_types).unwrap();

        let dicts = dict_builder.build();
        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_op(&mut reader, &dicts).unwrap();

        // Compare by extracting values
        match (&op, &decoded) {
            (Op::CreateRelation(r1), Op::CreateRelation(r2)) => {
                assert_eq!(r1.id, r2.id);
                assert_eq!(r1.relation_type, r2.relation_type);
                assert_eq!(r1.from, r2.from);
                assert_eq!(r1.from_is_value_ref, r2.from_is_value_ref);
                assert_eq!(r1.to, r2.to);
                assert_eq!(r1.to_is_value_ref, r2.to_is_value_ref);
                assert_eq!(r1.entity, r2.entity);
                match (&r1.position, &r2.position) {
                    (Some(p1), Some(p2)) => assert_eq!(p1.as_ref(), p2.as_ref()),
                    (None, None) => {}
                    _ => panic!("position mismatch"),
                }
            }
            _ => panic!("expected CreateRelation"),
        }
    }

    #[test]
    fn test_create_relation_auto_entity_roundtrip() {
        // Test with auto-derived entity (entity = None)
        let op = Op::CreateRelation(CreateRelation {
            id: [10u8; 16],
            relation_type: [1u8; 16],
            from: [2u8; 16],
            from_is_value_ref: false,
            to: [3u8; 16],
            to_is_value_ref: false,
            entity: None,
            position: Some(Cow::Owned("abc".to_string())),
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
        });

        let mut dict_builder = DictionaryBuilder::new();
        let property_types = rustc_hash::FxHashMap::default();

        let mut writer = Writer::new();
        encode_op(&mut writer, &op, &mut dict_builder, &property_types).unwrap();

        let dicts = dict_builder.build();
        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_op(&mut reader, &dicts).unwrap();

        match (&op, &decoded) {
            (Op::CreateRelation(r1), Op::CreateRelation(r2)) => {
                assert_eq!(r1.id, r2.id);
                assert_eq!(r1.relation_type, r2.relation_type);
                assert_eq!(r1.from, r2.from);
                assert_eq!(r1.from_is_value_ref, r2.from_is_value_ref);
                assert_eq!(r1.to, r2.to);
                assert_eq!(r1.to_is_value_ref, r2.to_is_value_ref);
                assert_eq!(r1.entity, r2.entity);
                assert!(r1.entity.is_none());
                assert!(r2.entity.is_none());
            }
            _ => panic!("expected CreateRelation"),
        }
    }

    #[test]
    fn test_create_relation_with_versions() {
        let op = Op::CreateRelation(CreateRelation {
            id: [10u8; 16],
            relation_type: [1u8; 16],
            from: [2u8; 16],
            from_is_value_ref: false,
            to: [3u8; 16],
            to_is_value_ref: false,
            entity: Some([4u8; 16]),
            position: Some(Cow::Owned("abc".to_string())),
            from_space: Some([5u8; 16]),
            from_version: Some([6u8; 16]),
            to_space: Some([7u8; 16]),
            to_version: Some([8u8; 16]),
        });

        let mut dict_builder = DictionaryBuilder::new();
        let property_types = rustc_hash::FxHashMap::default();

        let mut writer = Writer::new();
        encode_op(&mut writer, &op, &mut dict_builder, &property_types).unwrap();

        let dicts = dict_builder.build();
        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_op(&mut reader, &dicts).unwrap();

        match (&op, &decoded) {
            (Op::CreateRelation(r1), Op::CreateRelation(r2)) => {
                assert_eq!(r1.id, r2.id);
                assert_eq!(r1.relation_type, r2.relation_type);
                assert_eq!(r1.from, r2.from);
                assert_eq!(r1.from_is_value_ref, r2.from_is_value_ref);
                assert_eq!(r1.to, r2.to);
                assert_eq!(r1.to_is_value_ref, r2.to_is_value_ref);
                assert_eq!(r1.entity, r2.entity);
                assert_eq!(r1.from_space, r2.from_space);
                assert_eq!(r1.from_version, r2.from_version);
                assert_eq!(r1.to_space, r2.to_space);
                assert_eq!(r1.to_version, r2.to_version);
            }
            _ => panic!("expected CreateRelation"),
        }
    }

    #[test]
    fn test_create_relation_with_value_ref_endpoint() {
        // Test with to endpoint being a value ref (inline ID)
        let op = Op::CreateRelation(CreateRelation {
            id: [10u8; 16],
            relation_type: [1u8; 16],
            from: [2u8; 16],
            from_is_value_ref: false,
            to: [99u8; 16], // Value ref ID
            to_is_value_ref: true,
            entity: None,
            position: None,
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
        });

        let mut dict_builder = DictionaryBuilder::new();
        let property_types = rustc_hash::FxHashMap::default();

        let mut writer = Writer::new();
        encode_op(&mut writer, &op, &mut dict_builder, &property_types).unwrap();

        let dicts = dict_builder.build();
        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_op(&mut reader, &dicts).unwrap();

        match (&op, &decoded) {
            (Op::CreateRelation(r1), Op::CreateRelation(r2)) => {
                assert_eq!(r1.id, r2.id);
                assert_eq!(r1.from, r2.from);
                assert_eq!(r1.from_is_value_ref, r2.from_is_value_ref);
                assert!(!r1.from_is_value_ref);
                assert_eq!(r1.to, r2.to);
                assert_eq!(r1.to_is_value_ref, r2.to_is_value_ref);
                assert!(r1.to_is_value_ref);
            }
            _ => panic!("expected CreateRelation"),
        }
    }

    #[test]
    fn test_create_value_ref_roundtrip() {
        let op = Op::CreateValueRef(CreateValueRef {
            id: [1u8; 16],
            entity: [2u8; 16],
            property: [3u8; 16],
            language: None,
            space: None,
        });

        let mut dict_builder = DictionaryBuilder::new();
        let property_types = rustc_hash::FxHashMap::default();

        let mut writer = Writer::new();
        encode_op(&mut writer, &op, &mut dict_builder, &property_types).unwrap();

        let dicts = dict_builder.build();
        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_op(&mut reader, &dicts).unwrap();

        match (&op, &decoded) {
            (Op::CreateValueRef(v1), Op::CreateValueRef(v2)) => {
                assert_eq!(v1.id, v2.id);
                assert_eq!(v1.entity, v2.entity);
                assert_eq!(v1.property, v2.property);
                assert_eq!(v1.language, v2.language);
                assert_eq!(v1.space, v2.space);
            }
            _ => panic!("expected CreateValueRef"),
        }
    }

    #[test]
    fn test_create_value_ref_with_language_and_space() {
        let op = Op::CreateValueRef(CreateValueRef {
            id: [1u8; 16],
            entity: [2u8; 16],
            property: [3u8; 16],
            language: Some([4u8; 16]),
            space: Some([5u8; 16]),
        });

        let mut dict_builder = DictionaryBuilder::new();
        let property_types = rustc_hash::FxHashMap::default();

        let mut writer = Writer::new();
        encode_op(&mut writer, &op, &mut dict_builder, &property_types).unwrap();

        let dicts = dict_builder.build();
        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_op(&mut reader, &dicts).unwrap();

        match (&op, &decoded) {
            (Op::CreateValueRef(v1), Op::CreateValueRef(v2)) => {
                assert_eq!(v1.id, v2.id);
                assert_eq!(v1.entity, v2.entity);
                assert_eq!(v1.property, v2.property);
                assert_eq!(v1.language, v2.language);
                assert_eq!(v1.space, v2.space);
            }
            _ => panic!("expected CreateValueRef"),
        }
    }

    #[test]
    fn test_update_relation_roundtrip() {
        // Test with all set fields
        let op = Op::UpdateRelation(UpdateRelation {
            id: [1u8; 16],
            from_space: Some([2u8; 16]),
            from_version: Some([3u8; 16]),
            to_space: Some([4u8; 16]),
            to_version: Some([5u8; 16]),
            position: Some(Cow::Owned("xyz".to_string())),
            unset: vec![],
        });

        let mut dict_builder = DictionaryBuilder::new();
        dict_builder.add_object([1u8; 16]); // Pre-add the relation ID
        let property_types = rustc_hash::FxHashMap::default();

        let mut writer = Writer::new();
        encode_op(&mut writer, &op, &mut dict_builder, &property_types).unwrap();

        let dicts = dict_builder.build();
        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_op(&mut reader, &dicts).unwrap();

        match (&op, &decoded) {
            (Op::UpdateRelation(r1), Op::UpdateRelation(r2)) => {
                assert_eq!(r1.id, r2.id);
                assert_eq!(r1.from_space, r2.from_space);
                assert_eq!(r1.from_version, r2.from_version);
                assert_eq!(r1.to_space, r2.to_space);
                assert_eq!(r1.to_version, r2.to_version);
                match (&r1.position, &r2.position) {
                    (Some(p1), Some(p2)) => assert_eq!(p1.as_ref(), p2.as_ref()),
                    (None, None) => {}
                    _ => panic!("position mismatch"),
                }
                assert_eq!(r1.unset, r2.unset);
            }
            _ => panic!("expected UpdateRelation"),
        }
    }

    #[test]
    fn test_update_relation_with_unset() {
        // Test with unset fields
        let op = Op::UpdateRelation(UpdateRelation {
            id: [1u8; 16],
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
            position: None,
            unset: vec![
                UnsetRelationField::FromSpace,
                UnsetRelationField::ToVersion,
                UnsetRelationField::Position,
            ],
        });

        let mut dict_builder = DictionaryBuilder::new();
        dict_builder.add_object([1u8; 16]); // Pre-add the relation ID
        let property_types = rustc_hash::FxHashMap::default();

        let mut writer = Writer::new();
        encode_op(&mut writer, &op, &mut dict_builder, &property_types).unwrap();

        let dicts = dict_builder.build();
        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_op(&mut reader, &dicts).unwrap();

        match (&op, &decoded) {
            (Op::UpdateRelation(r1), Op::UpdateRelation(r2)) => {
                assert_eq!(r1.id, r2.id);
                // Check that unset fields are preserved (order may differ due to bit decoding)
                assert_eq!(r1.unset.len(), r2.unset.len());
                for field in &r1.unset {
                    assert!(r2.unset.contains(field), "missing unset field: {:?}", field);
                }
            }
            _ => panic!("expected UpdateRelation"),
        }
    }

}
