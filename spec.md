# GRC-20 v2 Specification

**Status:** Draft
**Version:** 0.17.0

## 1. Introduction

GRC-20 v2 is a binary property graph format for decentralized knowledge networks. It defines how to represent, modify, and synchronize graph data across distributed systems.

### 1.1 Design Principles

- **Property graph model** — Entities connected by relations; relations are first-class and can hold attributes
- **Event-sourced** — All state changes expressed as operations; history is append-only
- **Sequentially ordered** — On-chain governance provides total ordering; indexers replay ops deterministically
- **Binary-first** — Optimized for compressed wire size and decode speed
- **Pluralistic** — Multiple spaces can hold conflicting views; consumers choose trust

### 1.2 Terminology

| Term | Definition |
|------|------------|
| Entity | A node in the graph, identified by ID |
| Relation | A directed edge between objects, identified by ID |
| Object | Either an Entity or Relation (used when referencing both) |
| Property | A named, typed attribute definition |
| Value | A property instance on an object |
| Type | A classification tag for entities |
| Op | An atomic operation that modifies graph state |
| Edit | A batch of ops with metadata |
| Space | A governance container for edits |

---

## 2. Data Model

### 2.1 Identifiers

All identifiers are RFC 4122 UUIDs.

```
ID := UUID (16 bytes)
```

**Random IDs:** Use UUIDv4 (random) or UUIDv7 (time-ordered). UUIDv7 is RECOMMENDED for entities and relations as it enables time-based sorting.

**Derived IDs:** Content-addressed IDs use UUIDv8 with SHA-256:

```
derived_uuid(input_bytes) -> UUID:
  hash = SHA-256(input_bytes)[0:16]
  hash[6] = (hash[6] & 0x0F) | 0x80  // version 8
  hash[8] = (hash[8] & 0x3F) | 0x80  // RFC 4122 variant
  return hash
```

**Display format:** Non-hyphenated lowercase hex is RECOMMENDED. Implementations MAY accept hyphenated or Base58 on input.

**Interning:** Properties and relation types are stored once in schema dictionaries (Section 4.3) and referenced by index throughout the edit. Types are entities referenced via the object dictionary.

### 2.2 Entities

```
Entity {
  id: ID
  values: List<Value>
}
```

Values are unique per (entityId, propertyId), or per (entityId, propertyId, language) for TEXT values. When multiple values for a given (entity, property) pair are required, use relations instead.

Type membership is expressed via `Types` relations (Section 7.3), not a dedicated types field.

**Lifecycle states:**
- `ALIVE` — Entity exists and accepts updates
- `DEAD` — Entity is tombstoned; subsequent updates are ignored

### 2.3 Types

Types are entities that classify other entities via `Types` relations. An entity can have multiple types simultaneously. Types are created using CreateEntity; type names and metadata are added as values in the knowledge layer.

Types are tags, not classes: no inheritance, no cardinality constraints, no property enforcement.

### 2.4 Properties

Properties define typed attributes:

```
Property {
  id: ID
  data_type: DataType
}

DataType := BOOL | INT64 | FLOAT64 | DECIMAL | TEXT | BYTES
          | TIMESTAMP | DATE | POINT | EMBEDDING
```

Property names are defined via values in the knowledge layer, not in the protocol.

**Data type enum values:**

| Type | Value | Description |
|------|-------|-------------|
| BOOL | 1 | Boolean |
| INT64 | 2 | 64-bit signed integer |
| FLOAT64 | 3 | 64-bit IEEE 754 float |
| DECIMAL | 4 | Arbitrary-precision decimal |
| TEXT | 5 | UTF-8 string |
| BYTES | 6 | Opaque byte array |
| TIMESTAMP | 7 | Microseconds since epoch |
| DATE | 8 | ISO 8601 date string |
| POINT | 9 | WGS84 coordinate |
| EMBEDDING | 10 | Dense vector |

**Data type semantics:**

