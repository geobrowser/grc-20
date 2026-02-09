# GRC-20 v2 Design FAQ

Common questions and justifications for design decisions.

---

## Numerical Types

### Q: Why exactly INTEGER, FLOAT, and DECIMAL? Are these the right choices for a global knowledge graph standard?

**A:** Yes. This is the canonical "Goldilocks" trio for modern data systems—the exact right balance between minimalism (few types to implement) and coverage (handling every real-world use case).

Many standards fail here: Protobuf lacks DECIMAL (causing pain for finance), and SQL over-complicates integers (SMALLINT, INTEGER, BIGINT).

**The Domain Coverage Map**

These three types map perfectly to the three fundamental ways humans measure the world, with zero semantic overlap:

| Type | Domain | What it Represents | Examples |
|------|--------|-------------------|----------|
| INTEGER | Discrete / Countable | Exact counts, IDs, offsets, time | `count: 42`, `offset: 8492` |
| FLOAT | Continuous / Approximate | Physical measurements where precision is relative | `latitude: 37.77`, `probability: 0.999` |
| DECIMAL | Human / Constructed | Values defined by human rules that must be exact | `price: $19.99`, `balance: 1.5 ETH` |

**Why not more types?**

| Rejected Type | Impulse | Reality |
|---------------|---------|---------|
| INT32/INT16/BYTE | "Save space on disk!" | Varints already compress small values. Storing `5` in INTEGER takes 1 byte. Zero benefit, added complexity. |
| UINT64 | "Positive-only numbers!" | IDs use UUIDs (128-bit). INTEGER maxes at ~9 quintillion—you won't overflow. Signed is safer (no underflow bugs). |
| BIGINT | "Crypto has 256-bit values!" | DECIMAL with `exponent: 0` *is* a BigInt. `1 ETH (wei) = { mantissa: 1e18, exponent: 0 }`. |

**Why DECIMAL is particularly strong:**

```
DECIMAL { exponent: int32, mantissa: int64 | bytes }
```

- **Variable precision**: Unlike SQL's `DECIMAL(18,2)`, allows `$10` and `$0.0000001` in the same field
- **The "bytes" escape hatch**: Most values fit in int64 (~18 digits), but can overflow to arbitrary precision for crypto whale balances, hyper-inflationary currencies, or scientific constants

**Implementation sanity:**

| Language | INTEGER | FLOAT | DECIMAL |
|----------|-------|---------|---------|
| JavaScript | `BigInt` | `Number` | Library (standard practice) |
| Python | `int` (native arbitrary precision) | `float` | `decimal.Decimal` |
| Rust/Go/C++ | Native | Native | Struct |

**Verdict:** Keep exactly these three.
- INTEGER: The skeleton (counts, times)
- FLOAT: The flesh (physics, probability)
- DECIMAL: The contract (money, math)

This covers 100% of numerical use cases with 0% redundancy.

---

## Binary Format

### Q: Why a custom binary format instead of Protocol Buffers?

**A:** Three reasons: determinism, size, and native implementability.

| Factor | Custom Format | Protobuf |
|--------|---------------|----------|
| **Determinism** | Achievable (sorted dictionaries) | Hard (map ordering, unknown fields) |
| **Wire size** | Smaller (dictionary interning) | Larger (16-byte UUIDs repeated) |
| **Native implementation** | ~200 lines | ~500-1000 lines |
| **Code generation** | None required | Required or manual |

**Dictionary interning** is the key advantage. In a typical edit with 10K entities:
- Our format: Entity references are 1-4 byte varint indices
- Protobuf: Entity references are 16 bytes each + field tag

This saves ~12 bytes per reference, which compounds across relations and property values.

**Protobuf's determinism problem:** Protobuf explicitly does NOT guarantee deterministic encoding. Map fields have undefined order, unknown fields are preserved in undefined order. You can use `deterministic=true` in some languages, but it's not universal.

For content-addressed storage (IPFS), predictable encoding matters. (Though we relaxed this to SHOULD, not MUST—see below.)

### Q: Why is deterministic encoding not required?

**A:** Because edits are encoded once by their author.

The workflow:
1. Author creates Edit
2. Author encodes it (their bytes, their way)
3. CID computed from those bytes
4. Published to IPFS, anchored on-chain

Nobody else needs to reproduce the exact bytes—they fetch by CID and decode. Two implementations encoding the "same" Edit would produce different Edit IDs anyway (different `created_at`, different author signatures).

**What we'd gain from mandatory determinism:**
- Verify integrity by re-encoding (but you can verify by hash instead)
- Deduplicate identical Edits (but they have different IDs anyway)

