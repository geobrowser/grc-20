import type { Id } from "../types/id.js";
import type { DecimalMantissa, PropertyValue, Value } from "../types/value.js";
import { DataType, EmbeddingSubType, embeddingBytesForDims } from "../types/value.js";
import { DecodeError, Reader, Writer } from "./primitives.js";
import {
  parseDateRfc3339,
  formatDateRfc3339,
  parseTimeRfc3339,
  formatTimeRfc3339,
  parseDatetimeRfc3339,
  formatDatetimeRfc3339,
} from "../util/datetime.js";

/**
 * Dictionary builder for tracking property/language/unit indices.
 */
export interface DictionaryIndices {
  getPropertyIndex(id: Id): number;
  getLanguageIndex(id: Id | undefined): number;
  getUnitIndex(id: Id | undefined): number;
  getDataType(propertyId: Id): DataType;
}

/**
 * Dictionary lookups for decoding.
 */
export interface DictionaryLookups {
  getProperty(index: number): { id: Id; dataType: DataType };
  getLanguage(index: number): Id | undefined;
  getUnit(index: number): Id | undefined;
}

/**
 * Encodes a value payload (without property index).
 */
export function encodeValuePayload(writer: Writer, value: Value): void {
  switch (value.type) {
    case "boolean":
      writer.writeByte(value.value ? 0x01 : 0x00);
      break;

    case "integer":
      writer.writeSignedVarint(value.value);
      break;

    case "float":
      if (Number.isNaN(value.value)) {
        throw new Error("NaN is not allowed in Float");
      }
      writer.writeFloat64(value.value);
      break;

    case "decimal":
      encodeDecimal(writer, value.exponent, value.mantissa);
      break;

    case "text":
      writer.writeString(value.value);
      break;

    case "bytes":
      writer.writeLengthPrefixedBytes(value.value);
      break;

    case "date": {
      // Parse RFC 3339 date string
      const { days, offsetMin } = parseDateRfc3339(value.value);
      // DATE: 6 bytes (int32 days + int16 offset_min), little-endian
      writer.writeInt32LE(days);
      writer.writeInt16LE(offsetMin);
      break;
    }

    case "time": {
      // Parse RFC 3339 time string
      const { timeMicros, offsetMin } = parseTimeRfc3339(value.value);
      // TIME: 8 bytes (int48 time_micros + int16 offset_min), little-endian
      writer.writeInt48LE(timeMicros);
      writer.writeInt16LE(offsetMin);
      break;
    }

    case "datetime": {
      // Parse RFC 3339 datetime string
      const { epochMicros, offsetMin } = parseDatetimeRfc3339(value.value);
      // DATETIME: 10 bytes (int64 epoch_micros + int16 offset_min), little-endian
      writer.writeInt64LE(epochMicros);
      writer.writeInt16LE(offsetMin);
      break;
    }

    case "schedule":
      writer.writeString(value.value);
      break;

    case "point":
      if (Number.isNaN(value.lat) || Number.isNaN(value.lon)) {
        throw new Error("NaN is not allowed in Point coordinates");
      }
      if (value.lat < -90 || value.lat > 90) {
        throw new Error("latitude out of range [-90, +90]");
      }
      if (value.lon < -180 || value.lon > 180) {
        throw new Error("longitude out of range [-180, +180]");
      }
      if (value.alt !== undefined && Number.isNaN(value.alt)) {
        throw new Error("NaN is not allowed in Point altitude");
      }
      // Write ordinate count: 2 for 2D, 3 for 3D
      const ordinateCount = value.alt !== undefined ? 3 : 2;
      writer.writeByte(ordinateCount);
      // Write in wire order: latitude, longitude, altitude (optional)
      writer.writeFloat64(value.lat);
      writer.writeFloat64(value.lon);
      if (value.alt !== undefined) {
        writer.writeFloat64(value.alt);
      }
      break;

    case "rect":
      if (Number.isNaN(value.minLat) || Number.isNaN(value.minLon) ||
          Number.isNaN(value.maxLat) || Number.isNaN(value.maxLon)) {
        throw new Error("NaN is not allowed in Rect coordinates");
      }
      if (value.minLat < -90 || value.minLat > 90 || value.maxLat < -90 || value.maxLat > 90) {
        throw new Error("latitude out of range [-90, +90]");
      }
      if (value.minLon < -180 || value.minLon > 180 || value.maxLon < -180 || value.maxLon > 180) {
        throw new Error("longitude out of range [-180, +180]");
      }
      // RECT: 32 bytes (4 x float64), little-endian
      // Wire order: min_lat, min_lon, max_lat, max_lon
      writer.writeFloat64(value.minLat);
      writer.writeFloat64(value.minLon);
      writer.writeFloat64(value.maxLat);
      writer.writeFloat64(value.maxLon);
      break;

    case "embedding": {
      const expected = embeddingBytesForDims(value.subType, value.dims);
      if (value.data.length !== expected) {
        throw new Error(`embedding data length ${value.data.length} doesn't match expected ${expected}`);
      }
      writer.writeByte(value.subType);
      writer.writeVarintNumber(value.dims);
      writer.writeBytes(value.data);
      break;
    }
  }
}