| Type | Encoding | Description |
|------|----------|-------------|
| BOOL | 1 byte | 0x00 = false, 0x01 = true; other values invalid |
| INT64 | Signed varint | -2^63 to 2^63-1 |
| FLOAT64 | IEEE 754 double, little-endian | 64-bit floating point |
| DECIMAL | exponent + mantissa | value = mantissa × 10^exponent |
| TEXT | UTF-8 string | Length-prefixed |
| BYTES | Raw bytes | Length-prefixed, opaque |
| TIMESTAMP | Signed varint | Microseconds since Unix epoch |
| DATE | UTF-8 string | ISO 8601 (variable precision) |
| POINT | Two FLOAT64, little-endian | [latitude, longitude] WGS84 |
| EMBEDDING | sub_type + dims + bytes | Dense vector for similarity search |

#### DECIMAL

Fixed-point decimal for currency and financial data.

```
DECIMAL {
  exponent: int32
  mantissa: int64 | bytes
}
```

Examples:
- `$12.34` → `{ exponent: -2, mantissa: 1234 }`
- `0.000001` → `{ exponent: -6, mantissa: 1 }`

**Normalization (NORMATIVE):** DECIMAL values MUST be encoded in normalized form: mantissa has no trailing zeros, and zero is represented as `{ exponent: 0, mantissa: 0 }`. This ensures deterministic encoding for content addressing.

Applications needing to preserve precision (e.g., "12.30" vs "12.3") should store the original string in a TEXT property alongside the DECIMAL value.

#### DATE

ISO 8601 format for **calendar dates with variable precision**. Use DATE for semantic dates where precision matters (historical events, birthdays, publication years). Use TIMESTAMP for exact instants.

```
"2024-03-15"         // Date only
"2024-03"            // Month precision
"2024"               // Year only
"-0100"              // 100 BCE
```

**DATE vs TIMESTAMP:** DATE preserves the original precision and is stored as a string. TIMESTAMP is always microsecond-precision UTC stored as an integer. A birthday is a DATE ("1990-05-20"); a login event is a TIMESTAMP.

**Sorting (NORMATIVE):** Indexers MUST parse DATE strings into a numeric representation for sorting. Lexicographical string sorting does NOT work for BCE years. Dates with different precisions sort by their earliest possible instant; when two dates resolve to the same instant, the more precise date sorts before the less precise (e.g., `2024-01-01` < `2024-01` < `2024`).

**Validation (NORMATIVE):** DATE strings MUST conform to ISO 8601 calendar date format. Full datetime with timezone (e.g., "2024-03-15T14:30Z") SHOULD use TIMESTAMP instead. Implementations SHOULD reject clearly malformed dates (e.g., month 13, day 32) but MAY defer full validation to the application layer.

#### POINT

WGS84 geographic coordinate.

```
POINT {
  latitude: float64   // -90 to +90
  longitude: float64  // -180 to +180
}
```

**Coordinate order (NORMATIVE):** `[latitude, longitude]`.

**Bounds validation (NORMATIVE):** Latitude MUST be in range [-90, +90]. Longitude MUST be in range [-180, +180]. Values outside these ranges MUST be rejected (E005).

For complex geometry (polygons, lines), use BYTES with WKB encoding.

#### EMBEDDING

Dense vector for semantic similarity search.

```
EMBEDDING {
  sub_type: uint8       // 0x00=float32, 0x01=int8, 0x02=binary
  dimensions: varint
  data: raw_bytes
}
```

| Sub-type | Encoding | Bytes per dim |
|----------|----------|---------------|
| 0x00 float32 | IEEE 754, little-endian | 4 |
| 0x01 int8 | Signed byte | 1 |
| 0x02 binary | Bit-packed, LSB-first | 1/8 |

**Binary bit order (NORMATIVE):** For subtype 0x02, dimension `i` maps to byte `i / 8`, bit position `i % 8` where bit 0 is the least significant bit. Bits beyond `dims` in the final byte MUST be zero.

### 2.5 Values

A value is a property instance on an object:

```
Value {
  property: index
  value: bytes
  language: ID?    // TEXT only: language entity reference
  unit: ID?        // INT64, FLOAT64, DECIMAL only: unit entity reference
}
```

The value's type is determined by the property's `data_type`.

**Value uniqueness:**

Values are unique per (entityId, propertyId), with TEXT values additionally differentiated by language. Setting a value replaces any existing value for that (property, language) combination. For ordered or multiple values, use relations with positions.

