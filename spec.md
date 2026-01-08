# GRC-20 v2 Specification

**Status:** Draft
**Version:** 0.16.0

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

**Interning:** Properties, types, and relation types are stored once in schema dictionaries (Section 4.3) and referenced by index throughout the edit.

### 2.2 Entities

```
Entity {
  id: ID
  values: List<Value>
}
```

An entity can have multiple values for the same property.

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
          | TIMESTAMP | DATE | POINT | EMBEDDING | REF
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
| REF | 11 | Object reference |

**Data type semantics:**

| Type | Encoding | Description |
|------|----------|-------------|
| BOOL | 1 byte | 0x00 = false, 0x01 = true |
| INT64 | Signed varint | -2^63 to 2^63-1 |
| FLOAT64 | IEEE 754 double, little-endian | 64-bit floating point |
| DECIMAL | exponent + mantissa | value = mantissa × 10^exponent |
| TEXT | UTF-8 string | Length-prefixed |
| BYTES | Raw bytes | Length-prefixed, opaque |
| TIMESTAMP | Signed varint | Microseconds since Unix epoch |
| DATE | UTF-8 string | ISO 8601 (variable precision) |
| POINT | Two FLOAT64, little-endian | [latitude, longitude] WGS84 |
| EMBEDDING | sub_type + dims + bytes | Dense vector for similarity search |
| REF | ID | Non-traversable object reference |

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

**Canonicalization (NORMATIVE):** Generators SHOULD normalize DECIMAL values to their most compact representation (mantissa with no trailing zeros). Exception: when precision signaling is intended.

#### DATE

ISO 8601 format for semantic time with variable precision:

```
"2024-03-15"         // Date only
"2024-03"            // Month precision
"2024"               // Year only
"-0100"              // 100 BCE
"2024-03-15T14:30Z"  // Full datetime with timezone
```

**Sorting (NORMATIVE):** Indexers MUST parse DATE strings into a numeric representation for sorting. Lexicographical string sorting does NOT work for BCE years.

#### POINT

WGS84 geographic coordinate.

```
POINT {
  latitude: float64   // -90 to +90
  longitude: float64  // -180 to +180
}
```

**Coordinate order (NORMATIVE):** `[latitude, longitude]`.

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

#### REF vs Relation

| Concept | Type | Use Case | Indexed |
|---------|------|----------|---------|
| REF | Property value | Metadata pointer: `unit: kg` | By value only |
| Relation | Directed edge | Graph edge: `Alice knows Bob` | Forward and reverse |

### 2.5 Values

A value is a property instance on an object:

```
Value {
  property: index
  value: bytes
}
```

The value's type is determined by the property's `data_type`.

**Multi-value semantics:**

An object can have multiple values for the same property (set semantics). Values are unordered; use relations with positions for ordered collections.

**Value identity:**

Values can be identified by a hash of their content:

```
value_id = SHA-256(property_id || payload_bytes)[0:16]
```

Same property + same payload = same `value_id`. Adding the same value twice is idempotent.

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
  to_space: ID?        // Optional space hint for target
  position: string?
  values: List<Value>?
}
```

The `entity` field links to an entity that represents this relation as a node. This enables relations to be referenced by other relations (meta-edges) and to participate in the graph as first-class nodes. The reified entity does not need any values defined on it.

**ID modes:**

1. **Instance mode** (default): Random ID. Multiple relations can exist between same endpoints.
2. **Unique mode**: Deterministic ID derived from content.

**Unique mode ID derivation (NORMATIVE):**
```
id = derived_uuid(from_id || to_id || type_id)
```

Where each component is the raw 16-byte UUID. Space hints and entity are NOT included in the hash.

**Relation attributes:**

Relations can hold arbitrary values (e.g., `start_date`, `end_date`, `confidence`, `version`).

**Ordering:**

Use `position` with fractional indexing. Positions are strings from alphabet `0-9A-Za-z` (62 characters, ASCII order). Maximum position length: 64 characters.

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

**UpdateEntity:**
```
UpdateEntity {
  id: ID | index
  set_properties: List<Value>?        // LWW replace
  add_values: List<Value>?            // Set union
  remove_values: List<Value>?         // Set subtraction (by content)
  remove_values_by_hash: List<ID>?    // Set subtraction (by value_id)
  unset_properties: List<ID | index>?
}
```

| Field | Strategy | Use Case |
|-------|----------|----------|
| `set_properties` | LWW Replace | Name, Age |
| `add_values` | Set Union | Tags, Emails |
| `remove_values` | Set Subtraction | Remove small values |
| `remove_values_by_hash` | Set Subtraction | Remove large values (embeddings) |
| `unset_properties` | Clear All | Reset property |

**`set_properties` semantics (NORMATIVE):** For a given property, `set_properties` replaces the property's entire value set with all `set_properties` entries for that property within this op. If multiple entries have identical identity, they deduplicate.

**`remove_values` semantics (NORMATIVE):** Removes values whose `value_id` matches the `value_id` of the removal target (computed from property and payload bytes).

**`remove_values_by_hash` semantics (NORMATIVE):** Removes values whose `value_id` matches any of the provided IDs. This avoids retransmitting large payloads (e.g., embeddings) for removal.

**Application order within op (NORMATIVE):**
1. `unset_properties`
2. `set_properties`
3. `remove_values`
4. `remove_values_by_hash`
5. `add_values`

Removals are processed before additions, allowing a single op to "replace value X with value Y" by removing X and adding Y.

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
  entity: ID               // Reified entity for this relation
  type: ID | index
  from: ID | index
  to: ID | index
  from_space: ID?          // Optional space hint for source
  to_space: ID?            // Optional space hint for target
  position: string?
  values: List<Value>?
}
```

