# GRC-20 v2 Binary Encoding

**Status:** Draft
**Version:** 0.19.0

This document defines the binary wire format for GRC-20 edits. It is the encoding companion to the [GRC-20 v2 Specification](spec.md), which defines the data model, operations, and semantics.

Cross-references to the main spec are noted as "(spec Section X.Y)".

---

## 1. Primitive Encoding

**Varint:** Unsigned LEB128
```
0-127:       1 byte   [0xxxxxxx]
128-16383:   2 bytes  [1xxxxxxx 0xxxxxxx]
```

> **NORMATIVE:** Varints MUST NOT exceed 10 bytes (sufficient for u64). Varints MUST use minimal encoding—the fewest bytes required to represent the value. Overlong encodings (e.g., encoding 1 as `81 00` instead of `01`) MUST be rejected (E005). This applies to all varints, not just canonical mode.

**Signed varint:** ZigZag encoding then varint
```
zigzag(n) = (n << 1) ^ (n >> 63)
```

**String:** Varint length prefix + UTF-8 bytes

**UUID:** Raw 16 bytes (no length prefix), big-endian (network byte order). Byte `i` corresponds to hex digits `2i` and `2i+1` of the standard 32-character hex string. For example, UUID `550e8400-e29b-41d4-a716-446655440000` is encoded as bytes `[0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, ...]`.

> **NORMATIVE:** All IEEE 754 floats (FLOAT, POINT, EMBEDDING float32) are little-endian.

---

## 2. Common Reference Types

All reference types are dictionary indices. External references are not supported—all referenced items must be declared in the appropriate dictionary.

**ObjectRef:**
```
index: varint    // Must be < object_count
```

ObjectRef references entities and relations only (not value refs). Value refs are always referenced by inline ID in relation endpoints.

**PropertyRef:**
```
index: varint    // Must be < property_count
```

**RelationTypeRef:**
```
index: varint    // Must be < relation_type_count
```

**LanguageRef:**
```
index: varint    // 0 = English, 1+ = language_ids[index-1]
```

**UnitRef:**
```
index: varint    // 0 = no unit, 1+ = unit_ids[index-1]
```

---

## 3. Edit Format

```
Magic: "GRC2" (4 bytes)
Version: uint8

-- Header
edit_id: ID
name_len: varint
name: UTF-8 bytes              // May be empty (name_len = 0)
author_count: varint
authors: ID[]
created_at: signed_varint

-- Schema dictionaries
property_count: varint
properties: (ID, DataType)[]     // ID + uint8 data type per entry
relation_type_count: varint
relation_type_ids: ID[]
language_count: varint
language_ids: ID[]               // Language entity IDs for localized TEXT values
unit_count: varint
unit_ids: ID[]                   // Unit entity IDs for numerical values
object_count: varint
object_ids: ID[]
context_id_count: varint
context_ids: ID[]                // IDs used in contexts (root_ids and edge to_entity_ids)

-- Contexts
context_count: varint
contexts: Context[]              // Context metadata for grouping

-- Operations
op_count: varint
ops: Op[]
```

> **NORMATIVE:** Decoders MUST reject edits with unknown Version values.

**ContextRef:**
```
index: varint    // Must be < context_id_count
```

**Context encoding:**
```
Context:
  root_id: ContextRef              // Index into context_ids
  edge_count: varint
  edges: ContextEdge[]

ContextEdge:
  type_id: RelationTypeRef         // Index into relation_type_ids
  to_entity_id: ContextRef         // Index into context_ids
```

---

## 4. Op Encoding

```
Op:
  op_type: uint8
  payload: <type-specific>
  [if op_type has context_ref support]:
    context_ref: varint?         // Index into contexts[] (0xFFFFFFFF = none)

op_type values:
  1 = CreateEntity
  2 = UpdateEntity
  3 = DeleteEntity
  4 = RestoreEntity
  5 = CreateRelation
  6 = UpdateRelation
  7 = DeleteRelation
  8 = RestoreRelation
  9 = CreateValueRef
```

**Context reference encoding:** All entity and relation ops (CreateEntity, UpdateEntity, DeleteEntity, RestoreEntity, CreateRelation, UpdateRelation, DeleteRelation, RestoreRelation) include a context reference to indicate which context they belong to. The `context_ref` field is encoded as a varint where `0xFFFFFFFF` means no context, and other values are indices into the edit's `contexts` array. CreateValueRef does not support context.

**CreateEntity:**
```
id: ID
value_count: varint
values: Value[]
context_ref: varint              // 0xFFFFFFFF = no context, else index into contexts[]
```

