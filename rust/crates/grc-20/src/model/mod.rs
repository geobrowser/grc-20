//! Data model types for GRC-20.
//!
//! This module contains all the core types for representing GRC-20 data:
//! - Identifiers (UUIDs)
//! - Values (typed property instances)
//! - Operations (state changes)
//! - Edits (batched operations)
//! - Builders (ergonomic construction)

pub mod builder;
pub mod edit;
pub mod id;
pub mod op;
pub mod value;

pub use builder::{EditBuilder, EntityBuilder, RelationBuilder, UpdateEntityBuilder};
pub use edit::{Context, ContextEdge, DictionaryBuilder, Edit, WireDictionaries};
pub use id::{derived_uuid, format_id, parse_id, relation_entity_id, text_value_id, value_id, Id, NIL_ID};
pub use op::{
    validate_position, CreateEntity, CreateRelation, CreateValueRef, DeleteEntity, DeleteRelation,
    Op, RestoreEntity, RestoreRelation, UnsetLanguage, UnsetRelationField, UnsetValue, UpdateEntity,
    UpdateRelation,
};
pub use value::{DataType, DecimalMantissa, EmbeddingSubType, Property, PropertyValue, Value};
