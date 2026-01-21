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
   * Adds a POINT value (longitude, latitude, optional altitude).
   */
  point(property: Id, lon: number, lat: number, alt?: number): this {
    this.values.push({
      property,
      value: { type: "point", lon, lat, alt },
    });
    return this;
  }

  /**
   * Adds a DATE value.
   * @param days - Signed days since Unix epoch (1970-01-01)
   * @param offsetMin - Signed UTC offset in minutes (e.g., +330 for +05:30)
   */
  date(property: Id, days: number, offsetMin: number = 0): this {
    this.values.push({
      property,
      value: { type: "date", days, offsetMin },
    });
    return this;
  }

  /**
   * Adds a TIME value.
   * @param timeUs - Microseconds since midnight (0 to 86,399,999,999)
   * @param offsetMin - Signed UTC offset in minutes (e.g., +330 for +05:30)
   */
  time(property: Id, timeUs: bigint, offsetMin: number = 0): this {
    this.values.push({
      property,
      value: { type: "time", timeUs, offsetMin },
    });
    return this;
  }

  /**
   * Adds a DATETIME value.
   * @param epochUs - Microseconds since Unix epoch (1970-01-01T00:00:00Z)
   * @param offsetMin - Signed UTC offset in minutes (e.g., +330 for +05:30)
   */
  datetime(property: Id, epochUs: bigint, offsetMin: number = 0): this {
    this.values.push({
      property,
      value: { type: "datetime", epochUs, offsetMin },
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
