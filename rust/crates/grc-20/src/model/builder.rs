//! Builder API for ergonomic Edit construction.
//!
//! Provides a fluent interface for building Edits with operations.
//!
//! # Example
//!
//! ```rust
//! use grc_20::model::builder::EditBuilder;
//! use grc_20::genesis::{properties, relation_types};
//! use grc_20::Value;
//! use std::borrow::Cow;
//!
//! let edit = EditBuilder::new([1u8; 16])
//!     .name("Create Alice")
//!     .author([2u8; 16])
//!     .create_entity([3u8; 16], |e| e
//!         .text(properties::name(), "Alice", None)
//!         .text(properties::description(), "A person", None)
//!     )
//!     .build();
//! ```

use std::borrow::Cow;

use crate::model::{
    CreateEntity, CreateRelation, DeleteEntity, DeleteRelation,
    Edit, Id, Op, PropertyValue, RestoreEntity, RestoreRelation, UnsetRelationField,
    UnsetLanguage, UnsetValue, UpdateEntity, UpdateRelation, Value,
};

/// Builder for constructing an Edit with operations.
#[derive(Debug, Clone)]
pub struct EditBuilder<'a> {
    id: Id,
    name: Cow<'a, str>,
    authors: Vec<Id>,
    created_at: i64,
    ops: Vec<Op<'a>>,
}

impl<'a> EditBuilder<'a> {
    /// Creates a new EditBuilder with the given edit ID.
    pub fn new(id: Id) -> Self {
        Self {
            id,
            name: Cow::Borrowed(""),
            authors: Vec::new(),
            created_at: 0,
            ops: Vec::new(),
        }
    }

    /// Sets the edit name.
    pub fn name(mut self, name: impl Into<Cow<'a, str>>) -> Self {
        self.name = name.into();
        self
    }

    /// Adds an author to the edit.
    pub fn author(mut self, author_id: Id) -> Self {
        self.authors.push(author_id);
        self
    }

    /// Sets multiple authors at once.
    pub fn authors(mut self, author_ids: impl IntoIterator<Item = Id>) -> Self {
        self.authors.extend(author_ids);
        self
    }

    /// Sets the creation timestamp (microseconds since Unix epoch).
    pub fn created_at(mut self, timestamp: i64) -> Self {
        self.created_at = timestamp;
        self
    }

