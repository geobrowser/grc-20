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
    Edit, Op, CreateEntity, PropertyValue, Value,
    encode_edit, decode_edit, genesis::properties,
};
use std::borrow::Cow;

// Create an edit with an entity
let edit = Edit {
    id: [1u8; 16],
    name: Cow::Borrowed("My Edit"),
    authors: vec![[2u8; 16]],
    created_at: 1704067200_000_000, // microseconds since epoch
    ops: vec![
        // Create an entity with a value
        Op::CreateEntity(CreateEntity {
            id: [3u8; 16],
            values: vec![PropertyValue {
                property: properties::name(),
                value: Value::Text {
                    value: Cow::Borrowed("Alice"),
                    language: None,
                },
            }],
            context: None, // Optional context for change grouping
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

All 12 GRC-20 data types are supported:

| Type | Rust Representation | Wire Size |
|------|---------------------|-----------|
| BOOL | `Value::Bool(bool)` | 1 byte |
| INT64 | `Value::Int64 { value, unit }` | varint |
| FLOAT64 | `Value::Float64 { value, unit }` | 8 bytes |
| DECIMAL | `Value::Decimal { exponent, mantissa, unit }` | variable |
| TEXT | `Value::Text { value, language }` | variable |
| BYTES | `Value::Bytes(Vec<u8>)` | variable |
| DATE | `Value::Date { days, offset_min }` | 6 bytes |
| TIME | `Value::Time { time_us, offset_min }` | 8 bytes |
| DATETIME | `Value::Datetime { epoch_us, offset_min }` | 10 bytes |
| SCHEDULE | `Value::Schedule(String)` | variable |
| POINT | `Value::Point { lon, lat, alt }` | 17-25 bytes |
| EMBEDDING | `Value::Embedding { sub_type, dims, data }` | variable |

**Temporal types use fixed-width binary encoding:**
- `DATE`: `days` (i32, days since 1970-01-01) + `offset_min` (i16, UTC offset in minutes)
- `TIME`: `time_us` (i48, microseconds since midnight) + `offset_min` (i16)
- `DATETIME`: `epoch_us` (i64, microseconds since Unix epoch) + `offset_min` (i16)

### Operations

All 9 operation types:

- `CreateEntity` — Create or upsert an entity with values
- `UpdateEntity` — Modify entity values (set/unset)
- `DeleteEntity` — Tombstone an entity
- `RestoreEntity` — Restore a deleted entity
- `CreateRelation` — Create a directed relation with optional position and space/version pins
- `UpdateRelation` — Update relation's mutable fields (position)
- `DeleteRelation` — Tombstone a relation
- `RestoreRelation` — Restore a deleted relation
- `CreateValueRef` — Create a referenceable value for use as relation endpoints

### Builder API

Fluent builders for constructing edits:

```rust
use grc_20::{EditBuilder, genesis::{properties, languages}};

let edit = EditBuilder::new(edit_id)
    .name("My Edit")
    .author(author_id)
    .create_entity(entity_id, |e| e
        .text(properties::name(), "Hello", None)
        .int64(count_prop, 42, None)
        .float64(temp_prop, 98.6, Some(fahrenheit_unit))
        .point(location_prop, 40.7128, -74.006)
        .date(birth_prop, "1990-05-15")
        .time(start_prop, "09:00:00")
        .datetime(created_prop, "2024-01-15T10:30:00Z")
    )
    .update_entity(entity_id, |u| u
        .set_text(properties::name(), "Updated", None)
        .unset_all(old_prop)
    )
    .create_relation(|r| r
        .id(relation_id)
        .from(from_id)
        .to(to_id)
        .relation_type(relation_type_id)
        .position("a0")
    )
    .update_relation(relation_id, |r| r
        .position("b0")
    )
    .build();
```

### Language-Aware Text

Multi-language support for TEXT values:

```rust
use grc_20::genesis::languages;

// Set text with language variants
let edit = EditBuilder::new(edit_id)
    .create_entity(entity_id, |e| e
        .text(name_prop, "Hello", Some(languages::english()))
        .text(name_prop, "Hola", Some(languages::spanish()))
        .text(name_prop, "Bonjour", Some(languages::french()))
    )
    .update_entity(entity_id, |u| u
        // Unset specific language variant
        .unset_text(name_prop, Some(languages::french()))
    )
    .build();
```

### Canonical Encoding

Deterministic encoding for content addressing:

```rust
use grc_20::{encode_edit, EncodeOptions};

// Canonical mode ensures identical edits produce identical bytes
let bytes = encode_edit(&edit, EncodeOptions::canonical())?;

// Use for content hashing, signatures, deduplication
let hash = sha256(&bytes);
```

### Zero-Copy Decoding

Performance optimization with borrowed data:

```rust
use grc_20::decode_edit_borrowed;

// Decode with zero-copy string borrowing
let edit = decode_edit_borrowed(&bytes)?;

// Strings borrow from input buffer - no allocation
assert!(matches!(edit.name, Cow::Borrowed(_)));
```
- `CreateValueRef` — Create a referenceable ID for a value slot

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

Implements GRC-20 v2 specification version 0.19.0.

## License

MIT OR Apache-2.0
