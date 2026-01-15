//! Operation types for GRC-20 state changes.
//!
//! All state changes in GRC-20 are expressed as operations (ops).

use std::borrow::Cow;

use crate::model::{Id, PropertyValue};

/// An atomic operation that modifies graph state (spec Section 3.1).
#[derive(Debug, Clone, PartialEq)]
pub enum Op<'a> {
    CreateEntity(CreateEntity<'a>),
    UpdateEntity(UpdateEntity<'a>),
    DeleteEntity(DeleteEntity),
    RestoreEntity(RestoreEntity),
    CreateRelation(CreateRelation<'a>),
    UpdateRelation(UpdateRelation<'a>),
    DeleteRelation(DeleteRelation),
    RestoreRelation(RestoreRelation),
    CreateValueRef(CreateValueRef),
}

impl Op<'_> {
    /// Returns the op type code for wire encoding.
    pub fn op_type(&self) -> u8 {
        match self {
            Op::CreateEntity(_) => 1,
            Op::UpdateEntity(_) => 2,
            Op::DeleteEntity(_) => 3,
            Op::RestoreEntity(_) => 4,
            Op::CreateRelation(_) => 5,
            Op::UpdateRelation(_) => 6,
            Op::DeleteRelation(_) => 7,
            Op::RestoreRelation(_) => 8,
            Op::CreateValueRef(_) => 9,
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
/// 1. unset_values
/// 2. set_properties
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UpdateEntity<'a> {
    /// The entity to update.
    pub id: Id,
    /// Replace value for these properties (LWW).
    pub set_properties: Vec<PropertyValue<'a>>,
    /// Clear values for these properties (optionally specific language for TEXT).
    pub unset_values: Vec<UnsetValue>,
}

/// Specifies which language slot to clear for an UnsetValue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsetLanguage {
    /// Clear all language slots (wire format: 0xFFFFFFFF).
    All,
    /// Clear only the non-linguistic slot (wire format: 0).
    NonLinguistic,
    /// Clear a specific language slot (wire format: 1+).
    Specific(Id),
}

impl Default for UnsetLanguage {
    fn default() -> Self {
        Self::All
    }
}

/// Specifies a value to unset, with optional language targeting (TEXT only).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnsetValue {
    /// The property whose value to clear.
    pub property: Id,
    /// Which language slot(s) to clear.
    /// For TEXT properties: All clears all slots, NonLinguistic clears non-linguistic slot,
    ///   Specific clears a specific language slot.
    /// For non-TEXT properties: must be All.
    pub language: UnsetLanguage,
}

impl UnsetValue {
    /// Creates an UnsetValue that clears all values for a property.
    pub fn all(property: Id) -> Self {
        Self { property, language: UnsetLanguage::All }
    }

    /// Creates an UnsetValue that clears the non-linguistic slot for a TEXT property.
    pub fn non_linguistic(property: Id) -> Self {
        Self { property, language: UnsetLanguage::NonLinguistic }
    }

    /// Creates an UnsetValue that clears a specific language for a TEXT property.
    pub fn language(property: Id, language: Id) -> Self {
        Self { property, language: UnsetLanguage::Specific(language) }
    }
}

impl<'a> UpdateEntity<'a> {
    /// Creates a new UpdateEntity for the given entity ID.
    pub fn new(id: Id) -> Self {
        Self {
            id,
            set_properties: Vec::new(),
            unset_values: Vec::new(),
        }
    }

    /// Returns true if this update has no actual changes.
    pub fn is_empty(&self) -> bool {
        self.set_properties.is_empty() && self.unset_values.is_empty()
    }
}


/// Deletes an entity (spec Section 3.2).
///
/// Transitions the entity to DELETED state. Subsequent updates are ignored
/// until restored via RestoreEntity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteEntity {
    /// The entity to delete.
    pub id: Id,
}

/// Restores a deleted entity (spec Section 3.2).
///
/// Transitions a DELETED entity back to ACTIVE state.
/// If the entity is ACTIVE or does not exist, this is a no-op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreEntity {
    /// The entity to restore.
    pub id: Id,
}

/// Creates a new relation (spec Section 3.3).
///
/// Also implicitly creates the reified entity if it doesn't exist.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateRelation<'a> {
    /// The relation's unique identifier.
    pub id: Id,
    /// The relation type entity ID.
    pub relation_type: Id,
    /// Source entity or value ref ID.
    pub from: Id,
    /// If true, `from` is a value ref ID (inline encoding).
    /// If false, `from` is an entity ID (ObjectRef encoding).
    pub from_is_value_ref: bool,
    /// Optional space pin for source entity.
    pub from_space: Option<Id>,
    /// Optional version (edit ID) to pin source entity.
    pub from_version: Option<Id>,
    /// Target entity or value ref ID.
    pub to: Id,
    /// If true, `to` is a value ref ID (inline encoding).
    /// If false, `to` is an entity ID (ObjectRef encoding).
    pub to_is_value_ref: bool,
    /// Optional space pin for target entity.
    pub to_space: Option<Id>,
    /// Optional version (edit ID) to pin target entity.
    pub to_version: Option<Id>,
    /// Explicit reified entity ID.
    /// If None, entity ID is auto-derived from the relation ID.
    pub entity: Option<Id>,
    /// Optional ordering position (fractional indexing).
    pub position: Option<Cow<'a, str>>,
}

