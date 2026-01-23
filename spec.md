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
| Object | An Entity, Relation, or Value Ref (used when referencing all three) |
| Property | An entity representing a named attribute |
| Value | A property instance on an object |
| Value Ref | A referenceable handle for a value slot, identified by ID |
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

**Random IDs:** Use UUIDv4 (random).

**Derived IDs:** Content-addressed IDs SHOULD use UUIDv8 with SHA-256:

```
derived_uuid(input_bytes) -> UUID:
  hash = SHA-256(input_bytes)[0:16]
  hash[6] = (hash[6] & 0x0F) | 0x80  // version 8
  hash[8] = (hash[8] & 0x3F) | 0x80  // RFC 4122 variant
  return hash
```

When deriving from string prefixes (e.g., `"grc20:relation-entity:"`), the string is UTF-8 encoded with no trailing NUL byte.

**Display format:** Non-hyphenated lowercase hex is RECOMMENDED. Implementations MAY accept hyphenated or Base58 on input.

### 2.2 Entities

```
Entity {
  id: ID
  values: List<Value>
}
```

Values are unique per (entityId, propertyId), or per (entityId, propertyId, language) for TEXT values. When multiple values for a given (entity, property) pair are required, use relations instead.

Type membership is expressed via `Types` relations (Section 7.3), not a dedicated types field.

### 2.3 Types

Types are entities that classify other entities via `Types` relations. An entity can have multiple types simultaneously. Types are created using CreateEntity; type names and metadata are added as values in the knowledge layer.

Types are tags, not classes: no inheritance, no cardinality constraints, no property enforcement.

### 2.4 Properties

Properties are entities that define attributes. Property names, descriptions, and data types are defined via values and relations in the knowledge layer, not in the protocol.

```
DataType := BOOL | INT64 | FLOAT64 | DECIMAL | TEXT | BYTES
          | DATE | TIME | DATETIME | SCHEDULE | POINT | RECT | EMBEDDING
```

**Data types in edits:** Each edit declares the data type for each property it uses (Section 4.3). All values for a given property within an edit MUST use the same data type. Different edits MAY use different data types for the same property—the data type is per-value metadata, not a global constraint.

**Data type hints:** Property entities SHOULD have a `Data Type` relation (Section 7.3) pointing to a data type entity (Section 7.5) to indicate the expected type. This is advisory—applications use it for UX and query defaults, but the protocol does not enforce it.

**Data type enum values:**

| Type | Value | Description |
|------|-------|-------------|
| BOOL | 1 | Boolean |
| INT64 | 2 | 64-bit signed integer |
| FLOAT64 | 3 | 64-bit IEEE 754 float |
| DECIMAL | 4 | Arbitrary-precision decimal |
| TEXT | 5 | UTF-8 string |
| BYTES | 6 | Opaque byte array |
| DATE | 7 | Calendar date with timezone |
| TIME | 8 | Time of day with timezone |
| DATETIME | 9 | Timestamp with timezone |
| SCHEDULE | 10 | RFC 5545 schedule or availability |
| POINT | 11 | WGS84 coordinate |
| RECT | 12 | Axis-aligned bounding box |
| EMBEDDING | 13 | Dense vector |

**Data type semantics:**

| Type | Encoding | Description |
|------|----------|-------------|
| BOOL | 1 byte | 0x00 = false, 0x01 = true; other values invalid |
| INT64 | Signed varint | -2^63 to 2^63-1 |
| FLOAT64 | IEEE 754 double, little-endian | 64-bit floating point |
| DECIMAL | exponent + mantissa | value = mantissa × 10^exponent |
| TEXT | UTF-8 string | Length-prefixed |
| BYTES | Raw bytes | Length-prefixed, opaque |
| DATE | 6 bytes | days_since_epoch (int32) + offset_min (int16) |
| TIME | 8 bytes | time_us (int48) + offset_min (int16) |
| DATETIME | 10 bytes | epoch_us (int64) + offset_min (int16) |
| SCHEDULE | UTF-8 string | RFC 5545 iCalendar component |
| POINT | 2-3 FLOAT64, little-endian | [lat, lon] or [lat, lon, alt] WGS84 |
| RECT | 4 FLOAT64, little-endian | [min_lat, min_lon, max_lat, max_lon] WGS84 |
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

Calendar date represented as a fixed 6-byte binary value.

```
DATE {
  days: int32       // Signed days since Unix epoch (1970-01-01)
  offset_min: int16 // Signed UTC offset in minutes (e.g., +330 for +05:30)
}
```

**Wire format (NORMATIVE):** 6 bytes, fixed-width:
- Bytes 0–3: `days` (signed 32-bit integer, little-endian)
- Bytes 4–5: `offset_min` (signed 16-bit integer, little-endian)

The `days` field represents the calendar date as days since 1970-01-01. The `offset_min` indicates the timezone context where the date is meaningful.

