---
GRC: 20
Title: Knowledge Graph
Authors: Yaniv Tal, Byron Guina, Preston Mantel, Nik Graf
Created: 2026-02-12
Stage: Final
---

## 1. Introduction

GRC-20 is a binary property graph format for decentralized knowledge networks. It defines how to represent, modify, and synchronize graph data across trust boundaries in a distributed setting.

### 1.1 Design Principles

- **Property graph model** - Entities connected by relations; relations are first-class and can hold attributes
- **Event-sourced** - All state changes expressed as operations; history is append-only
- **Total ordering** - On-chain publishing provides provenence and global consensus over state
- **Pluralistic** - Multiple spaces can hold conflicting views; consumers choose trust
- **Binary encoding** - Optimized for compressed wire size and decode speed

### 1.2 Terminology

| Term | Definition |
|------|------------|
| Entity | A node in the graph, identified by ID |
| Relation | A directed edge between entities, identified by ID |
| Object | An Entity, Relation, or Value Ref (used when referencing all three) |
| Property | An entity representing a named attribute |
| Value | A property instance on an object |
| Value Ref | A referenceable handle for a value slot, identified by ID |
| Type | A classification label for entities |
| Op | An atomic operation that modifies graph state |
| Edit | A batch of ops with metadata |
| Space | A governance container for edits |

### 1.3 How It Fits Together

Let's say we want to add Albert Einstein to a Science space, let's walk through the steps.

**Entities, properties and values.** We start by creating an entity with a unique UUID - Einstein's identity in the graph. We attach values: a Name ("Albert Einstein") and a Description ("Theoretical physicist, Nobel laureate"), each referencing a property ID with a TEXT data type.

**Type membership.** To indicate that Einstein is a Person, we create a `Types` relation - a directed edge from the Einstein entity to an entity representing the Person type.

**Relations.** A "Discovered" relation can be created from the "Einstein" entity to a "Relativity" topic entity to map that connection. Properties can be included on the relation to describe details about the relationship.

**Ops.** Each mutation - creating, updating, or deleting an entity or relation - is expressed as an atomic operation. Ops are replayed sequentially to reconstruct state.

**Edits.** These operations are bundled into an edit - a batch of ops with author IDs, a timestamp, and dictionaries for efficient encoding. An edit is a self-contained patch with no parent references; ordering is determined by on-chain governance.

**Publishing.** The edit is serialized to a compact binary format (see `encoding.md`), published to content-addressed storage (IPFS), and its hash is recorded on-chain. Indexers replay accepted edits in governance-defined order, building the resolved state of the space deterministically.

The sections that follow define each of these concepts precisely for implementers.

---

## 2. Data Model

### 2.1 Identifiers

All identifiers are RFC 4122 UUIDs.

```
ID := UUID (16 bytes)
```

**Display format:** Non-hyphenated lowercase hex is RECOMMENDED. Implementations MAY accept hyphenated or Base58 on input.

### 2.2 Entities

Entities are the fundamental nodes of the knowledge graph - the people, places, ideas, and things you want to describe. Every piece of structured knowledge starts with an entity.

```
Entity {
  id: ID
  values: List<Value>
}
```

Values are unique per (entityId, propertyId), or per (entityId, propertyId, language) for TEXT values. When multiple values for a given (entity, property) pair are required, use relations instead.

Type membership is expressed via `Types` relations (Section 7.1), not a dedicated types field.

### 2.3 Types

Types classify entities - they're how you say "this entity is a Person" or "this entity is a Company." They answer the question: *what kind of thing is this?*

Types are entities that classify other entities via `Types` relations. An entity can have multiple types simultaneously. Since Types are just entities, they're created using CreateEntity with metadata added as values in the knowledge layer.

Types are labels, not classes. There's no inheritance and no enforcement at the protocol layer. Cardinality, inference and inheritance rules, and property constraints can be expressed in the knowledge layer.

### 2.4 Properties

Properties define the attributes you can attach to entities - Name, Description, Date of Birth, Population. They're themselves entities in the graph, so their names, descriptions, and data types are defined using values and relations in the knowledge layer.

```
DataType := BOOLEAN | INTEGER | FLOAT | DECIMAL | TEXT | BYTES
          | DATE | TIME | DATETIME | SCHEDULE | POINT | RECT | EMBEDDING
```

**Data types in edits:** Each edit declares the data type for each property it uses (Section 4.3). This allows values to omit type tags and enables type-specific encoding.

> **NORMATIVE:** All values for a given property within an edit MUST use the same data type. Different edits MAY use different data types for the same property - the data type is per-value metadata, not a global constraint.

**Data type hints:** Property entities SHOULD have a `Data type` relation (Section 7.3) pointing to a data type entity (Section 7.6) to indicate the expected type. This is advisory - applications use it for UX and query defaults, but the protocol does not enforce it.

**Data type enum values:**

| Type | Value | Description |
|------|-------|-------------|
| BOOLEAN | 1 | Boolean |
| INTEGER | 2 | 64-bit signed integer |
| FLOAT | 3 | 64-bit IEEE 754 float |
| DECIMAL | 4 | Arbitrary-precision decimal |
| TEXT | 5 | UTF-8 string |
| BYTES | 6 | Opaque byte array |
| DATE | 7 | Calendar date with timezone |
| TIME | 8 | Time of day with timezone |
| DATETIME | 9 | Timestamp with timezone |
| SCHEDULE | 10 | RFC 5545/7953 schedule or availability |
| POINT | 11 | WGS84 coordinate |
| RECT | 12 | Axis-aligned bounding box |
| EMBEDDING | 13 | Dense vector |

**Data type semantics:**

