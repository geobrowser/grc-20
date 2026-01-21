//! Semantic validation for GRC-20 edits.
//!
//! This module provides validation beyond structural encoding checks.
//! Structural validation happens during decode; semantic validation
//! requires additional context (schema, entity state).
//!
//! **Note:** With the per-edit typing model, type enforcement is advisory.
//! The protocol does not enforce that a property always uses the same type
//! across edits. Applications can use SchemaContext to opt-in to type checking.

use std::collections::HashMap;

use crate::error::ValidationError;
use crate::model::{DataType, Edit, Id, Op, PropertyValue, Value};

/// Schema context for semantic validation.
///
/// Applications can use this to register expected types for properties
/// and validate that values match those types. This is advisory—the
/// protocol does not enforce global type consistency.
#[derive(Debug, Clone, Default)]
pub struct SchemaContext {
    /// Known property data types (advisory).
    properties: HashMap<Id, DataType>,
}

impl SchemaContext {
    /// Creates a new empty schema context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a property with its expected data type.
    pub fn add_property(&mut self, id: Id, data_type: DataType) {
        self.properties.insert(id, data_type);
    }

    /// Gets the expected data type for a property, if registered.
    pub fn get_property_type(&self, id: &Id) -> Option<DataType> {
        self.properties.get(id).copied()
    }
}

/// Validates an edit against a schema context.
///
/// This performs semantic validation that requires context:
/// - Value types match property data types (when registered in schema)
///
/// Note: Type checking is advisory. Unknown properties are allowed.
/// Entity lifecycle (DELETED/ACTIVE) validation requires state context
/// and is not performed here.
pub fn validate_edit(edit: &Edit, schema: &SchemaContext) -> Result<(), ValidationError> {
    for op in &edit.ops {
        match op {
            Op::CreateEntity(ce) => {
                validate_property_values(&ce.values, schema)?;
            }
            Op::UpdateEntity(ue) => {
                validate_property_values(&ue.set_properties, schema)?;
            }
            _ => {}
        }
    }

    Ok(())
}

/// Validates that property values match their declared types.
fn validate_property_values(
    values: &[PropertyValue],
    schema: &SchemaContext,
) -> Result<(), ValidationError> {
    for pv in values {
        if let Some(expected_type) = schema.get_property_type(&pv.property) {
            let actual_type = pv.value.data_type();
            if expected_type != actual_type {
                return Err(ValidationError::TypeMismatch {
                    property: pv.property,
                    expected: expected_type,
                });
            }
        }
        // Note: If property is not in schema, we allow it (might be defined elsewhere)
    }
    Ok(())
}

/// Validates a single value (independent of property context).
///
/// This checks value-level constraints like:
/// - NaN not allowed in floats
/// - Point bounds
/// - Decimal normalization
/// - Position string format
pub fn validate_value(value: &Value) -> Option<&'static str> {
    value.validate()
}

/// Validates a position string according to spec rules.
///
/// Position strings must:
/// - Only contain characters 0-9, A-Z, a-z (62 chars)
/// - Not exceed 64 characters
pub fn validate_position(pos: &str) -> Result<(), &'static str> {
    crate::model::validate_position(pos)
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::model::CreateEntity;

    #[test]
    fn test_validate_type_mismatch() {
        let mut schema = SchemaContext::new();
        schema.add_property([1u8; 16], DataType::Int64);

        let edit = Edit {
            id: [0u8; 16],
            name: Cow::Borrowed(""),
            authors: vec![],
            created_at: 0,
                        ops: vec![Op::CreateEntity(CreateEntity {
                id: [2u8; 16],
                values: vec![PropertyValue {
                    property: [1u8; 16],
                    value: Value::Text {
                        value: Cow::Owned("not an int".to_string()),
                        language: None,
                    },
                }],
                context: None,
            })],
        };

        let result = validate_edit(&edit, &schema);
        assert!(matches!(result, Err(ValidationError::TypeMismatch { .. })));
    }

    #[test]
    fn test_validate_type_match() {
        let mut schema = SchemaContext::new();
        schema.add_property([1u8; 16], DataType::Int64);

        let edit = Edit {
            id: [0u8; 16],
            name: Cow::Borrowed(""),
            authors: vec![],
            created_at: 0,
                        ops: vec![Op::CreateEntity(CreateEntity {
                id: [2u8; 16],
                values: vec![PropertyValue {
                    property: [1u8; 16],
                    value: Value::Int64 { value: 42, unit: None },
                }],
                context: None,
            })],
        };

        let result = validate_edit(&edit, &schema);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_unknown_property() {
        let schema = SchemaContext::new(); // Empty schema

        let edit = Edit {
            id: [0u8; 16],
            name: Cow::Borrowed(""),
            authors: vec![],
            created_at: 0,
                        ops: vec![Op::CreateEntity(CreateEntity {
                id: [2u8; 16],
                values: vec![PropertyValue {
                    property: [99u8; 16], // Unknown property
                    value: Value::Text {
                        value: Cow::Owned("test".to_string()),
                        language: None,
                    },
                }],
                context: None,
            })],
        };

        // Unknown properties are allowed (advisory type checking)
        let result = validate_edit(&edit, &schema);
        assert!(result.is_ok());
    }
}