**Examples:**
- March 15, 2024 UTC: `days = 19797`, `offset_min = 0`
- March 15, 2024 in +05:30: `days = 19797`, `offset_min = 330`

**Range:** int32 days provides a range of ±5.8 million years from 1970.

**Validation (NORMATIVE):** Implementations MUST reject:
- `offset_min` outside range [-1440, +1440] (±24 hours)

**Sorting (NORMATIVE):** DATE values sort by `days` first, then by `offset_min` (both as signed integers).

#### TIME

Time of day represented as a fixed 8-byte binary value.

```
TIME {
  time_us: int48    // Microseconds since midnight (0 to 86,399,999,999)
  offset_min: int16 // Signed UTC offset in minutes (e.g., +330 for +05:30)
}
```

**Wire format (NORMATIVE):** 8 bytes, fixed-width:
- Bytes 0–5: `time_us` (signed 48-bit integer, little-endian)
- Bytes 6–7: `offset_min` (signed 16-bit integer, little-endian)

The `time_us` field represents microseconds since midnight in the local timezone indicated by `offset_min`.

**Examples:**
- 14:30:00 UTC: `time_us = 52200000000`, `offset_min = 0`
- 14:30:00.500 in +05:30: `time_us = 52200500000`, `offset_min = 330`
- Midnight UTC: `time_us = 0`, `offset_min = 0`

**Range:** int48 microseconds easily covers a full day (max 86,399,999,999 µs).

**Validation (NORMATIVE):** Implementations MUST reject:
- `time_us` outside range [0, 86,399,999,999]
- `offset_min` outside range [-1440, +1440] (±24 hours)

**Sorting (NORMATIVE):** TIME values sort by their UTC-normalized instant (`time_us - offset_min * 60_000_000`). Tie-break by `offset_min`.

#### DATETIME

Combined date and time represented as a fixed 10-byte binary value.

```
DATETIME {
  epoch_us: int64   // Microseconds since Unix epoch (1970-01-01T00:00:00Z)
  offset_min: int16 // Signed UTC offset in minutes (e.g., +330 for +05:30)
}
```

**Wire format (NORMATIVE):** 10 bytes, fixed-width:
- Bytes 0–7: `epoch_us` (signed 64-bit integer, little-endian)
- Bytes 8–9: `offset_min` (signed 16-bit integer, little-endian)

The `epoch_us` field represents the instant in UTC. The `offset_min` preserves the original timezone context for display purposes.

**Examples:**
- 2024-03-15T14:30:00Z: `epoch_us = 1710513000000000`, `offset_min = 0`
- 2024-03-15T14:30:00+05:30: `epoch_us = 1710493200000000`, `offset_min = 330`

**Why microseconds:** int64 microseconds provides a range of ±292,000 years with sub-millisecond precision. int64 nanoseconds would overflow around year 2262.

**Validation (NORMATIVE):** Implementations MUST reject:
- `offset_min` outside range [-1440, +1440] (±24 hours)

