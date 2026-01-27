/**
 * RFC 3339 date/time parsing and formatting utilities.
 *
 * Converts between RFC 3339 formatted strings and GRC-20 internal representations:
 * - Date: days since Unix epoch (1970-01-01) + offset in minutes
 * - Time: microseconds since midnight + offset in minutes
 * - Datetime: microseconds since Unix epoch + offset in minutes
 */

const MICROSECONDS_PER_SECOND = 1_000_000n;
const MICROSECONDS_PER_MINUTE = 60n * MICROSECONDS_PER_SECOND;
const MICROSECONDS_PER_HOUR = 60n * MICROSECONDS_PER_MINUTE;
const MILLISECONDS_PER_DAY = 24 * 60 * 60 * 1000;

/**
 * Parses a timezone offset string (Z, +HH:MM, -HH:MM) and returns offset in minutes.
 */
function parseTimezoneOffset(offset: string): number {
  if (offset === "Z" || offset === "z") {
    return 0;
  }

  const match = offset.match(/^([+-])(\d{2}):(\d{2})$/);
  if (!match) {
    throw new Error(`Invalid timezone offset: ${offset}`);
  }

  const sign = match[1] === "+" ? 1 : -1;
  const hours = parseInt(match[2], 10);
  const minutes = parseInt(match[3], 10);

  if (hours > 23 || minutes > 59) {
    throw new Error(`Invalid timezone offset: ${offset}`);
  }

  const totalMinutes = sign * (hours * 60 + minutes);
  if (totalMinutes < -1440 || totalMinutes > 1440) {
    throw new Error(`Timezone offset out of range [-24:00, +24:00]: ${offset}`);
  }

  return totalMinutes;
}

/**
 * Formats an offset in minutes as a timezone string (Z, +HH:MM, -HH:MM).
 */
function formatTimezoneOffset(offsetMin: number): string {
  if (offsetMin === 0) {
    return "Z";
  }

  const sign = offsetMin >= 0 ? "+" : "-";
  const absOffset = Math.abs(offsetMin);
  const hours = Math.floor(absOffset / 60);
  const minutes = absOffset % 60;

  return `${sign}${hours.toString().padStart(2, "0")}:${minutes.toString().padStart(2, "0")}`;
}

/**
 * Parses fractional seconds string and returns microseconds.
 */
function parseFractionalSeconds(frac: string | undefined): bigint {
  if (!frac) {
    return 0n;
  }

  // Pad or truncate to 6 digits (microseconds)
  const padded = frac.padEnd(6, "0").slice(0, 6);
  return BigInt(parseInt(padded, 10));
}

/**
 * Formats microseconds as fractional seconds string, omitting if zero.
 */
function formatFractionalSeconds(us: bigint): string {
  if (us === 0n) {
    return "";
  }

  // Convert to 6-digit string and trim trailing zeros
  const str = us.toString().padStart(6, "0");
  const trimmed = str.replace(/0+$/, "");
  return `.${trimmed}`;
}

// =====================
// DATE functions
// =====================

/**
 * Parses an RFC 3339 date string (YYYY-MM-DD) and returns days since Unix epoch.
 * Optionally accepts timezone offset suffix for date-with-offset format.
 */
export function parseDateRfc3339(dateStr: string): { days: number; offsetMin: number } {
  // Match YYYY-MM-DD with optional timezone offset
  const match = dateStr.match(/^(\d{4})-(\d{2})-(\d{2})(?:(Z|[+-]\d{2}:\d{2}))?$/);
  if (!match) {
    throw new Error(`Invalid RFC 3339 date: ${dateStr}`);
  }

  const year = parseInt(match[1], 10);
  const month = parseInt(match[2], 10);
  const day = parseInt(match[3], 10);
  const offsetStr = match[4];

  // Validate month and day
  if (month < 1 || month > 12) {
    throw new Error(`Invalid month in date: ${dateStr}`);
  }
  if (day < 1 || day > 31) {
    throw new Error(`Invalid day in date: ${dateStr}`);
  }

  // Calculate days since epoch using UTC Date
  const date = Date.UTC(year, month - 1, day);
  const days = Math.floor(date / MILLISECONDS_PER_DAY);

  const offsetMin = offsetStr ? parseTimezoneOffset(offsetStr) : 0;

  return { days, offsetMin };
}