**What it costs:**
- Spec complexity (must specify exact ordering rules)
- Testing burden (verify all implementations produce identical bytes)
- Implementation constraints (must sort, can't use hash tables directly)

**Verdict:** SHOULD sort dictionaries for consistency, but MAY vary. Simpler spec, easier implementation, no real-world downside.

### Q: Why LEB128 varints?

**A:** Universal, simple, battle-tested.

- Used by: Protobuf, WebAssembly, DWARF, Android DEX
- Implementation: ~20 lines in any language
- Efficiency: Small values (0-127) = 1 byte, scales to 64-bit

We use unsigned LEB128 for lengths/indices, ZigZag + LEB128 for signed integers (so -1 encodes as 1 byte, not 10).

### Q: Why zstd compression?

**A:** Best compression ratio at reasonable speed, with wide library support.

| Algorithm | Ratio | Speed | Library availability |
|-----------|-------|-------|---------------------|
| gzip | Good | Slow | Everywhere |
| lz4 | OK | Very fast | Good |
| **zstd** | **Excellent** | **Fast** | **Excellent** |
| brotli | Excellent | Slow | Good |

Zstd at level 3 gives near-optimal compression without noticeable latency. Pure implementations exist for Go, Rust, and via Wasm for browsers.

---

## Identifiers

### Q: Why UUIDs instead of auto-incrementing IDs?

**A:** Decentralization requires coordination-free ID generation.

Auto-increment requires a central authority to avoid collisions. UUIDs can be generated by any participant, anywhere, with negligible collision probability.

**UUID properties:**
- 128-bit = 2^128 possible values
- Generated without coordination
- Opaque (no embedded semantics to leak or break)
- Not dereferenceable (unlike URIs)

**Why not content-addressed hashes?**
Entities are mutable—their content changes over time. A content hash would change on every update, breaking all references.

### Q: Why 16 raw bytes instead of string UUIDs?

**A:** 2.25x smaller, faster to compare.

| Format | Size | Example |
|--------|------|---------|
| Hyphenated | 36 bytes | `550e8400-e29b-41d4-a716-446655440000` |
| Hex | 32 bytes | `550e8400e29b41d4a716446655440000` |
| **Raw bytes** | **16 bytes** | `[0x55, 0x0e, ...]` |

With thousands of IDs per edit, this adds up. Display format is an implementation choice (hex, Base58, etc.).

---

## Data Types

### Q: Why TEXT for DATE instead of integer timestamp?

**A:** Semantic precision and BCE dates.

An integer timestamp forces false precision:
- "1850" becomes `1850-01-01T00:00:00.000000Z`
- "Sometime in March 2024" becomes... what exactly?

ISO 8601 strings naturally express precision:
```
"2024-03-15"         // Date only
"2024-03"            // Month precision
"2024"               // Year only
"-0100"              // 100 BCE
```

TIMESTAMP (int64 microseconds) is for machine times: `created_at`, sync events, audit logs.
DATE (ISO 8601 string) is for human times: birthdates, historical events.

### Q: Why is SCHEDULE not a primitive type?

**A:** The "infinite expansion" problem.

The impulse for SCHEDULE as a primitive:
- Indexers could auto-build availability indices
- "Find entities available at time T" queries
- Standard format (RFC 7953 VAVAILABILITY)

**Why we rejected it:**

1. **Requires a calendar engine.** Evaluating "is this entity available on January 15, 2026?" requires:
   - Parsing RRULE recurrence patterns
   - Expanding exception dates
   - Handling timezone conversions
   - Computing occurrence sets

   This is NOT simple string comparison—it's a full calendar implementation. Every indexer would need to bundle a calendar library, violating our "simple primitives" principle.

2. **No single standard.** The ecosystem is fragmented:
   - RFC 5545 (iCalendar): Full spec with VEVENT, VTODO, etc.
   - RFC 7953 (VAVAILABILITY): Subset for availability
   - RRULE alone: Just recurrence rules
   - cron expressions: Scheduling format

   Picking one creates friction with users of others.

3. **Infinite expansion.** If SCHEDULE, why not:
   - CURRENCY (with exchange rate semantics)?
   - PHONE (with E.164 validation)?
   - EMAIL (with RFC 5322 parsing)?

   Each has the same argument: "indexers could validate/index it better."

**The alternative (better):**

Use TEXT + schema hints at the knowledge layer:

```
Property {
  id: "business_hours"
  data_type: TEXT
  constraints: { format: "ical-vavailability" }  // Knowledge layer hint
}
```

Indexers that understand the hint can build availability indices. Others treat it as opaque text. This is extensible, doesn't bloat the core protocol, and lets different ecosystems use their preferred format.

**Result:** 11 data types that compose cleanly. Availability is domain-specific, handled at the knowledge layer.

### Q: Why is POINT `[latitude, longitude]` instead of `[longitude, latitude]`?

**A:** Human convention wins for a human-readable type.

**The two conventions:**

| Convention | Order | Who uses it | Rationale |
|------------|-------|-------------|-----------|
| Lat/Lon | [Y, X] | Humans, Neo4j, most mapping apps | "37.77°N, 122.41°W" - how humans say coordinates |
| Lon/Lat | [X, Y] | WKB, GeoJSON, PostGIS | Mathematical convention (X before Y) |

**Why lat/lon for POINT:**

1. **Human readability.** POINT is a *simple* type for *simple* use cases. When someone writes `{ lat: 37.77, lon: -122.41 }`, they expect that order.

2. **Neo4j precedent.** The most popular graph database defaults to `[lat, lon]` for WGS84 points. Knowledge graph users expect this.

3. **Minimal footprint.** POINT is 16 bytes (2 × float64). It's for "where is this entity?" not "draw this polygon."

**For complex geometry:** Use BYTES with WKB encoding, which follows `[lon, lat]` convention. This is explicitly documented in the spec.

**Why not variable-length for 3D?**

Fixed 2D (16 bytes) because:
- 99% of location data is 2D (where on Earth's surface)
- Variable length adds decoding complexity
- For altitude/depth, use BYTES + WKB Point Z

**Summary:** POINT = human-friendly simple case. WKB in BYTES = GIS-compatible complex case.

### Q: Why no LIST data type?

**A:** Every use case for LIST is better served by existing mechanisms.

**Use case 1: Geometry (polygons, lines, multi-points)**

Solution: BYTES with WKB (Well-Known Binary)

- WKB is compact, battle-tested, supported by every GIS tool
- Spatial indexers (PostGIS, H3) already understand WKB
- Includes metadata LIST would lack (SRID, Z/M dimensions)
- Reinventing POLYGON would be strictly worse than the standard

The spec documents this in Section 2.4 under "Geometry in BYTES."

**Use case 2: Tags, categories, simple primitive arrays**

Solution: Multi-valued properties (Section 2.5)

```
Entity {
  values: [
    { property: lucky_number, value: 7, position: "a" }
    { property: lucky_number, value: 13, position: "n" }
    { property: lucky_number, value: 42, position: "z" }
  ]
}
```

Benefits over LIST:
- **Per-element history**: Who added 42? When?
- **Fine-grained conflict resolution**: Concurrent adds both preserved
- **Queryable**: "entities where lucky_number = 42"
- **Individual updates**: Delete one element without rewriting array

Cost: 3 values instead of 1 packed array. Acceptable for knowledge graph use cases where element semantics matter.

**Use case 3: Truly opaque packed arrays (rare)**

Solution: BYTES with application-specific encoding

If you have a dense array of primitives with no semantic meaning per-element:
- Pack as BYTES (e.g., little-endian int32 array)
- Document the format in property metadata
- Accept that indexers treat it as opaque

**Why not add LIST anyway?**

| Problem | Impact |
|---------|--------|
| Element type in schema | `List<Int>` vs `List<Text>`? Adds type parameterization complexity |
| Coarse conflict resolution | Two concurrent edits = LWW on whole list, not merge |
| No per-element history | Can't answer "who added this item?" |
| Not queryable | Can't index into list elements efficiently |
| Nesting | `List<List<Int>>`? Where does it stop? |

**Summary:**

| Use case | Solution | Rationale |
|----------|----------|-----------|
| Geometry | BYTES + WKB | Industry standard, indexable |
| Tags, categories | Multi-values | Per-element semantics |
| Dense opaque arrays | BYTES | Rare, accept opacity |
| Embeddings | EMBEDDING | Special-cased for vector indexers |

The spec intentionally trades ergonomics for better semantics. The "friction" cases are either solved by standards (WKB) or rare enough that BYTES suffices.

---

### Q: Why EMBEDDING instead of just BYTES?

**A:** Same reason—indexer semantics.

BYTES is opaque. EMBEDDING tells indexers:
- This is a dense vector for similarity search
- Build HNSW/IVF indices automatically
- Enable `cosine_similarity()` in queries
- Validate dimensionality constraints

The sub-type field (float32/int8/binary) enables:
- 4x smaller quantized embeddings (int8)
- 32x smaller binary embeddings
- Direct memory-mapping for GPU operations

---

## Schema & Validation

### Q: Why no schema enforcement at the serialization layer?

**A:** Separation of concerns.

The serializer's job: safely convert bytes ↔ data structures.

Schema enforcement requires context the serializer doesn't have:
- Property definitions might be in a different edit
- Entity lifecycle state requires the full graph
- Different spaces may have different rules

**Layered validation:**
1. **Structural** (serializer): Magic, version, lengths, indices, UTF-8
2. **Semantic** (separate module): Type checking, lifecycle, with graph context

This keeps the protocol minimal while enabling sophisticated schema systems at higher layers.

### Q: Why are types "tags, not classes"?

**A:** Flexibility for real-world modeling.

Traditional class hierarchies:
- Single inheritance = artificial constraints
- "Is a Person a User or is a User a Person?"
- Schema migrations are painful

Tag-based types:
- Entity can have multiple types simultaneously
- `Elon Musk: [Person, CEO, Engineer, Investor]`
- Add/remove types without migration
- No inheritance hierarchy to maintain

---

## Relations

### Q: Why are relations first-class entities with their own IDs?

**A:** Relations need identity for several reasons:

1. **Attributes**: "Alice worked at Acme" needs `start_date`, `end_date`
2. **References**: Other entities can reference the relation itself
3. **Meta-edges**: "I dispute the claim that Alice works at Acme"
4. **History**: Track when the relation was created/modified

Lightweight edges (just source/target/type) could work for simple graphs, but knowledge graphs need rich edges.

### Q: Why two relation ID modes (instance vs unique)?

**A:** Different semantics for different use cases.

**Instance mode** (random UUID): Multiple relations of same type between same entities.
- "Alice worked at Acme" (2015-2018)
- "Alice worked at Acme" (2020-present)

**Unique mode** (deterministic ID): At most one relation of this type.
- "Alice is member of DAO" (can't be member twice)
- ID = `SHA-256(from || to || type)[0:16]`
- Creates are idempotent: same inputs = same ID = no duplicate

---

## Cross-Language

### Q: Why is native implementation emphasized over FFI bindings?

**A:** Ecosystem health.

FFI bindings create:
- Binary distribution complexity
- Platform-specific builds
- Wasm overhead in browsers
- Dependency on Rust toolchain

Native implementations give each ecosystem:
- First-class tooling
- Idiomatic APIs
- No binary dependencies
- Community ownership

The format is designed for this:
- Varint/ZigZag: ~20 lines each
- Sequential decoding: no backtracking
- No code generation required
- Zstd: libraries exist everywhere

FFI bindings are optional convenience for ecosystems that want them.

---

## Architecture

### Q: Why "blind writes" instead of validating state before accepting operations?

**A:** The "read-before-write" pattern is fatal to performance and breaks distributed systems semantics.

**The bottleneck:**

```
Traditional:  Receive(op) → Read(state) → Validate → Write(op)
                                ↑
                           O(log N) disk I/O per operation
                           Cuts throughput by 50-90%

GRC-20:       Receive(op) → Validate(structure) → Append(op)
                                ↑
                           O(1) - no disk reads
```

**Why state validation can't work in decentralized systems:**

| Scenario | Problem |
|----------|---------|
| Two users create "The Moon" offline | Who "fails"? Both are valid. |
| Update arrives before Create | Reject valid op? Or accept and repair? |
| Delete arrives out of order | CRDT convergence breaks |
| Malicious node | Ignores your "reject if" rules anyway |

**The insight:** In event sourcing, operations are facts about what authors asserted. The system's job is to record all facts, then compute state at query time.

**Resolution moves to read time:**

```
Write: "I assert entity 123 has name='Alice'"  → Logged immediately
Read:  "What is entity 123?"                   → Replay ops, apply merge rules, return state
```

**What this enables:**

| Capability | How |
|------------|-----|
| High throughput | Append-only ingestion, no disk reads |
| Offline-first | No coordination needed to create/update |
| Out-of-order tolerance | Missing ops don't block; state repairs when they arrive |
| CRDT convergence | All valid ops accepted; merge rules guarantee identical state |

**Implications:**

- **CreateEntity** = "initialize or merge" (idempotent upsert)
- **UpdateEntity** = "append mutation" (blind, no lifecycle check)
- **DeleteEntity** = "append tombstone" (hide request, not hard delete)

### Q: Can we truly delete data? Is delete permanent?

**A:** Delete is a "hide request", not permanent destruction.

**Why:**

1. **Event sourcing:** All history is preserved for audit, recovery, and legal compliance
2. **Decentralization:** You can't force other nodes to delete their copies
3. **Consistency:** Hard deletes break CRDT merge (what if delete arrives out of order?)

**What delete actually does:**

```
Timeline:
  T1: CreateEntity(id: 123, name: "Alice")
  T5: DeleteEntity(id: 123)
  T6: UpdateEntity(id: 123, name: "Bob")   -- After tombstone

Resolution at query time:
  - Entity 123 has status: DELETED
  - Update at T6 is ignored (tombstone dominance)
  - History is preserved: auditors can see all ops
```

**For compliance (GDPR right to erasure):**

Deleting the *entity* doesn't delete the *edit*. To truly purge:
1. The space must support compaction/rewriting history
2. All nodes must agree to purge
3. This is a governance decision, not a protocol feature

**Recommendation:** For sensitive data, encrypt at the application layer. "Delete" = rotate keys.

### Q: If CreateEntity is idempotent (merges if exists), why have it at all? Isn't it just UpdateEntity?

**A:** CreateEntity serves distinct purposes that UpdateEntity cannot.

**1. Intent signaling:**

CreateEntity says: "I am asserting this entity exists in the world."
UpdateEntity says: "I am modifying an entity I believe already exists."

This distinction matters for:
- **Audit trails:** "Who first claimed The Moon exists?" vs "Who changed its description?"
- **Genesis events:** The first Create establishes provenance
- **Debugging:** Seeing Create vs Update in logs tells you author intent

**2. Schema bootstrapping:**

CreateEntity typically carries the **initial types** that define what the entity is:

```
CreateEntity {
  id: 123
  types: [Person, Author]      -- Essential classification
  values: [{ name: "Alice" }]
}
```

UpdateEntity assumes types exist; CreateEntity establishes them.

**3. Offline conflict handling:**

Two users offline both "create" the same entity (e.g., "The Moon"):

| Without CreateEntity | With CreateEntity |
|---------------------|-------------------|
| Both send UpdateEntity | Both send CreateEntity |
| First one fails ("entity doesn't exist") | Both succeed (idempotent merge) |
| Race condition | Convergent |

**4. It's NOT a no-op:**

CreateEntity does real work:
- Allocates entity ID in the graph
- Establishes initial types
- Sets initial property values
- Creates audit trail entry

The "idempotent upsert" behavior means duplicate Creates don't fail—it doesn't mean Creates are meaningless.

**Summary:** Keep CreateEntity for intent, types, and provenance. It's semantically distinct from "modify existing thing."

### Q: Why include DeleteEntity? Can't we just use UpdateEntity to clear all properties?

**A:** Clearing properties does not express lifecycle termination. Without DeleteEntity, the graph accumulates empty but "alive" nodes that degrade query quality and complicate application logic.

**The workaround and its problems:**

Without DeleteEntity, users simulate deletion by unsetting all properties:

```
UpdateEntity {
  id: 123
  unset_properties: [name, description, ...]
}
```

This creates an entity that exists but contains nothing—a state with no clear semantics.

| Consequence | Description |
|-------------|-------------|
| **Persistent graph presence** | The entity remains ACTIVE and appears in traversals |
| **Empty results in queries** | `MATCH (p:Person)-[:WORKS_AT]->(c:Company)` returns property-less nodes |
| **Defensive query patterns** | Every query requires `WHERE n.name IS NOT NULL` filters |
| **Ambiguous semantics** | Cannot distinguish "data unknown" from "entity removed" |

**Semantic distinction:**

These represent different states that a knowledge graph must differentiate:

| State | Meaning | Example |
|-------|---------|---------|
| Properties cleared | Entity exists, data is absent or redacted | Classified individual with hidden identity |
| Entity deleted | Entity no longer exists in the domain | Company dissolved, account closed |

**Lifecycle modeling:**

DeleteEntity enables representation of real-world state transitions:
- Business closure
- Account termination
- Content moderation (removal from active graph)
- Error correction (removing mistakenly created entities)

**Compliance implications:**

| Requirement | Without DeleteEntity | With DeleteEntity |
|-------------|---------------------|-------------------|
| GDPR Article 17 | No standard "forgotten" state | Tombstone signals "do not process" |
| Content moderation | Flagged content remains queryable | Removed from active graph |

**DeleteEntity as state transition:**

Delete is an append-only operation like any other—it records a lifecycle event:

```
CreateEntity: ∅ → ACTIVE
DeleteEntity: ACTIVE → DELETED
```

The DELETED state instructs indexers to exclude the entity from standard queries while preserving history for audit and potential recovery.

**Summary:** A lifecycle model requires both creation and termination states. DeleteEntity provides the termination primitive that clearing properties cannot express.