**UpdateEntity:**
```
id: ObjectRef
flags: uint8
  bit 0 = has_set
  bit 1 = has_unset
  bits 2-7 = reserved (must be 0)

[if has_set]:
  count: varint
  values: Value[]
[if has_unset]:
  count: varint
  unset: UnsetValue[]
context_ref: varint              // 0xFFFFFFFF = no context, else index into contexts[]

UnsetValue:
  property: PropertyRef
  language: varint    // 0xFFFFFFFF = clear all languages, otherwise LanguageRef (0 = English, 1+ = specific language)
```

**DeleteEntity:**
```
id: ObjectRef
context_ref: varint              // 0xFFFFFFFF = no context, else index into contexts[]
```

**RestoreEntity:**
```
id: ObjectRef
context_ref: varint              // 0xFFFFFFFF = no context, else index into contexts[]
```

**CreateRelation:**
```
id: ID
type: RelationTypeRef
flags: uint8
  bit 0 = has_from_space
  bit 1 = has_from_version
  bit 2 = has_to_space
  bit 3 = has_to_version
  bit 4 = has_entity           // If 0, entity is auto-derived from relation ID
  bit 5 = has_position
  bit 6 = from_is_value_ref    // If 1, from is inline ID; if 0, from is ObjectRef
  bit 7 = to_is_value_ref      // If 1, to is inline ID; if 0, to is ObjectRef
[if from_is_value_ref]: from: ID
[else]: from: ObjectRef
[if to_is_value_ref]: to: ID
[else]: to: ObjectRef
[if has_from_space]: from_space: ID
[if has_from_version]: from_version: ID
[if has_to_space]: to_space: ID
[if has_to_version]: to_version: ID
[if has_entity]: entity: ID    // Explicit relation entity ID
[if has_position]: position: String
context_ref: varint            // 0xFFFFFFFF = no context, else index into contexts[]
```

**Endpoint encoding:** Entity and relation endpoints use ObjectRef (dictionary index). Value ref endpoints use inline ID (16 bytes) to avoid bloating the object dictionary. The `from_is_value_ref` and `to_is_value_ref` flags indicate which encoding is used.

**Entity derivation:** When `has_entity = 0`, the entity ID is computed as `derived_uuid("grc20:relation-entity:" || relation_id)` (spec Section 2.6).

**UpdateRelation:**
```
id: ObjectRef
set_flags: uint8
  bit 0 = has_from_space
  bit 1 = has_from_version
  bit 2 = has_to_space
  bit 3 = has_to_version
  bit 4 = has_position
  bits 5-7 = reserved (must be 0)
unset_flags: uint8
  bit 0 = unset_from_space
  bit 1 = unset_from_version
  bit 2 = unset_to_space
  bit 3 = unset_to_version
  bit 4 = unset_position
  bits 5-7 = reserved (must be 0)
[if has_from_space]: from_space: ID
[if has_from_version]: from_version: ID
[if has_to_space]: to_space: ID
[if has_to_version]: to_version: ID
[if has_position]: position: String
context_ref: varint              // 0xFFFFFFFF = no context, else index into contexts[]
```

**DeleteRelation:**
```
id: ObjectRef
context_ref: varint              // 0xFFFFFFFF = no context, else index into contexts[]
```

**RestoreRelation:**
```
id: ObjectRef
context_ref: varint              // 0xFFFFFFFF = no context, else index into contexts[]
```

**CreateValueRef:**
```
id: ID
entity: ObjectRef
property: PropertyRef
flags: uint8
  bit 0 = has_language
  bit 1 = has_space
  bits 2-7 = reserved (must be 0)
[if has_language]: language: LanguageRef
[if has_space]: space: ID
```

---

## 5. Value Encoding

```
Value:
  property: PropertyRef
  payload: <type-specific>
  [if DataType == TEXT]: language: LanguageRef
  [if DataType in (INTEGER, FLOAT, DECIMAL)]: unit: UnitRef
```

The payload type is determined by the property's DataType (from the properties dictionary).

**Language (TEXT only):** The `language` field is only present for TEXT values. A value with `language = 0` is English. Values with different languages for the same property are distinct and can coexist.

**Unit (numerical types only):** The `unit` field is only present for INTEGER, FLOAT, and DECIMAL values. A value with `unit = 0` has no unit. Unlike language, unit does NOT affect value uniqueness—it is metadata for interpretation only.

