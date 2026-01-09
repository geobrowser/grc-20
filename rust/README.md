# grc-20

Rust implementation of the GRC-20 v2 binary property graph format for decentralized knowledge networks.

## Overview

GRC-20 is a binary format designed for:

- **Event-sourced data** — All state changes expressed as operations
- **Binary-first** — Optimized for compressed wire size and decode speed
- **Pluralistic** — Multiple spaces can hold conflicting views

This crate provides encoding, decoding, and validation for the GRC-20 v2 binary format.

## Installation

```toml
[dependencies]
grc-20 = "0.1"
```

## Quick Start

```rust
use grc_20::{
    Edit, Op, CreateEntity, CreateProperty, PropertyValue, Value, DataType,
    encode_edit, decode_edit, genesis::properties,
};

// Create an edit with a property and entity
let edit = Edit {
    id: [1u8; 16],
    name: "My Edit".to_string(),
    authors: vec![[2u8; 16]],
    created_at: 1704067200_000_000, // microseconds since epoch
    ops: vec![
        // Define a property
        Op::CreateProperty(CreateProperty {
            id: [10u8; 16],
            data_type: DataType::Text,
        }),
        // Create an entity with a value
        Op::CreateEntity(CreateEntity {
            id: [3u8; 16],
            values: vec![PropertyValue {
                property: [10u8; 16],
                value: Value::Text {
                    value: "Alice".to_string(),
                    language: None,
                },
            }],
        }),
    ],
};

// Encode to binary
let bytes = encode_edit(&edit).unwrap();

// Decode back
let decoded = decode_edit(&bytes).unwrap();
assert_eq!(edit.id, decoded.id);
```

## Features

### Data Types

All 11 GRC-20 data types are supported:

| Type | Rust Representation |
|------|---------------------|
| BOOL | `Value::Bool(bool)` |
| INT64 | `Value::Int64(i64)` |
| FLOAT64 | `Value::Float64(f64)` |
| DECIMAL | `Value::Decimal { exponent, mantissa }` |
| TEXT | `Value::Text { value, language }` |
| BYTES | `Value::Bytes(Vec<u8>)` |
| TIMESTAMP | `Value::Timestamp(i64)` (microseconds) |
| DATE | `Value::Date(String)` (ISO 8601) |
| POINT | `Value::Point { lat, lon }` |
| EMBEDDING | `Value::Embedding { sub_type, dims, data }` |
| REF | `Value::Ref(Id)` |

### Operations

All 7 operation types:

- `CreateEntity` — Create or upsert an entity
- `UpdateEntity` — Modify entity values (set, add, remove, unset)
- `DeleteEntity` — Tombstone an entity
- `CreateRelation` — Create a directed edge (instance or unique mode)
- `UpdateRelation` — Update relation position
- `DeleteRelation` — Tombstone a relation
- `CreateProperty` — Define a property in the schema

### Compression

Transparent zstd compression support:

```rust
use grc_20::{encode_edit_compressed, decode_edit};

// Encode with compression (level 3)
let compressed = encode_edit_compressed(&edit, 3).unwrap();

// Decode automatically detects compression
let decoded = decode_edit(&compressed).unwrap();
```

### Genesis IDs

Well-known IDs from the Genesis Space:

```rust
use grc_20::genesis::{properties, types, relation_types, languages};

// Core properties
let name_prop = properties::name();
let description_prop = properties::description();

// Core types
let person_type = types::person();
let organization_type = types::organization();

// Core relation types
let types_rel = relation_types::types();

// Languages
let english = languages::english();
let spanish = languages::from_code("es");
```

### Validation

Structural validation during decode, semantic validation with schema context:

```rust
use grc_20::{validate_edit, SchemaContext, DataType};

let mut schema = SchemaContext::new();
schema.add_property([10u8; 16], DataType::Text);

// Validates type consistency
validate_edit(&edit, &schema)?;
```

## Security

The decoder is designed for untrusted input:

- All allocations bounded by configurable limits
- Varints limited to prevent overflow
- Invalid data rejected with descriptive errors
- No panics on malformed input

## Wire Format

Edits use a binary format with optional compression:

- **Uncompressed:** `GRC2` magic + version + data
- **Compressed:** `GRC2Z` magic + uncompressed size + zstd frame

The decoder automatically detects and handles both formats.

## Spec Compliance

Implements GRC-20 v2 specification version 0.16.0.

## License

MIT OR Apache-2.0