| Type | Encoding | Description |
|------|----------|-------------|
| BOOLEAN | 1 byte | 0x00 = false, 0x01 = true; other values invalid |
| INTEGER | Signed varint | -2^63 to 2^63-1 |
| FLOAT | IEEE 754 double, little-endian | 64-bit floating point |
| DECIMAL | exponent + mantissa | value = mantissa × 10^exponent |
| TEXT | UTF-8 string | Length-prefixed |
| BYTES | Raw bytes | Length-prefixed, opaque |
| DATE | 6 bytes | days_since_epoch (int32) + offset_min (int16) |
| TIME | 8 bytes | time_micros (int48) + offset_min (int16) |
| DATETIME | 10 bytes | epoch_micros (int64) + offset_min (int16) |
| SCHEDULE | UTF-8 string | RFC 5545/7953 iCalendar content |
| POINT | 2-3 FLOAT, little-endian | [lat, lon] or [lat, lon, alt] WGS84 |
| RECT | 4 FLOAT, little-endian | [min_lat, min_lon, max_lat, max_lon] WGS84 |
| EMBEDDING | sub_type + dims + bytes | Dense vector for similarity search |

#### BOOLEAN

A single byte: `0x00` for false, `0x01` for true.

**Example:**
- `true` → `0x01`
- `false` → `0x00`

> **NORMATIVE:** Any byte value other than `0x00` or `0x01` is invalid and MUST be rejected (E005).

#### INTEGER

A 64-bit signed integer encoded as a signed varint (ZigZag + LEB128).

**Examples:**
- `0` → `0x00`
- `1` → `0x02`
- `-1` → `0x01`
- `300` → `0xD8 0x04`
- `-2^63` to `2^63-1` range

#### FLOAT

A 64-bit IEEE 754 double-precision float, stored in little-endian byte order.

**Examples:**
- `1.0` → `0x00 0x00 0x00 0x00 0x00 0x00 0xF0 0x3F`
- `3.14159` → `0x6E 0x86 0x1B 0xF0 0xF9 0x21 0x09 0x40`
- `-0.5` → `0x00 0x00 0x00 0x00 0x00 0x00 0xE0 0xBF`

> **NORMATIVE:** NaN is prohibited. Encoders MUST NOT emit NaN values; decoders MUST reject them (E005). ±Infinity are permitted.

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

> **NORMATIVE:** DECIMAL values MUST be encoded in normalized form: mantissa has no trailing zeros, and zero is represented as `{ exponent: 0, mantissa: 0 }`. This ensures deterministic encoding for content addressing. Decoders and indexers SHOULD accept legacy non-normalized DECIMAL payloads and normalize them after decoding so already-published data remains interoperable.

Applications needing to preserve display precision (e.g., "12.30" vs "12.3") can use the property's format to indicate the intended presentation. The format is specified on the property entity in the knowledge graph. The normalized DECIMAL ensures deterministic encoding; the property's format controls how all its values render.

#### TEXT

A UTF-8 string, length-prefixed with a varint.

**Example:**
- `"hello"` → varint `5` followed by bytes `0x68 0x65 0x6C 0x6C 0x6F`
- `"日本語"` → varint `9` followed by 9 UTF-8 bytes

#### BYTES

An opaque byte array, length-prefixed with a varint.

**Example:**
- 3 bytes `[0xDE, 0xAD, 0xFF]` → varint `3` followed by `0xDE 0xAD 0xFF`

#### DATE

Calendar date represented as a fixed 6-byte binary value.

```
DATE {
  days: int32       // Signed days since Unix epoch (1970-01-01)
  offset_min: int16 // Signed UTC offset in minutes (e.g., +330 for +05:30)
}
```

**Wire format:** 6 bytes, fixed-width:
- Bytes 0–3: `days` (signed 32-bit integer, little-endian)
- Bytes 4–5: `offset_min` (signed 16-bit integer, little-endian)

The `days` field represents the calendar date as days since 1970-01-01. The `offset_min` indicates the timezone context where the date is meaningful.

**Examples:**
- March 15, 2024 UTC: `days = 19797`, `offset_min = 0` → bytes `0x55 0x4D 0x00 0x00 0x00 0x00`
- March 15, 2024 in +05:30: `days = 19797`, `offset_min = 330` → bytes `0x55 0x4D 0x00 0x00 0x4A 0x01`
- January 1, 1970 UTC: `days = 0`, `offset_min = 0` → bytes `0x00 0x00 0x00 0x00 0x00 0x00`

**Range:** int32 days provides a range of ±5.8 million years from 1970.

> **NORMATIVE:** Implementations MUST reject `offset_min` outside range [-1440, +1440] (±24 hours).

> **NORMATIVE:** DATE values sort by `days` first, then by `offset_min` (both as signed integers).

#### TIME

Time of day represented as a fixed 8-byte binary value.

```
TIME {
  time_micros: int48   // Microseconds since midnight (0 to 86,399,999,999)
  offset_min: int16    // Signed UTC offset in minutes (e.g., +330 for +05:30)
}
```

**Wire format:** 8 bytes, fixed-width:
- Bytes 0–5: `time_micros` (signed 48-bit integer, little-endian)
- Bytes 6–7: `offset_min` (signed 16-bit integer, little-endian)

The `time_micros` field represents microseconds since midnight in the local timezone indicated by `offset_min`.

**Examples:**
- 14:30:00 UTC: `time_micros = 52200000000`, `offset_min = 0` → bytes `0x00 0xE8 0x76 0x28 0x0C 0x00 0x00 0x00`
- 14:30:00.500 in +05:30: `time_micros = 52200500000`, `offset_min = 330` → bytes `0x20 0xA1 0x7A 0x28 0x0C 0x00 0x4A 0x01`
- Midnight UTC: `time_micros = 0`, `offset_min = 0` → bytes `0x00 0x00 0x00 0x00 0x00 0x00 0x00 0x00`

**Range:** int48 microseconds easily covers a full day (max 86,399,999,999 µs).

