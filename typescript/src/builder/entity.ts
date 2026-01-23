import type { Id } from "../types/id.js";
import type { DecimalMantissa, PropertyValue, Value } from "../types/value.js";
import { EmbeddingSubType } from "../types/value.js";

/**
 * Builder for entity values (used in CreateEntity).
 */
export class EntityBuilder {
  private values: PropertyValue[] = [];

  /**
   * Adds a property value.
   */
  value(property: Id, value: Value): this {
    this.values.push({ property, value });
    return this;
  }

  /**
   * Adds a TEXT value.
   */
  text(property: Id, value: string, language?: Id): this {
    this.values.push({
      property,
      value: { type: "text", value, language },
    });
    return this;
  }

  /**
   * Adds an INT64 value.
   */
  int64(property: Id, value: bigint, unit?: Id): this {
    this.values.push({
      property,
      value: { type: "int64", value, unit },
    });
    return this;
  }

  /**
   * Adds a FLOAT64 value.
   */
  float64(property: Id, value: number, unit?: Id): this {
    this.values.push({
      property,
      value: { type: "float64", value, unit },
    });
    return this;
  }

  /**
   * Adds a BOOL value.
   */
  bool(property: Id, value: boolean): this {
    this.values.push({
      property,
      value: { type: "bool", value },
    });
    return this;
  }

  /**
   * Adds a BYTES value.
   */
  bytes(property: Id, value: Uint8Array): this {
    this.values.push({
      property,
      value: { type: "bytes", value },
    });
    return this;
  }

  /**
   * Adds a POINT value (latitude, longitude, optional altitude).
   */
  point(property: Id, lat: number, lon: number, alt?: number): this {
    this.values.push({
      property,
      value: { type: "point", lat, lon, alt },
    });
    return this;
  }

  /**
   * Adds a DATE value.
   * @param value - RFC 3339 date string (e.g., "2024-01-15" or "2024-01-15+05:30")
   */
  date(property: Id, value: string): this {
    this.values.push({
      property,
      value: { type: "date", value },
    });
    return this;
  }

  /**
   * Adds a TIME value.
   * @param value - RFC 3339 time string (e.g., "14:30:45.123456Z" or "14:30:45+05:30")
   */
  time(property: Id, value: string): this {
    this.values.push({
      property,
      value: { type: "time", value },
    });
    return this;
  }

  /**
   * Adds a DATETIME value.
   * @param value - RFC 3339 datetime string (e.g., "2024-01-15T14:30:45.123456Z")
   */
  datetime(property: Id, value: string): this {
    this.values.push({
      property,
      value: { type: "datetime", value },
    });
    return this;
  }

  /**
   * Adds a SCHEDULE value (RFC 5545 iCalendar format).
   */
  schedule(property: Id, value: string): this {
    this.values.push({
      property,
      value: { type: "schedule", value },
    });
    return this;
  }

  /**
   * Adds a DECIMAL value.
   */
  decimal(property: Id, exponent: number, mantissa: DecimalMantissa, unit?: Id): this {
    this.values.push({
      property,
      value: { type: "decimal", exponent, mantissa, unit },
    });
    return this;
  }

  /**
   * Adds a DECIMAL value from a bigint mantissa.
   */
  decimalI64(property: Id, exponent: number, mantissa: bigint, unit?: Id): this {
    return this.decimal(property, exponent, { type: "i64", value: mantissa }, unit);
  }

  /**
   * Adds an EMBEDDING value.
   */
  embedding(
    property: Id,
    subType: EmbeddingSubType,
    dims: number,
    data: Uint8Array
  ): this {
    this.values.push({
      property,
      value: { type: "embedding", subType, dims, data },
    });
    return this;
  }

  /**
   * Returns the built values array.
   */
  getValues(): PropertyValue[] {
    return this.values;
  }
}