**Sorting (NORMATIVE):** DATETIME values sort by `epoch_us` first, then by `offset_min` (both as signed integers).

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
  latitude: float64   // -90 to +90 (required)
  longitude: float64  // -180 to +180 (required)
  altitude: float64?  // meters above WGS84 ellipsoid (optional)
}
```

**Coordinate order (NORMATIVE):** `[latitude, longitude]` or `[latitude, longitude, altitude]`.

**Bounds validation (NORMATIVE):** Latitude MUST be in range [-90, +90]. Longitude MUST be in range [-180, +180]. Values outside these ranges MUST be rejected (E005). Altitude has no bounds restrictions.

For complex geometry (polygons, lines), use BYTES with WKB encoding.

#### RECT

Axis-aligned bounding box in WGS84 coordinates, represented as a fixed 32-byte binary value.

```
RECT {
  min_lat: float64  // Southern edge, -90 to +90
  min_lon: float64  // Western edge, -180 to +180
  max_lat: float64  // Northern edge, -90 to +90
  max_lon: float64  // Eastern edge, -180 to +180
}
```

**Wire format (NORMATIVE):** 32 bytes, fixed-width:
- Bytes 0–7: `min_lat` (IEEE 754 double, little-endian)
- Bytes 8–15: `min_lon` (IEEE 754 double, little-endian)
- Bytes 16–23: `max_lat` (IEEE 754 double, little-endian)
- Bytes 24–31: `max_lon` (IEEE 754 double, little-endian)

**Coordinate order (NORMATIVE):** `[min_lat, min_lon, max_lat, max_lon]` (southwest corner, then northeast corner).

**Examples:**
- Continental US: `min_lat = 24.5`, `min_lon = -125.0`, `max_lat = 49.4`, `max_lon = -66.9`
- Prime meridian crossing: `min_lat = 35.0`, `min_lon = -10.0`, `max_lat = 55.0`, `max_lon = 10.0`

**Bounds validation (NORMATIVE):** Implementations MUST reject:
- `min_lat` or `max_lat` outside range [-90, +90]
- `min_lon` or `max_lon` outside range [-180, +180]
- NaN values in any coordinate

**Note:** `min_lon > max_lon` is valid and indicates a bounding box that crosses the antimeridian (±180°).

#### EMBEDDING

Dense vector for semantic similarity search.

```
EMBEDDING {
  sub_type: FLOAT32 | INT8 | BINARY
  dimensions: int
  data: bytes
}
```

| Sub-type | Description | Bytes per dim |
|----------|-------------|---------------|
| FLOAT32 | IEEE 754 single-precision | 4 |
| INT8 | Signed 8-bit integer | 1 |
| BINARY | Bit-packed | 1/8 |

**Binary bit order (NORMATIVE):** For BINARY subtype, dimension `i` maps to byte `i / 8`, bit position `i % 8` where bit 0 is the least significant bit. Bits beyond `dims` in the final byte MUST be zero.

### 2.5 Values

A value is a property instance on an object:

```
Value {
  property: ID
  value: <type-specific>
  language: ID?    // TEXT only: language entity reference
  unit: ID?        // INT64, FLOAT64, DECIMAL only: unit entity reference
}
```

The value encoding is determined by the data type declared for the property in the edit's properties dictionary (Section 4.3).

**Value refs:** Value slots can be assigned an ID to enable relations to reference them for provenance, confidence, attribution, or other qualifiers. Value refs are created via the CreateValueRef operation (Section 3.4). Once created, a value ref identifies the *slot*, not a specific historical value—it remains stable as the value changes over time.

**Referencing values:** To make statements about a value, first create a value ref, then create relations targeting it:

```
// Register Alice's birthdate as a referenceable value
CreateValueRef {
  id: <value_ref_id>
  entity: Alice
  property: birthDate
}

// "The source for Alice's birthdate is her passport"
CreateRelation {
  type: <hasSource>
  from: <passport_entity>
  to: <value_ref_id>
}
```

These operations can be in the same edit. Once registered, the value ref can be referenced by any number of relations.

**Referencing historical values:** To reference a value as it existed at a specific point in time, combine the value ref with a version pin on the relation:

```
// "The source for Alice's age AS OF edit X was this document"
CreateRelation {
  type: <hasSource>
  from: <source_document>
  to: <alice_age_value_ref>
  to_version: <edit_X>
}
```

Without a version pin, the relation refers to the current value. With a version pin, it refers to the historical value at that edit.

**Version pin semantics (NORMATIVE):** When `to_version` (or `from_version`) is an edit ID, the reference resolves to the value as of the **end** of that edit—after all ops in that edit have been applied. This provides a deterministic snapshot point.

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
  type: ID
  from: ID             // Source entity or value ref
  from_space: ID?      // Optional space pin for source
  from_version: ID?    // Optional version pin for source
  to: ID               // Target entity or value ref
  to_space: ID?        // Optional space pin for target
  to_version: ID?      // Optional version pin for target
  entity: ID           // Reified entity representing this relation
  position: string?
}
```

**Endpoint constraint (NORMATIVE):** Relation endpoints must be entities or value refs, not relations:

- When `from_is_value_ref = 0` (or `to_is_value_ref = 0`), the endpoint MUST reference an entity
- When `from_is_value_ref = 1` (or `to_is_value_ref = 1`), the endpoint is a value ref ID
- Endpoints referencing relations are invalid; to create a meta-edge, target the relation's reified entity via its `entity` ID
- If an endpoint references a relation (type mismatch), the relation is treated as having a dangling reference per the "dangling references allowed" policy

The `entity` field links to an entity that represents this relation as a node. This enables relations to be referenced by other relations (meta-edges) and to participate in the graph as first-class nodes. Values are stored on the reified entity, not on the relation itself.

**Reified entity creation (NORMATIVE):** CreateRelation implicitly creates the reified entity if it does not exist. No separate CreateEntity op is required. If an entity with the given ID already exists, it is reused—its existing values are preserved and it becomes associated with this relation.

**Multiple relations:** Multiple relations of the same type can exist between the same entities. Each relation has a caller-provided ID.

**Entity ID derivation (NORMATIVE):**

The `entity` field can be explicit (caller-provided) or auto-derived:

- **Auto-derived (default):** When `entity` is absent in CreateRelation, the entity ID is deterministically computed:
  ```
  entity_id = derived_uuid("grc20:relation-entity:" || relation_id)
  ```

- **Explicit:** When `entity` is provided, that ID is used directly. This enables multiple relations to share a single reified entity (hypergraph/bundle patterns). When multiple relations share an entity, values set on that entity are shared across all those relations.

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