**Unit (numerical types only):** INT64, FLOAT64, and DECIMAL values can optionally specify a unit (e.g., kg, USD). Unlike language, unit does NOT affect value uniqueness—setting "100 kg" then "200 lbs" on the same property results in "200 lbs" (the unit is metadata for interpretation).

**Float value rules (NORMATIVE):** For FLOAT64, POINT, and EMBEDDING (float32 subtype):
- **NaN is prohibited.** Encoders MUST NOT emit NaN values; decoders MUST reject them (E005). Use a separate "unknown" or "missing" representation at the application layer.
- **Infinity:** ±Infinity are permitted.

### 2.6 Relations

Relations are directed edges with an associated entity for reification.

```
Relation {
  id: ID
  entity: ID           // Reified entity representing this relation
  type: ID | index
  from: ID             // Source entity
  to: ID               // Target entity
  from_space: ID?      // Optional space hint for source
  from_version: ID?    // Optional version (edit ID) to pin source
  to_space: ID?        // Optional space hint for target
  to_version: ID?      // Optional version (edit ID) to pin target
  position: string?
}
```

The `entity` field links to an entity that represents this relation as a node. This enables relations to be referenced by other relations (meta-edges) and to participate in the graph as first-class nodes. Values are stored on the reified entity, not on the relation itself.

**Reified entity creation (NORMATIVE):** CreateRelation implicitly creates the reified entity if it does not exist. No separate CreateEntity op is required. If an entity with the given ID already exists, it is reused—its existing values are preserved and it becomes associated with this relation.

**ID modes:**

1. **Instance mode** (default): Random ID. Multiple relations can exist between same endpoints.
2. **Unique mode**: Deterministic ID derived from content.

**Unique mode ID derivation (NORMATIVE):**
```
id = derived_uuid(from_id || to_id || type_id)
```

Where each component is the raw 16-byte UUID. Space hints and entity are NOT included in the hash.

**Entity ID in unique mode:** The `entity` field is always caller-provided (not derived). If a unique-mode relation already exists, the entire CreateRelation op is ignored, including the entity field. This means the first creator determines the reified entity ID.

**Unique mode client responsibility (NORMATIVE):** When two clients concurrently create the same unique-mode relation with different entity IDs, only the first-committed op establishes the reified entity. Clients submitting `CreateRelation` in unique mode MUST query the resolved state after submission to determine which entity ID was accepted before attaching values to it. Failure to do so may result in orphaned values on a disconnected entity.

**Ordering:**

Use `position` with fractional indexing. Positions are strings from alphabet `0-9A-Za-z` (62 characters, ASCII order).

Generation rules:
- First item: `a`
- Append: midpoint between last position and `zzzz`
- Insert between A and B: midpoint character sequence

```
midpoint("a", "z") = "n"
midpoint("a", "b") = "aV"
```

**Maximum length (NORMATIVE):** Position strings MUST NOT exceed 64 characters. If a client cannot generate a midpoint without exceeding this limit (positions too close), it MUST perform explicit reordering by issuing `UpdateRelation` ops with new, evenly-spaced positions. This protocol does not support implicit rebalancing.

**Position validation (NORMATIVE):** Positions containing characters outside `0-9A-Za-z` or exceeding 64 characters MUST be rejected (E005).

**Immutability (NORMATIVE):** The structural fields (`entity`, `type`, `from`, `to`) are immutable after creation. To change endpoints, delete and recreate.

### 2.7 Per-Space State

**NORMATIVE:** Resolved state is scoped to a space:

```
state(space_id, object_id) → Object | DEAD | NOT_FOUND
```

The same object ID can have different state in different spaces. Multi-space views are computed by resolver policy and MUST preserve provenance.

### 2.8 Schema Constraints

Schema constraints (required properties, cardinality, patterns) are **not part of this specification**. They belong at the knowledge layer.

---

## 3. Operations

All state changes are expressed as operations (ops).

### 3.1 Op Types

```
Op {
  oneof payload {
    CreateEntity     = 1
    UpdateEntity     = 2
    DeleteEntity     = 3
    CreateRelation   = 4
    UpdateRelation   = 5
    DeleteRelation   = 6
    CreateProperty   = 7
  }
}
```

### 3.2 Entity Operations

**CreateEntity:**
```
CreateEntity {
  id: ID
  values: List<Value>
}
```

