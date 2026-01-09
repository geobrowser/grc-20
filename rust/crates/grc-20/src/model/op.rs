//! Operation types for GRC-20 state changes.
//!
//! All state changes in GRC-20 are expressed as operations (ops).

use std::borrow::Cow;

use crate::model::{DataType, Id, PropertyValue};

/// An atomic operation that modifies graph state (spec Section 3.1).
#[derive(Debug, Clone, PartialEq)]
pub enum Op<'a> {
    CreateEntity(CreateEntity<'a>),
    UpdateEntity(UpdateEntity<'a>),
    DeleteEntity(DeleteEntity),
    CreateRelation(CreateRelation<'a>),
    UpdateRelation(UpdateRelation<'a>),
    DeleteRelation(DeleteRelation),
    CreateProperty(CreateProperty),
}

impl Op<'_> {
    /// Returns the op type code for wire encoding.
    pub fn op_type(&self) -> u8 {
        match self {
            Op::CreateEntity(_) => 1,
            Op::UpdateEntity(_) => 2,
            Op::DeleteEntity(_) => 3,
            Op::CreateRelation(_) => 4,
            Op::UpdateRelation(_) => 5,
            Op::DeleteRelation(_) => 6,
            Op::CreateProperty(_) => 7,
        }
    }
}

/// Creates a new entity (spec Section 3.2).
///
/// If the entity does not exist, creates it. If it already exists,
/// this acts as an update: values are applied as set_properties (LWW).
#[derive(Debug, Clone, PartialEq)]
pub struct CreateEntity<'a> {
    /// The entity's unique identifier.
    pub id: Id,
    /// Initial values for the entity.
    pub values: Vec<PropertyValue<'a>>,
}

/// Updates an existing entity (spec Section 3.2).
///
/// Application order within op:
/// 1. unset_properties
/// 2. set_properties
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UpdateEntity<'a> {
    /// The entity to update.
    pub id: Id,
    /// Replace value for these properties (LWW).
    pub set_properties: Vec<PropertyValue<'a>>,
    /// Clear values for these properties (optionally specific language for TEXT).
    pub unset_properties: Vec<UnsetProperty>,
}

/// Specifies a property to unset, optionally for a specific language (TEXT only).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnsetProperty {
    /// The property to clear.
    pub property: Id,
    /// For TEXT properties: if Some, clear only that language; if None, clear all languages.
    /// For non-TEXT properties: must be None.
    pub language: Option<Id>,
}

impl UnsetProperty {
    /// Creates an UnsetProperty that clears all values for a property.
    pub fn all(property: Id) -> Self {
        Self { property, language: None }
    }

    /// Creates an UnsetProperty that clears a specific language for a TEXT property.
    pub fn language(property: Id, language: Id) -> Self {
        Self { property, language: Some(language) }
    }
}

impl<'a> UpdateEntity<'a> {
    /// Creates a new UpdateEntity for the given entity ID.
    pub fn new(id: Id) -> Self {
        Self {
            id,
            set_properties: Vec::new(),
            unset_properties: Vec::new(),
        }
    }

    /// Returns true if this update has no actual changes.
    pub fn is_empty(&self) -> bool {
        self.set_properties.is_empty() && self.unset_properties.is_empty()
    }
}


/// Deletes an entity (spec Section 3.2).
///
/// Appends a tombstone to history. Subsequent updates are ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteEntity {
    /// The entity to delete.
    pub id: Id,
}

/// Relation ID mode for CreateRelation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationIdMode {
    /// Caller-provided ID. Multiple relations can exist between same endpoints.
    Many(Id),
    /// Deterministic ID derived from from_id || to_id || type_id.
    Unique,
}

/// Creates a new relation (spec Section 3.3).
///
/// Also implicitly creates the reified entity if it doesn't exist.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateRelation<'a> {
    /// The relation ID mode.
    pub id_mode: RelationIdMode,
    /// The relation type entity ID.
    pub relation_type: Id,
    /// Source entity ID.
    pub from: Id,
    /// Target entity ID.
    pub to: Id,
    /// Explicit reified entity ID (many mode only).
    /// If None, entity ID is auto-derived from the relation ID.
    /// Must be None in unique mode.
    pub entity: Option<Id>,
    /// Optional ordering position (fractional indexing).
    pub position: Option<Cow<'a, str>>,
    /// Optional space hint for source entity.
    pub from_space: Option<Id>,
    /// Optional version (edit ID) to pin source entity.
    pub from_version: Option<Id>,
    /// Optional space hint for target entity.
    pub to_space: Option<Id>,
    /// Optional version (edit ID) to pin target entity.
    pub to_version: Option<Id>,
}

impl CreateRelation<'_> {
    /// Computes the actual relation ID.
    ///
    /// For many mode, returns the provided ID.
    /// For unique mode, derives the ID from from || to || type.
    pub fn relation_id(&self) -> Id {
        use crate::model::id::unique_relation_id;
        match &self.id_mode {
            RelationIdMode::Many(id) => *id,
            RelationIdMode::Unique => unique_relation_id(&self.from, &self.to, &self.relation_type),
        }
    }

    /// Computes the reified entity ID.
    ///
    /// If explicit entity is provided, returns it.
    /// Otherwise, derives it from the relation ID.
    pub fn entity_id(&self) -> Id {
        use crate::model::id::relation_entity_id;
        match self.entity {
            Some(id) => id,
            None => relation_entity_id(&self.relation_id()),
        }
    }

    /// Returns true if this relation has an explicit entity ID.
    pub fn has_explicit_entity(&self) -> bool {
        self.entity.is_some()
    }
}

