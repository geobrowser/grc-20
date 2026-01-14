# GRC-20 v2 Specification

**Status:** Draft
**Version:** 0.19.0

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
| Relation | A directed edge between entities, identified by ID |
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

**Byte order (NORMATIVE):** UUID bytes are encoded in network byte order (big-endian), matching the canonical hex representation. Byte `i` corresponds to hex digits `2i` and `2i+1` of the standard 32-character hex string. For example, UUID `550e8400-e29b-41d4-a716-446655440000` is encoded as bytes `[0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, ...]`. This applies everywhere UUIDs appear: dictionary entries, inline IDs, derived ID inputs, and sorting comparisons.

**Random IDs:** Use UUIDv4 (random) or UUIDv7 (time-ordered). UUIDv7 is RECOMMENDED for entities and relations as it enables time-based sorting.

**Derived IDs:** Content-addressed IDs use UUIDv8 with SHA-256:

```
derived_uuid(input_bytes) -> UUID:
  hash = SHA-256(input_bytes)[0:16]
  hash[6] = (hash[6] & 0x0F) | 0x80  // version 8
  hash[8] = (hash[8] & 0x3F) | 0x80  // RFC 4122 variant
  return hash
```

When deriving from string prefixes (e.g., `"grc20:relation-entity:"`), the string is UTF-8 encoded with no trailing NUL byte.

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
- `ACTIVE` — Entity exists and accepts updates
- `DELETED` — Entity is tombstoned; subsequent updates are ignored

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
          | DATE | SCHEDULE | POINT | EMBEDDING
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
| DATE | 7 | ISO 8601 date string |
| SCHEDULE | 8 | RFC 5545 schedule or availability |
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
| DATE | UTF-8 string | ISO 8601 (variable precision) |
| SCHEDULE | UTF-8 string | RFC 5545 iCalendar component |
| POINT | 2-3 FLOAT64, little-endian | [lon, lat] or [lon, lat, alt] WGS84 |
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

Calendar dates with variable precision using the **proleptic Gregorian calendar**. Use DATE for semantic dates where precision matters (historical events, birthdays, publication years). Use INT64 for exact instants (microseconds since epoch).

```
"2024-03-15"         // Day precision
"2024-03"            // Month precision
"2024"               // Year only
"-0100"              // 100 BCE (astronomical year -100)
```

**Grammar (NORMATIVE):**
```abnf
date        = year / year-month / year-month-day
year        = [sign] 4DIGIT
year-month  = [sign] 4DIGIT "-" 2DIGIT
year-month-day = [sign] 4DIGIT "-" 2DIGIT "-" 2DIGIT
sign        = "+" / "-"
```

Week dates (`2024-W01`) and ordinal dates (`2024-001`) are NOT supported.

**Calendar basis (NORMATIVE):** All dates use the proleptic Gregorian calendar (Gregorian rules extended backwards before 1582).

**Year numbering (NORMATIVE):** Years use astronomical year numbering where year 0 exists:
- Year `0001` = 1 CE
- Year `0000` = 1 BCE
- Year `-0001` = 2 BCE
- Year `-0100` = 101 BCE

This follows ISO 8601 extended year format. Note: historical "BCE" numbering has no year zero, so BCE year N = astronomical year -(N-1).

**DATE vs INT64 timestamps:** DATE preserves the original precision and is stored as a string. For exact timestamps, use INT64 with microseconds since epoch. A birthday is a DATE ("1990-05-20"); a login event is an INT64 timestamp.

**Sorting (NORMATIVE):** Indexers MUST parse DATE strings into a numeric representation for sorting. Lexicographical string sorting does NOT work for BCE years. Dates sort by their earliest possible UTC instant on the proleptic Gregorian calendar; when two dates resolve to the same instant, the more precise date sorts first (e.g., `2024-01-01` < `2024-01` < `2024`). Tie-break by byte comparison of the original string if instants and precisions are equal.

**Validation (NORMATIVE):** DATE strings MUST conform to the grammar above. Full datetime with timezone (e.g., "2024-03-15T14:30Z") SHOULD use INT64 timestamp or SCHEDULE instead. Implementations MUST reject:
- Month outside 01-12
- Day outside valid range for the month (considering leap years)
- Malformed structure (wrong separators, wrong digit counts)