/**
 * Normalizes a decimal value by stripping trailing zeros from the mantissa
 * and adjusting the exponent accordingly.
 *
 * Examples:
 * - (mantissa: 100, exponent: -2) → (mantissa: 1, exponent: 0)
 * - (mantissa: 1230, exponent: -2) → (mantissa: 123, exponent: -1)
 * - (mantissa: 0, exponent: 5) → (mantissa: 0, exponent: 0)
 */
function normalizeDecimal(
  exponent: number,
  mantissa: DecimalMantissa
): { exponent: number; mantissa: DecimalMantissa } {
  if (mantissa.type === "i64") {
    let value = mantissa.value;
    let exp = exponent;

    // Zero mantissa must have exponent 0
    if (value === 0n) {
      return { exponent: 0, mantissa: { type: "i64", value: 0n } };
    }

    // Strip trailing zeros
    while (value !== 0n && value % 10n === 0n) {
      value = value / 10n;
      exp += 1;
    }

    return { exponent: exp, mantissa: { type: "i64", value } };
  } else {
    const bytes = mantissa.bytes;

    if (isBigMantissaZero(bytes)) {
      return { exponent: 0, mantissa: { type: "i64", value: 0n } };
    }

    if (!bigMantissaNeedsCanonicalization(exponent, bytes)) {
      return { exponent, mantissa };
    }

    let exp = exponent;
    const isNegative = (bytes[0] & 0x80) !== 0;
    let magnitude = twosComplementAbsBytes(bytes);
    while (true) {
      const { quotient, remainder } = divideUnsignedBeBy10(magnitude);
      if (remainder !== 0) {
        break;
      }
      magnitude = quotient;
      exp += 1;
    }

    const canonicalBytes = signedBytesFromMagnitude(isNegative, magnitude);
    const i64Value = twosComplementBytesToI64(canonicalBytes);
    if (i64Value !== null) {
      return { exponent: exp, mantissa: { type: "i64", value: i64Value } };
    }

    return { exponent: exp, mantissa: { type: "big", bytes: canonicalBytes } };
  }
}

/**
 * Converts a BigInt to minimal big-endian two's complement bytes.
 */
function bigIntToMinimalTwosComplement(value: bigint): Uint8Array {
  if (value === 0n) {
    return new Uint8Array([0]);
  }

  const isNegative = value < 0n;
  // Work with the absolute value to determine byte count,
  // then encode in two's complement
  const abs = isNegative ? -value : value;

  // Determine how many bytes we need
  const bitLen = abs.toString(2).length;
  // +1 for sign bit, then round up to bytes
  const byteLen = Math.ceil((bitLen + 1) / 8);

  // Encode in two's complement
  let encoded = isNegative
    ? (1n << BigInt(byteLen * 8)) + value // two's complement for negative
    : value;

  const bytes = new Uint8Array(byteLen);
  for (let i = byteLen - 1; i >= 0; i--) {
    bytes[i] = Number(encoded & 0xFFn);
    encoded >>= 8n;
  }

  return trimTwosComplement(bytes);
}

function isBigMantissaZero(bytes: Uint8Array): boolean {
  return bytes.every((byte) => byte === 0);
}

function hasRedundantSignExtension(bytes: Uint8Array): boolean {
  return bytes.length > 1
    && ((bytes[0] === 0x00 && (bytes[1] & 0x80) === 0)
      || (bytes[0] === 0xFF && (bytes[1] & 0x80) !== 0));
}

function trimTwosComplement(bytes: Uint8Array): Uint8Array {
  let start = 0;
  while (start < bytes.length - 1) {
    const first = bytes[start];
    const second = bytes[start + 1];
    if ((first === 0x00 && (second & 0x80) === 0) || (first === 0xFF && (second & 0x80) !== 0)) {
      start += 1;
      continue;
    }
    break;
  }
  return start === 0 ? bytes : bytes.slice(start);
}