> **NORMATIVE:** Implementations MUST reject:
> - `time_micros` outside range [0, 86,399,999,999]
> - `offset_min` outside range [-1440, +1440] (±24 hours)

> **NORMATIVE:** TIME values sort by their UTC-normalized instant (`time_micros - offset_min * 60_000_000`). Tie-break by `offset_min`.

#### DATETIME

Combined date and time represented as a fixed 10-byte binary value.

```
DATETIME {
  epoch_micros: int64   // Microseconds since Unix epoch (1970-01-01T00:00:00Z)
  offset_min: int16     // Signed UTC offset in minutes (e.g., +330 for +05:30)
}
```

**Wire format:** 10 bytes, fixed-width:
- Bytes 0–7: `epoch_micros` (signed 64-bit integer, little-endian)
- Bytes 8–9: `offset_min` (signed 16-bit integer, little-endian)

The `epoch_micros` field represents the instant in UTC. The `offset_min` preserves the original timezone context for display purposes.

**Examples:**
- 2024-03-15T14:30:00Z: `epoch_micros = 1710513000000000`, `offset_min = 0` → bytes `0x00 0xC0 0xE9 0x3D 0x4A 0x17 0x06 0x00 0x00 0x00`
- 2024-03-15T14:30:00+05:30: `epoch_micros = 1710493200000000`, `offset_min = 330` → bytes `0x00 0xC0 0xE0 0xFB 0x2F 0x17 0x06 0x00 0x4A 0x01`

**Why microseconds:** int64 microseconds provides a range of ±292,000 years with sub-millisecond precision. int64 nanoseconds would overflow around year 2262.

> **NORMATIVE:** Implementations MUST reject `offset_min` outside range [-1440, +1440] (±24 hours).

> **NORMATIVE:** DATETIME values sort by `epoch_micros` first, then by `offset_min` (both as signed integers).

#### SCHEDULE

iCalendar component for recurring events and availability, supporting RFC 5545 (iCalendar) and RFC 7953 (Calendar Availability).

**Examples:**
```
"DTSTART:20240315T090000Z\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR"    // Weekly on Mon/Wed/Fri
"DTSTART:20240101\nRRULE:FREQ=YEARLY"                           // Annual event
"FREEBUSY:20240315T090000Z/20240315T170000Z"                    // Free/busy period
"BEGIN:VAVAILABILITY\nDTSTART:20240101T090000\nDTEND:20240101T170000\nRRULE:FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR\nEND:VAVAILABILITY"  // Business hours
```

> **NORMATIVE:** SCHEDULE values contain iCalendar content as defined in RFC 5545 and RFC 7953. Valid forms include:
>
> - **Properties:** One or more iCalendar properties (e.g., `DTSTART` with `RRULE`, `RDATE`, `EXDATE`)
> - **VAVAILABILITY component:** RFC 7953 availability blocks for expressing recurring availability patterns
> - **FREEBUSY periods:** Time ranges indicating free/busy status

> **NORMATIVE:** Content lines MAY use RFC 5545 line folding (CRLF followed by a space or tab). Implementations MUST unfold before parsing.

> **NORMATIVE:** Implementations MUST validate that the value parses as valid iCalendar content per RFC 5545 and RFC 7953. Invalid property names, malformed date-times, or syntax errors MUST be rejected (E005).

#### POINT

WGS84 geographic coordinate with 2 or 3 ordinates.

```
POINT {
  latitude: float64   // -90 to +90 (required)
  longitude: float64  // -180 to +180 (required)
  altitude: float64?  // meters above WGS84 ellipsoid (optional)
}
```

**Examples:**
- San Francisco: `latitude = 37.7749`, `longitude = -122.4194` → 16 bytes (two IEEE 754 doubles, little-endian)
- Mount Everest with altitude: `latitude = 27.9881`, `longitude = 86.9250`, `altitude = 8848.86` → 24 bytes (three doubles)

> **NORMATIVE:** Coordinate order is `[latitude, longitude]` or `[latitude, longitude, altitude]`.

> **NORMATIVE:** Latitude MUST be in range [-90, +90]. Longitude MUST be in range [-180, +180]. Values outside these ranges MUST be rejected (E005). Altitude has no bounds restrictions.

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

**Examples:**
- Continental US: `min_lat = 24.5`, `min_lon = -125.0`, `max_lat = 49.4`, `max_lon = -66.9` → 32 bytes (four doubles, little-endian)
- Prime meridian crossing: `min_lat = 35.0`, `min_lon = -10.0`, `max_lat = 55.0`, `max_lon = 10.0`

> **NORMATIVE:** Wire format is 32 bytes, fixed-width:
> - Bytes 0–7: `min_lat` (IEEE 754 double, little-endian)
> - Bytes 8–15: `min_lon` (IEEE 754 double, little-endian)
> - Bytes 16–23: `max_lat` (IEEE 754 double, little-endian)
> - Bytes 24–31: `max_lon` (IEEE 754 double, little-endian)

> **NORMATIVE:** Coordinate order is `[min_lat, min_lon, max_lat, max_lon]` (southwest corner, then northeast corner).

> **NORMATIVE:** Implementations MUST reject:
> - `min_lat` or `max_lat` outside range [-90, +90]
> - `min_lon` or `max_lon` outside range [-180, +180]
> - NaN values in any coordinate

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

**Examples:**
- A 3-dimensional FLOAT32 embedding `[0.1, 0.5, -0.3]` → sub_type `0x00`, dims varint `3`, followed by 12 bytes (3 × 4-byte little-endian floats)
- A 768-dimensional INT8 embedding → sub_type `0x01`, dims varint `768`, followed by 768 bytes
- A 128-dimensional BINARY embedding → sub_type `0x02`, dims varint `128`, followed by 16 bytes (128 / 8)

> **NORMATIVE:** For BINARY subtype, dimension `i` maps to byte `i / 8`, bit position `i % 8` where bit 0 is the least significant bit. Bits beyond `dims` in the final byte MUST be zero.

