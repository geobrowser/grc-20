//! Operation encoding/decoding for GRC-20 binary format.
//!
//! Implements the wire format for operations (spec Section 6.4).

use crate::codec::primitives::{Reader, Writer};
use crate::codec::value::{decode_position, decode_property_value, validate_position};
use crate::error::{DecodeError, EncodeError};
use crate::limits::MAX_VALUES_PER_ENTITY;
use crate::model::{
    CreateEntity, CreateProperty, CreateRelation, DataType, DeleteEntity, DeleteRelation,
    DictionaryBuilder, Op, PropertyValue, RelationIdMode, UnsetProperty, UpdateEntity, UpdateRelation,
    WireDictionaries,
};

// Op type constants
const OP_CREATE_ENTITY: u8 = 1;
const OP_UPDATE_ENTITY: u8 = 2;
const OP_DELETE_ENTITY: u8 = 3;
const OP_CREATE_RELATION: u8 = 4;
const OP_UPDATE_RELATION: u8 = 5;
const OP_DELETE_RELATION: u8 = 6;
const OP_CREATE_PROPERTY: u8 = 7;

// UpdateEntity flags
const FLAG_HAS_SET_PROPERTIES: u8 = 0x01;
const FLAG_HAS_UNSET_PROPERTIES: u8 = 0x02;
const UPDATE_ENTITY_RESERVED_MASK: u8 = 0xFC;

// CreateRelation flags (bit order matches field order in spec Section 6.4)
const FLAG_HAS_FROM_SPACE: u8 = 0x01;
const FLAG_HAS_FROM_VERSION: u8 = 0x02;
const FLAG_HAS_TO_SPACE: u8 = 0x04;
const FLAG_HAS_TO_VERSION: u8 = 0x08;
const FLAG_HAS_ENTITY: u8 = 0x10;
const FLAG_HAS_POSITION: u8 = 0x20;
const CREATE_RELATION_RESERVED_MASK: u8 = 0xC0;

// UpdateRelation flags (only position is mutable)
const UPDATE_FLAG_HAS_POSITION: u8 = 0x01;
const UPDATE_RELATION_RESERVED_MASK: u8 = 0xFE;

// Relation ID modes
const MODE_UNIQUE: u8 = 0;
const MODE_MANY: u8 = 1;

// =============================================================================
// DECODING
// =============================================================================

/// Decodes an Op from the reader (zero-copy).
pub fn decode_op<'a>(reader: &mut Reader<'a>, dicts: &WireDictionaries) -> Result<Op<'a>, DecodeError> {
    let op_type = reader.read_byte("op_type")?;

    match op_type {
        OP_CREATE_ENTITY => decode_create_entity(reader, dicts),
        OP_UPDATE_ENTITY => decode_update_entity(reader, dicts),
        OP_DELETE_ENTITY => decode_delete_entity(reader),
        OP_CREATE_RELATION => decode_create_relation(reader, dicts),
        OP_UPDATE_RELATION => decode_update_relation(reader, dicts),
        OP_DELETE_RELATION => decode_delete_relation(reader),
        OP_CREATE_PROPERTY => decode_create_property(reader),
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

    if flags & FLAG_HAS_UNSET_PROPERTIES != 0 {
        let count = reader.read_varint("unset_properties_count")? as usize;
        if count > MAX_VALUES_PER_ENTITY {
            return Err(DecodeError::LengthExceedsLimit {
                field: "unset_properties",
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

            let lang_index = reader.read_varint("unset.language")? as usize;
            let language = if lang_index == 0 {
                None
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
            };

            update.unset_properties.push(UnsetProperty { property, language });
        }
    }

    Ok(Op::UpdateEntity(update))
}

fn decode_delete_entity<'a>(
    reader: &mut Reader<'a>,
) -> Result<Op<'a>, DecodeError> {
    let id = reader.read_id("entity_id")?;
    Ok(Op::DeleteEntity(DeleteEntity { id }))
}