**Mutability (NORMATIVE):** The structural fields (`entity`, `type`, `from`, `to`) are immutable after creation. To change endpoints, delete and recreate. The `position`, `from_space`, `from_version`, `to_space`, and `to_version` fields are mutable via UpdateRelation.

### 2.7 Per-Space State

**NORMATIVE:** Resolved state is scoped to a space:

```
state(space_id, object_id) → Object | DELETED | NOT_FOUND
```

Where Object is an Entity, Relation, or Value Ref. The same object ID can have different state in different spaces. Multi-space views are computed by resolver policy and MUST preserve provenance.

**Value Ref state:** For value refs, the state includes the slot binding (entity, property, language, space) determined by LWW resolution. Value refs are never DELETED (they are immutable once created).

**Object ID namespace (NORMATIVE):** Entity IDs, Relation IDs, and Value Ref IDs share a single namespace within each space. A given UUID identifies exactly one kind of object:

| Scenario | Resolution |
|----------|------------|
| CreateEntity where Relation with same ID exists | Ignored (ID already in use) |
| CreateEntity where Value Ref with same ID exists | Ignored (ID already in use) |
| CreateRelation where Entity with same ID exists | Ignored (ID already in use) |
| CreateRelation where Value Ref with same ID exists | Ignored (ID already in use) |
| CreateRelation with explicit `entity` that equals `relation.id` | Invalid; `entity` MUST differ from the relation ID |
| CreateValueRef where Entity with same ID exists | Ignored (ID already in use) |
| CreateValueRef where Relation with same ID exists | Ignored (ID already in use) |

The auto-derived entity ID (`derived_uuid("grc20:relation-entity:" || relation_id)`) is guaranteed to differ from the relation ID due to the prefix, so this constraint only applies to explicit `entity` values in many-mode.

**Rationale:** A single namespace simplifies the state model and prevents ambiguity in `state()` lookups. Reified entities are distinct objects that happen to represent relations as nodes. Value refs are objects that represent handles to value slots.

### 2.8 Schema Constraints

Schema constraints (required properties, cardinality, patterns, data type enforcement) are **not part of this specification**. They belong at the knowledge layer. The protocol stores values with their declared types but does not enforce that a property always uses the same type across edits.

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
    CreateValueRef   = 9
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

**Semantics (NORMATIVE):** If the entity does not exist, create it. If it already exists, this acts as an update: values are applied as `set` (LWW replace per property).

> **Note:** CreateEntity is effectively an "upsert" operation. This is intentional: it simplifies edit generation (no need to track whether an entity exists) and supports idempotent replay. However, callers should be aware that CreateEntity on an existing entity will **replace** values for any properties included in the op.

**UpdateEntity:**
```
UpdateEntity {
  id: ID
  set: List<Value>?            // LWW replace
  unset: List<UnsetValue>?
}

UnsetValue {
  property: ID
  language: ALL | ID?    // TEXT only: ALL = clear all, absent = English, ID = specific language
}
```

| Field | Strategy | Use Case |
|-------|----------|----------|
| `set` | LWW Replace | Name, Age |
| `unset` | Clear | Reset property or specific language |

**`set` semantics (NORMATIVE):** For a given property (and language, for TEXT), `set` replaces the existing value. For TEXT values, each language is treated independently—setting a value for one language does not affect values in other languages.

**`unset` semantics (NORMATIVE):** Clears values for properties. For TEXT properties, the `language` field specifies which slot to clear: `ALL` clears all language slots, absent clears the English slot, and a specific language ID clears that language slot. For non-TEXT properties, `language` MUST be `ALL` and the single value is cleared.

**Application order within op (NORMATIVE):**
1. `unset`
2. `set`

> **Serializer rule:** The same (property, language) MUST NOT appear in both `set` and `unset`. Serializers SHOULD squash by keeping only the `set` entry. See Section 3.6.

**DeleteEntity:**
```
DeleteEntity {
  id: ID
}
```

Transitions the entity to DELETED state (tombstoned).

**Tombstone semantics (NORMATIVE):**
- Once DELETED, subsequent UpdateEntity ops for this entity are ignored.
- Once DELETED, subsequent CreateEntity ops for this entity are ignored (tombstone absorbs upserts).
- The entity can only be restored via explicit RestoreEntity.
- Tombstones are deterministic: all indexers replaying the same log converge on the same DELETED state.

> **Serializer rule:** An edit MUST NOT contain DeleteEntity followed by CreateEntity for the same ID. Serializers SHOULD squash to an UpdateEntity or omit the delete. See Section 3.6.