#### SCHEDULE

RFC 5545 iCalendar component for recurring events and availability.

```
"DTSTART:20240315T090000Z\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR"   // Weekly on Mon/Wed/Fri
"DTSTART:20240101\nRRULE:FREQ=YEARLY"                           // Annual event
"FREEBUSY:20240315T090000Z/20240315T170000Z"                    // Availability window
```

**Grammar (NORMATIVE):** SCHEDULE values contain one or more iCalendar properties as defined in RFC 5545. The value MUST be a valid sequence of iCalendar content lines (properties). Common patterns:

- **Recurring events:** `DTSTART` with optional `RRULE`, `RDATE`, or `EXDATE`
- **Availability:** `FREEBUSY` periods

**Line folding (NORMATIVE):** Content lines MAY use RFC 5545 line folding (CRLF followed by a space or tab). Implementations MUST unfold before parsing.

**Validation (NORMATIVE):** Implementations MUST validate that the value parses as valid iCalendar properties per RFC 5545. Invalid property names, malformed date-times, or syntax errors MUST be rejected (E005).

#### POINT

WGS84 geographic coordinate with 2 or 3 ordinates.

```
POINT {
  longitude: float64  // -180 to +180 (required)
  latitude: float64   // -90 to +90 (required)
  altitude: float64?  // meters above WGS84 ellipsoid (optional)
}
```

**Coordinate order (NORMATIVE):** `[longitude, latitude]` or `[longitude, latitude, altitude]`.

**Bounds validation (NORMATIVE):** Longitude MUST be in range [-180, +180]. Latitude MUST be in range [-90, +90]. Values outside these ranges MUST be rejected (E005). Altitude has no bounds restrictions.

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
  from_space: ID?      // Optional space pin for source
  from_version: ID?    // Optional version pin for source
  to_space: ID?        // Optional space pin for target
  to_version: ID?      // Optional version pin for target
  position: string?
}
```

**Endpoint constraint (NORMATIVE):** The `from` and `to` fields MUST reference entities, not relations. To create a meta-edge (a relation that references another relation), target the other relation's reified entity via its `entity` ID.

The `entity` field links to an entity that represents this relation as a node. This enables relations to be referenced by other relations (meta-edges) and to participate in the graph as first-class nodes. Values are stored on the reified entity, not on the relation itself.

**Reified entity creation (NORMATIVE):** CreateRelation implicitly creates the reified entity if it does not exist. No separate CreateEntity op is required. If an entity with the given ID already exists, it is reused—its existing values are preserved and it becomes associated with this relation.

**ID modes:**

1. **Unique mode** (default): Deterministic ID derived from `(from, to, type)`. Only one relation can exist for a given triple.
2. **Many mode**: Caller-provided ID. Multiple relations can exist between same endpoints.

**Unique mode ID derivation (NORMATIVE):**
```
id = derived_uuid(from_id || to_id || type_id)
```

Where each component is the raw 16-byte UUID. Space pins and entity are NOT included in the hash.

**Entity ID derivation (NORMATIVE):**

The `entity` field can be explicit (caller-provided) or auto-derived:

- **Auto-derived (default):** When `entity` is absent in CreateRelation, the entity ID is deterministically computed:
  ```
  entity_id = derived_uuid("grc20:relation-entity:" || relation_id)
  ```
  Where `relation_id` is the 16-byte UUID (either provided in many mode or derived in unique mode).

- **Explicit:** When `entity` is provided, that ID is used directly. This enables multiple relations to share a single reified entity (hypergraph/bundle patterns).

**Unique mode constraint (NORMATIVE):** In unique mode, `entity` MUST be absent (auto-derived). This ensures unique-mode relations are fully deterministic and race-free—concurrent creators automatically converge on the same relation ID and entity ID without coordination.

**Many mode flexibility:** In many mode, `entity` MAY be provided to enable sharing a reified entity across multiple relations. When multiple relations share an entity, values set on that entity are shared across all those relations.

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

**Position validation (NORMATIVE):** Positions containing characters outside `0-9A-Za-z` or exceeding 64 characters MUST be rejected (E005). Empty position strings are NOT permitted.

**Ordering semantics (NORMATIVE):**
1. Relations with a `position` sort before relations without a position.
2. Positions compare lexicographically using ASCII byte order over `0-9A-Za-z` (i.e., `0` < `9` < `A` < `Z` < `a` < `z`).
3. If positions are equal, tie-break by relation ID bytes (lexicographic unsigned comparison).
4. Relations without positions are ordered by relation ID bytes.

**Immutability (NORMATIVE):** The structural fields (`entity`, `type`, `from`, `to`) are immutable after creation. To change endpoints, delete and recreate.

### 2.7 Per-Space State

**NORMATIVE:** Resolved state is scoped to a space:

```
state(space_id, object_id) → Object | DELETED | NOT_FOUND
```

The same object ID can have different state in different spaces. Multi-space views are computed by resolver policy and MUST preserve provenance.

**Object ID namespace (NORMATIVE):** Entity IDs and Relation IDs share a single namespace within each space. A given UUID identifies exactly one kind of object:

| Scenario | Resolution |
|----------|------------|
| CreateEntity where Relation with same ID exists | Ignored (ID already in use) |
| CreateRelation where Entity with same ID exists | Ignored (ID already in use) |
| CreateRelation with explicit `entity` that equals `relation.id` | Invalid; `entity` MUST differ from the relation ID |

The auto-derived entity ID (`derived_uuid("grc20:relation-entity:" || relation_id)`) is guaranteed to differ from the relation ID due to the prefix, so this constraint only applies to explicit `entity` values in many-mode.

**Rationale:** A single namespace simplifies the state model and prevents ambiguity in `state()` lookups. Reified entities are distinct objects that happen to represent relations as nodes.

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
    RestoreEntity    = 4
    CreateRelation   = 5
    UpdateRelation   = 6
    DeleteRelation   = 7
    RestoreRelation  = 8
    CreateProperty   = 9
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
  language: uint32    // 0xFFFFFFFF = clear all, 0 = non-linguistic, 1+ = specific language
}
```

