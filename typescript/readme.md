# @geoprotocol/grc-20

TypeScript library for encoding and decoding GRC-20 binary property graph data.

## Installation

```bash
npm install @geoprotocol/grc-20
```

## Quick Start

```typescript
import {
  EditBuilder,
  encodeEdit,
  decodeEdit,
  randomId,
  properties,
} from "@geoprotocol/grc-20";

// Create an edit with an entity
const edit = new EditBuilder(randomId())
  .setName("Create Alice")
  .addAuthor(randomId())
  .setCreatedNow()
  .createEntity(randomId(), (e) =>
    e
      .text(properties.name(), "Alice", undefined)
      .text(properties.description(), "A person", undefined)
  )
  .build();

// Encode to binary
const bytes = encodeEdit(edit);

// Decode back
const decoded = decodeEdit(bytes);
```

## Features

- **Type-safe API** - Full TypeScript definitions
- **Builder pattern** - Fluent API for constructing edits
- **Binary codec** - Pure TypeScript encoder/decoder
- **Zstd compression** - Optional WASM-based compression (lazy-loaded)
- **Tree-shakeable** - Separate entry points for minimal bundles
- **Cross-platform** - Works in Node.js and browsers

## Bundle Sizes

| Entry Point | Gzipped |
|-------------|---------|
| Full library | ~12 KB |
| Types only | ~1.4 KB |
| Builder only | ~1.2 KB |
| Codec only | ~8 KB |
| Genesis IDs | ~1.7 KB |
| Utilities | ~1.6 KB |
| Zstd WASM (lazy) | ~81 KB |

The Zstd WASM module is only loaded when compression functions are used.

### Lazy Loading

For optimal initial load, import the codec separately:

```typescript
// Initial load (~4.4 KB gzipped)
import { EditBuilder, randomId, properties } from "@geoprotocol/grc-20/builder";
import { properties } from "@geoprotocol/grc-20/genesis";

// Lazy load codec when needed (~8 KB gzipped)
const { encodeEdit } = await import("@geoprotocol/grc-20/codec");

// WASM only loads if you use compression (~81 KB additional)
const { encodeEditAuto } = await import("@geoprotocol/grc-20/codec");
```

## API Reference

### Types

```typescript
import {
  Id,              // 16-byte UUID (Uint8Array branded type)
  Edit,            // Batch of operations with metadata
  Op,              // Union of all operation types
  Value,           // Union of all value types
  DataType,        // Enum: Bool, Int64, Float64, Text, etc.
  PropertyValue,   // Property ID + Value pair
} from "@geoprotocol/grc-20";
```

### Builders

```typescript
import {
  EditBuilder,          // Build Edit objects
  EntityBuilder,        // Build entity values
  UpdateEntityBuilder,  // Build update operations
  RelationBuilder,      // Build relation operations
  UpdateRelationBuilder,// Build relation update operations
} from "@geoprotocol/grc-20";
```

#### EditBuilder

```typescript
const edit = new EditBuilder(editId)
  .setName("My Edit")
  .addAuthor(authorId)
  .setCreatedAt(BigInt(Date.now()) * 1000n)  // microseconds
  .createEntity(entityId, e => e
    .text(propId, "value", languageId)
    .int64(propId, 42n, unitId)
    .float64(propId, 3.14, undefined)
    .decimal(propId, { exponent: -2, mantissa: 1234n }, undefined)
    .bool(propId, true)
    .bytes(propId, new Uint8Array([1, 2, 3]))
    .point(propId, 40.7128, -74.006)
    .date(propId, "2024-01-15")
    .time(propId, "10:30:00")
    .datetime(propId, "2024-01-15T10:30:00Z")
  )
  .updateEntity(entityId, u => u
    .setText(propId, "new value", undefined)
    .setInt64(propId, 100n, undefined)
    .unsetText(propId, languageId)  // Unset specific language
    .unsetAll(propId)               // Unset all values for property
  )
  .deleteEntity(entityId)
  .restoreEntity(entityId)
  .createRelation(r => r
    .id(relationId)
    .from(fromId)
    .to(toId)
    .relationType(relationTypeId)
  )
  .deleteRelation(relationId)
  .restoreRelation(relationId)
  .createValueRef(valueRefId, entityId, propId, {
    type: "text",
    value: "Referenceable value"
  })
  .build();
```

### Ops (Functional API)

`createEdit` defaults `id` to `randomId()` and `createdAt` to `0n` when omitted.

```typescript
import {
  createEdit,
  createEntity,
  createRelation,
  randomId,
  properties,
  relationTypes,
} from "@geoprotocol/grc-20";

const entityId = randomId();
const authorId = randomId();

const ops = [
  createEntity({
    id: entityId,
    values: [
      { property: properties.name(), value: { type: "text", value: "Alice" } },
      { property: properties.description(), value: { type: "text", value: "A person" } },
    ],
  }),
  createRelation({
    id: randomId(),
    relationType: relationTypes.types(),
    from: entityId,
    to: randomId(),
  }),
];

const edit = createEdit({
  name: "Create Alice",
  author: authorId,
  ops,
});
```

### Codec

```typescript
import { encodeEdit, decodeEdit } from "@geoprotocol/grc-20";

// Encode (uncompressed)
const bytes = encodeEdit(edit);
const bytesCanonical = encodeEdit(edit, { canonical: true });

// Decode (uncompressed)
const edit = decodeEdit(bytes);
```

### Compression