**Payloads:**
```
Boolean: uint8 (0x00 or 0x01)
Integer: signed_varint
Float: 8 bytes, IEEE 754, little-endian
Decimal:
  exponent: signed_varint
  mantissa_type: uint8 (0x00 = varint, 0x01 = bytes)
  if 0x00: mantissa: signed_varint
  if 0x01: len: varint, mantissa: bytes[len]
Text: len: varint, data: UTF-8 bytes
Bytes: len: varint, data: bytes
Date: days: int32 (LE), offset_min: int16 (LE) — 6 bytes total
Time: time_micros: int48 (LE), offset_min: int16 (LE) — 8 bytes total
Datetime: epoch_micros: int64 (LE), offset_min: int16 (LE) — 10 bytes total
Schedule: len: varint, data: UTF-8 bytes (RFC 5545/7953)
Point: ordinate_count: uint8 (2 or 3), latitude: Float, longitude: Float, [altitude: Float]
Rect: min_lat: Float, min_lon: Float, max_lat: Float, max_lon: Float — 32 bytes total
Embedding:
  sub_type: uint8 (0x00=f32, 0x01=i8, 0x02=binary)
  dims: varint
  data: raw bytes
    f32: dims × 4 bytes, little-endian
    i8: dims × 1 byte
    binary: ceil(dims / 8) bytes
```

> **NORMATIVE:** DECIMAL encoding rules:
> - If mantissa fits in signed 64-bit integer (-2^63 to 2^63-1), `mantissa_type` MUST be `0x00` (varint).
> - `mantissa_type = 0x01` (bytes) is reserved for values outside int64 range.
> - When `mantissa_type = 0x01`, mantissa bytes MUST be big-endian two's complement, minimal-length (no redundant sign extension).
> - Non-compliant encodings MUST be rejected (E005).

---

## 6. Compression

Edits SHOULD be compressed with zstd for transport efficiency.

```
Magic: "GRC2Z" (5 bytes)
uncompressed_size: varint
compressed_data: zstd frame
```

> **NORMATIVE:** The `GRC2Z` format wraps the uncompressed `GRC2` payload. CIDs and signatures are computed over the uncompressed payload, not the compressed bytes (spec Section 4.1). Implementations MAY use any zstd compression level; level 3+ is RECOMMENDED for a good size/speed tradeoff.

---

## 7. Encoding Modes

The binary format supports two encoding modes:

- **Fast mode (default):** Dictionary order is implementation-defined (typically insertion order). Optimized for encode speed.
- **Canonical mode:** Deterministic encoding for reproducible bytes. Required for signing and content deduplication.

### 7.1 Canonical Encoding

Canonical encoding produces deterministic bytes for the same logical edit. Use canonical mode when:

- Computing content hashes for deduplication
- Creating signatures over edit content
- Ensuring cross-implementation reproducibility
- Blockchain anchoring where byte-level determinism matters

> **NORMATIVE:** Canonical encoding rules:
>
> 1. **Sorted dictionaries:** All dictionaries (`properties`, `relation_type_ids`, `language_ids`, `unit_ids`, `object_ids`, `context_ids`) MUST be sorted by ID bytes in ascending lexicographic order (unsigned byte comparison).
>
> 2. **Sorted authors:** The `authors` list MUST be sorted by ID bytes in ascending lexicographic order. Duplicate author IDs are NOT permitted.
>
> 3. **Sorted value lists:** `CreateEntity.values` and `UpdateEntity.set` MUST be sorted by `(propertyRef, languageRef)` in ascending order (property index first, then language index). Duplicate `(property, language)` entries are NOT permitted.
>
> 4. **Sorted unset lists:** `UpdateEntity.unset` MUST be sorted by `(propertyRef, language)` in ascending order. Duplicate entries (same property and language) are NOT permitted.
>
> 5. **Minimal varints:** This is a general requirement (Section 1), not canonical-only.
>
> 6. **Consistent field encoding:** Optional fields use presence flags as specified in Section 4. No additional padding or alignment bytes.
>
> 7. **No duplicate dictionary entries:** Each dictionary MUST NOT contain duplicate IDs. Edits with duplicate IDs in any dictionary MUST be rejected.

**Performance note:** Canonical encoding requires sorting dictionaries and authors after collection, which is substantially slower than fast mode. Implementations SHOULD offer both modes.

---

## 8. Derived UUID Algorithm

Some IDs are deterministically derived from their content rather than randomly generated. This is used for relation entity IDs (spec Section 2.6), language IDs (spec Section 7.4), and data type entity IDs (spec Section 7.5).

Content-addressed IDs SHOULD use UUIDv8 with SHA-256:

```
derived_uuid(input_bytes) -> UUID:
  hash = SHA-256(input_bytes)[0:16]
  hash[6] = (hash[6] & 0x0F) | 0x80  // version 8
  hash[8] = (hash[8] & 0x3F) | 0x80  // RFC 4122 variant
  return hash
```

When deriving from string prefixes (e.g., `"grc20:relation-entity:"`), the string is UTF-8 encoded with no trailing NUL byte.

**Examples:**
- Relation entity ID: `derived_uuid("grc20:relation-entity:" || relation_id_bytes)`
- Language ID for English: `derived_uuid("grc20:genesis:language:en")`
- Data type ID for Text: `derived_uuid("grc20:genesis:datatype:text")`

---

## 9. Validation

### 9.1 Structural Validation (Write-Time)

Indexers MUST reject edits that fail structural validation:

| Check | Reject if |
|-------|-----------|
| Magic | Not `GRC2` or `GRC2Z` |
| Version | Unknown version |
| Lengths | Truncated/overflow |
| Dictionary counts | Greater than 0xFFFFFFFE |
| Reference indices | Index ≥ respective dictionary count |
| Dictionary duplicates | Same ID appears twice in any dictionary |
| Author duplicates | Same author ID appears twice (canonical mode) |
| Value duplicates | Same `(property, language)` appears twice in values/set (canonical mode) |
| Unset duplicates | Same `(property, language)` appears twice in unset (canonical mode) |
| Language indices (TEXT) | Index not 0xFFFFFFFF and index > 0 and (index - 1) ≥ language_count |
| UnsetValue language (non-TEXT) | Language value is not 0xFFFFFFFF |
| Unit indices (numerical) | Index > 0 and (index - 1) ≥ unit_count |
| UTF-8 | Invalid encoding |
| Varint encoding | Overlong encoding or exceeds 10 bytes |
| Reserved bits | Non-zero |
| Mantissa bytes | Non-minimal encoding |
| DECIMAL normalization | Mantissa has trailing zeros, or zero not encoded as {0,0} |
| Signatures | Invalid (if governance requires) |
| BOOLEAN values | Not 0x00 or 0x01 |
| POINT bounds | Latitude outside [-90, +90] or longitude outside [-180, +180] |
| POINT ordinate count | ordinate_count not 2 or 3 |
| RECT bounds | Latitude outside [-90, +90] or longitude outside [-180, +180] |
| DATE offset_min | Outside range [-1440, +1440] |
| TIME time_micros | Outside range [0, 86399999999] |
| TIME offset_min | Outside range [-1440, +1440] |
| DATETIME offset_min | Outside range [-1440, +1440] |
| Position strings | Empty, characters outside `0-9A-Za-z`, or length > 64 |
| EMBEDDING dims | Data length doesn't match dims × bytes-per-element for subtype |
| Zstd decompression | Decompressed size doesn't match declared `uncompressed_size` |
| Float values | NaN payload (see float rules in spec Section 2.5) |
| Relation entity self-reference | CreateRelation has explicit `entity` equal to relation ID |
| CreateValueRef language mismatch | `has_language = 1` but property's DataType is not TEXT |
| Context ID indices | Index ≥ context_id_count |
| Context indices | context_ref ≥ context_count (unless 0xFFFFFFFF) |
| Context edge type indices | edge type_id ≥ relation_type_count |

**Implementation-defined limits:** This specification does not mandate limits on ops per edit, values per entity, or TEXT/BYTES payload sizes. Implementations and governance systems MAY impose their own limits to prevent resource exhaustion.

**Security guidance (RECOMMENDED):** Decoders process untrusted input and SHOULD enforce defensive limits:

| Resource | Recommended Limit | Rationale |
|----------|-------------------|-----------|
| `uncompressed_size` (zstd) | ≤ 64 MiB | Prevent memory exhaustion |
| Compression ratio | ≤ 100:1 | Detect compression bombs |
| Dictionary counts | ≤ 100,000 each | Prevent allocation attacks |
| Ops per edit | ≤ 1,000,000 | Bound processing time |
| String/bytes length | ≤ 16 MiB | Prevent single-value DoS |
| Embedding dimensions | ≤ 65,536 | Practical vector limits |

Decoders SHOULD reject zstd frames with trailing data after decompression.

### 9.2 Error Codes

| Code | Reason |
|------|--------|
| E001 | Invalid magic/version |
| E002 | Index out of bounds |
| E003 | Invalid signature |
| E004 | Invalid UTF-8 encoding |
| E005 | Malformed varint/length/reserved bits/encoding |