### 2.5 Values

A value is a property instance attached to an object - it's how you say "this entity's Name is Albert Einstein" or "this entity's Population is 8,336,817." Values connect an entity to a piece of data through a property.

```
Value {
  property: ID
  value: <type-specific>
  language: ID?    // TEXT only: language entity reference
  unit: ID?        // INTEGER, FLOAT, DECIMAL only: unit entity reference
}
```

The value encoding is determined by the data type declared for the property in the edit's properties dictionary (Section 4.3).

**Value refs:** Value slots can be assigned an ID to enable relations to reference them for provenance, confidence, attribution, or other qualifiers. A value ref (a referenceable handle for a value slot, enabling relations to target values) is created via the CreateValueRef operation (Section 3.4). Once created, a value ref identifies the *slot*, not a specific historical value - it remains stable as the value changes over time.

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

> **NORMATIVE:** When `to_version` (or `from_version`) is an edit ID, the reference resolves to the value as of the **end** of that edit - after all ops in that edit have been applied. This provides a deterministic snapshot point.

**Value uniqueness:**

Values are unique per (entityId, propertyId), with TEXT values additionally differentiated by language. Setting a value replaces any existing value for that (property, language) combination. For ordered or multiple values, use relations with positions.

**Unit (numerical types only):** INTEGER, FLOAT, and DECIMAL values can optionally specify a unit (e.g., kg, USD). Unlike language, unit does NOT affect value uniqueness - setting "100 kg" then "200 lbs" on the same property results in "200 lbs" (the unit is metadata for interpretation).

> **NORMATIVE:** For FLOAT, POINT, and EMBEDDING (float32 subtype):
> - **NaN is prohibited.** Encoders MUST NOT emit NaN values; decoders MUST reject them (E005). Use a separate "unknown" or "missing" representation at the application layer.
> - **Infinity:** ±Infinity are permitted.

### 2.6 Relations

Relations connect entities to each other - a person to a company, a company to their products. They're directed edges, but they're also first-class nodes in the graph, which means you can attach metadata to them.

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
  entity: ID?          // Optional explicit relation entity; absent = auto-derived
  position: string?
}
```

**Endpoint constraint:** Relation endpoints must be entities or value refs, not relations. When you need a meta-edge (a relation about a relation), target the relation entity (the companion entity node) via its `entity` ID.

> **NORMATIVE:** Endpoint constraints:
> - When `from_is_value_ref = 0` (or `to_is_value_ref = 0`), the endpoint MUST reference an entity
> - When `from_is_value_ref = 1` (or `to_is_value_ref = 1`), the endpoint is a value ref ID
> - Endpoints referencing relations are invalid; to create a meta-edge, target the relation entity via its `entity` ID
> - If an endpoint references a relation (type mismatch), the relation is treated as having a dangling reference (a reference to an entity that doesn't exist yet or has been deleted) per the "dangling references allowed" policy

The `entity` field links to an entity that represents this relation as a node. This enables relations to be referenced by other relations (meta-edges) and to participate in the graph as first-class nodes. Values are stored on the relation entity, not on the relation itself.

> **NORMATIVE:** CreateRelation implicitly creates the relation entity if it does not exist. No separate CreateEntity op is required. If an entity with the given ID already exists, it is reused - its existing values are preserved and it becomes associated with this relation.

**Multiple relations:** Multiple relations of the same type can exist between the same entities. Each relation has a caller-provided ID.

**Relation entity ID derivation:** The `entity` field can be explicit (caller-provided) or auto-derived:

- **Auto-derived (default):** When `entity` is absent in CreateRelation, the relation entity ID is deterministically computed:
  ```
  entity_id = derived_uuid("grc20:relation-entity:" || relation_id)
  ```

- **Explicit:** When `entity` is provided, that ID is used directly. This enables multiple relations to share a single relation entity (hypergraph/bundle patterns). When multiple relations share an entity, values set on that entity are shared across all those relations.

> **NORMATIVE:** The auto-derived entity ID is computed as `derived_uuid("grc20:relation-entity:" || relation_id)`.

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

> **NORMATIVE:** Position strings MUST NOT exceed 64 characters. If a client cannot generate a midpoint without exceeding this limit (positions too close), it MUST perform explicit reordering by issuing `UpdateRelation` ops with new, evenly-spaced positions. This protocol does not support implicit rebalancing.

> **NORMATIVE:** Positions containing characters outside `0-9A-Za-z` or exceeding 64 characters MUST be rejected (E005). Empty position strings are NOT permitted.

> **NORMATIVE:** Ordering semantics:
> 1. Relations with a `position` sort before relations without a position.
> 2. Positions compare lexicographically using ASCII byte order over `0-9A-Za-z` (i.e., `0` < `9` < `A` < `Z` < `a` < `z`).
> 3. If positions are equal, tie-break by relation ID bytes (lexicographic unsigned comparison).
> 4. Relations without positions are ordered by relation ID bytes.

> **NORMATIVE:** The structural fields (`entity`, `type`, `from`, `to`) are immutable after creation. To change endpoints, delete and recreate. The `position`, `from_space`, `from_version`, `to_space`, and `to_version` fields are mutable via UpdateRelation.

### 2.7 Per-Space State

Each space maintains its own resolved view of the graph. The same entity can have different values - or exist in one space but not another. This is how GRC-20 supports pluralism: multiple spaces can hold conflicting views, and consumers choose which to trust.

> **NORMATIVE:** Resolved state is scoped to a space:
>
> ```
> state(space_id, object_id) → Object | DELETED | NOT_FOUND
> ```
>
> Where Object is an Entity, Relation, or Value Ref. The same object ID can have different state in different spaces. Multi-space views are computed by resolver policy and MUST preserve provenance.

**Value Ref state:** For value refs, the state includes the slot binding (entity, property, language, space) determined by LWW (Last-Writer-Wins - the operation with the highest position wins) resolution. Value refs are never DELETED (they are immutable once created).

**Object ID namespace:** Entity IDs, Relation IDs, and Value Ref IDs share a single namespace within each space. A given UUID identifies exactly one kind of object.

> **NORMATIVE:** Object ID namespace collisions are resolved as follows:
>
> | Scenario | Resolution |
> |----------|------------|
> | CreateEntity where Relation with same ID exists | Ignored (ID already in use) |
> | CreateEntity where Value Ref with same ID exists | Ignored (ID already in use) |
> | CreateRelation where Entity with same ID exists | Ignored (ID already in use) |
> | CreateRelation where Value Ref with same ID exists | Ignored (ID already in use) |
> | CreateRelation with explicit `entity` that equals `relation.id` | Invalid; `entity` MUST differ from the relation ID |
> | CreateValueRef where Entity with same ID exists | Ignored (ID already in use) |
> | CreateValueRef where Relation with same ID exists | Ignored (ID already in use) |

The auto-derived entity ID (`derived_uuid("grc20:relation-entity:" || relation_id)`) is guaranteed to differ from the relation ID due to the prefix, so this constraint only applies to explicit `entity` values in many-mode.

**Rationale:** A single namespace simplifies the state model and prevents ambiguity in `state()` lookups. Relation entities are distinct objects that happen to represent relations as nodes. Value refs are objects that represent handles to value slots.

### 2.8 Schema Constraints

Schema constraints (required properties, cardinality, patterns, data type enforcement) are **not part of this specification**. They belong at the knowledge layer. The protocol stores values with their declared types but does not enforce that a property always uses the same type across edits.

---

## 3. Operations

All state changes are expressed as operations (ops). Ops are the atomic building blocks - each one describes a single mutation to the graph. They're batched into edits for publishing, but the semantics are defined per-op.

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

### 3.2 Core Operations

**CreateEntity:**
```
CreateEntity {
  id: ID
  values: List<Value>
}
```

If the entity does not exist, create it. If it already exists, this acts as an update: values are applied as `set` (LWW replace per property). Tombstone semantics apply (Section 3.5).

> **NORMATIVE:** CreateEntity is effectively an "upsert" operation. This is intentional: it simplifies edit generation (no need to track whether an entity exists) and supports idempotent replay. However, callers should be aware that CreateEntity on an existing entity will **replace** values for any properties included in the op.

Serializer requirements apply (Section 3.7).

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

Tombstone semantics apply (Section 3.5).

> **NORMATIVE:** `set` semantics: For a given property (and language, for TEXT), `set` replaces the existing value. For TEXT values, each language is treated independently - setting a value for one language does not affect values in other languages.

> **NORMATIVE:** `unset` semantics: Clears values for properties. For TEXT properties, the `language` field specifies which slot to clear: `ALL` clears all language slots, absent clears the English slot, and a specific language ID clears that language slot. For non-TEXT properties, `language` MUST be `ALL` and the single value is cleared.

> **NORMATIVE:** Application order within op:
> 1. `unset`
> 2. `set`

Serializer requirements apply (Section 3.7): The same (property, language) MUST NOT appear in both `set` and `unset`.

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
  entity: ID?              // Explicit relation entity ID; absent = auto-derived
  position: string?
}
```

