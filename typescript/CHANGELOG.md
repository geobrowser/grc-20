# @geoprotocol/grc-20

## 0.2.3

### Patch Changes

- de64e1b: Add validation to `writeId()` to ensure IDs are exactly 16 bytes

  Previously, if an incorrectly-sized byte array was passed as an ID (e.g., a TextEncoder-encoded Ethereum address instead of a proper 16-byte UUID), the library would silently write the wrong number of bytes, corrupting the binary output. Now `writeId()` throws a clear error: `"writeId expects 16-byte ID, got X bytes"`.

## 0.1.7

### Patch Changes

- add Ops API