impl CreateRelation<'_> {
    /// Computes the reified entity ID.
    ///
    /// If explicit entity is provided, returns it.
    /// Otherwise, derives it from the relation ID.
    pub fn entity_id(&self) -> Id {
        use crate::model::id::relation_entity_id;
        match self.entity {
            Some(id) => id,
            None => relation_entity_id(&self.id),
        }
    }

    /// Returns true if this relation has an explicit entity ID.
    pub fn has_explicit_entity(&self) -> bool {
        self.entity.is_some()
    }
}

/// Fields that can be unset on a relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnsetRelationField {
    FromSpace,
    FromVersion,
    ToSpace,
    ToVersion,
    Position,
}

/// Updates a relation's mutable fields (spec Section 3.3).
///
/// The structural fields (entity, type, from, to) are immutable.
/// The space pins, version pins, and position can be updated or unset.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UpdateRelation<'a> {
    /// The relation to update.
    pub id: Id,
    /// Set space pin for source entity.
    pub from_space: Option<Id>,
    /// Set version pin for source entity.
    pub from_version: Option<Id>,
    /// Set space pin for target entity.
    pub to_space: Option<Id>,
    /// Set version pin for target entity.
    pub to_version: Option<Id>,
    /// Set position for ordering.
    pub position: Option<Cow<'a, str>>,
    /// Fields to clear/unset.
    pub unset: Vec<UnsetRelationField>,
}

impl UpdateRelation<'_> {
    /// Creates a new UpdateRelation for the given relation ID.
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

    /// Returns true if this update has no actual changes.
    pub fn is_empty(&self) -> bool {
        self.from_space.is_none()
            && self.from_version.is_none()
            && self.to_space.is_none()
            && self.to_version.is_none()
            && self.position.is_none()
            && self.unset.is_empty()
    }
}

/// Deletes a relation (spec Section 3.3).
///
/// Transitions the relation to DELETED state. Does NOT delete the reified entity.
/// Subsequent updates are ignored until restored via RestoreRelation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteRelation {
    /// The relation to delete.
    pub id: Id,
}

/// Restores a deleted relation (spec Section 3.3).
///
/// Transitions a DELETED relation back to ACTIVE state.
/// If the relation is ACTIVE or does not exist, this is a no-op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreRelation {
    /// The relation to restore.
    pub id: Id,
}

/// Creates a referenceable ID for a value slot (spec Section 3.4).
///
/// This enables relations to target specific values for provenance,
/// confidence, attribution, or other qualifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateValueRef {
    /// The value ref's unique identifier.
    pub id: Id,
    /// The entity holding the value.
    pub entity: Id,
    /// The property of the value.
    pub property: Id,
    /// The language (TEXT values only).
    pub language: Option<Id>,
    /// The space containing the value (default: current space).
    pub space: Option<Id>,
}

/// Validates a position string according to spec rules.
///
/// Position strings must:
/// - Not be empty
/// - Only contain characters 0-9, A-Z, a-z (62 chars, ASCII order)
/// - Not exceed 64 characters
pub fn validate_position(pos: &str) -> Result<(), &'static str> {
    if pos.is_empty() {
        return Err("position cannot be empty");
    }
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
        assert!(validate_position("a").is_ok());

        // Empty is not allowed
        assert!(validate_position("").is_err());

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
    fn test_entity_id_derivation() {
        use crate::model::id::relation_entity_id;

        let rel_id = [5u8; 16];
        let from = [1u8; 16];
        let to = [2u8; 16];
        let rel_type = [3u8; 16];

        // Auto-derived entity (entity = None)
        let rel_auto = CreateRelation {
            id: rel_id,
            relation_type: rel_type,
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
        };
        assert_eq!(rel_auto.entity_id(), relation_entity_id(&rel_id));
        assert!(!rel_auto.has_explicit_entity());

        // Explicit entity
        let explicit_entity = [6u8; 16];
        let rel_explicit = CreateRelation {
            id: rel_id,
            relation_type: rel_type,
            from,
            from_is_value_ref: false,
            to,
            to_is_value_ref: false,
            entity: Some(explicit_entity),
            position: None,
            from_space: None,
            from_version: None,
            to_space: None,
            to_version: None,
        };
        assert_eq!(rel_explicit.entity_id(), explicit_entity);
        assert!(rel_explicit.has_explicit_entity());
    }

    #[test]
    fn test_update_relation_is_empty() {
        let update = UpdateRelation::new([0; 16]);
        assert!(update.is_empty());

        let mut update2 = UpdateRelation::new([0; 16]);
        update2.from_space = Some([1; 16]);
        assert!(!update2.is_empty());

        let mut update3 = UpdateRelation::new([0; 16]);
        update3.unset.push(UnsetRelationField::Position);
        assert!(!update3.is_empty());
    }
}
