# GRC-20 v2 Requirements

## P0: Critical (Must Have)

### Security

1. **Bounded resource consumption** - Decoding untrusted input must not cause unbounded memory allocation, CPU time, or stack depth
2. **Varint safety** - Varint parsing must reject overlong encodings (>10 bytes) and handle truncated input gracefully
3. **Length validation** - All length-prefixed fields must be validated against configurable limits before allocation
4. **Index bounds checking** - All dictionary index references must be validated before use
5. **UTF-8 validation** - All text fields must be validated as valid UTF-8
6. **No panics on malformed input** - Decoder must return errors, never panic or crash

### Correctness

7. **Roundtrip fidelity** - decode(encode(x)) == x for all valid inputs
8. **Spec compliance** - Wire format must exactly match spec Section 6

### Data Integrity

9. **All value types supported** - Must encode/decode all 11 value types: Bool, Int64, Float64, Decimal, Text, Bytes, Timestamp, Date, Point, Embedding, Ref (note: REF is the data type, distinct from Relation entity)
10. **All op types supported** - Must encode/decode all 9 op types: CreateEntity, UpdateEntity, DeleteEntity, CreateRelation, UpdateRelation, DeleteRelation, CreateProperty, CreateType, Snapshot
11. **Lossless precision** - Decimal, Timestamp, and Float64 must preserve full precision through roundtrip

## P1: High Priority (Should Have)

### Cross-Language Portability

12. **Native implementability** - Format must be implementable in any language without FFI/Wasm
13. **No exotic dependencies** - Only require: byte arrays, 64-bit integers, IEEE 754 floats, UTF-8, zstd
14. **Simple primitives** - Varint (LEB128) and ZigZag encodable in ~20 lines per language
15. **Sequential decoding** - Wire format readable in single forward pass, no backtracking

### Compression

16. **Transparent compression** - Auto-detect GRC2 (uncompressed) vs GRC2Z (zstd) on decode
17. **Configurable compression level** - Encoder should accept compression level parameter
18. **Compression optional** - Uncompressed format must be valid and usable

### Error Handling

19. **Fail-fast decoding** - Stop at first error, return error with context
20. **Rich error types** - Errors must indicate failure type and relevant context (field name, index, etc.)
21. **No partial results** - Decoder returns complete Edit or error, never partial state

### Performance

22. **Pre-allocation with limits** - Use count fields to pre-allocate, but validate against limits first
23. **Zero-copy where practical** - Avoid unnecessary allocations in decode hot paths
24. **Efficient dictionary lookup** - O(1) index-to-ID resolution during decode

## P2: Medium Priority (Nice to Have)

### Validation Layers

25. **Structural validation in codec** - Magic, version, lengths, indices, UTF-8
26. **Semantic validation separate** - Type checking and lifecycle validation in optional module
27. **Configurable strictness** - Allow callers to choose validation depth

### Usability

28. **Simple public API** - `encode_edit(&Edit) -> Result<Vec<u8>>` and `decode_edit(&[u8]) -> Result<Edit>`
29. **FFI-friendly types** - Public types use `[u8; 16]` for IDs, not library-specific UUID types
30. **No required code generation** - Format is self-describing enough to decode without schema

### Testing

31. **Property-based tests** - Roundtrip arbitrary valid Edits
32. **Fuzz testing** - Decoder must not crash on arbitrary byte sequences
33. **Malformed input corpus** - Explicit tests for truncation, overflow, invalid tags
34. **Cross-implementation tests** - Test vectors for validating decoded structures (not bytes)

## P3: Low Priority (Future)

### Observability

35. **Size estimation** - Ability to estimate encoded size before encoding
36. **Decode metrics** - Optional counters for ops decoded, bytes read, etc.

### Streaming

37. **Op-by-op iteration** - Future: decode ops lazily without loading full Edit into memory
38. **Partial decode** - Future: decode only header/dictionaries without parsing all ops

### Ecosystem

39. **Wasm bindings** - Optional wasm-bindgen wrapper for browser/JS
40. **Python bindings** - Optional PyO3 wrapper for Python ecosystem
41. **Go bindings** - Optional wazero-based wrapper for Go ecosystem
42. **CLI tool** - Command-line encoder/decoder for debugging and testing

## Non-Requirements (Explicitly Out of Scope)

- **Deterministic encoding** - Byte-identical output across implementations not required; edits encoded once by author
- **Schema enforcement** - Serializer does not validate value types against property declarations
- **Relation target validation** - Serializer does not check if referenced entities exist
- **Causal ordering** - Serializer does not handle DAG; ordering provided by on-chain governance events
- **Signature verification** - Serializer does not validate author signatures
- **Deterministic relation ID computation** - Serializer does not verify unique-mode relation IDs
- **Merge conflict resolution** - Serializer does not implement LWW or merge logic
- **Network transport** - Serializer is bytes-in/bytes-out, not a network protocol
- **Entity lifecycle validation** - Serializer does not check if entity is DEAD before accepting updates
- **Duplicate detection** - Serializer does not check if entity/relation already exists
- **State lookups** - Serializer performs no database reads; validation is structural only (see spec Section 8)