/**
 * Formats days since Unix epoch as RFC 3339 date string.
 */
export function formatDateRfc3339(days: number, offsetMin: number = 0): string {
  const date = new Date(days * MILLISECONDS_PER_DAY);
  const year = date.getUTCFullYear();
  const month = (date.getUTCMonth() + 1).toString().padStart(2, "0");
  const day = date.getUTCDate().toString().padStart(2, "0");

  const offset = formatTimezoneOffset(offsetMin);
  return `${year}-${month}-${day}${offset}`;
}

// =====================
// TIME functions
// =====================

/**
 * Parses an RFC 3339 time string (HH:MM:SS[.ssssss][Z|+HH:MM]) and returns
 * microseconds since midnight and offset in minutes.
 */
export function parseTimeRfc3339(timeStr: string): { timeMicros: bigint; offsetMin: number } {
  // Match HH:MM:SS[.fractional][timezone]
  const match = timeStr.match(
    /^(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,6}))?(Z|[+-]\d{2}:\d{2})$/
  );
  if (!match) {
    throw new Error(`Invalid RFC 3339 time: ${timeStr}`);
  }

  const hours = parseInt(match[1], 10);
  const minutes = parseInt(match[2], 10);
  const seconds = parseInt(match[3], 10);
  const fractional = match[4];
  const offsetStr = match[5];
  if (!offsetStr) {
    throw new Error(`Timezone offset required in time: ${timeStr}`);
  }

  // Validate ranges
  if (hours > 23) {
    throw new Error(`Invalid hours in time: ${timeStr}`);
  }
  if (minutes > 59) {
    throw new Error(`Invalid minutes in time: ${timeStr}`);
  }
  if (seconds > 59) {
    throw new Error(`Invalid seconds in time: ${timeStr}`);
  }

  const microseconds = parseFractionalSeconds(fractional);
  const timeMicros =
    BigInt(hours) * MICROSECONDS_PER_HOUR +
    BigInt(minutes) * MICROSECONDS_PER_MINUTE +
    BigInt(seconds) * MICROSECONDS_PER_SECOND +
    microseconds;

  // Validate total is within day
  if (timeMicros > 86_399_999_999n) {
    throw new Error(`Time exceeds maximum (23:59:59.999999): ${timeStr}`);
  }

  const offsetMin = parseTimezoneOffset(offsetStr);

  return { timeMicros, offsetMin };
}

/**
 * Formats microseconds since midnight as RFC 3339 time string.
 */
export function formatTimeRfc3339(timeMicros: bigint, offsetMin: number = 0): string {
  const hours = Number(timeMicros / MICROSECONDS_PER_HOUR);
  const remaining1 = timeMicros % MICROSECONDS_PER_HOUR;
  const minutes = Number(remaining1 / MICROSECONDS_PER_MINUTE);
  const remaining2 = remaining1 % MICROSECONDS_PER_MINUTE;
  const seconds = Number(remaining2 / MICROSECONDS_PER_SECOND);
  const microseconds = remaining2 % MICROSECONDS_PER_SECOND;

  const hh = hours.toString().padStart(2, "0");
  const mm = minutes.toString().padStart(2, "0");
  const ss = seconds.toString().padStart(2, "0");
  const frac = formatFractionalSeconds(microseconds);
  const offset = formatTimezoneOffset(offsetMin);

  return `${hh}:${mm}:${ss}${frac}${offset}`;
}

// =====================
// DATETIME functions
// =====================