The library supports Zstd compression via a lazy-loaded WASM module. The WASM is only downloaded when compression functions are first used.

```typescript
import {
  preloadCompression,
  isCompressionReady,
  encodeEditAuto,
  decodeEditAuto,
  encodeEditCompressed,
  decodeEditCompressed,
} from "@geoprotocol/grc-20";

// Preload WASM on app startup (recommended)
await preloadCompression();

// Check if compression is ready
if (isCompressionReady()) {
  console.log("Compression ready!");
}

// Auto encode/decode (recommended for most use cases)
// Compresses if data > 256 bytes, otherwise uncompressed
const data = await encodeEditAuto(edit);
const decoded = await decodeEditAuto(data);

// Control compression threshold
const data = await encodeEditAuto(edit, { threshold: 0 });       // always compress
const data = await encodeEditAuto(edit, { threshold: 1024 });    // compress if > 1KB
const data = await encodeEditAuto(edit, { threshold: Infinity }); // never compress

// Explicit compressed encode/decode
const compressed = await encodeEditCompressed(edit);
const decoded = await decodeEditCompressed(compressed);
```

#### Browser Usage (without bundler)

If using native ES modules without a bundler, add an import map for the WASM dependency:

```html
<script type="importmap">
{
  "imports": {
    "@bokuweb/zstd-wasm": "https://esm.sh/@bokuweb/zstd-wasm@0.0.27"
  }
}
</script>
```

### ID Utilities

```typescript
import {
  randomId,             // Generate random UUIDv4
  parseId,              // Parse hex string to Id
  formatId,             // Format Id as hex string
  derivedUuid,          // Derive UUIDv8 from bytes (SHA-256, sync)
  derivedUuidAsync,     // Derive UUIDv8 from bytes (SHA-256, async)
  derivedUuidFromString,
  uniqueRelationId,     // Derive relation ID from endpoints
  relationEntityId,     // Derive entity ID from relation ID
  idsEqual,             // Compare two Ids for equality
  compareIds,           // Compare two Ids for ordering (-1, 0, 1)
  NIL_ID,               // Zero UUID
} from "@geoprotocol/grc-20";
```

### Validation

Validate values and positions before encoding:

```typescript
import { validateValue, validatePosition } from "@geoprotocol/grc-20";

// Validate a value matches its declared type
const result = validateValue(value, DataType.Text);
if (!result.valid) {
  console.error(result.error);
}

// Validate position string format
const posResult = validatePosition("a0");
if (!posResult.valid) {
  console.error(posResult.error);
}
```

### Data Types Reference

| Type | TypeScript Representation |
|------|---------------------------|
| `BOOL` | `{ type: "bool", value: boolean }` |
| `INT64` | `{ type: "int64", value: bigint, unit?: Id }` |
| `FLOAT64` | `{ type: "float64", value: number, unit?: Id }` |
| `DECIMAL` | `{ type: "decimal", exponent: number, mantissa: bigint, unit?: Id }` |
| `TEXT` | `{ type: "text", value: string, language?: Id }` |
| `BYTES` | `{ type: "bytes", value: Uint8Array }` |
| `DATE` | `{ type: "date", value: string }` (ISO 8601) |
| `TIME` | `{ type: "time", value: string }` (HH:MM:SS) |
| `DATETIME` | `{ type: "datetime", value: string }` (ISO 8601) |
| `SCHEDULE` | `{ type: "schedule", value: string }` (cron-like) |
| `POINT` | `{ type: "point", lat: number, lon: number }` |
| `EMBEDDING` | `{ type: "embedding", subType: EmbeddingSubType.Float32 \| EmbeddingSubType.Int8 \| EmbeddingSubType.Binary, data: number[] }` |

### Genesis IDs

Well-known IDs from the Genesis Space:

```typescript
import { properties, types, relationTypes, languages } from "@geoprotocol/grc-20";

// Properties
properties.name()        // a126ca530c8e48d5b88882c734c38935 - Name (TEXT)
properties.description() // 9b1f76ff9711404c861e59dc3fa7d037 - Description (TEXT)
properties.cover()       // 34f535072e6b42c5a84443981a77cfa2 - Cover image URL (TEXT)

// Types
types.image()            // f3f790c4c74e4d23a0a91e8ef84e30d9 - Image entity

// Relation Types
relationTypes.types()    // 8f151ba4de204e3c9cb499ddf96f48f1 - Type membership

// Languages (derived from BCP 47 codes)
languages.english()      // or languages.fromCode("en")
languages.spanish()
languages.french()
// ... etc
```

## Entry Points

For tree-shaking, use specific entry points:

```typescript
import { ... } from "@geoprotocol/grc-20";          // Full library
import { ... } from "@geoprotocol/grc-20/types";    // Types only
import { ... } from "@geoprotocol/grc-20/builder";  // Builders only
import { ... } from "@geoprotocol/grc-20/codec";    // Codec only
import { ... } from "@geoprotocol/grc-20/genesis";  // Genesis IDs only
import { ... } from "@geoprotocol/grc-20/util";     // Utilities only
```

## Development

```bash
# Install dependencies
npm install

# Build
npm run build

# Test (Node.js)
npm test

# Test (Browser via Playwright)
npm run test:browser

# Test both
npm run test:all

# Analyze bundle sizes
npm run bundle:analyze

# Run performance benchmark
npm run benchmark

# Run browser demo
npm run demo
# Then open http://localhost:3000/examples/browser-demo.html
```

## License

MIT