| Field | Strategy | Use Case |
|-------|----------|----------|
| `set_properties` | LWW Replace | Name, Age |
| `unset_properties` | Clear | Reset property or specific language |

**`set_properties` semantics (NORMATIVE):** For a given property (and language, for TEXT), `set_properties` replaces the existing value. For TEXT values, each language is treated independently—setting a value for one language does not affect values in other languages.

**`unset_properties` semantics (NORMATIVE):** Clears values for properties. For TEXT properties, the `language` field specifies which slot to clear: `0xFFFFFFFF` clears all language slots, `0` clears the non-linguistic slot, and `1+` clears a specific language slot. For non-TEXT properties, `language` MUST be `0xFFFFFFFF` (clear all) and the single value is cleared.

**Application order within op (NORMATIVE):**
1. `unset_properties`
2. `set_properties`

**DeleteEntity:**
```
DeleteEntity {
  id: ID | index
}
```

Transitions the entity to DELETED state (tombstoned).

**Tombstone semantics (NORMATIVE):**
- Once DELETED, subsequent UpdateEntity ops for this entity are ignored.
- Once DELETED, subsequent CreateEntity ops for this entity are ignored (tombstone absorbs upserts).
- The entity can only be restored via explicit RestoreEntity.
- Tombstones are deterministic: all indexers replaying the same log converge on the same DELETED state.

**RestoreEntity:**
```
RestoreEntity {
  id: ID | index
}
```

Transitions a DELETED entity back to ACTIVE state.

**Semantics (NORMATIVE):**
- If the entity is DELETED, restore it to ACTIVE. Property values are preserved (delete hides, restore reveals).
- If the entity is ACTIVE or does not exist, the op is ignored (no-op).
- After restore, subsequent updates apply normally.

**Design rationale:** Explicit restore prevents accidental resurrection by stale/offline clients while allowing governance-controlled undo. Random CreateEntity/UpdateEntity cannot bring back deleted entities—only intentional RestoreEntity can.

### 3.3 Relation Operations