> **NORMATIVE:** If the relation does not exist, create it along with its relation entity (if that entity does not already exist). If the relation already exists with the same ID, the op is ignored (relations are immutable except for position). To add values to the relation, use UpdateEntity on the relation entity ID.

Tombstone semantics apply (Section 3.5).

**Entity ID resolution:**
- If `entity` is absent: `entity_id = derived_uuid("grc20:relation-entity:" || relation_id)`
- If `entity` is present: `entity_id = entity`. This enables multiple relations to share a single relation entity.

> **NORMATIVE:** Tombstone interactions for CreateRelation:
> - If the relation ID exists but is DELETED, CreateRelation is ignored (tombstone absorbs; use RestoreRelation to revive).
> - If the relation entity ID exists but is DELETED, the relation is still created, but the relation entity remains DELETED. Values cannot be added to the relation until the entity is restored via RestoreEntity. This is an edge case that occurs when an entity is explicitly deleted after being used as a relation entity, or when the same ID is reused.

### 3.3 Lifecycle Operations

*Delete and restore operations for managing entity and relation lifecycles.*

**DeleteEntity:**
```
DeleteEntity {
  id: ID
}
```

Transitions the entity to DELETED state (tombstoned - marked as deleted; ignores subsequent updates until explicitly restored). Tombstone semantics apply (Section 3.5).

Serializer requirements apply (Section 3.7).

**RestoreEntity:**
```
RestoreEntity {
  id: ID
}
```

Transitions a DELETED entity back to ACTIVE state. Property values are preserved (delete hides, restore reveals).

> **NORMATIVE:**
> - If the entity is DELETED, restore it to ACTIVE.
> - If the entity is ACTIVE or does not exist, the op is ignored (no-op).
> - After restore, subsequent updates apply normally.

**Design rationale:** Explicit restore prevents accidental resurrection by stale/offline clients while allowing governance-controlled undo. Random CreateEntity/UpdateEntity cannot bring back deleted entities - only intentional RestoreEntity can.

**DeleteRelation:**
```
DeleteRelation {
  id: ID
}
```

Transitions the relation to DELETED state. Tombstone semantics apply (Section 3.5).

Serializer requirements apply (Section 3.7).

> **NORMATIVE:** Deleting a relation does NOT delete its relation entity. The entity remains accessible and may hold values, be referenced by other relations, or be explicitly deleted via DeleteEntity. Orphaned relation entities are permitted; applications MAY garbage-collect them at a higher layer.