fn decode_create_relation<'a>(
    reader: &mut Reader<'a>,
    dicts: &WireDictionaries,
) -> Result<Op<'a>, DecodeError> {
    let mode = reader.read_byte("relation_mode")?;

    let id_mode = match mode {
        MODE_UNIQUE => RelationIdMode::Unique,
        MODE_MANY => {
            let id = reader.read_id("relation_id")?;
            RelationIdMode::Many(id)
        }
        _ => {
            return Err(DecodeError::MalformedEncoding {
                context: "invalid relation mode",
            });
        }
    };

    let type_index = reader.read_varint("relation_type")? as usize;
    if type_index >= dicts.relation_types.len() {
        return Err(DecodeError::IndexOutOfBounds {
            dict: "relation_types",
            index: type_index,
            size: dicts.relation_types.len(),
        });
    }
    let relation_type = dicts.relation_types[type_index];

    let from_index = reader.read_varint("from")? as usize;
    if from_index >= dicts.objects.len() {
        return Err(DecodeError::IndexOutOfBounds {
            dict: "objects",
            index: from_index,
            size: dicts.objects.len(),
        });
    }
    let from = dicts.objects[from_index];

    let to_index = reader.read_varint("to")? as usize;
    if to_index >= dicts.objects.len() {
        return Err(DecodeError::IndexOutOfBounds {
            dict: "objects",
            index: to_index,
            size: dicts.objects.len(),
        });
    }
    let to = dicts.objects[to_index];

    let flags = reader.read_byte("relation_flags")?;

    // Check reserved bits
    if flags & CREATE_RELATION_RESERVED_MASK != 0 {
        return Err(DecodeError::ReservedBitsSet {
            context: "CreateRelation flags",
        });
    }

    // Validate: unique mode must not have explicit entity
    let has_entity = flags & FLAG_HAS_ENTITY != 0;
    if mode == MODE_UNIQUE && has_entity {
        return Err(DecodeError::MalformedEncoding {
            context: "unique mode relation cannot have explicit entity",
        });
    }

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

    let entity = if has_entity {
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
        id_mode,
        relation_type,
        from,
        to,
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

    let flags = reader.read_byte("relation_flags")?;

    // Check reserved bits
    if flags & UPDATE_RELATION_RESERVED_MASK != 0 {
        return Err(DecodeError::ReservedBitsSet {
            context: "UpdateRelation flags",
        });
    }

    let position = if flags & UPDATE_FLAG_HAS_POSITION != 0 {
        Some(decode_position(reader)?)
    } else {
        None
    };

    Ok(Op::UpdateRelation(UpdateRelation { id, position }))
}

fn decode_delete_relation<'a>(
    reader: &mut Reader<'a>,
) -> Result<Op<'a>, DecodeError> {
    let id = reader.read_id("relation_id")?;
    Ok(Op::DeleteRelation(DeleteRelation { id }))
}