    /// Sets the creation timestamp to now.
    pub fn created_now(mut self) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let micros = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as i64)
            .unwrap_or(0);
        self.created_at = micros;
        self
    }

    // =========================================================================
    // Entity Operations
    // =========================================================================

    /// Adds a CreateEntity operation using a builder function.
    pub fn create_entity<F>(mut self, id: Id, f: F) -> Self
    where
        F: FnOnce(EntityBuilder<'a>) -> EntityBuilder<'a>,
    {
        let builder = f(EntityBuilder::new());
        self.ops.push(Op::CreateEntity(CreateEntity {
            id,
            values: builder.values,
        }));
        self
    }

    /// Adds a CreateEntity operation with no values.
    pub fn create_empty_entity(mut self, id: Id) -> Self {
        self.ops.push(Op::CreateEntity(CreateEntity {
            id,
            values: Vec::new(),
        }));
        self
    }

    /// Adds an UpdateEntity operation using a builder function.
    pub fn update_entity<F>(mut self, id: Id, f: F) -> Self
    where
        F: FnOnce(UpdateEntityBuilder<'a>) -> UpdateEntityBuilder<'a>,
    {
        let builder = f(UpdateEntityBuilder::new(id));
        self.ops.push(Op::UpdateEntity(UpdateEntity {
            id: builder.id,
            set_properties: builder.set_properties,
            unset_values: builder.unset_values,
        }));
        self
    }

    /// Adds a DeleteEntity operation.
    pub fn delete_entity(mut self, id: Id) -> Self {
        self.ops.push(Op::DeleteEntity(DeleteEntity { id }));
        self
    }

    /// Adds a RestoreEntity operation.
    pub fn restore_entity(mut self, id: Id) -> Self {
        self.ops.push(Op::RestoreEntity(RestoreEntity { id }));
        self
    }

    // =========================================================================
    // Relation Operations
    // =========================================================================

    /// Adds a CreateRelation operation with an explicit ID.
    pub fn create_relation_simple(
        mut self,
        id: Id,
        from: Id,
        to: Id,
        relation_type: Id,
    ) -> Self {
        self.ops.push(Op::CreateRelation(CreateRelation {
            id,
            relation_type,
            from,
            from_is_value_ref: false,
            to,
            to_is_value_ref: false,
            entity: None,
            position: None,
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
        }));
        self
    }

    /// Adds a CreateRelation operation with full control using a builder.
    pub fn create_relation<F>(mut self, f: F) -> Self
    where
        F: FnOnce(RelationBuilder<'a>) -> RelationBuilder<'a>,
    {
        let builder = f(RelationBuilder::new());
        if let Some(relation) = builder.build() {
            self.ops.push(Op::CreateRelation(relation));
        }
        self
    }

    /// Adds an UpdateRelation operation using a builder function.
    pub fn update_relation<F>(mut self, id: Id, f: F) -> Self
    where
        F: FnOnce(UpdateRelationBuilder<'a>) -> UpdateRelationBuilder<'a>,
    {
        let builder = f(UpdateRelationBuilder::new(id));
        self.ops.push(Op::UpdateRelation(UpdateRelation {
            id: builder.id,
            from_space: builder.from_space,
            from_version: builder.from_version,
            to_space: builder.to_space,
            to_version: builder.to_version,
            position: builder.position,
            unset: builder.unset,
        }));
        self
    }

    /// Adds an UpdateRelation operation to only update the position.
    pub fn update_relation_position(mut self, id: Id, position: Option<Cow<'a, str>>) -> Self {
        self.ops.push(Op::UpdateRelation(UpdateRelation {
            id,
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
            position,
            unset: vec![],
        }));
        self
    }

    /// Adds a DeleteRelation operation.
    pub fn delete_relation(mut self, id: Id) -> Self {
        self.ops.push(Op::DeleteRelation(DeleteRelation { id }));
        self
    }

    /// Adds a RestoreRelation operation.
    pub fn restore_relation(mut self, id: Id) -> Self {
        self.ops.push(Op::RestoreRelation(RestoreRelation { id }));
        self
    }

    // =========================================================================
    // Raw Operations
    // =========================================================================

    /// Adds a raw operation directly.
    pub fn op(mut self, op: Op<'a>) -> Self {
        self.ops.push(op);
        self
    }

    /// Adds multiple raw operations.
    pub fn ops(mut self, ops: impl IntoIterator<Item = Op<'a>>) -> Self {
        self.ops.extend(ops);
        self
    }

    // =========================================================================
    // Build
    // =========================================================================

    /// Builds the final Edit.
    pub fn build(self) -> Edit<'a> {
        Edit {
            id: self.id,
            name: self.name,
            authors: self.authors,
            created_at: self.created_at,
            ops: self.ops,
        }
    }

    /// Returns the number of operations added so far.
    pub fn op_count(&self) -> usize {
        self.ops.len()
    }
}

/// Builder for entity values (used in CreateEntity).
#[derive(Debug, Clone, Default)]
pub struct EntityBuilder<'a> {
    values: Vec<PropertyValue<'a>>,
}

impl<'a> EntityBuilder<'a> {
    /// Creates a new empty EntityBuilder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a property value.
    pub fn value(mut self, property: Id, value: Value<'a>) -> Self {
        self.values.push(PropertyValue { property, value });
        self
    }

    /// Adds a TEXT value.
    pub fn text(
        mut self,
        property: Id,
        value: impl Into<Cow<'a, str>>,
        language: Option<Id>,
    ) -> Self {
        self.values.push(PropertyValue {
            property,
            value: Value::Text {
                value: value.into(),
                language,
            },
        });
        self
    }

    /// Adds an INT64 value.
    pub fn int64(mut self, property: Id, value: i64, unit: Option<Id>) -> Self {
        self.values.push(PropertyValue {
            property,
            value: Value::Int64 { value, unit },
        });
        self
    }

    /// Adds a FLOAT64 value.
    pub fn float64(mut self, property: Id, value: f64, unit: Option<Id>) -> Self {
        self.values.push(PropertyValue {
            property,
            value: Value::Float64 { value, unit },
        });
        self
    }

    /// Adds a BOOL value.
    pub fn bool(mut self, property: Id, value: bool) -> Self {
        self.values.push(PropertyValue {
            property,
            value: Value::Bool(value),
        });
        self
    }

    /// Adds a BYTES value.
    pub fn bytes(mut self, property: Id, value: impl Into<Cow<'a, [u8]>>) -> Self {
        self.values.push(PropertyValue {
            property,
            value: Value::Bytes(value.into()),
        });
        self
    }

    /// Adds a POINT value (longitude, latitude, optional altitude).
    pub fn point(mut self, property: Id, lon: f64, lat: f64, alt: Option<f64>) -> Self {
        self.values.push(PropertyValue {
            property,
            value: Value::Point { lon, lat, alt },
        });
        self
    }

    /// Adds a DATE value (ISO 8601 date string).
    pub fn date(mut self, property: Id, value: impl Into<Cow<'a, str>>) -> Self {
        self.values.push(PropertyValue {
            property,
            value: Value::Date(value.into()),
        });
        self
    }

    /// Adds a TIME value (ISO 8601 time string with timezone).
    pub fn time(mut self, property: Id, value: impl Into<Cow<'a, str>>) -> Self {
        self.values.push(PropertyValue {
            property,
            value: Value::Time(value.into()),
        });
        self
    }

    /// Adds a DATETIME value (ISO 8601 datetime string).
    pub fn datetime(mut self, property: Id, value: impl Into<Cow<'a, str>>) -> Self {
        self.values.push(PropertyValue {
            property,
            value: Value::Datetime(value.into()),
        });
        self
    }

    /// Adds a SCHEDULE value (RFC 5545 iCalendar format).
    pub fn schedule(mut self, property: Id, value: impl Into<Cow<'a, str>>) -> Self {
        self.values.push(PropertyValue {
            property,
            value: Value::Schedule(value.into()),
        });
        self
    }

    /// Adds a DECIMAL value.
    pub fn decimal(
        mut self,
        property: Id,
        exponent: i32,
        mantissa: crate::model::DecimalMantissa<'a>,
        unit: Option<Id>,
    ) -> Self {
        self.values.push(PropertyValue {
            property,
            value: Value::Decimal { exponent, mantissa, unit },
        });
        self
    }

    /// Adds an EMBEDDING value.
    pub fn embedding(
        mut self,
        property: Id,
        sub_type: crate::model::EmbeddingSubType,
        dims: usize,
        data: impl Into<Cow<'a, [u8]>>,
    ) -> Self {
        self.values.push(PropertyValue {
            property,
            value: Value::Embedding {
                sub_type,
                dims,
                data: data.into(),
            },
        });
        self
    }
}

/// Builder for UpdateEntity operations.
#[derive(Debug, Clone)]
pub struct UpdateEntityBuilder<'a> {
    id: Id,
    set_properties: Vec<PropertyValue<'a>>,
    unset_values: Vec<UnsetValue>,
}