**Semantics (NORMATIVE):** If the entity does not exist, create it. If it already exists, this acts as an update: values are applied as `set_properties` (LWW replace per property).

> **Note:** CreateEntity is effectively an "upsert" operation. This is intentional: it simplifies edit generation (no need to track whether an entity exists) and supports idempotent replay. However, callers should be aware that CreateEntity on an existing entity will **replace** values for any properties included in the op.

**UpdateEntity:**
```
UpdateEntity {
  id: ID | index
  set_properties: List<Value>?        // LWW replace
  unset_properties: List<UnsetProperty>?
}

UnsetProperty {
  property: ID | index
  language: ID?    // TEXT only: if present, clear only that language; if absent, clear all
}
```

| Field | Strategy | Use Case |
|-------|----------|----------|
| `set_properties` | LWW Replace | Name, Age |
| `unset_properties` | Clear | Reset property or specific language |

**`set_properties` semantics (NORMATIVE):** For a given property (and language, for TEXT), `set_properties` replaces the existing value. For TEXT values, each language is treated independently—setting a value for one language does not affect values in other languages.

**`unset_properties` semantics (NORMATIVE):** Clears values for properties. For TEXT properties, if `language` is specified, only that language slot is cleared; if `language` is absent, all language slots for that property are cleared. For non-TEXT properties, `language` must be absent and the single value is cleared.

**Application order within op (NORMATIVE):**
1. `unset_properties`
2. `set_properties`

**DeleteEntity:**
```
DeleteEntity {
  id: ID | index
}
```

Appends tombstone to history. Subsequent updates to this entity are ignored.

### 3.3 Relation Operations

**CreateRelation:**
```
CreateRelation {
  id: ID?                  // Present = instance mode; absent = unique mode
  type: ID | index
  from: ID | index
  from_space: ID?          // Optional space hint for source
  from_version: ID?        // Optional version (edit ID) to pin source
  to: ID | index
  to_space: ID?            // Optional space hint for target
  to_version: ID?          // Optional version (edit ID) to pin target
  entity: ID               // Reified entity for this relation
  position: string?
}
```

**Semantics (NORMATIVE):** If the relation does not exist, create it along with its reified entity (if that entity does not already exist). If the relation already exists with the same ID, the op is ignored (relations are immutable except for position). To add values to the relation, use UpdateEntity on the reified entity ID.

**UpdateRelation:**
```
UpdateRelation {
  id: ID | index
  position: string?
}
```

Updates the relation's position. All other fields (`entity`, `type`, `from`, `to`, space hints, version pins) are immutable after creation.

**DeleteRelation:**
```
DeleteRelation {
  id: ID | index
}
```

Appends tombstone to the relation. Subsequent updates ignored.

**Reified entity lifecycle (NORMATIVE):** Deleting a relation does NOT delete its reified entity. The entity remains accessible and may hold values, be referenced by other relations, or be explicitly deleted via DeleteEntity. Orphaned reified entities are permitted; applications MAY garbage-collect them at a higher layer.

### 3.4 Schema Operations

**CreateProperty:**
```
CreateProperty {
  id: ID
  data_type: DataType
}
```

**Semantics (NORMATIVE):** If the property does not exist, create it with the specified DataType. If the property already exists, the op is ignored—the original DataType is preserved (first-writer-wins). Properties are immutable once created.

**DataType consistency (NORMATIVE):** An edit's properties dictionary MUST declare DataTypes consistent with the global schema. If a property was previously created with DataType X, all subsequent edits MUST declare it as X in their dictionary. Indexers SHOULD reject edits that declare inconsistent DataTypes for known properties.

Types are entities created via CreateEntity. Type names and metadata are added as values in the knowledge layer.

### 3.5 State Resolution

Operations are validated **structurally** at write time and **semantically** at read time.

**Write-time:** Validate structure, append to log. No state lookups.

**Read-time:** Replay operations in log order, apply resolution rules, return computed state.

**Resolution rules:**

1. Replay ops in log order (Section 4.2)
2. Apply merge rules (Section 4.2.1)
3. Tombstone dominance: updates after delete are ignored
4. Return resolved state or DEAD status

---

## 4. Edits

Ops are batched into edits for publishing.

### 4.1 Edit Structure