function bigMantissaNeedsCanonicalization(exponent: number, bytes: Uint8Array): boolean {
  return hasRedundantSignExtension(bytes)
    || isBigMantissaZero(bytes)
    || isBigMantissaDivisibleBy10(bytes)
    || twosComplementBytesToI64(bytes) !== null;
}

function isBigMantissaDivisibleBy10(bytes: Uint8Array): boolean {
  if (bytes.length === 0) {
    return true;
  }

  const isNegative = (bytes[0] & 0x80) !== 0;
  if (isNegative) {
    return twosComplementAbsMod10(bytes) === 0;
  }

  let remainder = 0;
  for (const byte of bytes) {
    remainder = (remainder * 6 + byte) % 10;
  }
  return remainder === 0;
}

function twosComplementAbsMod10(bytes: Uint8Array): number {
  let remainder = 0;
  for (const byte of bytes) {
    remainder = (remainder * 6 + (~byte & 0xFF)) % 10;
  }
  return (remainder + 1) % 10;
}

function twosComplementAbsBytes(bytes: Uint8Array): Uint8Array {
  if (bytes.length === 0) {
    return new Uint8Array();
  }

  if ((bytes[0] & 0x80) === 0) {
    let start = 0;
    while (start < bytes.length && bytes[start] === 0) {
      start += 1;
    }
    return bytes.slice(start);
  }

  const magnitude = new Uint8Array(bytes.length);
  let carry = 1;
  for (let i = bytes.length - 1; i >= 0; i--) {
    const sum = (~bytes[i] & 0xFF) + carry;
    magnitude[i] = sum & 0xFF;
    carry = sum >> 8;
  }

  let start = 0;
  while (start < magnitude.length && magnitude[start] === 0) {
    start += 1;
  }
  return magnitude.slice(start);
}

function divideUnsignedBeBy10(bytes: Uint8Array): { quotient: Uint8Array; remainder: number } {
  const quotient = new Uint8Array(bytes.length);
  let remainder = 0;

  for (let i = 0; i < bytes.length; i++) {
    const current = (remainder << 8) | bytes[i];
    quotient[i] = Math.floor(current / 10);
    remainder = current % 10;
  }

  let start = 0;
  while (start < quotient.length && quotient[start] === 0) {
    start += 1;
  }
  return { quotient: quotient.slice(start), remainder };
}

function signedBytesFromMagnitude(isNegative: boolean, magnitude: Uint8Array): Uint8Array {
  if (magnitude.length === 0) {
    return new Uint8Array([0]);
  }

  let start = 0;
  while (start < magnitude.length && magnitude[start] === 0) {
    start += 1;
  }
  const trimmed = magnitude.slice(start);
  if (trimmed.length === 0) {
    return new Uint8Array([0]);
  }

  if (!isNegative) {
    if ((trimmed[0] & 0x80) === 0) {
      return trimmed;
    }
    const bytes = new Uint8Array(trimmed.length + 1);
    bytes.set(trimmed, 1);
    return bytes;
  }

  const fitsInCurrentWidth = trimmed[0] < 0x80
    || (trimmed[0] === 0x80 && trimmed.slice(1).every((byte) => byte === 0));
  const width = fitsInCurrentWidth ? trimmed.length : trimmed.length + 1;
  const bytes = new Uint8Array(width);
  bytes.set(trimmed, width - trimmed.length);

  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = ~bytes[i] & 0xFF;
  }

  let carry = 1;
  for (let i = bytes.length - 1; i >= 0; i--) {
    const sum = bytes[i] + carry;
    bytes[i] = sum & 0xFF;
    carry = sum >> 8;
  }

  return trimTwosComplement(bytes);
}

function twosComplementBytesToI64(bytes: Uint8Array): bigint | null {
  if (bytes.length === 0) {
    return 0n;
  }
  if (bytes.length > 8) {
    return null;
  }

  const fill = (bytes[0] & 0x80) !== 0 ? 0xFF : 0x00;
  let value = 0n;
  for (let i = 0; i < 8 - bytes.length; i++) {
    value = (value << 8n) | BigInt(fill);
  }
  for (const byte of bytes) {
    value = (value << 8n) | BigInt(byte);
  }
  if ((value & (1n << 63n)) !== 0n) {
    value -= 1n << 64n;
  }
  return value;
}