impl<'a> UpdateEntityBuilder<'a> {
    /// Creates a new UpdateEntityBuilder for the given entity ID.
    pub fn new(id: Id) -> Self {
        Self {
            id,
            set_properties: Vec::new(),
            unset_values: Vec::new(),
        }
    }

    /// Sets a property value.
    pub fn set(mut self, property: Id, value: Value<'a>) -> Self {
        self.set_properties.push(PropertyValue { property, value });
        self
    }

    /// Sets a TEXT value.
    pub fn set_text(
        mut self,
        property: Id,
        value: impl Into<Cow<'a, str>>,
        language: Option<Id>,
    ) -> Self {
        self.set_properties.push(PropertyValue {
            property,
            value: Value::Text {
                value: value.into(),
                language,
            },
        });
        self
    }

    /// Sets an INT64 value.
    pub fn set_int64(mut self, property: Id, value: i64, unit: Option<Id>) -> Self {
        self.set_properties.push(PropertyValue {
            property,
            value: Value::Int64 { value, unit },
        });
        self
    }

    /// Sets a FLOAT64 value.
    pub fn set_float64(mut self, property: Id, value: f64, unit: Option<Id>) -> Self {
        self.set_properties.push(PropertyValue {
            property,
            value: Value::Float64 { value, unit },
        });
        self
    }

    /// Sets a BOOL value.
    pub fn set_bool(mut self, property: Id, value: bool) -> Self {
        self.set_properties.push(PropertyValue {
            property,
            value: Value::Bool(value),
        });
        self
    }