```
Edit {
  id: ID
  name: string              // May be empty
  authors: List<ID>
  created_at: Timestamp
  properties: List<(ID, DataType)>
  relation_type_ids: List<ID>
  language_ids: List<ID>    // Language entities for localized TEXT values
  unit_ids: List<ID>        // Unit entities for numerical values
  object_ids: List<ID>
  ops: List<Op>
}
```

Edits are standalone patches. They contain no parent references—ordering is provided by on-chain governance.

**`created_at`** is metadata for audit/display only. It is NOT used for conflict resolution.

**Encoding modes:** This specification defines two encoding modes:

- **Fast mode (default):** Dictionary order is implementation-defined. Optimized for encode speed.
- **Canonical mode:** Deterministic encoding for reproducible bytes. Required for signing and content deduplication.

Content-addressing (CID) is based on the bytes actually produced by the encoder. For reproducible CIDs across implementations, use canonical mode.

### 4.2 Sequential Ordering

The state of a space is the result of replaying all accepted edits in the order defined by the governance log.

**Log position:** Onchain space contracts emit an event when the proposal is accepted.
```
LogPosition := (block_number, tx_index, log_index)
```

Indexers MUST apply edits sequentially by LogPosition. The chain provides total ordering. 

**Op position:**
```
OpPosition := (LogPosition, op_index)
```

Where `op_index` is the zero-based index in the edit's `ops[]` array.

#### 4.2.1 Merge Rules

All values use Last-Writer-Wins (LWW) semantics based on OpPosition. Values are unique per (entityId, propertyId, language) where language only applies to TEXT values.

**`set_properties` (LWW):** Replaces the value for a property (and language, for TEXT). When concurrent edits both use `set_properties` on the same (property, language) combination, the op with the highest OpPosition wins.

**Property value conflicts:**

| Scenario | Resolution |
|----------|------------|
| Concurrent `set_properties` | Higher OpPosition wins (LWW) |
| Delete vs Update | Delete wins (tombstone dominance) |

**Structural conflicts:**

| Conflict | Resolution |
|----------|------------|
| Create same entity ID | First creates; later creates apply values as `set_properties` (LWW) |
| Create same relation ID | First creates; later creates ignored (relations are immutable) |
| Delete vs Delete | Idempotent |

**Intra-edit conflicts:** If multiple ops in the same edit modify the same field, the op with the higher `op_index` wins.

### 4.3 Schema Dictionaries

Edits contain dictionaries mapping IDs to indices:

```
properties[0] = (ID of "name", TEXT)
properties[1] = (ID of "age", INT64)
relation_type_ids[0] = <ID of "Types" relation type>
```

The property dictionary includes both ID and DataType. This allows values to omit type tags.

**Property dictionary requirement (NORMATIVE):** All properties referenced in an edit MUST be declared in the properties dictionary. External property references are not allowed.

**CreateProperty and dictionary interaction:** The properties dictionary enables compact indexing within an edit. CreateProperty defines a property in the global schema. To create a new property AND use it in the same edit, include both: a CreateProperty op to define it, and an entry in the properties dictionary to reference it by index. The dictionary is for wire efficiency; CreateProperty is for schema persistence.

**Relation type dictionary requirement (NORMATIVE):** All relation types referenced in an edit MUST be declared in the `relation_type_ids` dictionary.

**Language dictionary requirement (NORMATIVE):** All non-default languages referenced in TEXT values MUST be declared in the `language_ids` dictionary. Language index 0 means default (no entry required); indices 1+ reference `language_ids[index-1]`. Only TEXT values have the language field.

**Unit dictionary requirement (NORMATIVE):** All units referenced in numerical values (INT64, FLOAT64, DECIMAL) MUST be declared in the `unit_ids` dictionary. Unit index 0 means no unit; indices 1+ reference `unit_ids[index-1]`. Only numerical values have the unit field.

**Object dictionary requirement (NORMATIVE):** All objects (entities and relations) referenced in an edit MUST be declared in the `object_ids` dictionary. This includes: operation targets (UpdateEntity, DeleteEntity, etc.) and relation endpoints (`from`, `to`).