/**
 * Encodes a decimal value. Normalizes the mantissa/exponent before encoding
 * to ensure trailing zeros are stripped.
 */
function encodeDecimal(writer: Writer, exponent: number, mantissa: DecimalMantissa): void {
  const normalized = normalizeDecimal(exponent, mantissa);

  writer.writeSignedVarint(BigInt(normalized.exponent));

  if (normalized.mantissa.type === "i64") {
    writer.writeByte(0x00); // mantissa_type = varint
    writer.writeSignedVarint(normalized.mantissa.value);
  } else {
    writer.writeByte(0x01); // mantissa_type = bytes
    writer.writeLengthPrefixedBytes(normalized.mantissa.bytes);
  }
}

/**
 * Encodes a property value (with property index, language, unit).
 */
export function encodePropertyValue(
  writer: Writer,
  pv: PropertyValue,
  dicts: DictionaryIndices
): void {
  // Write property index
  const propIndex = dicts.getPropertyIndex(pv.property);
  writer.writeVarintNumber(propIndex);

  // Write payload
  encodeValuePayload(writer, pv.value);

  // Write language index for TEXT
  if (pv.value.type === "text") {
    const langIndex = dicts.getLanguageIndex(pv.value.language);
    writer.writeVarintNumber(langIndex);
  }

  // Write unit index for numerical types
  if (pv.value.type === "integer" || pv.value.type === "float" || pv.value.type === "decimal") {
    const unitIndex = dicts.getUnitIndex(pv.value.unit);
    writer.writeVarintNumber(unitIndex);
  }
}

/**
 * Decodes a value payload based on data type.
 */