    /// Sets a POINT value.
    pub fn set_point(mut self, property: Id, lon: f64, lat: f64, alt: Option<f64>) -> Self {
        self.set_properties.push(PropertyValue {
            property,
            value: Value::Point { lon, lat, alt },
        });
        self
    }

    /// Sets a DATE value (ISO 8601 date string).
    pub fn set_date(mut self, property: Id, value: impl Into<Cow<'a, str>>) -> Self {
        self.set_properties.push(PropertyValue {
            property,
            value: Value::Date(value.into()),
        });
        self
    }

    /// Sets a TIME value (ISO 8601 time string with timezone).
    pub fn set_time(mut self, property: Id, value: impl Into<Cow<'a, str>>) -> Self {
        self.set_properties.push(PropertyValue {
            property,
            value: Value::Time(value.into()),
        });
        self
    }

    /// Sets a DATETIME value (ISO 8601 datetime string).
    pub fn set_datetime(mut self, property: Id, value: impl Into<Cow<'a, str>>) -> Self {
        self.set_properties.push(PropertyValue {
            property,
            value: Value::Datetime(value.into()),
        });
        self
    }

    /// Sets a SCHEDULE value.
    pub fn set_schedule(mut self, property: Id, value: impl Into<Cow<'a, str>>) -> Self {
        self.set_properties.push(PropertyValue {
            property,
            value: Value::Schedule(value.into()),
        });
        self
    }

    /// Sets a BYTES value.
    pub fn set_bytes(mut self, property: Id, value: impl Into<Cow<'a, [u8]>>) -> Self {
        self.set_properties.push(PropertyValue {
            property,
            value: Value::Bytes(value.into()),
        });
        self
    }

    /// Sets a DECIMAL value.
    pub fn set_decimal(
        mut self,
        property: Id,
        exponent: i32,
        mantissa: crate::model::DecimalMantissa<'a>,
        unit: Option<Id>,
    ) -> Self {
        self.set_properties.push(PropertyValue {
            property,
            value: Value::Decimal { exponent, mantissa, unit },
        });
        self
    }

    /// Sets an EMBEDDING value.
    pub fn set_embedding(
        mut self,
        property: Id,
        sub_type: crate::model::EmbeddingSubType,
        dims: usize,
        data: impl Into<Cow<'a, [u8]>>,
    ) -> Self {
        self.set_properties.push(PropertyValue {
            property,
            value: Value::Embedding {
                sub_type,
                dims,
                data: data.into(),
            },
        });
        self
    }

    /// Unsets a specific property+language combination.
    pub fn unset(mut self, property: Id, language: UnsetLanguage) -> Self {
        self.unset_values.push(UnsetValue { property, language });
        self
    }

    /// Unsets all values for a property (all languages).
    pub fn unset_all(mut self, property: Id) -> Self {
        self.unset_values.push(UnsetValue {
            property,
            language: UnsetLanguage::All,
        });
        self
    }

    /// Unsets the English value for a property.
    pub fn unset_english(mut self, property: Id) -> Self {
        self.unset_values.push(UnsetValue {
            property,
            language: UnsetLanguage::English,
        });
        self
    }

    /// Unsets a specific language for a property.
    pub fn unset_language(mut self, property: Id, language: Id) -> Self {
        self.unset_values.push(UnsetValue {
            property,
            language: UnsetLanguage::Specific(language),
        });
        self
    }
}

/// Builder for CreateRelation operations with full control.
#[derive(Debug, Clone, Default)]
pub struct RelationBuilder<'a> {
    id: Option<Id>,
    relation_type: Option<Id>,
    from: Option<Id>,
    from_is_value_ref: bool,
    to: Option<Id>,
    to_is_value_ref: bool,
    entity: Option<Id>,
    position: Option<Cow<'a, str>>,
    from_space: Option<Id>,
    from_version: Option<Id>,
    to_space: Option<Id>,
    to_version: Option<Id>,
}

impl<'a> RelationBuilder<'a> {
    /// Creates a new empty RelationBuilder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the relation ID.
    pub fn id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the relation type.
    pub fn relation_type(mut self, id: Id) -> Self {
        self.relation_type = Some(id);
        self
    }

    /// Sets the source entity.
    pub fn from(mut self, id: Id) -> Self {
        self.from = Some(id);
        self
    }