/// Updates a relation's position (spec Section 3.3).
///
/// All other fields (entity, type, from, to, space hints, version pins) are immutable.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateRelation<'a> {
    /// The relation to update.
    pub id: Id,
    /// Optional new position for ordering.
    pub position: Option<Cow<'a, str>>,
}

/// Deletes a relation (spec Section 3.3).
///
/// Appends a tombstone. Does NOT delete the reified entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteRelation {
    /// The relation to delete.
    pub id: Id,
}

/// Creates a new property in the schema (spec Section 3.4).
///
/// Properties are immutable once created (first-writer-wins).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateProperty {
    /// The property's unique identifier.
    pub id: Id,
    /// The data type for values of this property.
    pub data_type: DataType,
}

/// Validates a position string according to spec rules.
///
/// Position strings must:
/// - Only contain characters 0-9, A-Z, a-z (62 chars, ASCII order)
/// - Not exceed 64 characters
pub fn validate_position(pos: &str) -> Result<(), &'static str> {
    if pos.len() > 64 {
        return Err("position exceeds 64 characters");
    }
    for c in pos.chars() {
        if !c.is_ascii_alphanumeric() {
            return Err("position contains invalid character");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_op_type_codes() {
        assert_eq!(
            Op::CreateEntity(CreateEntity {
                id: [0; 16],
                values: vec![]
            })
            .op_type(),
            1
        );
        assert_eq!(Op::UpdateEntity(UpdateEntity::new([0; 16])).op_type(), 2);
        assert_eq!(Op::DeleteEntity(DeleteEntity { id: [0; 16] }).op_type(), 3);
    }

    #[test]
    fn test_validate_position() {
        assert!(validate_position("abc123").is_ok());
        assert!(validate_position("aV").is_ok());
        assert!(validate_position("").is_ok());
        assert!(validate_position("a").is_ok());

        // Invalid characters
        assert!(validate_position("abc-123").is_err());
        assert!(validate_position("abc_123").is_err());
        assert!(validate_position("abc 123").is_err());

        // Too long (65 chars)
        let long = "a".repeat(65);
        assert!(validate_position(&long).is_err());

        // Exactly 64 chars is ok
        let exact = "a".repeat(64);
        assert!(validate_position(&exact).is_ok());
    }

    #[test]
    fn test_update_entity_is_empty() {
        let update = UpdateEntity::new([0; 16]);
        assert!(update.is_empty());

        let mut update2 = UpdateEntity::new([0; 16]);
        update2.set_properties.push(PropertyValue {
            property: [1; 16],
            value: crate::model::Value::Bool(true),
        });
        assert!(!update2.is_empty());
    }

    #[test]
    fn test_relation_id_computation() {
        use crate::model::id::unique_relation_id;

        let from = [1u8; 16];
        let to = [2u8; 16];
        let rel_type = [3u8; 16];

        // Many mode returns the provided ID
        let many_id = [5u8; 16];
        let rel_many = CreateRelation {
            id_mode: RelationIdMode::Many(many_id),
            relation_type: rel_type,
            from,
            to,
            entity: None,
            position: None,
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
        };
        assert_eq!(rel_many.relation_id(), many_id);

        // Unique mode derives the ID
        let rel_unique = CreateRelation {
            id_mode: RelationIdMode::Unique,
            relation_type: rel_type,
            from,
            to,
            entity: None,
            position: None,
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
        };
        assert_eq!(rel_unique.relation_id(), unique_relation_id(&from, &to, &rel_type));
    }

    #[test]
    fn test_entity_id_derivation() {
        use crate::model::id::relation_entity_id;

        let from = [1u8; 16];
        let to = [2u8; 16];
        let rel_type = [3u8; 16];
        let many_id = [5u8; 16];

        // Auto-derived entity (entity = None)
        let rel_auto = CreateRelation {
            id_mode: RelationIdMode::Many(many_id),
            relation_type: rel_type,
            from,
            to,
            entity: None,
            position: None,
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
        };
        assert_eq!(rel_auto.entity_id(), relation_entity_id(&many_id));
        assert!(!rel_auto.has_explicit_entity());

        // Explicit entity
        let explicit_entity = [6u8; 16];
        let rel_explicit = CreateRelation {
            id_mode: RelationIdMode::Many(many_id),
            relation_type: rel_type,
            from,
            to,
            entity: Some(explicit_entity),
            position: None,
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
        };
        assert_eq!(rel_explicit.entity_id(), explicit_entity);
        assert!(rel_explicit.has_explicit_entity());

        // Unique mode with auto entity
        let rel_unique = CreateRelation {
            id_mode: RelationIdMode::Unique,
            relation_type: rel_type,
            from,
            to,
            entity: None,
            position: None,
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
        };
        let expected_rel_id = rel_unique.relation_id();
        assert_eq!(rel_unique.entity_id(), relation_entity_id(&expected_rel_id));
    }
}