export function decodeValuePayload(reader: Reader, dataType: DataType): Value {
  switch (dataType) {
    case DataType.Boolean: {
      const byte = reader.readByte();
      if (byte !== 0x00 && byte !== 0x01) {
        throw new DecodeError("E005", `invalid bool value: ${byte}`);
      }
      return { type: "boolean", value: byte === 0x01 };
    }

    case DataType.Integer: {
      const value = reader.readSignedVarint();
      return { type: "integer", value };
    }

    case DataType.Float: {
      const value = reader.readFloat64();
      if (Number.isNaN(value)) {
        throw new DecodeError("E005", "float value is NaN");
      }
      return { type: "float", value };
    }

    case DataType.Decimal: {
      const exponent = Number(reader.readSignedVarint());
      const mantissaType = reader.readByte();
      let mantissa: DecimalMantissa;
      if (mantissaType === 0x00) {
        mantissa = { type: "i64", value: reader.readSignedVarint() };
      } else if (mantissaType === 0x01) {
        const bytes = reader.readLengthPrefixedBytes();
        if (bytes.length === 0 || hasRedundantSignExtension(bytes)) {
          throw new DecodeError("E005", "decimal mantissa bytes are not minimal");
        }
        mantissa = { type: "big", bytes };
      } else {
        throw new DecodeError("E005", `invalid decimal mantissa type: ${mantissaType}`);
      }
      return { type: "decimal", ...normalizeDecimal(exponent, mantissa) };
    }

    case DataType.Text: {
      const value = reader.readString();
      return { type: "text", value };
    }

    case DataType.Bytes: {
      const value = reader.readLengthPrefixedBytes();
      return { type: "bytes", value };
    }

    case DataType.Date: {
      // DATE: 6 bytes (int32 days + int16 offset_min), little-endian
      const days = reader.readInt32LE();
      const offsetMin = reader.readInt16LE();
      // Validate offset_min range
      if (offsetMin < -1440 || offsetMin > 1440) {
        throw new DecodeError("E005", "DATE offsetMin outside range [-1440, +1440]");
      }
      // Format as RFC 3339
      const value = formatDateRfc3339(days, offsetMin);
      return { type: "date", value };
    }

    case DataType.Time: {
      // TIME: 8 bytes (int48 time_micros + int16 offset_min), little-endian
      const timeMicros = reader.readInt48LE();
      const offsetMin = reader.readInt16LE();
      // Validate time_micros range
      if (timeMicros < 0n || timeMicros > 86_399_999_999n) {
        throw new DecodeError("E005", "TIME timeMicros outside range [0, 86399999999]");
      }
      // Validate offset_min range
      if (offsetMin < -1440 || offsetMin > 1440) {
        throw new DecodeError("E005", "TIME offsetMin outside range [-1440, +1440]");
      }
      // Format as RFC 3339
      const value = formatTimeRfc3339(timeMicros, offsetMin);
      return { type: "time", value };
    }

    case DataType.Datetime: {
      // DATETIME: 10 bytes (int64 epoch_micros + int16 offset_min), little-endian
      const epochMicros = reader.readInt64LE();
      const offsetMin = reader.readInt16LE();
      // Validate offset_min range
      if (offsetMin < -1440 || offsetMin > 1440) {
        throw new DecodeError("E005", "DATETIME offsetMin outside range [-1440, +1440]");
      }
      // Format as RFC 3339
      const value = formatDatetimeRfc3339(epochMicros, offsetMin);
      return { type: "datetime", value };
    }

    case DataType.Schedule: {
      const value = reader.readString();
      return { type: "schedule", value };
    }

    case DataType.Point: {
      const ordinateCount = reader.readByte();
      if (ordinateCount !== 2 && ordinateCount !== 3) {
        throw new DecodeError("E005", `POINT ordinate_count must be 2 or 3, got ${ordinateCount}`);
      }
      // Read in wire order: latitude, longitude, altitude (optional)
      const lat = reader.readFloat64();
      const lon = reader.readFloat64();
      const alt = ordinateCount === 3 ? reader.readFloat64() : undefined;
      if (Number.isNaN(lat) || Number.isNaN(lon)) {
        throw new DecodeError("E005", "NaN is not allowed in Point coordinates");
      }
      if (lat < -90 || lat > 90) {
        throw new DecodeError("E005", `POINT latitude ${lat} out of range [-90, +90]`);
      }
      if (lon < -180 || lon > 180) {
        throw new DecodeError("E005", `POINT longitude ${lon} out of range [-180, +180]`);
      }
      if (alt !== undefined && Number.isNaN(alt)) {
        throw new DecodeError("E005", "NaN is not allowed in Point altitude");
      }
      return { type: "point", lat, lon, alt };
    }

    case DataType.Rect: {
      // RECT: 32 bytes (4 x float64), little-endian
      // Wire order: min_lat, min_lon, max_lat, max_lon
      const minLat = reader.readFloat64();
      const minLon = reader.readFloat64();
      const maxLat = reader.readFloat64();
      const maxLon = reader.readFloat64();
      if (Number.isNaN(minLat) || Number.isNaN(minLon) ||
          Number.isNaN(maxLat) || Number.isNaN(maxLon)) {
        throw new DecodeError("E005", "NaN is not allowed in Rect coordinates");
      }
      if (minLat < -90 || minLat > 90 || maxLat < -90 || maxLat > 90) {
        throw new DecodeError("E005", "RECT latitude out of range [-90, +90]");
      }
      if (minLon < -180 || minLon > 180 || maxLon < -180 || maxLon > 180) {
        throw new DecodeError("E005", "RECT longitude out of range [-180, +180]");
      }
      return { type: "rect", minLat, minLon, maxLat, maxLon };
    }

    case DataType.Embedding: {
      const subTypeByte = reader.readByte();
      if (subTypeByte > 2) {
        throw new DecodeError("E005", `invalid embedding sub-type: ${subTypeByte}`);
      }
      const subType = subTypeByte as EmbeddingSubType;
      const dims = reader.readVarintNumber();
      const expectedBytes = embeddingBytesForDims(subType, dims);
      const data = new Uint8Array(reader.readBytes(expectedBytes));
      return { type: "embedding", subType, dims, data };
    }

    default:
      throw new DecodeError("E005", `invalid data type: ${dataType}`);
  }
}

/**
 * Decodes a property value (with property index, language, unit).
 */
export function decodePropertyValue(
  reader: Reader,
  dicts: DictionaryLookups
): PropertyValue {
  // Read property index
  const propIndex = reader.readVarintNumber();
  const prop = dicts.getProperty(propIndex);

  // Read payload
  let value = decodeValuePayload(reader, prop.dataType);

  // Read language index for TEXT
  if (prop.dataType === DataType.Text) {
    const langIndex = reader.readVarintNumber();
    const language = dicts.getLanguage(langIndex);
    value = { ...value, language } as Value;
  }

  // Read unit index for numerical types
  if (
    prop.dataType === DataType.Integer ||
    prop.dataType === DataType.Float ||
    prop.dataType === DataType.Decimal
  ) {
    const unitIndex = reader.readVarintNumber();
    const unit = dicts.getUnit(unitIndex);
    value = { ...value, unit } as Value;
  }

  return { property: prop.id, value };
}