    /// Sets the target entity.
    pub fn to(mut self, id: Id) -> Self {
        self.to = Some(id);
        self
    }

    /// Sets an explicit reified entity ID.
    pub fn entity(mut self, id: Id) -> Self {
        self.entity = Some(id);
        self
    }

    /// Sets the position string for ordering.
    pub fn position(mut self, pos: impl Into<Cow<'a, str>>) -> Self {
        self.position = Some(pos.into());
        self
    }

    /// Sets the from_space pin.
    pub fn from_space(mut self, space_id: Id) -> Self {
        self.from_space = Some(space_id);
        self
    }

    /// Sets the from_version pin.
    pub fn from_version(mut self, version_id: Id) -> Self {
        self.from_version = Some(version_id);
        self
    }

    /// Sets the to_space pin.
    pub fn to_space(mut self, space_id: Id) -> Self {
        self.to_space = Some(space_id);
        self
    }

    /// Sets the to_version pin.
    pub fn to_version(mut self, version_id: Id) -> Self {
        self.to_version = Some(version_id);
        self
    }

    /// Builds the CreateRelation, returning None if required fields are missing.
    pub fn build(self) -> Option<CreateRelation<'a>> {
        Some(CreateRelation {
            id: self.id?,
            relation_type: self.relation_type?,
            from: self.from?,
            from_is_value_ref: self.from_is_value_ref,
            to: self.to?,
            to_is_value_ref: self.to_is_value_ref,
            entity: self.entity,
            position: self.position,
            from_space: self.from_space,
            from_version: self.from_version,
            to_space: self.to_space,
            to_version: self.to_version,
        })
    }

    /// Sets `from` as a value ref ID (inline encoding).
    pub fn from_value_ref(mut self, id: Id) -> Self {
        self.from = Some(id);
        self.from_is_value_ref = true;
        self
    }

    /// Sets `to` as a value ref ID (inline encoding).
    pub fn to_value_ref(mut self, id: Id) -> Self {
        self.to = Some(id);
        self.to_is_value_ref = true;
        self
    }
}

/// Builder for UpdateRelation operations.
#[derive(Debug, Clone)]
pub struct UpdateRelationBuilder<'a> {
    id: Id,
    from_space: Option<Id>,
    from_version: Option<Id>,
    to_space: Option<Id>,
    to_version: Option<Id>,
    position: Option<Cow<'a, str>>,
    unset: Vec<UnsetRelationField>,
}

impl<'a> UpdateRelationBuilder<'a> {
    /// Creates a new UpdateRelationBuilder for the given relation ID.
    pub fn new(id: Id) -> Self {
        Self {
            id,
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
            position: None,
            unset: Vec::new(),
        }
    }

    /// Sets the from_space pin.
    pub fn set_from_space(mut self, space_id: Id) -> Self {
        self.from_space = Some(space_id);
        self
    }

    /// Sets the from_version pin.
    pub fn set_from_version(mut self, version_id: Id) -> Self {
        self.from_version = Some(version_id);
        self
    }

    /// Sets the to_space pin.
    pub fn set_to_space(mut self, space_id: Id) -> Self {
        self.to_space = Some(space_id);
        self
    }

    /// Sets the to_version pin.
    pub fn set_to_version(mut self, version_id: Id) -> Self {
        self.to_version = Some(version_id);
        self
    }

    /// Sets the position for ordering.
    pub fn set_position(mut self, pos: impl Into<Cow<'a, str>>) -> Self {
        self.position = Some(pos.into());
        self
    }

    /// Unsets the from_space pin.
    pub fn unset_from_space(mut self) -> Self {
        self.unset.push(UnsetRelationField::FromSpace);
        self
    }

    /// Unsets the from_version pin.
    pub fn unset_from_version(mut self) -> Self {
        self.unset.push(UnsetRelationField::FromVersion);
        self
    }

    /// Unsets the to_space pin.
    pub fn unset_to_space(mut self) -> Self {
        self.unset.push(UnsetRelationField::ToSpace);
        self
    }

    /// Unsets the to_version pin.
    pub fn unset_to_version(mut self) -> Self {
        self.unset.push(UnsetRelationField::ToVersion);
        self
    }