**Unique-mode relations:** In unique mode, the relation ID is derived (Section 2.6). To reference a unique-mode relation in the same edit (e.g., UpdateRelation to set position), the encoder MUST compute the derived ID and include it in `object_ids`. CreateRelation itself does not require the relation ID in the dictionary since it encodes the ID inline (instance mode) or derives it (unique mode).

**Size limits (NORMATIVE):** All dictionary counts MUST be ≤ 4,294,967,294 (0xFFFFFFFE). All reference indices MUST be < their respective dictionary count. Out-of-bounds indices MUST be rejected (E002).

**Dictionary ordering:**
- **Fast mode:** Dictionary order is implementation-defined (typically insertion order).
- **Canonical mode (NORMATIVE):** Dictionary entries MUST be sorted by ID bytes (lexicographic, unsigned byte comparison). This ensures identical logical edits produce identical bytes.

### 4.4 Canonical Encoding

Canonical encoding produces deterministic bytes for the same logical edit. Use canonical mode when:

- Computing content hashes for deduplication
- Creating signatures over edit content
- Ensuring cross-implementation reproducibility
- Blockchain anchoring where byte-level determinism matters

**Canonical encoding rules (NORMATIVE):**

1. **Sorted dictionaries:** All dictionaries (`properties`, `relation_type_ids`, `language_ids`, `unit_ids`, `object_ids`) MUST be sorted by ID bytes in ascending lexicographic order (unsigned byte comparison).

2. **Minimal varints:** Varints MUST use the minimum number of bytes required. Overlong encodings (e.g., encoding 1 as `81 00` instead of `01`) are invalid.

3. **Consistent field encoding:** Optional fields use presence flags as specified in Section 6. No additional padding or alignment bytes.

**Performance note:** Canonical encoding requires sorting dictionaries after collection, which may be slower than fast mode. Implementations SHOULD offer both modes.

### 4.5 Edit Publishing

1. Serialize edit to binary format (Section 6)
2. Publish to content-addressed storage (IPFS)
3. Publish hash onchain

---

## 5. Spaces

Spaces are governance containers for edits.

### 5.1 Pluralism

The same object ID can exist in multiple spaces with different data. Consumers choose which spaces to trust. There is no global merge.

### 5.2 Cross-Space References

Object IDs are globally unique. Relations can optionally include space and version hints for their endpoints:

```
Relation {
  ...
  from_space: ID?      // Optional provenance hint for source
  from_version: ID?    // Optional version (edit ID) to pin source
  to_space: ID?        // Optional provenance hint for target
  to_version: ID?      // Optional version (edit ID) to pin target
}
```

Space hints are provenance metadata for performance, not hard requirements. Resolvers MAY use hints to prefer a specific space when resolving the target.

**Version pinning:** The `from_version` and `to_version` fields allow pinning relation endpoints to a specific version (edit ID). This enables immutable citations where the relation always refers to the entity as it existed at that specific edit, rather than the current resolved state. Version pins are immutable after creation.

---

## 6. Binary Format

### 6.1 Primitive Encoding

**Varint:** Unsigned LEB128
```
0-127:       1 byte   [0xxxxxxx]
128-16383:   2 bytes  [1xxxxxxx 0xxxxxxx]
```

**Signed varint:** ZigZag encoding then varint
```
zigzag(n) = (n << 1) ^ (n >> 63)
```

**String:** Varint length prefix + UTF-8 bytes

**UUID:** Raw 16 bytes (no length prefix)

**Float endianness (NORMATIVE):** All IEEE 754 floats (FLOAT64, POINT, EMBEDDING float32) are little-endian.

### 6.2 Common Reference Types

All reference types are dictionary indices. External references are not supported—all referenced items must be declared in the appropriate dictionary.

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
index: varint    // 0 = default (no language), 1+ = language_ids[index-1]
```

**UnitRef:**
```
index: varint    // 0 = no unit, 1+ = unit_ids[index-1]
```

**ObjectRef:**
```
index: varint    // Must be < object_count
```

### 6.3 Edit Format

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

-- Operations
op_count: varint
ops: Op[]
```

**Version rejection (NORMATIVE):** Decoders MUST reject edits with unknown Version values.

### 6.4 Op Encoding

```
Op:
  op_type: uint8
  payload: <type-specific>

op_type values:
  1 = CreateEntity
  2 = UpdateEntity
  3 = DeleteEntity
  4 = CreateRelation
  5 = UpdateRelation
  6 = DeleteRelation
  7 = CreateProperty
```