/**
 * Parses an RFC 3339 datetime string and returns microseconds since Unix epoch
 * and offset in minutes.
 */
export function parseDatetimeRfc3339(datetimeStr: string): { epochMicros: bigint; offsetMin: number } {
  // Match YYYY-MM-DDTHH:MM:SS[.fractional][timezone]
  const match = datetimeStr.match(
    /^(\d{4})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,6}))?(Z|[+-]\d{2}:\d{2})$/
  );
  if (!match) {
    throw new Error(`Invalid RFC 3339 datetime: ${datetimeStr}`);
  }

  const year = parseInt(match[1], 10);
  const month = parseInt(match[2], 10);
  const day = parseInt(match[3], 10);
  const hours = parseInt(match[4], 10);
  const minutes = parseInt(match[5], 10);
  const seconds = parseInt(match[6], 10);
  const fractional = match[7];
  const offsetStr = match[8];
  if (!offsetStr) {
    throw new Error(`Timezone offset required in datetime: ${datetimeStr}`);
  }

  // Validate ranges
  if (month < 1 || month > 12) {
    throw new Error(`Invalid month in datetime: ${datetimeStr}`);
  }
  if (day < 1 || day > 31) {
    throw new Error(`Invalid day in datetime: ${datetimeStr}`);
  }
  if (hours > 23) {
    throw new Error(`Invalid hours in datetime: ${datetimeStr}`);
  }
  if (minutes > 59) {
    throw new Error(`Invalid minutes in datetime: ${datetimeStr}`);
  }
  if (seconds > 59) {
    throw new Error(`Invalid seconds in datetime: ${datetimeStr}`);
  }

  const offsetMin = parseTimezoneOffset(offsetStr);
  const microseconds = parseFractionalSeconds(fractional);

  // Calculate epoch milliseconds in UTC
  // Note: Date.UTC gives us milliseconds for the given UTC time components
  const epochMs = Date.UTC(year, month - 1, day, hours, minutes, seconds);

  // Convert to microseconds and add fractional component
  // The epochMs is in UTC, but the datetime string represents local time with offset
  // We need to subtract the offset to get the actual UTC time
  const epochMicrosUTC = BigInt(epochMs) * 1000n + microseconds;

  // Adjust for timezone offset: local time + offset = UTC
  // So: UTC = local - offset
  const offsetUs = BigInt(offsetMin) * MICROSECONDS_PER_MINUTE;
  const epochMicros = epochMicrosUTC - offsetUs;

  return { epochMicros, offsetMin };
}

/**
 * Formats microseconds since Unix epoch as RFC 3339 datetime string.
 */
export function formatDatetimeRfc3339(epochMicros: bigint, offsetMin: number = 0): string {
  // Adjust for timezone offset: local time = UTC + offset
  const offsetUs = BigInt(offsetMin) * MICROSECONDS_PER_MINUTE;
  const localUs = epochMicros + offsetUs;

  // Convert to milliseconds for Date constructor
  const epochMs = Number(localUs / 1000n);
  const microseconds = localUs % 1_000_000n;
  // Handle negative microseconds (modulo can be negative in JS for negative numbers)
  const microsecondsPositive = microseconds < 0n ? microseconds + 1_000_000n : microseconds;

  const date = new Date(epochMs);

  const year = date.getUTCFullYear();
  const month = (date.getUTCMonth() + 1).toString().padStart(2, "0");
  const day = date.getUTCDate().toString().padStart(2, "0");
  const hours = date.getUTCHours().toString().padStart(2, "0");
  const minutes = date.getUTCMinutes().toString().padStart(2, "0");
  const seconds = date.getUTCSeconds().toString().padStart(2, "0");

  const frac = formatFractionalSeconds(microsecondsPositive);
  const offset = formatTimezoneOffset(offsetMin);

  return `${year}-${month}-${day}T${hours}:${minutes}:${seconds}${frac}${offset}`;
}