    /// Unsets the position.
    pub fn unset_position(mut self) -> Self {
        self.unset.push(UnsetRelationField::Position);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_builder_basic() {
        let edit_id = [1u8; 16];
        let author_id = [2u8; 16];
        let entity_id = [3u8; 16];
        let prop_id = [4u8; 16];

        let edit = EditBuilder::new(edit_id)
            .name("Test Edit")
            .author(author_id)
            .created_at(1234567890)
            .create_entity(entity_id, |e| {
                e.text(prop_id, "Hello", None)
                    .int64([5u8; 16], 42, None)
            })
            .build();

        assert_eq!(edit.id, edit_id);
        assert_eq!(edit.name, "Test Edit");
        assert_eq!(edit.authors, vec![author_id]);
        assert_eq!(edit.created_at, 1234567890);
        assert_eq!(edit.ops.len(), 1);

        match &edit.ops[0] {
            Op::CreateEntity(ce) => {
                assert_eq!(ce.id, entity_id);
                assert_eq!(ce.values.len(), 2);
            }
            _ => panic!("Expected CreateEntity"),
        }
    }

    #[test]
    fn test_edit_builder_relations() {
        let edit = EditBuilder::new([1u8; 16])
            .create_relation_simple([5u8; 16], [2u8; 16], [3u8; 16], [4u8; 16])
            .create_relation_simple([6u8; 16], [2u8; 16], [3u8; 16], [4u8; 16])
            .build();

        assert_eq!(edit.ops.len(), 2);

        match &edit.ops[0] {
            Op::CreateRelation(cr) => {
                assert_eq!(cr.id, [5u8; 16]);
            }
            _ => panic!("Expected CreateRelation"),
        }

        match &edit.ops[1] {
            Op::CreateRelation(cr) => {
                assert_eq!(cr.id, [6u8; 16]);
            }
            _ => panic!("Expected CreateRelation"),
        }
    }

    #[test]
    fn test_update_entity_builder() {
        let entity_id = [1u8; 16];
        let prop_id = [2u8; 16];

        let edit = EditBuilder::new([0u8; 16])
            .update_entity(entity_id, |u| {
                u.set_text(prop_id, "New value", None)
                    .unset_all([3u8; 16])
            })
            .build();

        assert_eq!(edit.ops.len(), 1);

        match &edit.ops[0] {
            Op::UpdateEntity(ue) => {
                assert_eq!(ue.id, entity_id);
                assert_eq!(ue.set_properties.len(), 1);
                assert_eq!(ue.unset_values.len(), 1);
            }
            _ => panic!("Expected UpdateEntity"),
        }
    }

    #[test]
    fn test_relation_builder_full() {
        let edit = EditBuilder::new([0u8; 16])
            .create_relation(|r| {
                r.id([1u8; 16])
                    .from([2u8; 16])
                    .to([3u8; 16])
                    .relation_type([4u8; 16])
                    .entity([5u8; 16])
                    .position("aaa")
                    .from_space([6u8; 16])
            })
            .build();

        assert_eq!(edit.ops.len(), 1);

        match &edit.ops[0] {
            Op::CreateRelation(cr) => {
                assert_eq!(cr.id, [1u8; 16]);
                assert_eq!(cr.entity, Some([5u8; 16]));
                assert_eq!(cr.position.as_deref(), Some("aaa"));
                assert_eq!(cr.from_space, Some([6u8; 16]));
            }
            _ => panic!("Expected CreateRelation"),
        }
    }

    #[test]
    fn test_entity_builder_all_types() {
        let edit = EditBuilder::new([0u8; 16])
            .create_entity([1u8; 16], |e| {
                e.text([2u8; 16], "text", None)
                    .int64([3u8; 16], 123, None)
                    .float64([4u8; 16], 3.14, None)
                    .bool([5u8; 16], true)
                    .point([6u8; 16], -74.0060, 40.7128, None)
                    .date([7u8; 16], "2024-01-15")
                    .schedule([8u8; 16], "BEGIN:VEVENT\r\nDTSTART:20240315T090000Z\r\nEND:VEVENT")
                    .bytes([9u8; 16], vec![1, 2, 3, 4])
            })
            .build();

        match &edit.ops[0] {
            Op::CreateEntity(ce) => {
                assert_eq!(ce.values.len(), 8);
            }
            _ => panic!("Expected CreateEntity"),
        }
    }
}