**CreateEntity:**
```
id: ID
value_count: varint
values: Value[]
```

**UpdateEntity:**
```
id: ObjectRef
flags: uint8
  bit 0 = has_set_properties
  bit 1 = has_unset_properties
  bits 2-7 = reserved (must be 0)

[if has_set_properties]:
  count: varint
  values: Value[]
[if has_unset_properties]:
  count: varint
  unset_properties: UnsetProperty[]

UnsetProperty:
  property: PropertyRef
  language: LanguageRef    // 0 = clear all, 1+ = clear specific language (TEXT only)
```

**DeleteEntity:**
```
id: ObjectRef
```

**CreateRelation:**
```
mode: uint8                    // 0 = unique, 1 = instance
[if mode == 1]: id: ID
entity: ID                     // Reified entity ID
type: RelationTypeRef
from: ObjectRef
to: ObjectRef
flags: uint8
  bit 0 = has_position
  bit 1 = has_from_space
  bit 2 = has_from_version
  bit 3 = has_to_space
  bit 4 = has_to_version
  bits 5-7 = reserved (must be 0)
[if has_position]: position: String
[if has_from_space]: from_space: ID
[if has_from_version]: from_version: ID
[if has_to_space]: to_space: ID
[if has_to_version]: to_version: ID
```

**UpdateRelation:**
```
id: ObjectRef
flags: uint8
  bit 0 = has_position
  bits 1-7 = reserved (must be 0)
[if has_position]: position: String
```

**DeleteRelation:**
```
id: ObjectRef
```

**CreateProperty:**
```
id: ID
data_type: uint8               // See DataType enum (Section 2.4)
```

### 6.5 Value Encoding

```
Value:
  property: PropertyRef
  payload: <type-specific>
  [if DataType == TEXT]: language: LanguageRef
  [if DataType in (INT64, FLOAT64, DECIMAL)]: unit: UnitRef
```

The payload type is determined by the property's DataType (from the properties dictionary).

**Language (TEXT only):** The `language` field is only present for TEXT values. A value with `language = 0` is the default (unlocalized or English). Values with different languages for the same property are distinct and can coexist.

**Unit (numerical types only):** The `unit` field is only present for INT64, FLOAT64, and DECIMAL values. A value with `unit = 0` has no unit. Unlike language, unit does NOT affect value uniqueness—it is metadata for interpretation only.

**Payloads:**
```
Bool: uint8 (0x00 or 0x01)
Int64: signed_varint
Float64: 8 bytes, IEEE 754, little-endian
Decimal:
  exponent: signed_varint
  mantissa_type: uint8 (0x00 = varint, 0x01 = bytes)
  if 0x00: mantissa: signed_varint
  if 0x01: len: varint, mantissa: bytes[len]
Text: len: varint, data: UTF-8 bytes
Bytes: len: varint, data: bytes
Timestamp: signed_varint (microseconds)
Date: len: varint, data: UTF-8 bytes (ISO 8601)
Point: latitude: Float64, longitude: Float64
Embedding:
  sub_type: uint8 (0x00=f32, 0x01=i8, 0x02=binary)
  dims: varint
  data: raw bytes
    f32: dims × 4 bytes, little-endian
    i8: dims × 1 byte
    binary: ceil(dims / 8) bytes
```

**DECIMAL encoding rules (NORMATIVE):**
- If mantissa fits in signed 64-bit integer (-2^63 to 2^63-1), `mantissa_type` MUST be `0x00` (varint).
- `mantissa_type = 0x01` (bytes) is reserved for values outside int64 range.
- When `mantissa_type = 0x01`, mantissa bytes MUST be big-endian two's complement, minimal-length (no redundant sign extension).
- Non-compliant encodings MUST be rejected (E005).

### 6.6 Compression

Edits SHOULD be compressed with zstd level 3+.

```
Magic: "GRC2Z" (5 bytes)
uncompressed_size: varint
compressed_data: zstd frame
```

---

## 7. Genesis Space

The Genesis Space provides well-known IDs.

### 7.1 Core Properties