**Semantics:** If relation exists, merge values using `set_properties` semantics.

**UpdateRelation:**
```
UpdateRelation {
  id: ID | index
  position: string?
  set_properties: List<Value>?
  add_values: List<Value>?
  remove_values: List<Value>?
  remove_values_by_hash: List<ID>?
  unset_properties: List<ID | index>?
}
```

Same field semantics and application order as UpdateEntity.

**DeleteRelation:**
```
DeleteRelation {
  id: ID | index
}
```

Appends tombstone. Subsequent updates ignored.

### 3.4 Schema Operations

**CreateProperty:**
```
CreateProperty {
  id: ID
  data_type: DataType
}
```

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
  object_ids: List<ID>
  ops: List<Op>
}
```

Edits are standalone patches. They contain no parent references—ordering is provided by on-chain governance.

**`created_at`** is metadata for audit/display only. It is NOT used for conflict resolution.

**Byte-level determinism:** This specification does not require byte-level deterministic encoding. The same logical edit MAY produce different byte sequences across implementations. Content-addressing (CID) is based on the bytes actually produced by the encoder.

### 4.2 Sequential Ordering

The state of a space is the result of replaying all accepted edits in the order defined by the governance log.

**On-chain event:**
```
event EditAccepted {
  space_id: SpaceID
  edit_cid: CID
  edit_id: EditID
  block_number: uint256
  tx_index: uint256
  log_index: uint256
}
```

**Log position:**
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

**LWW (Last-Write-Wins):** For single-value properties, the op with the highest OpPosition wins.

**Property value conflicts:**

| Scenario | Resolution |
|----------|------------|
| Different values for same property | Higher OpPosition wins |
| Delete vs Update | Delete wins (tombstone dominance) |

**Multi-value properties:** Additions from different edits are all preserved (set union).

**Structural conflicts:**

| Conflict | Resolution |
|----------|------------|
| Create same object ID | First by OpPosition wins; later creates become updates |
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

Other dictionaries (relation types, objects) allow external references via sentinel + full ID.

**Size limits (NORMATIVE):** All dictionary counts MUST be ≤ 4,294,967,294 (0xFFFFFFFE). All IDRef indices MUST be < dictionary count or equal to 0xFFFFFFFF (external sentinel). Values outside these ranges MUST be rejected (E005).

Dictionary entries SHOULD be sorted by ID bytes (lexicographic).

### 4.4 Edit Publishing

1. Serialize edit to binary format (Section 6)
2. Publish to content-addressed storage (IPFS)
3. Publish hash onchain

---

## 5. Spaces

Spaces are governance containers for edits.

### 5.1 Pluralism

The same object ID can exist in multiple spaces with different data. Consumers choose which spaces to trust. There is no global merge.

### 5.2 Cross-Space References

Object IDs are globally unique. Relations can optionally include space hints for their endpoints:

```
Relation {
  ...
  from_space: ID?    // Optional provenance hint for source
  to_space: ID?      // Optional provenance hint for target
}
```

Space hints are provenance metadata for performance, not hard requirements. Resolvers MAY use hints to prefer a specific space when resolving the target.

**Version pinning:** Use relation values for immutable citations:

```
values: [{ property: version_property_id, value: <edit_id> }]
```

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

**IDRef** (dictionary reference with external sentinel):
```
index: varint
if index == 0xFFFFFFFF:
  id: bytes[16]