**RestoreEntity:**
```
RestoreEntity {
  id: ID
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
  id: ID
  type: ID
  from: ID
  from_space: ID?          // Optional space pin for source
  from_version: ID?        // Optional version pin for source
  to: ID
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
- If `entity` is present: `entity_id = entity`. This enables multiple relations to share a single reified entity.

**UpdateRelation:**
```
UpdateRelation {
  id: ID
  from_space: ID?          // Set space pin for source
  from_version: ID?        // Set version pin for source
  to_space: ID?            // Set space pin for target
  to_version: ID?          // Set version pin for target
  position: string?        // Set position
  unset: Set<Field>?       // Fields to clear: from_space, from_version, to_space, to_version, position
}
```

Updates the relation's mutable fields. Use `unset` to clear a field (remove a pin or position). The structural fields (`entity`, `type`, `from`, `to`) are immutable after creation—to change endpoints, delete and recreate.

**Application order within op (NORMATIVE):**
1. `unset`
2. Set fields (`from_space`, `from_version`, `to_space`, `to_version`, `position`)

> **Serializer rule:** The same field MUST NOT appear in both set and unset. Serializers SHOULD squash by keeping only the set value. See Section 3.6.

**DeleteRelation:**
```
DeleteRelation {
  id: ID
}
```

Transitions the relation to DELETED state (tombstoned).

**Tombstone semantics (NORMATIVE):**
- Once DELETED, subsequent UpdateRelation ops for this relation are ignored.
- Once DELETED, subsequent CreateRelation ops with the same relation ID are ignored (tombstone absorbs).
- The relation can only be restored via explicit RestoreRelation.

> **Serializer rule:** An edit MUST NOT contain DeleteRelation followed by CreateRelation for the same ID. Serializers SHOULD squash by omitting the delete. See Section 3.6.

**RestoreRelation:**
```
RestoreRelation {
  id: ID
}
```

Transitions a DELETED relation back to ACTIVE state.

**Semantics (NORMATIVE):**
- If the relation is DELETED, restore it to ACTIVE.
- If the relation is ACTIVE or does not exist, the op is ignored (no-op).
- After restore, subsequent updates apply normally.

**Reified entity lifecycle (NORMATIVE):** Deleting a relation does NOT delete its reified entity. The entity remains accessible and may hold values, be referenced by other relations, or be explicitly deleted via DeleteEntity. Orphaned reified entities are permitted; applications MAY garbage-collect them at a higher layer.

### 3.4 Value Ref Operations

**CreateValueRef:**
```
CreateValueRef {
  id: ID
  entity: ID           // Entity holding the value
  property: ID         // Property of the value
  language: ID?        // Language (TEXT values only)
  space: ID?           // Space containing the value (default: current space)
}
```

Creates a referenceable ID for a value slot, enabling relations to target that value.

**Semantics (NORMATIVE):** A value slot is identified by (entity, property, language, space), where language is only present for TEXT properties. CreateValueRef proposes that a slot should have a given ID.

**Merge rules (NORMATIVE):** Value ref registration uses LWW semantics keyed by slot:

- The authoritative mapping is `slot → value_ref_id`, resolved by LWW keyed on slot
- When multiple CreateValueRef ops target the same slot, the op with the highest OpPosition wins
- The reverse mapping `value_ref_id → slot` is derived: for a given ID, find all slots whose resolved slot→id equals that ID
- If multiple slots resolve to the same ID (due to concurrent ops), relations targeting that ID resolve to the slot whose winning CreateValueRef had the highest OpPosition

Once a value ref wins LWW for a slot, it can be used as an endpoint in relations. The value ref identifies the slot, not a specific value—it remains stable as the value changes over time.

**Cross-space value refs:** The `space` field specifies which space contains the value. If omitted, defaults to the current space. This enables referencing values in other spaces for cross-space provenance.

**Resolution order (NORMATIVE):** When a relation targets a value ref:

1. Resolve the value ref's slot binding (entity, property, language, space) via LWW in the **relation's space**
2. The slot's `space` field determines where the underlying value is read from
3. The relation's `to_space` pin, if present, applies to the value ref object itself (which space's value ref binding to use), not where the underlying value lives
4. The relation's `to_version` pin, if present, specifies which historical value to reference within the resolved slot

**Language field (NORMATIVE):** The `language` field MUST only be present when the property's DataType (as declared in this edit's properties dictionary) is TEXT. For non-TEXT properties, `has_language` MUST be 0; violations are rejected (E005). This mirrors the language constraints on Value encoding.

**Immutability (NORMATIVE):** Value refs cannot be deleted or modified once created. A value ref's slot binding is determined by LWW at resolution time. There is no DeleteValueRef or UpdateValueRef operation.

**Indexer performance (RECOMMENDED):** To resolve value ref endpoints in O(1) time, indexers SHOULD maintain a reverse index:

```
value_ref_id → (winning_slot, winning_oppos)
```

This index is updated incrementally during replay. Without it, resolution requires scanning all slots that map to a given ID to find the highest OpPosition winner.

### 3.5 State Resolution

Operations are validated **structurally** at write time and **semantically** at read time.

**Write-time:** Validate structure, append to log. No state lookups required.

**Read-time:** Replay operations in log order, apply resolution rules, return computed state.

**Resolution rules:**

1. Replay ops in log order (Section 4.2)
2. Apply merge rules (Section 4.2.1)
3. Tombstone dominance: updates after delete are ignored
4. Return resolved state or DELETED status

### 3.6 Serializer Requirements

Indexers are lenient and will process edits even if they contain redundant or contradictory operations. However, spec-compliant clients SHOULD NOT produce such edits. Serializers SHOULD automatically rewrite operations to ensure clean output.

**Redundant value operations:** An UpdateEntity op MUST NOT include the same (property, language) in both `set` and `unset`. Serializers SHOULD squash by keeping only the `set` entry (since unset is applied first, the set would overwrite anyway).

**Redundant relation field operations:** An UpdateRelation op MUST NOT include the same field in both set and `unset`. Serializers SHOULD squash by keeping only the set value.

**Delete-then-create in same edit:** An edit MUST NOT contain a DeleteEntity followed by a CreateEntity for the same entity ID. Serializers SHOULD squash to a single UpdateEntity that clears and replaces values, or omit the delete if the intent is to overwrite.

**Delete-then-create relations:** An edit MUST NOT contain a DeleteRelation followed by a CreateRelation for the same relation ID. Serializers SHOULD squash by omitting the delete if the relation is being recreated, or by keeping only the delete if appropriate.

**Rationale:** These constraints simplify reasoning about edit semantics and prevent accidental patterns that may indicate client bugs. Indexers remain lenient to handle legacy or non-compliant clients gracefully.

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
  properties: List<(ID, DataType)>  // Per-edit type declarations
  relation_type_ids: List<ID>
  language_ids: List<ID>    // Language entities for localized TEXT values
  unit_ids: List<ID>        // Unit entities for numerical values
  object_ids: List<ID>
  context_ids: List<ID>     // IDs used in contexts (root_ids and edge to_entity_ids)
  contexts: List<Context>   // Context metadata for grouping (Section 4.5)
  ops: List<Op>
}
```