**RestoreRelation:**
```
RestoreRelation {
  id: ID
}
```

Transitions a DELETED relation back to ACTIVE state.

> **NORMATIVE:**
> - If the relation is DELETED, restore it to ACTIVE.
> - If the relation is ACTIVE or does not exist, the op is ignored (no-op).
> - After restore, subsequent updates apply normally.

### 3.4 Advanced Operations

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

Updates the relation's mutable fields. Use `unset` to clear a field (remove a pin or position). The structural fields (`entity`, `type`, `from`, `to`) are immutable after creation - to change endpoints, delete and recreate.

Tombstone semantics apply (Section 3.5).

> **NORMATIVE:** Application order within op:
> 1. `unset`
> 2. Set fields (`from_space`, `from_version`, `to_space`, `to_version`, `position`)

Serializer requirements apply (Section 3.7): The same field MUST NOT appear in both set and unset.

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

A value slot is identified by (entity, property, language, space), where language is only present for TEXT properties.

> **NORMATIVE:** Value ref registration uses LWW semantics keyed by slot:
>
> - The authoritative mapping is `slot → value_ref_id`, resolved by LWW keyed on slot
> - When multiple CreateValueRef ops target the same slot, the op with the highest OpPosition (the operation's position in the global log: block number, transaction index, log index, op index) wins
> - The reverse mapping `value_ref_id → slot` is derived: for a given ID, find all slots whose resolved slot→id equals that ID
> - If multiple slots resolve to the same ID (due to concurrent ops), relations targeting that ID resolve to the slot whose winning CreateValueRef had the highest OpPosition

Once a value ref wins LWW for a slot, it can be used as an endpoint in relations. The value ref identifies the slot, not a specific value - it remains stable as the value changes over time.

**Cross-space value refs:** The `space` field specifies which space contains the value. If omitted, defaults to the current space. This enables referencing values in other spaces for cross-space provenance.

> **NORMATIVE:** Resolution order when a relation targets a value ref:
>
> 1. Resolve the value ref's slot binding (entity, property, language, space) via LWW in the **relation's space**
> 2. The slot's `space` field determines where the underlying value is read from
> 3. The relation's `to_space` pin, if present, applies to the value ref object itself (which space's value ref binding to use), not where the underlying value lives
> 4. The relation's `to_version` pin, if present, specifies which historical value to reference within the resolved slot

> **NORMATIVE:** The `language` field MUST only be present when the property's DataType (as declared in this edit's properties dictionary) is TEXT. For non-TEXT properties, `has_language` MUST be 0; violations are rejected (E005). This mirrors the language constraints on Value encoding.

> **NORMATIVE:** Value refs cannot be deleted or modified once created. A value ref's slot binding is determined by LWW at resolution time. There is no DeleteValueRef or UpdateValueRef operation.

**Indexer performance (RECOMMENDED):** To resolve value ref endpoints in O(1) time, indexers SHOULD maintain a reverse index:

```
value_ref_id → (winning_slot, winning_oppos)
```

This index is updated incrementally during replay. Without it, resolution requires scanning all slots that map to a given ID to find the highest OpPosition winner.

### 3.5 Tombstone Semantics

Tombstones control the lifecycle of entities and relations. When an entity or relation is deleted, it enters a tombstoned state - it's marked as deleted and ignores subsequent updates until explicitly restored.

> **NORMATIVE:** The authoritative tombstone rules are:
>
> 1. **Delete transitions to DELETED state.** DeleteEntity and DeleteRelation set the object's state to DELETED.
> 2. **Updates are absorbed.** Once DELETED, subsequent UpdateEntity/UpdateRelation ops for that object are ignored.
> 3. **Creates are absorbed.** Once DELETED, subsequent CreateEntity ops for that entity are ignored (tombstone absorbs upserts). Subsequent CreateRelation ops for that relation are ignored.
> 4. **Only explicit restore revives.** A DELETED entity can only be restored via RestoreEntity; a DELETED relation only via RestoreRelation.
> 5. **Restore reveals preserved state.** Property values are preserved through delete/restore — delete hides, restore reveals. After restore, subsequent updates apply normally.
> 6. **Restore of non-deleted is a no-op.** If the object is ACTIVE or does not exist, RestoreEntity/RestoreRelation is ignored.
> 7. **Delete is idempotent.** Deleting an already-DELETED or NOT_FOUND object is ignored.
> 8. **Tombstones are deterministic.** All indexers replaying the same log converge on the same DELETED state.

### 3.6 State Resolution

Operations are validated **structurally** at write time and **semantically** at read time.

**Write-time:** Validate structure, append to log. No state lookups required.

**Read-time:** Replay operations in log order, apply resolution rules, return computed state.

**Resolution rules:**

1. Replay ops in log order (Section 4.2)
2. Apply merge rules (Section 4.2.1)
3. Tombstone dominance: updates after delete are ignored (Section 3.5)
4. Return resolved state or DELETED status

### 3.7 Serializer Requirements

Indexers are lenient and will process edits even if they contain redundant or contradictory operations. However, spec-compliant clients SHOULD NOT produce such edits. Serializers SHOULD automatically rewrite operations to ensure clean output.

> **NORMATIVE:** Redundant value operations: An UpdateEntity op MUST NOT include the same (property, language) in both `set` and `unset`. Serializers SHOULD squash by keeping only the `set` entry (since unset is applied first, the set would overwrite anyway).

> **NORMATIVE:** Redundant relation field operations: An UpdateRelation op MUST NOT include the same field in both set and `unset`. Serializers SHOULD squash by keeping only the set value.

> **NORMATIVE:** Delete-then-create in same edit: An edit MUST NOT contain a DeleteEntity followed by a CreateEntity for the same entity ID. Serializers SHOULD squash to a single UpdateEntity that clears and replaces values, or omit the delete if the intent is to overwrite.

