import type { Id } from "./id.js";
import {
  parseDateRfc3339,
  parseTimeRfc3339,
  parseDatetimeRfc3339,
} from "../util/datetime.js";

/**
 * Data types for property values (spec Section 2.4).
 */
export enum DataType {
  Bool = 1,
  Int64 = 2,
  Float64 = 3,
  Decimal = 4,
  Text = 5,
  Bytes = 6,
  Date = 7,
  Time = 8,
  Datetime = 9,
  Schedule = 10,
  Point = 11,
  Embedding = 12,
}

/**
 * Embedding sub-types (spec Section 2.4).
 */
export enum EmbeddingSubType {
  /** 32-bit IEEE 754 float, little-endian (4 bytes per dim) */
  Float32 = 0,
  /** Signed 8-bit integer (1 byte per dim) */
  Int8 = 1,
  /** Bit-packed binary, LSB-first (1/8 byte per dim) */
  Binary = 2,
}

/**
 * Returns the number of bytes needed for the given number of dimensions.
 */
export function embeddingBytesForDims(subType: EmbeddingSubType, dims: number): number {
  switch (subType) {
    case EmbeddingSubType.Float32:
      return dims * 4;
    case EmbeddingSubType.Int8:
      return dims;
    case EmbeddingSubType.Binary:
      return Math.ceil(dims / 8);
  }
}

/**
 * Decimal mantissa representation.
 */
export type DecimalMantissa =
  | { type: "i64"; value: bigint }
  | { type: "big"; bytes: Uint8Array };

/**
 * A typed value that can be stored on an entity or relation.
 */
export type Value =
  | { type: "bool"; value: boolean }
  | { type: "int64"; value: bigint; unit?: Id }
  | { type: "float64"; value: number; unit?: Id }
  | { type: "decimal"; exponent: number; mantissa: DecimalMantissa; unit?: Id }
  | { type: "text"; value: string; language?: Id }
  | { type: "bytes"; value: Uint8Array }
  | {
      /** Calendar date in RFC 3339 format (YYYY-MM-DD with optional timezone). */
      type: "date";
      /** RFC 3339 date string (e.g., "2024-01-15" or "2024-01-15+05:30"). */
      value: string;
    }
  | {
      /** Time of day in RFC 3339 format. */
      type: "time";
      /** RFC 3339 time string (e.g., "14:30:45.123456Z" or "14:30:45+05:30"). */
      value: string;
    }
  | {
      /** Combined date and time in RFC 3339 format. */
      type: "datetime";
      /** RFC 3339 datetime string (e.g., "2024-01-15T14:30:45.123456Z"). */
      value: string;
    }
  | { type: "schedule"; value: string }
  | { type: "point"; lat: number; lon: number; alt?: number }
  | { type: "embedding"; subType: EmbeddingSubType; dims: number; data: Uint8Array };

/**
 * Returns the DataType for a Value.
 */
export function valueDataType(value: Value): DataType {
  switch (value.type) {
    case "bool":
      return DataType.Bool;
    case "int64":
      return DataType.Int64;
    case "float64":
      return DataType.Float64;
    case "decimal":
      return DataType.Decimal;
    case "text":
      return DataType.Text;
    case "bytes":
      return DataType.Bytes;
    case "date":
      return DataType.Date;
    case "time":
      return DataType.Time;
    case "datetime":
      return DataType.Datetime;
    case "schedule":
      return DataType.Schedule;
    case "point":
      return DataType.Point;
    case "embedding":
      return DataType.Embedding;
  }
}

/**
 * A property-value pair that can be attached to an object.
 */
export interface PropertyValue {
  property: Id;
  value: Value;
}

/**
 * A property definition in the schema.
 */
export interface Property {
  id: Id;
  dataType: DataType;
}

/**
 * Validates a value according to spec rules.
 * Returns an error message if invalid, undefined if valid.
 */
export function validateValue(value: Value): string | undefined {
  switch (value.type) {
    case "float64":
      if (Number.isNaN(value.value)) {
        return "NaN is not allowed in Float64";
      }
      break;
    case "decimal": {
      const isZero =
        value.mantissa.type === "i64"
          ? value.mantissa.value === 0n
          : value.mantissa.bytes.every((b) => b === 0);
      if (isZero && value.exponent !== 0) {
        return "zero DECIMAL must have exponent 0";
      }
      // Check for trailing zeros in non-zero mantissa
      if (!isZero && value.mantissa.type === "i64") {
        if (value.mantissa.value % 10n === 0n) {
          return "DECIMAL mantissa has trailing zeros (not normalized)";
        }
      }
      break;
    }
    case "point":
      if (value.lat < -90 || value.lat > 90) {
        return "latitude out of range [-90, +90]";
      }
      if (value.lon < -180 || value.lon > 180) {
        return "longitude out of range [-180, +180]";
      }
      if (Number.isNaN(value.lat) || Number.isNaN(value.lon)) {
        return "NaN is not allowed in Point coordinates";
      }
      if (value.alt !== undefined && Number.isNaN(value.alt)) {
        return "NaN is not allowed in Point altitude";
      }
      break;
    case "date":
      try {
        parseDateRfc3339(value.value);
      } catch (e) {
        return e instanceof Error ? e.message : "Invalid RFC 3339 date";
      }
      break;
    case "time":
      try {
        parseTimeRfc3339(value.value);
      } catch (e) {
        return e instanceof Error ? e.message : "Invalid RFC 3339 time";
      }
      break;
    case "datetime":
      try {
        parseDatetimeRfc3339(value.value);
      } catch (e) {
        return e instanceof Error ? e.message : "Invalid RFC 3339 datetime";
      }
      break;
    case "embedding": {
      const expected = embeddingBytesForDims(value.subType, value.dims);
      if (value.data.length !== expected) {
        return `embedding data length ${value.data.length} doesn't match expected ${expected} for ${value.dims} dims`;
      }
      // Check for NaN in float32 embeddings
      if (value.subType === EmbeddingSubType.Float32) {
        const view = new DataView(value.data.buffer, value.data.byteOffset, value.data.byteLength);
        for (let i = 0; i < value.dims; i++) {
          const f = view.getFloat32(i * 4, true);
          if (Number.isNaN(f)) {
            return "NaN is not allowed in float32 embedding";
          }
        }
      }
      break;
    }
  }
  return undefined;
}
