# @geoprotocol/grc-20

## 0.4.1

### Patch Changes

- 968e4f0: Harden decimal encoding and decoding in the TypeScript codec.

  Decimal values are now normalized canonically during encode and decode, with better validation for exponent bounds, empty or non-minimal big mantissas, oversized mantissa byte lengths, and decimal inputs that would require excessive normalization work.

## 0.2.3

### Patch Changes

- de64e1b: Add validation to `writeId()` to ensure IDs are exactly 16 bytes

  Previously, if an incorrectly-sized byte array was passed as an ID (e.g., a TextEncoder-encoded Ethereum address instead of a proper 16-byte UUID), the library would silently write the wrong number of bytes, corrupting the binary output. Now `writeId()` throws a clear error: `"writeId expects 16-byte ID, got X bytes"`.

## 0.1.7

### Patch Changes

- add Ops API