> **NORMATIVE:** Delete-then-create relations: An edit MUST NOT contain a DeleteRelation followed by a CreateRelation for the same relation ID. Serializers SHOULD squash by omitting the delete if the relation is being recreated, or by keeping only the delete if appropriate.

**Rationale:** These constraints simplify reasoning about edit semantics and prevent accidental patterns that may indicate client bugs. Indexers remain lenient to handle legacy or non-compliant clients gracefully.

---

## 4. Edits

Edits are how changes get published. They batch operations together with metadata - author attribution, timestamps, and the dictionaries that make the binary format compact. An edit is a self-contained patch: it carries everything needed to apply its changes.

### 4.1 Edit Structure

```
Edit {
  id: ID
  name: string                     // May be empty
  authors: List<ID>                // The person, organization or agent Space IDs
  created_at: Timestamp
  properties: List<(ID, DataType)> // Per-edit type declarations
  relation_type_ids: List<ID>      // The _id fields are dictionaries for efficient encoding
  language_ids: List<ID>           // Language entities for localized TEXT values
  unit_ids: List<ID>               // Unit entities for numerical values
  object_ids: List<ID>
  context_ids: List<ID>            // IDs used in contexts (root_ids and edge to_entity_ids)
  contexts: List<Context>          // Context metadata for grouping (Section 4.4)
  ops: List<Op>
}
```

Edits are standalone patches. They contain no parent references - ordering is provided by on-chain governance.

**Properties dictionary:** The `properties` list declares the data type for each property used in this edit. All values for a given property within the edit use this type. Different edits MAY declare different types for the same property ID - there is no global type enforcement.

**`created_at`** is metadata for audit/display only. It is NOT used for conflict resolution.

**Encoding modes:** The binary format supports fast mode (default, optimized for encode speed) and canonical mode (deterministic bytes for signing and content deduplication). See [encoding.md](encoding.md) for details.

> **NORMATIVE:** CIDs and signatures MUST be computed over **uncompressed** canonical-mode bytes (the `GRC2` payload). Compression is a transport optimization and is not part of the signed/hashed content. This ensures:
> - Different zstd implementations/settings don't cause CID divergence
> - Signatures remain valid regardless of transport compression
> - Decompressed content can be verified against the original CID

### 4.2 Sequential Ordering

The state of a space is the result of replaying all accepted edits in the order defined by the governance log.

**Log position:** Onchain space contracts emit an event when the proposal is accepted.
```
LogPosition := (block_number, tx_index, log_index)
```

> **NORMATIVE:** Indexers MUST apply edits sequentially by LogPosition. The chain provides total ordering.

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
properties[1] = (ID of "age", INTEGER)
relation_type_ids[0] = <ID of "Types" relation type>
```

The property dictionary includes both ID and DataType. This allows values to omit type tags and enables type-specific encoding.

> **NORMATIVE:** All properties referenced in an edit MUST be declared in the properties dictionary with a data type. All values for a given property within the edit use the declared type. External property references are not allowed.

**Per-edit typing:** The data type in the properties dictionary applies only to this edit. Different edits MAY declare different types for the same property ID. Indexers store values with their declared types and support querying by type.

> **NORMATIVE:** All relation types referenced in an edit MUST be declared in the `relation_type_ids` dictionary.

> **NORMATIVE:** All languages referenced in TEXT values MUST be declared in the `language_ids` dictionary. Only TEXT values have the language field.

> **NORMATIVE:** All units referenced in numerical values (INTEGER, FLOAT, DECIMAL) MUST be declared in the `unit_ids` dictionary. Only numerical values have the unit field.

> **NORMATIVE:** All entities and relations referenced in an edit MUST be declared in the `object_ids` dictionary. This includes: operation targets (UpdateEntity, DeleteEntity, etc.) and relation endpoints when targeting entities. CreateRelation encodes the relation ID inline, so it does not require a dictionary entry unless referenced by other ops in the same edit.

> **NORMATIVE:** All entity IDs used in context metadata (root_id and edge to_entity_id fields) MUST be declared in the `context_ids` dictionary. Context indices reference this dictionary, not the objects dictionary, to keep context metadata separate from operation object references.

**Value ref endpoints:** Value ref IDs are NOT included in `object_ids`. When a relation endpoint targets a value ref, the ID is encoded inline (see [encoding.md](encoding.md)) rather than as an ObjectRef. This avoids bloating the object dictionary with value ref IDs in provenance-heavy edits.

### 4.4 Edit Contexts

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

The `edges` list represents the path from `root_id` to the entity being modified. For example, if entity "TextBlock_9" is a block of entity "San Francisco", the context would be:
- `root_id`: SanFrancisco
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

### 4.5 Edit Publishing

1. Serialize edit to binary format (Section 6)
2. Publish to content-addressed storage (IPFS)
3. Publish hash onchain

---

## 5. Spaces

Spaces are governance containers for edits. They define who can publish, how proposals are accepted, and what the rules are. Each space maintains its own view of the graph - its own resolved state.

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

Edits are serialized to a compact binary format for transport and storage. The format uses varint encoding, dictionary-indexed references, and optional zstd compression.

For the complete binary encoding specification, see **[encoding.md](encoding.md)**.

**Key points:**
- Magic bytes: `GRC2` (uncompressed) or `GRC2Z` (zstd compressed)
- All identifiers are 16-byte UUIDs
- Dictionary references use varint indices for compactness
- CIDs and signatures are computed over uncompressed bytes

---

## 7. Root Space

The Root Space provides the shared vocabulary that makes the protocol useful out of the box. It defines well-known IDs for universal concepts - properties like Name and Description, schema properties like Types and Date type, content types like Image, and the language and data type entities that other spaces reference. These are the minimum IDs needed at the protocol level; applications may define additional schema on top.

### 7.1 Core Entity Properties

| Name | UUID | Date type | Description |
|------|------|-----------|-------------|
| Name | `a126ca530c8e48d5b88882c734c38935` | TEXT | Primary label |
| Description | `9b1f76ff9711404c861e59dc3fa7d037` | TEXT | Summary text |
| Types | `8f151ba4de204e3c9cb499ddf96f48f1` | RELATION → Type | Type membership |
| Cover | `34f535072e6b42c5a84443981a77cfa2` | RELATION → Image | Cover image with IPFS URL on entity |

### 7.2 Type Properties

Properties on Type entities that define the schema for their instances.

| Name | UUID | Date type | Description |
|------|------|-----------|-------------|
| Properties | `01412f8381894ab1836565c7fd358cc1` | RELATION → Property | Which properties a type defines |

### 7.3 Property Properties

Properties on Property entities that define data type, constraints, and display behavior.

| Name | UUID | Date type | Description |
|------|------|-----------|-------------|
| Data type | `6d29d57849bb4959baf72cc696b1671a` | RELATION → Data type | Expected data type (Section 7.6) |
| To entity types | `9eea393f17dd4971a62ea603e8bfec20` | RELATION → Type | Hints for which types a relation property should point to |
| Renderable type | `2316bbe1c76f463583f23e03b4f1fe46` | RELATION → Renderable type | How the property renders in UI (e.g., URL, Image) |
| Format | `396f8c72dfd04b5791ea09c1b9321b2f` | TEXT | Display format string (see below) |
| Is type property | `d2c1a10114e3464a8272f4e75b0f1407` | CHECKBOX | Whether this property's properties should be added as hints on the containing type |

**Format string by renderable type:**

- **Number:** ICU decimal format string (e.g., `¤#,##0.00` for currency, `#,##0.##` for plain numbers)
- **URL:** Template string with `{}` placeholder for the value (e.g., `https://x.com/{}`, `https://github.com/{}`)