Edits are standalone patches. They contain no parent references—ordering is provided by on-chain governance.

**Properties dictionary:** The `properties` list declares the data type for each property used in this edit. All values for a given property within the edit use this type. Different edits MAY declare different types for the same property ID—there is no global type enforcement.

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

**`set` (LWW):** Replaces the value for a property (and language, for TEXT). When concurrent edits both use `set` on the same (property, language) combination, the op with the highest OpPosition wins.

**Property value conflicts:**

| Scenario | Resolution |
|----------|------------|
| Concurrent `set` | Higher OpPosition wins (LWW) |
| Delete vs Update | Delete wins (tombstone dominance) |

**Structural conflicts:**

| Conflict | Resolution |
|----------|------------|
| Create same entity ID | First creates; later creates apply values as `set` (LWW) |
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

The property dictionary includes both ID and DataType. This allows values to omit type tags and enables type-specific encoding.

**Property dictionary requirement (NORMATIVE):** All properties referenced in an edit MUST be declared in the properties dictionary with a data type. All values for a given property within the edit use the declared type. External property references are not allowed.

**Per-edit typing:** The data type in the properties dictionary applies only to this edit. Different edits MAY declare different types for the same property ID. Indexers store values with their declared types and support querying by type.

**Relation type dictionary requirement (NORMATIVE):** All relation types referenced in an edit MUST be declared in the `relation_type_ids` dictionary.

**Language dictionary requirement (NORMATIVE):** All languages referenced in TEXT values MUST be declared in the `language_ids` dictionary. Language index 0 means English (no entry required); indices 1+ reference `language_ids[index-1]`. Only TEXT values have the language field.

**Unit dictionary requirement (NORMATIVE):** All units referenced in numerical values (INT64, FLOAT64, DECIMAL) MUST be declared in the `unit_ids` dictionary. Unit index 0 means no unit; indices 1+ reference `unit_ids[index-1]`. Only numerical values have the unit field.

**Object dictionary requirement (NORMATIVE):** All entities and relations referenced in an edit MUST be declared in the `object_ids` dictionary. This includes: operation targets (UpdateEntity, DeleteEntity, etc.) and relation endpoints when targeting entities. CreateRelation encodes the relation ID inline, so it does not require a dictionary entry unless referenced by other ops in the same edit.

**Context ID dictionary requirement (NORMATIVE):** All entity IDs used in context metadata (root_id and edge to_entity_id fields) MUST be declared in the `context_ids` dictionary. Context indices reference this dictionary, not the objects dictionary, to keep context metadata separate from operation object references.

**Value ref endpoints:** Value ref IDs are NOT included in `object_ids`. When a relation endpoint targets a value ref, the ID is encoded inline (see Section 6.4) rather than as an ObjectRef. This avoids bloating the object dictionary with value ref IDs in provenance-heavy edits.

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