```

Used for RelationTypeRef and ObjectRef with their respective dictionaries.

**PropertyRef** (dictionary reference, no external):
```
index: varint    // Must be < property_count
```

All properties used in an edit MUST be declared in the properties dictionary. External property references are not allowed.

**ObjectRef** (dictionary reference with external sentinel):
```
index: varint
if index == 0xFFFFFFFF:
  id: bytes[16]
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
object: ObjectRef
flags: uint8
  bit 0 = has_set_properties
  bit 1 = has_add_values
  bit 2 = has_remove_values
  bit 3 = has_unset_properties
  bit 4 = has_remove_values_by_hash
  bits 5-7 = reserved (must be 0)

[if has_set_properties]:
  count: varint
  values: Value[]
[if has_add_values]:
  count: varint
  values: Value[]
[if has_remove_values]:
  count: varint
  values: Value[]
[if has_unset_properties]:
  count: varint
  properties: PropertyRef[]
[if has_remove_values_by_hash]:
  count: varint
  value_ids: ID[]              // 16-byte value_id hashes
```

**DeleteEntity:**
```
object: ObjectRef
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
  bit 1 = has_values
  bit 2 = has_from_space
  bit 3 = has_to_space
  bits 4-7 = reserved (must be 0)
[if has_position]: position: String
[if has_values]:
  count: varint
  values: Value[]
[if has_from_space]: from_space: ID
[if has_to_space]: to_space: ID
```

**UpdateRelation:**
```
object: ObjectRef
flags: uint8
  bit 0 = has_position
  bit 1 = has_set_properties
  bit 2 = has_add_values
  bit 3 = has_remove_values
  bit 4 = has_unset_properties
  bit 5 = has_remove_values_by_hash
  bits 6-7 = reserved (must be 0)
[if has_position]: position: String
[if has_set_properties]:
  count: varint
  values: Value[]
[if has_add_values]:
  count: varint
  values: Value[]
[if has_remove_values]:
  count: varint
  values: Value[]
[if has_unset_properties]:
  count: varint
  properties: PropertyRef[]
[if has_remove_values_by_hash]:
  count: varint
  value_ids: ID[]              // 16-byte value_id hashes
```

**DeleteRelation:**
```
object: ObjectRef
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
```

The payload type is determined by the property's DataType (from the properties dictionary).

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
Ref: ObjectRef
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

### 7.4 ID Derivation

Genesis IDs are derived using `derived_uuid` (Section 2.1):
```
id = derived_uuid("grc20:genesis:" + name)
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
| Indices | Out of bounds or > 0xFFFFFFFE |
| Property refs | External sentinel (0xFFFFFFFF) used |
| UTF-8 | Invalid encoding |
| Reserved bits | Non-zero |
| Mantissa bytes | Non-minimal encoding |
| Signatures | Invalid (if governance requires) |

### 8.2 Semantic Resolution (Read-Time)

| Concern | Resolution |
|---------|------------|
| Object lifecycle | Tombstone dominance |
| Duplicate creates | Merge (first creates, later updates) |
| Concurrent edits | LWW by OpPosition |
| Out-of-order arrival | Buffer until ordered position known |

### 8.3 Error Codes

| Code | Reason |
|------|--------|
| E001 | Invalid magic/version |
| E002 | Index out of bounds |
| E003 | Invalid signature |
| E004 | Invalid UTF-8 encoding |
| E005 | Malformed varint/length/reserved bits/encoding |

---

## 9. Sync Protocol

Sync is driven by on-chain governance events.

### 9.1 Full Sync

1. Replay all `EditAccepted` events from genesis
2. Fetch Edit content from IPFS by CID
3. Apply ops in LogPosition order

### 9.2 Incremental Sync

1. Subscribe to new `EditAccepted` events
2. Fetch Edit, apply ops in order
3. Chain provides ordering—no local ordering logic needed

### 9.3 Chain Reorgs

1. Detect reorg via block hash mismatch
2. Roll back to last finalized block
3. Re-apply events from new chain

---

## Appendix A: Size Analysis

For 10K entities + 20K relations:

| Component | Size |
|-----------|------|
| Entity IDs | 160 KB |
| Relation IDs | 320 KB |
| Relation refs (indexed) | ~60 KB |
| Property values | ~1.2 MB |
| Schema overhead | ~10 KB |
| **Total uncompressed** | **~1.8 MB** |
| **Zstd compressed** | **~0.95 MB** |