These are advisory - applications use them for UX and query defaults, but the protocol does not enforce type consistency.

### 7.4 Content Types

| Name | UUID | Description |
|------|------|-------------|
| Image | `f3f790c4c74e4d23a0a91e8ef84e30d9` | Image entity |

### 7.5 Languages

Language entities for localized TEXT values are defined in the Linguistics space in the knowledge graph.

### 7.6 Data Type Entities

Data type entities represent the protocol's data types in the knowledge layer. Property entities use `Date type` relations (Section 7.3) to indicate their expected type.

| Name | UUID |
|------|------|
| Boolean | `7aa4792eeacd41868272fa7fc18298ac` |
| Integer | `149fd752d9d04f80820d1d942eea7841` |
| Float | `9b597aaec31c46c88565a370da0c2a65` |
| Decimal | `a3288c22a0564f6fb409fbcccb2c118c` |
| Text | `9edb6fcce4544aa5861139d7f024c010` |
| Bytes | `66b433247667496899b48a89bd1de22b` |
| Date | `e661d10292794449a22367dbae1be05a` |
| Time | `ad75102b03c04d59903813ede9482742` |
| Datetime | `167664f668f840e1976b20bd16ed8d47` |
| Schedule | `caf4dd12ba4844b99171aff6c1313b50` |
| Point | `df250d17e364413d97792ddaae841e34` |
| Rect | `1fcdf014220447bfb6d20c0cce7adca2` |
| Embedding | `f732849378ba4577a33fac5f1c964f18` |

**Usage:** To indicate that property X expects INTEGER values, create a `Date type` relation from X to the Integer entity. Applications query this relation to determine the expected type for UX rendering and query construction.

---

## 8. Resolution

Structural validation (write-time checks for binary format correctness) and error codes are defined in the encoding specification (`encoding.md`, Section 9).

**Derived ID pre-creation:** Because relation entity IDs are derived deterministically (`derived_uuid("grc20:relation-entity:" || relation_id)`), an attacker can pre-create an entity with that ID and set values before the relation exists. When the relation is later created, it adopts the existing entity with its values. This is known behavior, not a vulnerability - applications concerned about this can verify entity provenance at a higher layer.

**Authentication and authorization:** Signature schemes, key management, and authorization rules are defined by space governance, not this specification. The `authors` field is metadata; how it maps to cryptographic identities and what signatures are required (if any) is determined by the governance layer.

### 8.1 Semantic Resolution (Read-Time)

Semantic resolution determines how operations are interpreted against the current state.

When an operation arrives, the indexer checks:

1. **Does the object exist?**
   - No → Only CreateEntity and CreateRelation proceed (others are ignored)
2. **Is the object deleted (tombstoned)?**
   - Yes → Only RestoreEntity/RestoreRelation can revive it. All other ops are ignored. (See Section 3.5 for the full tombstone rules.)
3. **Is there a namespace collision?** (entity ID = existing relation ID, etc.)
   - Yes → The operation is ignored.
4. **Are there concurrent writes to the same field?**
   - Yes → Last-Writer-Wins by OpPosition.

**Summary resolution table:**

| Concern | Resolution |
|---------|------------|
| Object lifecycle | Tombstone dominance (Section 3.5) |
| Duplicate creates | Merge (first creates, later updates) |
| Concurrent edits | LWW by OpPosition |
| Out-of-order arrival | Buffer until ordered position known |

**Detailed operations resolution (NORMATIVE):**

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
| CreateRelation | Relation entity DELETED | Relation created, but entity stays DELETED |
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

---

## 9. Conclusion

GRC-20 defines a minimal, deterministic foundation for decentralized knowledge graphs. The protocol handles the base primitives - knowledge graph structure, binary encoding, and batched publishing - so that higher layers can focus on domain ontology and logic. Blockchain-based publishing provides provenance and global consensus over the state of the knowledge graph. GRC-20 encodes knowledge across space and time, with pluralism so communities can express different points of view as part of the same system.

With this foundation, users can build an open, composable, verifiable knowledge graph.