**CreateRelation:**
```
CreateRelation {
  id: ID?                  // Present = many mode; absent = unique mode
  type: ID | index
  from: ID | index
  from_space: ID?          // Optional space pin for source
  from_version: ID?        // Optional version pin for source
  to: ID | index
  to_space: ID?            // Optional space pin for target
  to_version: ID?          // Optional version pin for target
  entity: ID?              // Explicit reified entity; absent = auto-derived
  position: string?
}
```

**Semantics (NORMATIVE):** If the relation does not exist, create it along with its reified entity (if that entity does not already exist). If the relation already exists with the same ID, the op is ignored (relations are immutable except for position). To add values to the relation, use UpdateEntity on the reified entity ID.

**Tombstone interactions (NORMATIVE):**
- If the relation ID exists but is DELETED, CreateRelation is ignored (tombstone absorbs; use RestoreRelation to revive).
- If the reified entity ID exists but is DELETED, the relation is still created, but the reified entity remains DELETED. Values cannot be added to the relation until the entity is restored via RestoreEntity. This is an edge case that occurs when an entity is explicitly deleted after being used as a reified entity, or when the same ID is reused.

**Entity ID resolution:**
- If `entity` is absent: `entity_id = derived_uuid("grc20:relation-entity:" || relation_id)`
- If `entity` is present: `entity_id = entity` (many mode only)

**Constraint (NORMATIVE):** If `id` is absent (unique mode), `entity` MUST also be absent. Edits that provide `entity` in unique mode MUST be rejected (E005).

**UpdateRelation:**
```
UpdateRelation {
  id: ID | index
  position: string?
}
```

Updates the relation's position. All other fields (`entity`, `type`, `from`, `to`, space pins, version pins) are immutable after creation.

**DeleteRelation:**
```
DeleteRelation {
  id: ID | index
}
```

Transitions the relation to DELETED state (tombstoned).

**Tombstone semantics (NORMATIVE):**
- Once DELETED, subsequent UpdateRelation ops for this relation are ignored.
- Once DELETED, subsequent CreateRelation ops that would produce the same relation ID are ignored (tombstone absorbs).
- The relation can only be restored via explicit RestoreRelation.

**RestoreRelation:**
```
RestoreRelation {
  id: ID | index
}
```

Transitions a DELETED relation back to ACTIVE state.

**Semantics (NORMATIVE):**
- If the relation is DELETED, restore it to ACTIVE.
- If the relation is ACTIVE or does not exist, the op is ignored (no-op).
- After restore, subsequent updates apply normally.

**Unique-mode relation lifecycle:** Because unique-mode relation IDs are derived from `(from, to, type)`, deleting and later wanting to re-add the same relation would produce the same ID. Without RestoreRelation, this would be permanently blocked. RestoreRelation enables the full lifecycle: create → delete → restore → delete → ...

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

**Write-time:** Validate structure, append to log. No state lookups required except for DataType consistency checks (Section 8.1), which require knowledge of previously-established property DataTypes. Indexers SHOULD maintain a property ID → DataType index for efficient validation.

**Read-time:** Replay operations in log order, apply resolution rules, return computed state.

**Resolution rules:**

1. Replay ops in log order (Section 4.2)
2. Apply merge rules (Section 4.2.1)
3. Tombstone dominance: updates after delete are ignored
4. Return resolved state or DELETED status

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

**Content addressing (NORMATIVE):** CIDs and signatures MUST be computed over **uncompressed** canonical-mode bytes (the `GRC2` payload). Compression is a transport optimization and is not part of the signed/hashed content. This ensures:
- Different zstd implementations/settings don't cause CID divergence
- Signatures remain valid regardless of transport compression
- Decompressed content can be verified against the original CID

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

**Language dictionary requirement (NORMATIVE):** All languages referenced in TEXT values MUST be declared in the `language_ids` dictionary. Language index 0 means non-linguistic (language-agnostic content; no entry required); indices 1+ reference `language_ids[index-1]`. Only TEXT values have the language field. To store linguistic text (including English), the appropriate language ID must be in the dictionary.