| Name | Data Type | Description |
|------|-----------|-------------|
| Name | TEXT | Primary label |
| Description | TEXT | Summary text |
| Avatar | TEXT | Image URL |
| URL | TEXT | External link |
| Created | TIMESTAMP | Creation time |
| Modified | TIMESTAMP | Last modification |

### 7.2 Core Types

| Name | Description |
|------|-------------|
| Person | Human individual |
| Organization | Company, DAO, institution |
| Place | Geographic location |
| Topic | Subject or concept |

### 7.3 Core Relation Types

| Name | Description |
|------|-------------|
| Types | Type membership |
| PartOf | Composition/containment |
| RelatedTo | Generic association |

### 7.4 Languages IDs

Language entities for localized values. IDs derived as `derived_uuid("grc20:genesis:language:" + code)`.

### 7.5 ID Derivation

Genesis IDs are derived using `derived_uuid` (Section 2.1):
```
id = derived_uuid("grc20:genesis:" + name)
```

For languages, the derivation uses:
```
id = derived_uuid("grc20:genesis:language:" + code)
```

---

## 8. Validation

### 8.1 Structural Validation (Write-Time)

Indexers MUST reject edits that fail structural validation:

| Check | Reject if |
|-------|-----------|
| Magic | Not `GRC2` or `GRC2Z` |
| Version | Unknown version |
| Lengths | Truncated/overflow |
| Dictionary counts | Greater than 0xFFFFFFFE |
| Reference indices | Index ≥ respective dictionary count |
| Language indices (TEXT) | Index > 0 and (index - 1) ≥ language_count |
| Unit indices (numerical) | Index > 0 and (index - 1) ≥ unit_count |
| UTF-8 | Invalid encoding |
| Reserved bits | Non-zero |
| Mantissa bytes | Non-minimal encoding |
| DECIMAL normalization | Mantissa has trailing zeros, or zero not encoded as {0,0} |
| Signatures | Invalid (if governance requires) |
| BOOL values | Not 0x00 or 0x01 |
| POINT bounds | Latitude outside [-90, +90] or longitude outside [-180, +180] |
| Position strings | Characters outside `0-9A-Za-z` or length > 64 |
| EMBEDDING dims | Data length doesn't match dims × bytes-per-element for subtype |
| Zstd decompression | Decompressed size doesn't match declared `uncompressed_size` |
| DataType consistency | Edit dictionary declares DataType different from established schema |
| Float values | NaN payload (see float rules in Section 2.5) |

**Implementation-defined limits:** This specification does not mandate limits on ops per edit, values per entity, or TEXT/BYTES payload sizes. Implementations and governance systems MAY impose their own limits to prevent resource exhaustion.

**Authentication and authorization:** Signature schemes, key management, and authorization rules are defined by space governance, not this specification. The `authors` field is metadata; how it maps to cryptographic identities and what signatures are required (if any) is determined by the governance layer. Error code E003 is reserved for signature validation failures when governance requires signatures.

### 8.2 Semantic Resolution (Read-Time)

| Concern | Resolution |
|---------|------------|
| Object lifecycle | Tombstone dominance |
| Duplicate creates | Merge (first creates, later updates) |
| Concurrent edits | LWW by OpPosition |
| Out-of-order arrival | Buffer until ordered position known |

**Operations on non-existent objects (NORMATIVE):**

| Operation | Target State | Resolution |
|-----------|--------------|------------|
| UpdateEntity | NOT_FOUND | Ignored (no implicit create) |
| UpdateEntity | DEAD | Ignored (tombstone dominance) |
| DeleteEntity | NOT_FOUND | Ignored (idempotent) |
| DeleteEntity | DEAD | Ignored (idempotent) |
| CreateRelation | Endpoint NOT_FOUND | Relation created (dangling reference allowed) |
| CreateRelation | Endpoint DEAD | Relation created (dangling reference allowed) |

Dangling references are permitted to support cross-space links and out-of-order edit arrival. Applications MAY enforce referential integrity at a higher layer.

### 8.3 Error Codes

| Code | Reason |
|------|--------|
| E001 | Invalid magic/version |
| E002 | Index out of bounds |
| E003 | Invalid signature |
| E004 | Invalid UTF-8 encoding |
| E005 | Malformed varint/length/reserved bits/encoding |

---
