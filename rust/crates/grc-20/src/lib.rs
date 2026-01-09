//! GRC-20 v2: Binary property graph format for decentralized knowledge networks.
//!
//! This crate provides encoding, decoding, and validation for the GRC-20 v2
//! binary format as specified in the GRC-20 v2 Specification.
//!
//! # Overview
//!
//! GRC-20 is a property graph format designed for:
//! - **Event-sourced data**: All state changes are expressed as operations
//! - **Binary-first**: Optimized for compressed wire size and decode speed
//! - **Pluralistic**: Multiple spaces can hold conflicting views
//!
//! # Quick Start
//!
//! ```rust
//! use grc_20::{Edit, Op, CreateEntity, PropertyValue, Value, DataType};
//! use grc_20::codec::{encode_edit, decode_edit};
//! use grc_20::genesis::properties;
//!
//! // Create an edit with an entity
//! let edit = Edit {
//!     id: [1u8; 16],
//!     name: "My Edit".to_string(),
//!     authors: vec![[2u8; 16]],
//!     created_at: 1234567890,
//!     ops: vec![
//!         Op::CreateEntity(CreateEntity {
//!             id: [3u8; 16],
//!             values: vec![PropertyValue {
//!                 property: properties::name(),
//!                 value: Value::Text {
//!                     value: "Alice".to_string(),
//!                     language: None,
//!                 },
//!             }],
//!         }),
//!     ],
//! };
//!
//! // Encode to binary
//! let bytes = encode_edit(&edit).unwrap();
//!
//! // Decode back
//! let decoded = decode_edit(&bytes).unwrap();
//! assert_eq!(edit.id, decoded.id);
//! ```
//!
//! # Modules
//!
//! - [`model`]: Core data types (Entity, Relation, Value, Op, Edit)
//! - [`codec`]: Binary encoding/decoding with compression support
//! - [`validate`]: Semantic validation
//! - [`genesis`]: Well-known IDs from the Genesis Space
//! - [`error`]: Error types
//! - [`limits`]: Security limits for decoding
//!
//! # Security
//!
//! The decoder is designed to safely handle untrusted input:
//! - All allocations are bounded by configurable limits
//! - Varints are limited to prevent overflow
//! - Invalid data is rejected with descriptive errors
//!
//! # Wire Format
//!
//! Edits use a binary format with optional zstd compression:
//! - Uncompressed: `GRC2` magic + version + data
//! - Compressed: `GRC2Z` magic + uncompressed size + zstd data
//!
//! The decoder automatically detects and handles both formats.

pub mod codec;
pub mod error;
pub mod genesis;
pub mod limits;
pub mod model;
pub mod validate;

// Re-export commonly used types at crate root
pub use codec::{decode_edit, encode_edit, encode_edit_compressed, encode_edit_profiled};
pub use error::{DecodeError, EncodeError, ValidationError};
pub use model::{
    CreateEntity, CreateProperty, CreateRelation, DataType, DecimalMantissa, DeleteEntity,
    DeleteRelation, DictionaryBuilder, Edit, EmbeddingSubType, Id, Op, Property, PropertyValue,
    RelationIdMode, UpdateEntity, UpdateRelation, Value, WireDictionaries,
};
pub use model::id::{derived_uuid, format_id, parse_id, text_value_id, unique_relation_id, value_id, NIL_ID};
pub use validate::{validate_edit, validate_position, validate_value, SchemaContext};

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// GRC-20 spec version this crate implements.
pub const SPEC_VERSION: &str = "0.16.0";