fn decode_create_property<'a>(reader: &mut Reader<'a>) -> Result<Op<'a>, DecodeError> {
    let id = reader.read_id("property_id")?;
    let data_type_byte = reader.read_byte("data_type")?;
    let data_type = DataType::from_u8(data_type_byte)
        .ok_or(DecodeError::InvalidDataType { data_type: data_type_byte })?;

    Ok(Op::CreateProperty(CreateProperty { id, data_type }))
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
        Op::CreateRelation(cr) => encode_create_relation(writer, cr, dict_builder),
        Op::UpdateRelation(ur) => encode_update_relation(writer, ur, dict_builder),
        Op::DeleteRelation(dr) => encode_delete_relation(writer, dr, dict_builder),
        Op::CreateProperty(cp) => encode_create_property(writer, cp),
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
    if !ue.unset_properties.is_empty() {
        flags |= FLAG_HAS_UNSET_PROPERTIES;
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

    if !ue.unset_properties.is_empty() {
        writer.write_varint(ue.unset_properties.len() as u64);
        for unset in &ue.unset_properties {
            // We need the data type to add to dictionary, use a placeholder
            let idx = dict_builder.add_property(unset.property, DataType::Bool);
            writer.write_varint(idx as u64);
            let lang_index = dict_builder.add_language(unset.language);
            writer.write_varint(lang_index as u64);
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

fn encode_create_relation(
    writer: &mut Writer,
    cr: &CreateRelation<'_>,
    dict_builder: &mut DictionaryBuilder,
) -> Result<(), EncodeError> {
    // Validate: unique mode must not have explicit entity
    if matches!(cr.id_mode, RelationIdMode::Unique) && cr.entity.is_some() {
        return Err(EncodeError::InvalidInput {
            context: "unique mode relation cannot have explicit entity",
        });
    }

    writer.write_byte(OP_CREATE_RELATION);

    match &cr.id_mode {
        RelationIdMode::Unique => {
            writer.write_byte(MODE_UNIQUE);
        }
        RelationIdMode::Many(id) => {
            writer.write_byte(MODE_MANY);
            writer.write_id(id);
        }
    }

    let type_index = dict_builder.add_relation_type(cr.relation_type);
    writer.write_varint(type_index as u64);

    let from_index = dict_builder.add_object(cr.from);
    writer.write_varint(from_index as u64);

    let to_index = dict_builder.add_object(cr.to);
    writer.write_varint(to_index as u64);

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
    writer.write_byte(flags);

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

    let flags = if ur.position.is_some() { UPDATE_FLAG_HAS_POSITION } else { 0 };
    writer.write_byte(flags);

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

fn encode_create_property(writer: &mut Writer, cp: &CreateProperty) -> Result<(), EncodeError> {
    writer.write_byte(OP_CREATE_PROPERTY);
    writer.write_id(&cp.id);
    writer.write_byte(cp.data_type as u8);
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
        // Test with explicit entity (instance mode)
        let op = Op::CreateRelation(CreateRelation {
            id_mode: RelationIdMode::Many([10u8; 16]),
            relation_type: [1u8; 16],
            from: [2u8; 16],
            to: [3u8; 16],
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
                assert_eq!(r1.id_mode, r2.id_mode);
                assert_eq!(r1.relation_type, r2.relation_type);
                assert_eq!(r1.from, r2.from);
                assert_eq!(r1.to, r2.to);
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
            id_mode: RelationIdMode::Many([10u8; 16]),
            relation_type: [1u8; 16],
            from: [2u8; 16],
            to: [3u8; 16],
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
                assert_eq!(r1.id_mode, r2.id_mode);
                assert_eq!(r1.relation_type, r2.relation_type);
                assert_eq!(r1.from, r2.from);
                assert_eq!(r1.to, r2.to);
                assert_eq!(r1.entity, r2.entity);
                assert!(r1.entity.is_none());
                assert!(r2.entity.is_none());
            }
            _ => panic!("expected CreateRelation"),
        }
    }

    #[test]
    fn test_unique_mode_relation() {
        // Unique mode must use auto-derived entity (entity = None)
        let op = Op::CreateRelation(CreateRelation {
            id_mode: RelationIdMode::Unique,
            relation_type: [1u8; 16],
            from: [2u8; 16],
            to: [3u8; 16],
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

        // Direct comparison works since no borrowed strings
        match (&op, &decoded) {
            (Op::CreateRelation(r1), Op::CreateRelation(r2)) => {
                assert_eq!(r1.id_mode, r2.id_mode);
                assert_eq!(r1.relation_type, r2.relation_type);
                assert_eq!(r1.from, r2.from);
                assert_eq!(r1.to, r2.to);
                assert_eq!(r1.entity, r2.entity);
                assert!(r1.entity.is_none());
                assert!(r1.position.is_none() && r2.position.is_none());
            }
            _ => panic!("expected CreateRelation"),
        }
    }

    #[test]
    fn test_unique_mode_with_explicit_entity_rejected() {
        // Unique mode with explicit entity should be rejected
        let op = Op::CreateRelation(CreateRelation {
            id_mode: RelationIdMode::Unique,
            relation_type: [1u8; 16],
            from: [2u8; 16],
            to: [3u8; 16],
            entity: Some([4u8; 16]), // Invalid: explicit entity in unique mode
            position: None,
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
        });

        let mut dict_builder = DictionaryBuilder::new();
        let property_types = rustc_hash::FxHashMap::default();

        let mut writer = Writer::new();
        let result = encode_op(&mut writer, &op, &mut dict_builder, &property_types);

        assert!(result.is_err());
        match result {
            Err(crate::error::EncodeError::InvalidInput { context }) => {
                assert!(context.contains("unique mode"));
            }
            _ => panic!("expected InvalidInput error"),
        }
    }

    #[test]
    fn test_create_relation_with_versions() {
        let op = Op::CreateRelation(CreateRelation {
            id_mode: RelationIdMode::Many([10u8; 16]),
            relation_type: [1u8; 16],
            from: [2u8; 16],
            to: [3u8; 16],
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
                assert_eq!(r1.id_mode, r2.id_mode);
                assert_eq!(r1.relation_type, r2.relation_type);
                assert_eq!(r1.from, r2.from);
                assert_eq!(r1.to, r2.to);
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
    fn test_update_relation_roundtrip() {
        let op = Op::UpdateRelation(UpdateRelation {
            id: [1u8; 16],
            position: Some(Cow::Owned("xyz".to_string())),
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
                match (&r1.position, &r2.position) {
                    (Some(p1), Some(p2)) => assert_eq!(p1.as_ref(), p2.as_ref()),
                    (None, None) => {}
                    _ => panic!("position mismatch"),
                }
            }
            _ => panic!("expected UpdateRelation"),
        }
    }

    #[test]
    fn test_create_property_roundtrip() {
        let op = Op::CreateProperty(CreateProperty {
            id: [1u8; 16],
            data_type: DataType::Text,
        });

        let mut dict_builder = DictionaryBuilder::new();
        let property_types = rustc_hash::FxHashMap::default();

        let mut writer = Writer::new();
        encode_op(&mut writer, &op, &mut dict_builder, &property_types).unwrap();

        let dicts = dict_builder.build();
        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_op(&mut reader, &dicts).unwrap();

        assert_eq!(op, decoded);
    }
}