**Unit dictionary requirement (NORMATIVE):** All units referenced in numerical values (INT64, FLOAT64, DECIMAL) MUST be declared in the `unit_ids` dictionary. Unit index 0 means no unit; indices 1+ reference `unit_ids[index-1]`. Only numerical values have the unit field.

**Object dictionary requirement (NORMATIVE):** All objects (entities and relations) referenced in an edit MUST be declared in the `object_ids` dictionary. This includes: operation targets (UpdateEntity, DeleteEntity, etc.) and relation endpoints (`from`, `to`).

**Unique-mode relations:** In unique mode, the relation ID is derived (Section 2.6). To reference a unique-mode relation in the same edit (e.g., UpdateRelation to set position), the encoder MUST compute the derived ID and include it in `object_ids`. CreateRelation itself does not require the relation ID in the dictionary since it encodes the ID inline (many mode) or derives it (unique mode).

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

2. **Sorted authors:** The `authors` list MUST be sorted by ID bytes in ascending lexicographic order. Duplicate author IDs are NOT permitted.

3. **Sorted value lists:** `CreateEntity.values` and `UpdateEntity.set_properties` MUST be sorted by `(propertyRef, languageRef)` in ascending order (property index first, then language index). Duplicate `(property, language)` entries are NOT permitted.

4. **Sorted unset lists:** `UpdateEntity.unset_properties` MUST be sorted by `(propertyRef, language)` in ascending order. Duplicate entries (same property and language) are NOT permitted.

5. **Minimal varints:** (Note: This is now a general requirement per Section 6.1, not canonical-only.)

6. **Consistent field encoding:** Optional fields use presence flags as specified in Section 6. No additional padding or alignment bytes.

7. **No duplicate dictionary entries:** Each dictionary MUST NOT contain duplicate IDs. Edits with duplicate IDs in any dictionary MUST be rejected.

**Performance note:** Canonical encoding requires sorting dictionaries and authors after collection, which is substantially slower than fast mode. Implementations SHOULD offer both modes.

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

Object IDs are globally unique. Relations can optionally include space and version pins for their endpoints:

```
Relation {
  ...
  from_space: ID?      // Optional space pin for source
  from_version: ID?    // Optional version pin for source
  to_space: ID?        // Optional space pin for target
  to_version: ID?      // Optional version pin for target
}
```

**Space pins:** The `from_space` and `to_space` fields pin relation endpoints to a specific space. This enables precise cross-space references where the relation refers to the entity as it exists in that specific space, rather than relying on resolution heuristics. Space pins are immutable after creation.

**Version pins:** The `from_version` and `to_version` fields pin relation endpoints to a specific version (edit ID). This enables immutable citations where the relation always refers to the entity as it existed at that specific edit, rather than the current resolved state. Version pins are immutable after creation.

---

## 6. Binary Format

### 6.1 Primitive Encoding

**Varint:** Unsigned LEB128
```
0-127:       1 byte   [0xxxxxxx]
128-16383:   2 bytes  [1xxxxxxx 0xxxxxxx]
```

**Varint bounds (NORMATIVE):** Varints MUST NOT exceed 10 bytes (sufficient for u64). Varints MUST use minimal encoding—the fewest bytes required to represent the value. Overlong encodings (e.g., encoding 1 as `81 00` instead of `01`) MUST be rejected (E005). This applies to all varints, not just canonical mode.

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
index: varint    // 0 = non-linguistic, 1+ = language_ids[index-1]
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
  4 = RestoreEntity
  5 = CreateRelation
  6 = UpdateRelation
  7 = DeleteRelation
  8 = RestoreRelation
  9 = CreateProperty
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
  language: varint    // 0xFFFFFFFF = clear all languages, otherwise LanguageRef (0 = non-linguistic, 1+ = specific language)
```

**DeleteEntity:**
```
id: ObjectRef
```

**RestoreEntity:**
```
id: ObjectRef
```

**CreateRelation:**
```
mode: uint8                    // 0 = unique, 1 = many
[if mode == 1]: id: ID
type: RelationTypeRef
from: ObjectRef
to: ObjectRef
flags: uint8
  bit 0 = has_from_space
  bit 1 = has_from_version
  bit 2 = has_to_space
  bit 3 = has_to_version
  bit 4 = has_entity           // If 0, entity is auto-derived from relation ID
  bit 5 = has_position
  bits 6-7 = reserved (must be 0)
