---
"@geoprotocol/grc-20": patch
---

Harden decimal encoding and decoding in the TypeScript codec.

Decimal values are now normalized canonically during encode and decode, with better validation for exponent bounds, empty or non-minimal big mantissas, oversized mantissa byte lengths, and decimal inputs that would require excessive normalization work.