3. **Sorted value lists:** `CreateEntity.values` and `UpdateEntity.set` MUST be sorted by `(propertyRef, languageRef)` in ascending order (property index first, then language index). Duplicate `(property, language)` entries are NOT permitted.

4. **Sorted unset lists:** `UpdateEntity.unset` MUST be sorted by `(propertyRef, language)` in ascending order. Duplicate entries (same property and language) are NOT permitted.

5. **Minimal varints:** (Note: This is now a general requirement per Section 6.1, not canonical-only.)

6. **Consistent field encoding:** Optional fields use presence flags as specified in Section 6. No additional padding or alignment bytes.

7. **No duplicate dictionary entries:** Each dictionary MUST NOT contain duplicate IDs. Edits with duplicate IDs in any dictionary MUST be rejected.

**Performance note:** Canonical encoding requires sorting dictionaries and authors after collection, which is substantially slower than fast mode. Implementations SHOULD offer both modes.

### 4.5 Edit Contexts

Edits can include context metadata to support context-aware change grouping (e.g., grouping block changes under their parent entity).

**Context:**
```
Context {
  root_id: ID                // Root entity for this context
  edges: List<ContextEdge>   // Path from root to the changed entity
}
```

**ContextEdge:**
```
ContextEdge {
  type_id: ID         // Relation type ID (e.g., BLOCKS_ID)
  to_entity_id: ID    // Target entity ID at this edge
}
```

The `edges` list represents the path from `root_id` to the entity being modified. For example, if entity "TextBlock_9" is a block of entity "Byron", the context would be:
- `root_id`: Byron
- `edges`: `[{ type_id: BLOCKS_ID, to_entity_id: TextBlock_9 }]`

**Per-op context reference:** Each op can optionally reference a context by index:
```
context_ref: varint?   // Index into the edit's contexts array
```

If `context_ref` is omitted, the op has no explicit context. Multiple ops can share a single context entry via `context_ref`.

**Rationale:**
- Avoids repeating full context paths on every op
- Allows a single edit to span many contexts
- Enables UI grouping without changing the diff API surface

### 4.6 Edit Publishing

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

**Space pins:** The `from_space` and `to_space` fields pin relation endpoints to a specific space. This enables precise cross-space references where the relation refers to the entity as it exists in that specific space, rather than relying on resolution heuristics. Space pins can be updated via UpdateRelation.

**Version pins:** The `from_version` and `to_version` fields pin relation endpoints to a specific version (edit ID). This enables immutable citations where the relation always refers to the entity as it existed at that specific edit, rather than the current resolved state. Version pins can be updated via UpdateRelation.

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

**UUID:** Raw 16 bytes (no length prefix), big-endian (network byte order). Byte `i` corresponds to hex digits `2i` and `2i+1` of the standard 32-character hex string. For example, UUID `550e8400-e29b-41d4-a716-446655440000` is encoded as bytes `[0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, ...]`.

**Float endianness (NORMATIVE):** All IEEE 754 floats (FLOAT64, POINT, EMBEDDING float32) are little-endian.

### 6.2 Common Reference Types

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
context_id_count: varint
context_ids: ID[]                // IDs used in contexts (root_ids and edge to_entity_ids)

-- Contexts
context_count: varint
contexts: Context[]              // Context metadata for grouping

-- Operations
op_count: varint
ops: Op[]
```

**Version rejection (NORMATIVE):** Decoders MUST reject edits with unknown Version values.

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

### 6.4 Op Encoding

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
[if has_entity]: entity: ID    // Explicit reified entity
[if has_position]: position: String
context_ref: varint            // 0xFFFFFFFF = no context, else index into contexts[]
```

**Endpoint encoding:** Entity and relation endpoints use ObjectRef (dictionary index). Value ref endpoints use inline ID (16 bytes) to avoid bloating the object dictionary. The `from_is_value_ref` and `to_is_value_ref` flags indicate which encoding is used.

**Entity derivation:** When `has_entity = 0`, the entity ID is computed as `derived_uuid("grc20:relation-entity:" || relation_id)`.

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

### 6.5 Value Encoding

```
Value:
  property: PropertyRef
  payload: <type-specific>
  [if DataType == TEXT]: language: LanguageRef
  [if DataType in (INT64, FLOAT64, DECIMAL)]: unit: UnitRef
```

The payload type is determined by the property's DataType (from the properties dictionary).

**Language (TEXT only):** The `language` field is only present for TEXT values. A value with `language = 0` is English. Values with different languages for the same property are distinct and can coexist.

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
Date: days: int32 (LE), offset_min: int16 (LE) — 6 bytes total
Time: time_us: int48 (LE), offset_min: int16 (LE) — 8 bytes total
Datetime: epoch_us: int64 (LE), offset_min: int16 (LE) — 10 bytes total
Schedule: len: varint, data: UTF-8 bytes (RFC 5545)
Point: ordinate_count: uint8 (2 or 3), latitude: Float64, longitude: Float64, [altitude: Float64]
Rect: min_lat: Float64, min_lon: Float64, max_lat: Float64, max_lon: Float64 — 32 bytes total
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