[if has_from_space]: from_space: ID
[if has_from_version]: from_version: ID
[if has_to_space]: to_space: ID
[if has_to_version]: to_version: ID
[if has_entity]: entity: ID    // Explicit reified entity (many mode only)
[if has_position]: position: String
```

**Entity derivation:** When `has_entity = 0`, the entity ID is computed as `derived_uuid("grc20:relation-entity:" || relation_id)`. When `mode = 0` (unique), `has_entity` MUST be 0.

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

**RestoreRelation:**
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

**Language (TEXT only):** The `language` field is only present for TEXT values. A value with `language = 0` is non-linguistic (language-agnostic content such as URLs, identifiers, codes, or formulas). Values with different languages for the same property are distinct and can coexist. To store English text, use the well-known English language ID (Section 7.4).

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
Date: len: varint, data: UTF-8 bytes (ISO 8601)
Schedule: len: varint, data: UTF-8 bytes (RFC 5545)
Point: ordinate_count: uint8 (2 or 3), longitude: Float64, latitude: Float64, [altitude: Float64]
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

Edits SHOULD be compressed with zstd for transport efficiency.

```
Magic: "GRC2Z" (5 bytes)
uncompressed_size: varint
compressed_data: zstd frame
```

**Compression is a transport wrapper (NORMATIVE):** The `GRC2Z` format wraps the uncompressed `GRC2` payload. CIDs and signatures are computed over the uncompressed payload, not the compressed bytes (see Section 4.1). Implementations MAY use any zstd compression level; level 3+ is RECOMMENDED for a good size/speed tradeoff.

---

## 7. Genesis Space

The Genesis Space provides well-known IDs for universal concepts. These are the minimum IDs needed at the protocol level; applications may define additional schema on top.

### 7.1 Core Properties

| Name | UUID | Data Type | Description |
|------|------|-----------|-------------|
| Name | `a126ca530c8e48d5b88882c734c38935` | TEXT | Primary label |
| Description | `9b1f76ff9711404c861e59dc3fa7d037` | TEXT | Summary text |
| Cover | `34f535072e6b42c5a84443981a77cfa2` | TEXT | Cover image URL |

### 7.2 Core Type

| Name | UUID | Description |
|------|------|-------------|
| Image | `f3f790c4c74e4d23a0a91e8ef84e30d9` | Image entity |

### 7.3 Core Relation Type

| Name | UUID | Description |
|------|------|-------------|
| Types | `8f151ba4de204e3c9cb499ddf96f48f1` | Type membership |

### 7.4 Language IDs

Language entities for localized TEXT values. IDs are derived using BCP 47 language tags in lowercase with hyphen separators:
```
id = derived_uuid("grc20:genesis:language:" + bcp47_tag)
```

**Well-known language IDs:**

| Language | BCP 47 | Derivation |
|----------|--------|------------|
| English | en | `derived_uuid("grc20:genesis:language:en")` |
| Spanish | es | `derived_uuid("grc20:genesis:language:es")` |
| Chinese (Simplified) | zh-hans | `derived_uuid("grc20:genesis:language:zh-hans")` |
| Chinese (Traditional) | zh-hant | `derived_uuid("grc20:genesis:language:zh-hant")` |
| Arabic | ar | `derived_uuid("grc20:genesis:language:ar")` |
| Hindi | hi | `derived_uuid("grc20:genesis:language:hi")` |
| Portuguese | pt | `derived_uuid("grc20:genesis:language:pt")` |
| Russian | ru | `derived_uuid("grc20:genesis:language:ru")` |
| Japanese | ja | `derived_uuid("grc20:genesis:language:ja")` |
| French | fr | `derived_uuid("grc20:genesis:language:fr")` |
| German | de | `derived_uuid("grc20:genesis:language:de")` |
| Korean | ko | `derived_uuid("grc20:genesis:language:ko")` |

**Canonicalization (NORMATIVE):** Language tags MUST be normalized to lowercase before derivation. For example, `"EN"`, `"En"`, and `"en"` all derive the same ID using `"en"`.

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
| Dictionary duplicates | Same ID appears twice in any dictionary |
| Author duplicates | Same author ID appears twice (canonical mode) |
| Value duplicates | Same `(property, language)` appears twice in values/set_properties (canonical mode) |
| Unset duplicates | Same `(property, language)` appears twice in unset_properties (canonical mode) |
| Language indices (TEXT) | Index not 0xFFFFFFFF and index > 0 and (index - 1) ≥ language_count |
| UnsetProperty language (non-TEXT) | Language value is not 0xFFFFFFFF |
| Unit indices (numerical) | Index > 0 and (index - 1) ≥ unit_count |
| UTF-8 | Invalid encoding |
| Varint encoding | Overlong encoding or exceeds 10 bytes |
| Reserved bits | Non-zero |
| Mantissa bytes | Non-minimal encoding |
| DECIMAL normalization | Mantissa has trailing zeros, or zero not encoded as {0,0} |
| Signatures | Invalid (if governance requires) |
| BOOL values | Not 0x00 or 0x01 |
| POINT bounds | Longitude outside [-180, +180] or latitude outside [-90, +90] |
| POINT ordinate count | ordinate_count not 2 or 3 |
| DATE format | Does not match grammar, invalid month (>12), invalid day for month |
| Position strings | Empty, characters outside `0-9A-Za-z`, or length > 64 |
| EMBEDDING dims | Data length doesn't match dims × bytes-per-element for subtype |
| Zstd decompression | Decompressed size doesn't match declared `uncompressed_size` |
| DataType consistency | Edit dictionary declares DataType different from established schema |
| Float values | NaN payload (see float rules in Section 2.5) |
| Unique mode entity | CreateRelation has mode=0 (unique) and has_entity=1 |
| Relation entity self-reference | CreateRelation has explicit `entity` equal to relation ID |

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

**Derived ID pre-creation:** Because relation entity IDs are derived deterministically (`derived_uuid("grc20:relation-entity:" || relation_id)`), an attacker can pre-create an entity with that ID and set values before the relation exists. When the relation is later created, it adopts the existing entity with its values. This is known behavior, not a vulnerability—applications concerned about this can verify entity provenance at a higher layer.

**Authentication and authorization:** Signature schemes, key management, and authorization rules are defined by space governance, not this specification. The `authors` field is metadata; how it maps to cryptographic identities and what signatures are required (if any) is determined by the governance layer. Error code E003 is reserved for signature validation failures when governance requires signatures.

### 8.2 Semantic Resolution (Read-Time)

| Concern | Resolution |
|---------|------------|
| Object lifecycle | Tombstone dominance |
| Duplicate creates | Merge (first creates, later updates) |
| Concurrent edits | LWW by OpPosition |
| Out-of-order arrival | Buffer until ordered position known |

**Operations on non-existent or deleted objects (NORMATIVE):**

| Operation | Target State | Resolution |
|-----------|--------------|------------|
| UpdateEntity | NOT_FOUND | Ignored (no implicit create) |
| UpdateEntity | DELETED | Ignored (tombstone dominance) |
| DeleteEntity | NOT_FOUND | Ignored (idempotent) |
| DeleteEntity | DELETED | Ignored (idempotent) |
| CreateEntity | DELETED | Ignored (tombstone absorbs upserts) |
| UpdateRelation | NOT_FOUND | Ignored |
| UpdateRelation | DELETED | Ignored (tombstone dominance) |
| DeleteRelation | NOT_FOUND | Ignored (idempotent) |
| DeleteRelation | DELETED | Ignored (idempotent) |
| CreateRelation | Relation DELETED | Ignored (tombstone absorbs) |
| CreateRelation | Reified entity DELETED | Relation created, but entity stays DELETED |
| CreateRelation | Endpoint NOT_FOUND | Relation created (dangling reference allowed) |
| CreateRelation | Endpoint DELETED | Relation created (dangling reference allowed) |
| CreateEntity | Relation with same ID exists | Ignored (namespace collision) |
| CreateRelation | Entity with same ID exists | Ignored (namespace collision) |

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