| Name | UUID | Expected Type | Description |
|------|------|---------------|-------------|
| Name | `a126ca530c8e48d5b88882c734c38935` | TEXT | Primary label |
| Description | `9b1f76ff9711404c861e59dc3fa7d037` | TEXT | Summary text |
| Cover | `34f535072e6b42c5a84443981a77cfa2` | TEXT | Cover image URL |

The "Expected Type" column indicates the advisory data type for each property. These properties SHOULD have a `Data Type` relation (Section 7.3) pointing to the corresponding data type entity (Section 7.5).

### 7.2 Core Type

| Name | UUID | Description |
|------|------|-------------|
| Image | `f3f790c4c74e4d23a0a91e8ef84e30d9` | Image entity |

### 7.3 Core Relation Types

| Name | UUID | Description |
|------|------|-------------|
| Types | `8f151ba4de204e3c9cb499ddf96f48f1` | Type membership |
| Data Type | `84ce4adf1e9c4f52b9bdd6eeaa3004d8` | Property's expected data type |

The `Data Type` relation connects a property entity to a data type entity (Section 7.5). This is advisory—applications use it for UX and query defaults, but the protocol does not enforce type consistency.

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

### 7.5 Data Type Entities

Data type entities represent the protocol's data types in the knowledge layer. Property entities use `Data Type` relations (Section 7.3) to indicate their expected type. IDs are derived:
```
id = derived_uuid("grc20:genesis:datatype:" + type_name)
```

| Name | Type Name | Derivation |
|------|-----------|------------|
| Bool | bool | `derived_uuid("grc20:genesis:datatype:bool")` |
| Int64 | int64 | `derived_uuid("grc20:genesis:datatype:int64")` |
| Float64 | float64 | `derived_uuid("grc20:genesis:datatype:float64")` |
| Decimal | decimal | `derived_uuid("grc20:genesis:datatype:decimal")` |
| Text | text | `derived_uuid("grc20:genesis:datatype:text")` |
| Bytes | bytes | `derived_uuid("grc20:genesis:datatype:bytes")` |
| Date | date | `derived_uuid("grc20:genesis:datatype:date")` |
| Time | time | `derived_uuid("grc20:genesis:datatype:time")` |
| Datetime | datetime | `derived_uuid("grc20:genesis:datatype:datetime")` |
| Schedule | schedule | `derived_uuid("grc20:genesis:datatype:schedule")` |
| Point | point | `derived_uuid("grc20:genesis:datatype:point")` |
| Rect | rect | `derived_uuid("grc20:genesis:datatype:rect")` |
| Embedding | embedding | `derived_uuid("grc20:genesis:datatype:embedding")` |

**Usage:** To indicate that property X expects INT64 values, create a `Data Type` relation from X to the Int64 entity. Applications query this relation to determine the expected type for UX rendering and query construction.

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
| BOOL values | Not 0x00 or 0x01 |
| POINT bounds | Latitude outside [-90, +90] or longitude outside [-180, +180] |
| POINT ordinate count | ordinate_count not 2 or 3 |
| RECT bounds | Latitude outside [-90, +90] or longitude outside [-180, +180] |
| DATE offset_min | Outside range [-1440, +1440] |
| TIME time_us | Outside range [0, 86399999999] |
| TIME offset_min | Outside range [-1440, +1440] |
| DATETIME offset_min | Outside range [-1440, +1440] |
| Position strings | Empty, characters outside `0-9A-Za-z`, or length > 64 |
| EMBEDDING dims | Data length doesn't match dims × bytes-per-element for subtype |
| Zstd decompression | Decompressed size doesn't match declared `uncompressed_size` |
| Float values | NaN payload (see float rules in Section 2.5) |
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
| CreateEntity | Value Ref with same ID exists | Ignored (namespace collision) |
| CreateRelation | Entity with same ID exists | Ignored (namespace collision) |
| CreateRelation | Value Ref with same ID exists | Ignored (namespace collision) |
| CreateValueRef | Entity with same ID exists | Ignored (namespace collision) |
| CreateValueRef | Relation with same ID exists | Ignored (namespace collision) |
| CreateValueRef | Same slot, different IDs | LWW by OpPosition (slot → id mapping) |
| CreateValueRef | Same ID, different slots | All registrations proceed; id → slot is derived (see Section 3.4) |
| CreateRelation | `from_is_value_ref = 0` but `from` resolves to a relation | Treated as dangling reference (endpoint type mismatch) |
| CreateRelation | `to_is_value_ref = 0` but `to` resolves to a relation | Treated as dangling reference (endpoint type mismatch) |

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
